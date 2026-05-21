//! DuckDB-backed primitive stores: thread-safe connection pool plus two
//! tables — `blobs` (raw bytes per UUID/string key) and `json_docs`
//! (one JSON document per UUID with a logical key field for secondary
//! lookups).
//!
//! This module is the absorbed equivalent of bdslib's
//! `storageengine.rs` + `datastorage.rs`. The differences from the
//! original:
//!
//! - Returns `anyhow::Result` instead of `easy_error::Result`.
//! - No `rust_dynamic::value::Value` bridge; we pull `Vec<u8>` /
//!   `String` directly out of `duckdb::types::Value`.
//! - No shared `ScheduledThreadPool` — r2d2's per-pool default is
//!   fine for inkhaven's one-process / one-store usage.
//! - String-key blob helpers are gone (never reached from inkhaven).
//! - `JsonStorageConfig.key_field` is gone too (we always use the
//!   default key — that's the only mode inkhaven ever invoked).

use anyhow::{anyhow, Result};
use duckdb::{types::Value as DuckValue, DuckdbConnectionManager};
use r2d2::Pool;
use serde_json::Value as JsonValue;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

// ── StorageEngine ────────────────────────────────────────────────────

/// Thin wrapper over an r2d2-pooled DuckDB connection. Cloneable; all
/// clones share the same pool.
pub struct StorageEngine {
    pool: Pool<DuckdbConnectionManager>,
}

impl StorageEngine {
    /// Open or create the DuckDB database at `path`, run `init_sql` once
    /// to set up the schema, and return a connection pool of `pool_size`
    /// connections.
    pub fn new<P: AsRef<Path>>(path: P, init_sql: &str, pool_size: u32) -> Result<Self> {
        let manager = DuckdbConnectionManager::file(path)
            .map_err(|e| anyhow!("failed to create connection manager: {e}"))?;

        let pool = Pool::builder()
            .max_size(pool_size)
            .build(manager)
            .map_err(|e| anyhow!("failed to initialize connection pool: {e}"))?;

        {
            let conn = pool
                .get()
                .map_err(|e| anyhow!("could not get init connection: {e}"))?;
            conn.execute_batch(init_sql)
                .map_err(|e| anyhow!("initialization SQL failed: {e}"))?;
        }

        Ok(Self { pool })
    }

    /// Run a `SELECT` and collect every row as a `Vec` of raw DuckDB
    /// values. Used for the tiny set of internal queries this module
    /// emits — callers `match` on the variants directly.
    pub fn select_all(&self, sql: &str) -> Result<Vec<Vec<DuckValue>>> {
        let conn = self
            .pool
            .get()
            .map_err(|e| anyhow!("pool checkout failed: {e}"))?;

        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| anyhow!("query preparation failed: {e}"))?;

        // duckdb-rs panics if `column_count` is read before the
        // statement has been executed, so query first and read the
        // column count off each row as it arrives.
        let rows = stmt
            .query_map([], |row| {
                let n = row.as_ref().column_count();
                let mut out = Vec::with_capacity(n);
                for i in 0..n {
                    out.push(row.get::<_, DuckValue>(i)?);
                }
                Ok(out)
            })
            .map_err(|e| anyhow!("execution of select_all failed: {e}"))?;

        let mut results = Vec::new();
        for r in rows {
            results.push(r.map_err(|e| anyhow!("error fetching row: {e}"))?);
        }
        Ok(results)
    }

    /// Execute a DML statement (no result rows).
    pub fn execute(&self, sql: &str) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| anyhow!("pool checkout failed: {e}"))?;
        conn.execute(sql, [])
            .map_err(|e| anyhow!("SQL execution failed: {e}"))?;
        Ok(())
    }

    /// Same as [`execute`] but with positional parameters. We use this
    /// for any statement that interpolates user-controlled bytes
    /// (BLOB writes), so the bytes never need hex encoding.
    pub fn execute_with(&self, sql: &str, args: &[&dyn duckdb::ToSql]) -> Result<()> {
        let conn = self
            .pool
            .get()
            .map_err(|e| anyhow!("pool checkout failed: {e}"))?;
        conn.execute(sql, duckdb::params_from_iter(args.iter().copied()))
            .map_err(|e| anyhow!("SQL execution failed: {e}"))?;
        Ok(())
    }

    /// Force a DuckDB `CHECKPOINT` — drains WAL into the main `.db`
    /// file. Cheap when the WAL is empty (DuckDB short-circuits).
    /// Called from the background sync tick and the TUI shutdown
    /// path; per-save callers shouldn't invoke this directly because
    /// every commit is already durable.
    pub fn checkpoint(&self) -> Result<()> {
        self.execute("CHECKPOINT;")
    }
}

// ── BlobStorage ──────────────────────────────────────────────────────

const BLOB_INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS blobs (
        id         TEXT   NOT NULL PRIMARY KEY,
        created_at BIGINT NOT NULL,
        updated_at BIGINT NOT NULL,
        data       BLOB   NOT NULL
    );
";

/// Binary blob store keyed by UUID. Thread-safe (cloning yields a new
/// handle to the same pool).
#[derive(Clone)]
pub struct BlobStorage {
    engine: Arc<StorageEngine>,
}

impl BlobStorage {
    pub fn new<P: AsRef<Path>>(path: P, pool_size: u32) -> Result<Self> {
        let engine = StorageEngine::new(path, BLOB_INIT_SQL, pool_size)?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Insert a new blob keyed by `id`. Errors if `id` already exists.
    pub fn add_blob_with_key(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs();
        let id_str = id.to_string();
        self.engine.execute_with(
            "INSERT INTO blobs (id, created_at, updated_at, data) \
             VALUES (?, ?, ?, ?)",
            &[&id_str, &ts, &ts, &data],
        )
    }

    pub fn get_blob(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        let rows = self.engine.select_all(&format!(
            "SELECT data FROM blobs WHERE id = '{}'",
            sql_escape(&id.to_string()),
        ))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => {
                let v = row
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("blob row missing data column"))?;
                match v {
                    DuckValue::Blob(b) => Ok(Some(b)),
                    DuckValue::Null => Ok(None),
                    other => Err(anyhow!(
                        "unexpected blob column type: {other:?}"
                    )),
                }
            }
        }
    }

    pub fn update_blob(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs();
        let id_str = id.to_string();
        self.engine.execute_with(
            "UPDATE blobs SET data = ?, updated_at = ? WHERE id = ?",
            &[&data, &ts, &id_str],
        )
    }

    pub fn drop_blob(&self, id: Uuid) -> Result<()> {
        self.engine.execute(&format!(
            "DELETE FROM blobs WHERE id = '{}'",
            sql_escape(&id.to_string()),
        ))
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.engine.checkpoint()
    }
}

// ── JsonStorage ──────────────────────────────────────────────────────

const JSON_INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS json_docs (
        id         TEXT   NOT NULL PRIMARY KEY,
        created_at BIGINT NOT NULL,
        updated_at BIGINT NOT NULL,
        key        TEXT   NOT NULL,
        document   JSON   NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_json_docs_key ON json_docs (key);
";

/// JSON document store keyed by UUID, with a `key` column used by
/// bdslib for secondary lookups. Inkhaven always uses the same key
/// (`"doc"`) for every record — the field is preserved purely for
/// on-disk compatibility with existing projects.
#[derive(Clone)]
pub struct JsonStorage {
    engine: Arc<StorageEngine>,
    default_key: Arc<String>,
}

impl JsonStorage {
    pub fn new<P: AsRef<Path>>(path: P, pool_size: u32, default_key: &str) -> Result<Self> {
        let engine = StorageEngine::new(path, JSON_INIT_SQL, pool_size)?;
        Ok(Self {
            engine: Arc::new(engine),
            default_key: Arc::new(default_key.to_string()),
        })
    }

    pub fn add_json_with_id(&self, id: Uuid, document: JsonValue) -> Result<()> {
        let ts = now_unix_secs();
        let id_str = id.to_string();
        let doc_str = serde_json::to_string(&document)
            .map_err(|e| anyhow!("JSON serialisation failed: {e}"))?;
        self.engine.execute(&format!(
            "INSERT INTO json_docs (id, created_at, updated_at, key, document) \
             VALUES ('{}', {ts}, {ts}, '{}', '{}'::JSON)",
            sql_escape(&id_str),
            sql_escape(&self.default_key),
            sql_escape(&doc_str),
        ))
    }

    pub fn get_json(&self, id: Uuid) -> Result<Option<JsonValue>> {
        let rows = self.engine.select_all(&format!(
            "SELECT document FROM json_docs WHERE id = '{}'",
            sql_escape(&id.to_string()),
        ))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => {
                let v = row
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("json_docs row missing document column"))?;
                let text = match v {
                    DuckValue::Text(s) => s,
                    DuckValue::Null => return Ok(None),
                    other => {
                        return Err(anyhow!(
                            "unexpected json document column type: {other:?}"
                        ))
                    }
                };
                let val = serde_json::from_str(&text)
                    .map_err(|e| anyhow!("JSON parse failed: {e}"))?;
                Ok(Some(val))
            }
        }
    }

    pub fn update_json(&self, id: Uuid, document: JsonValue) -> Result<()> {
        let ts = now_unix_secs();
        let id_str = id.to_string();
        let doc_str = serde_json::to_string(&document)
            .map_err(|e| anyhow!("JSON serialisation failed: {e}"))?;
        self.engine.execute(&format!(
            "UPDATE json_docs \
             SET document = '{}'::JSON, key = '{}', updated_at = {ts} \
             WHERE id = '{}'",
            sql_escape(&doc_str),
            sql_escape(&self.default_key),
            sql_escape(&id_str),
        ))
    }

    pub fn drop_json(&self, id: Uuid) -> Result<()> {
        self.engine.execute(&format!(
            "DELETE FROM json_docs WHERE id = '{}'",
            sql_escape(&id.to_string()),
        ))
    }

    pub fn list_all(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        let rows = self
            .engine
            .select_all("SELECT id, document FROM json_docs")?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut cols = row.into_iter();
            let id_str = match cols.next() {
                Some(DuckValue::Text(s)) => s,
                Some(other) => {
                    return Err(anyhow!(
                        "json_docs row id column unexpected type: {other:?}"
                    ))
                }
                None => return Err(anyhow!("json_docs row missing id")),
            };
            let doc_str = match cols.next() {
                Some(DuckValue::Text(s)) => s,
                Some(other) => {
                    return Err(anyhow!(
                        "json_docs row document column unexpected type: {other:?}"
                    ))
                }
                None => return Err(anyhow!("json_docs row missing document")),
            };
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| anyhow!("invalid UUID in json_docs: {e}"))?;
            let doc: JsonValue = serde_json::from_str(&doc_str)
                .map_err(|e| anyhow!("JSON parse failed: {e}"))?;
            out.push((id, doc));
        }
        Ok(out)
    }

    pub fn checkpoint(&self) -> Result<()> {
        self.engine.checkpoint()
    }
}

// ── helpers ──────────────────────────────────────────────────────────

/// Escape a string for safe interpolation into a SQL single-quoted
/// literal. Doubles every `'` (`'` → `''`).
pub(crate) fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

fn now_unix_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    /// `App::shutdown_flush` calls `Store::checkpoint()`, which fans out to
    /// `JsonStorage::checkpoint()` + `BlobStorage::checkpoint()`. Verify
    /// both checkpoint paths drain a fresh write into the main DB file —
    /// i.e. no `.wal` should remain after checkpoint + drop.
    #[test]
    fn checkpoint_drains_wal_after_write() {
        let dir = TempDir::new().unwrap();

        // JSON path
        let json_path = dir.path().join("meta.db");
        {
            let store = JsonStorage::new(&json_path, 2, "doc").unwrap();
            let id = Uuid::now_v7();
            store
                .add_json_with_id(id, json!({"hello": "world"}))
                .unwrap();
            store.checkpoint().unwrap();
        }
        assert!(json_path.exists(), "main JSON file should exist");
        let wal = json_path.with_extension("db.wal");
        assert!(
            !wal.exists() || std::fs::metadata(&wal).unwrap().len() == 0,
            "WAL should be drained after checkpoint, found {} bytes",
            std::fs::metadata(&wal).map(|m| m.len()).unwrap_or(0)
        );

        // Blob path
        let blob_path = dir.path().join("blobs.db");
        {
            let store = BlobStorage::new(&blob_path, 2).unwrap();
            let id = Uuid::now_v7();
            store.add_blob_with_key(id, b"some bytes").unwrap();
            store.checkpoint().unwrap();
        }
        assert!(blob_path.exists());
        let wal = blob_path.with_extension("db.wal");
        assert!(
            !wal.exists() || std::fs::metadata(&wal).unwrap().len() == 0,
            "blob WAL should be drained after checkpoint",
        );
    }

    /// `App::shutdown_flush` doesn't error when nothing has been written —
    /// CHECKPOINT against an empty WAL is a no-op in DuckDB, but we want
    /// to be sure our wrapper doesn't choke.
    #[test]
    fn checkpoint_is_safe_on_empty_store() {
        let dir = TempDir::new().unwrap();
        let store = JsonStorage::new(dir.path().join("empty.db"), 2, "doc").unwrap();
        store.checkpoint().unwrap();
        store.checkpoint().unwrap(); // idempotent
    }

    /// Write-then-checkpoint-then-reopen round-trip — confirms data
    /// survives the explicit checkpoint exactly like it survives the
    /// pool-drop auto-checkpoint.
    #[test]
    fn data_survives_checkpoint_and_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("rt.db");
        let id = Uuid::now_v7();

        {
            let s = JsonStorage::new(&path, 2, "doc").unwrap();
            s.add_json_with_id(id, json!({"key": "value-1"})).unwrap();
            s.checkpoint().unwrap();
        }
        let s2 = JsonStorage::new(&path, 2, "doc").unwrap();
        let got = s2.get_json(id).unwrap();
        assert_eq!(got, Some(json!({"key": "value-1"})));
    }
}
