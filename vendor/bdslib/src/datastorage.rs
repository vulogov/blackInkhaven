use crate::common::error::{err_msg, Result};
use crate::common::hex::to_hex;
use crate::common::jsonfingerprint::extract_key;
use crate::common::sql::sql_escape;
use crate::common::timerange::now_unix_secs;
use crate::common::uuid::generate_v7;
use crate::StorageEngine;
use serde_json::Value as JsonValue;
use std::sync::Arc;
use uuid::Uuid;

// ── schema ────────────────────────────────────────────────────────────────────

const BLOB_INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS blobs (
        id         TEXT   NOT NULL PRIMARY KEY,
        created_at BIGINT NOT NULL,
        updated_at BIGINT NOT NULL,
        data       BLOB   NOT NULL
    );
";

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

// ── BlobStorage ───────────────────────────────────────────────────────────────

/// Thread-safe binary blob store backed by [`StorageEngine`].
///
/// The primary key column is `TEXT`, so records can be identified by either a
/// UUIDv7 (auto-generated or caller-supplied) or by an arbitrary string key.
/// All three key families share the same underlying table and can coexist.
///
/// Both `created_at` and `updated_at` are Unix timestamps (seconds).
/// `created_at` is set once at insertion and never changed; `updated_at` is
/// refreshed on every update call.
///
/// `BlobStorage` is `Clone`; all clones share the same underlying pool.
///
/// [`update_blob`]: BlobStorage::update_blob
#[derive(Clone)]
pub struct BlobStorage {
    engine: Arc<StorageEngine>,
}

impl BlobStorage {
    /// Open or create a blob store at `path`.
    ///
    /// The required table is created automatically if it does not exist.
    /// Pass `":memory:"` for an ephemeral in-process store.
    pub fn new(path: &str, pool_size: u32) -> Result<Self> {
        let engine = StorageEngine::new(path, BLOB_INIT_SQL, pool_size)?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Store `data`, record the current time as both `created_at` and
    /// `updated_at`, and return the generated UUIDv7.
    ///
    /// On the (astronomically unlikely) event that the generated UUID already
    /// exists, the existing record is replaced rather than returning an error.
    pub fn add_blob(&self, data: &[u8]) -> Result<Uuid> {
        let id = generate_v7();
        let ts = now_unix_secs()?;
        let hex = to_hex(data);
        self.engine.execute(&format!(
            "INSERT INTO blobs (id, created_at, updated_at, data) \
             VALUES ('{id}', {ts}, {ts}, from_hex('{hex}')) \
             ON CONFLICT (id) DO UPDATE SET data = excluded.data, updated_at = excluded.updated_at"
        ))?;
        Ok(id)
    }

    /// Store `data` under the caller-supplied `id`, recording the current time
    /// as both `created_at` and `updated_at`.
    ///
    /// Returns `Err` if a record with `id` already exists. Use [`update_blob`]
    /// to replace an existing record's payload, or call [`drop_blob`] first if
    /// both the data and the timestamps must be reset.
    ///
    /// [`update_blob`]: BlobStorage::update_blob
    /// [`drop_blob`]: BlobStorage::drop_blob
    pub fn add_blob_with_key(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs()?;
        let hex = to_hex(data);
        self.engine.execute(&format!(
            "INSERT INTO blobs (id, created_at, updated_at, data) \
             VALUES ('{id}', {ts}, {ts}, from_hex('{hex}'))"
        ))
    }

    /// Return the bytes stored under `id`, or `None` if no such record exists.
    pub fn get_blob(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        let rows = self
            .engine
            .select_all(&format!("SELECT data FROM blobs WHERE id = '{id}'"))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => {
                let bytes = row
                    .into_iter()
                    .next()
                    .ok_or_else(|| err_msg("blob row missing data column"))?
                    .cast_bin()
                    .map_err(|e| err_msg(e.to_string()))?;
                Ok(Some(bytes))
            }
        }
    }

    /// Replace the bytes stored under `id` and set `updated_at` to now.
    ///
    /// Returns `Ok(())` even if `id` does not exist (no-op).
    pub fn update_blob(&self, id: Uuid, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs()?;
        let hex = to_hex(data);
        self.engine.execute(&format!(
            "UPDATE blobs SET data = from_hex('{hex}'), updated_at = {ts} WHERE id = '{id}'"
        ))
    }

    /// Delete the record for `id`.
    ///
    /// Returns `Ok(())` even if `id` does not exist.
    pub fn drop_blob(&self, id: Uuid) -> Result<()> {
        self.engine
            .execute(&format!("DELETE FROM blobs WHERE id = '{id}'"))
    }

    // ── string-key API ────────────────────────────────────────────────────────

    /// Store `data` under the caller-supplied string `key`, recording the
    /// current time as both `created_at` and `updated_at`.
    ///
    /// `key` may be any non-empty string and is stored verbatim as the primary
    /// key. Single quotes and other characters that would normally require SQL
    /// escaping are handled automatically.
    ///
    /// Returns `Err` if a record with `key` already exists. Use
    /// [`update_blob_by_string_key`] to replace an existing record's payload,
    /// or call [`drop_blob_by_string_key`] first if the timestamps must also be
    /// reset.
    ///
    /// [`update_blob_by_string_key`]: BlobStorage::update_blob_by_string_key
    /// [`drop_blob_by_string_key`]: BlobStorage::drop_blob_by_string_key
    pub fn add_blob_with_string_key(&self, key: &str, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs()?;
        let hex = to_hex(data);
        self.engine.execute(&format!(
            "INSERT INTO blobs (id, created_at, updated_at, data) \
             VALUES ('{}', {ts}, {ts}, from_hex('{hex}'))",
            sql_escape(key),
        ))
    }

    /// Return the bytes stored under the string `key`, or `None` if no such
    /// record exists.
    pub fn get_blob_by_string_key(&self, key: &str) -> Result<Option<Vec<u8>>> {
        let rows = self.engine.select_all(&format!(
            "SELECT data FROM blobs WHERE id = '{}'",
            sql_escape(key),
        ))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => {
                let bytes = row
                    .into_iter()
                    .next()
                    .ok_or_else(|| err_msg("blob row missing data column"))?
                    .cast_bin()
                    .map_err(|e| err_msg(e.to_string()))?;
                Ok(Some(bytes))
            }
        }
    }

    /// Replace the bytes stored under the string `key` and set `updated_at` to
    /// now.
    ///
    /// Returns `Ok(())` even if `key` does not exist (no-op).
    pub fn update_blob_by_string_key(&self, key: &str, data: &[u8]) -> Result<()> {
        let ts = now_unix_secs()?;
        let hex = to_hex(data);
        self.engine.execute(&format!(
            "UPDATE blobs SET data = from_hex('{hex}'), updated_at = {ts} \
             WHERE id = '{}'",
            sql_escape(key),
        ))
    }

    /// Delete the record for the string `key`.
    ///
    /// Returns `Ok(())` even if `key` does not exist.
    pub fn drop_blob_by_string_key(&self, key: &str) -> Result<()> {
        self.engine.execute(&format!(
            "DELETE FROM blobs WHERE id = '{}'",
            sql_escape(key),
        ))
    }
}

// ── JsonStorage ───────────────────────────────────────────────────────────────

/// Configuration for [`JsonStorage`].
///
/// `key_field` is a dot-notation path used to extract a logical deduplication
/// key from every stored document. `default_key` is used when the path is
/// absent (`None`) or cannot be resolved.
///
/// # Example
///
/// ```rust
/// # use bdslib::datastorage::JsonStorageConfig;
/// // Extract "user.id" as the key; fall back to "anonymous" if absent.
/// let cfg = JsonStorageConfig {
///     key_field:   Some("user.id".to_string()),
///     default_key: "anonymous".to_string(),
/// };
/// ```
#[derive(Debug, Clone)]
pub struct JsonStorageConfig {
    /// Dot-notation path into the document to extract as the record key.
    /// `None` means always use `default_key`.
    pub key_field: Option<String>,
    /// Key to use when `key_field` is `None` or extraction fails.
    pub default_key: String,
}

impl Default for JsonStorageConfig {
    fn default() -> Self {
        Self {
            key_field: None,
            default_key: "default".to_string(),
        }
    }
}

/// Thread-safe JSON document store backed by [`StorageEngine`].
///
/// Each document is assigned a UUIDv7 and a logical `key` extracted from its
/// content. `created_at` and `updated_at` are Unix timestamps (seconds).
///
/// [`add_json`] is an **upsert-by-key**: if a document with the same key is
/// already present it is updated in-place and its original UUID is returned,
/// preserving `created_at`. A fresh UUID is returned only for genuinely new
/// keys.
///
/// DuckDB's `JSON` column type is used for the document field, enabling
/// DuckDB's built-in JSON functions (`json_extract`, `json_keys`, …) in
/// custom queries executed via the underlying [`StorageEngine`].
///
/// `JsonStorage` is `Clone`; all clones share the same underlying pool.
///
/// [`add_json`]: JsonStorage::add_json
#[derive(Clone)]
pub struct JsonStorage {
    engine: Arc<StorageEngine>,
    config: Arc<JsonStorageConfig>,
}

impl JsonStorage {
    /// Open or create a JSON document store at `path`.
    ///
    /// The required table is created automatically if it does not exist.
    /// Pass `":memory:"` for an ephemeral in-process store.
    pub fn new(path: &str, pool_size: u32, config: JsonStorageConfig) -> Result<Self> {
        let engine = StorageEngine::new(path, JSON_INIT_SQL, pool_size)?;
        Ok(Self {
            engine: Arc::new(engine),
            config: Arc::new(config),
        })
    }

    /// Store `document` and return the record UUID.
    ///
    /// The key is extracted from `document` using `config.key_field`, falling
    /// back to `config.default_key` when the field is absent or the path
    /// cannot be resolved.
    ///
    /// If a document with the same key already exists, [`update_json`] is
    /// called on that record and its unchanged UUID is returned. Otherwise a
    /// new UUIDv7 is generated and the document is inserted.
    ///
    /// [`update_json`]: JsonStorage::update_json
    pub fn add_json(&self, document: JsonValue) -> Result<Uuid> {
        let key = self.resolve_key(&document);
        let rows = self.engine.select_all(&format!(
            "SELECT id FROM json_docs WHERE key = '{}'",
            sql_escape(&key)
        ))?;

        if let Some(row) = rows.into_iter().next() {
            let id_str = row
                .into_iter()
                .next()
                .ok_or_else(|| err_msg("json_docs row missing id column"))?
                .cast_string()
                .map_err(|e| err_msg(e.to_string()))?;
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| err_msg(format!("invalid UUID in json_docs table: {e}")))?;
            self.update_json(id, document)?;
            Ok(id)
        } else {
            let id = generate_v7();
            let ts = now_unix_secs()?;
            let doc_str = serde_json::to_string(&document)
                .map_err(|e| err_msg(format!("JSON serialisation failed: {e}")))?;
            self.engine.execute(&format!(
                "INSERT INTO json_docs (id, created_at, updated_at, key, document) \
                 VALUES ('{id}', {ts}, {ts}, '{}', '{}'::JSON)",
                sql_escape(&key),
                sql_escape(&doc_str),
            ))?;
            Ok(id)
        }
    }

    /// Store `document` under the caller-supplied `id`, recording the current
    /// time as both `created_at` and `updated_at`.
    ///
    /// The `key` column is populated using the same `config.key_field` logic as
    /// [`add_json`]; the key is used for secondary lookups and deduplication
    /// but does not affect the primary key.
    ///
    /// Returns `Err` if a record with `id` already exists.
    ///
    /// [`add_json`]: JsonStorage::add_json
    pub fn add_json_with_id(&self, id: Uuid, document: JsonValue) -> Result<()> {
        let ts = now_unix_secs()?;
        let key = self.resolve_key(&document);
        let doc_str = serde_json::to_string(&document)
            .map_err(|e| err_msg(format!("JSON serialisation failed: {e}")))?;
        self.engine.execute(&format!(
            "INSERT INTO json_docs (id, created_at, updated_at, key, document) \
             VALUES ('{id}', {ts}, {ts}, '{}', '{}'::JSON)",
            sql_escape(&key),
            sql_escape(&doc_str),
        ))
    }

    /// Return the document stored under `id`, or `None` if no such record exists.
    pub fn get_json(&self, id: Uuid) -> Result<Option<JsonValue>> {
        let rows = self.engine.select_all(&format!(
            "SELECT document FROM json_docs WHERE id = '{id}'"
        ))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => {
                let text = row
                    .into_iter()
                    .next()
                    .ok_or_else(|| err_msg("json_docs row missing document column"))?
                    .cast_string()
                    .map_err(|e| err_msg(e.to_string()))?;
                let val = serde_json::from_str(&text)
                    .map_err(|e| err_msg(format!("JSON parse failed: {e}")))?;
                Ok(Some(val))
            }
        }
    }

    /// Replace the document stored under `id`, re-extract and update its key,
    /// and set `updated_at` to now.
    ///
    /// Returns `Ok(())` even if `id` does not exist (no-op).
    pub fn update_json(&self, id: Uuid, document: JsonValue) -> Result<()> {
        let ts = now_unix_secs()?;
        let key = self.resolve_key(&document);
        let doc_str = serde_json::to_string(&document)
            .map_err(|e| err_msg(format!("JSON serialisation failed: {e}")))?;
        self.engine.execute(&format!(
            "UPDATE json_docs \
             SET document = '{}'::JSON, key = '{}', updated_at = {ts} \
             WHERE id = '{id}'",
            sql_escape(&doc_str),
            sql_escape(&key),
        ))
    }

    /// Delete the document for `id`.
    ///
    /// Returns `Ok(())` even if `id` does not exist.
    pub fn drop_json(&self, id: Uuid) -> Result<()> {
        self.engine
            .execute(&format!("DELETE FROM json_docs WHERE id = '{id}'"))
    }

    /// Return all `(id, document)` pairs in the store.
    pub fn list_all(&self) -> Result<Vec<(Uuid, JsonValue)>> {
        let rows = self.engine.select_all("SELECT id, document FROM json_docs")?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut cols = row.into_iter();
            let id_str = cols.next()
                .ok_or_else(|| err_msg("json_docs row missing id"))?
                .cast_string().map_err(|e| err_msg(e.to_string()))?;
            let doc_str = cols.next()
                .ok_or_else(|| err_msg("json_docs row missing document"))?
                .cast_string().map_err(|e| err_msg(e.to_string()))?;
            let id = Uuid::parse_str(&id_str)
                .map_err(|e| err_msg(format!("invalid UUID in json_docs: {e}")))?;
            let doc: JsonValue = serde_json::from_str(&doc_str)
                .map_err(|e| err_msg(format!("JSON parse failed: {e}")))?;
            out.push((id, doc));
        }
        Ok(out)
    }

    // ── internal ──────────────────────────────────────────────────────────────

    fn resolve_key(&self, doc: &JsonValue) -> String {
        self.config
            .key_field
            .as_deref()
            .and_then(|path| extract_key(doc, path))
            .unwrap_or_else(|| self.config.default_key.clone())
    }
}
