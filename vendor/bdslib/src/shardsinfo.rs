use crate::common::error::{err_msg, Result};
use crate::common::sql::sql_escape;
use crate::common::timerange::to_unix_secs;
use crate::common::uuid::generate_v7;
use crate::StorageEngine;
use rust_dynamic::value::Value as DynamicValue;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS shards (
        shard_id TEXT   NOT NULL PRIMARY KEY,
        path     TEXT   NOT NULL,
        start_ts BIGINT NOT NULL,
        end_ts   BIGINT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_shards_start_ts ON shards (start_ts);
    CREATE INDEX IF NOT EXISTS idx_shards_end_ts   ON shards (end_ts);
";

/// Metadata record for a single shard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShardInfo {
    pub shard_id: Uuid,
    pub path: String,
    pub start_time: SystemTime,
    pub end_time: SystemTime,
}

/// Thread-safe storage for shard metadata backed by [`StorageEngine`].
///
/// Each shard covers the half-open interval `[start_time, end_time)`.
/// `ShardInfoEngine` is `Clone`; all clones share the same underlying
/// connection pool.
#[derive(Clone)]
pub struct ShardInfoEngine {
    engine: Arc<StorageEngine>,
}

impl ShardInfoEngine {
    /// Open or create a shard-info database at `path`.
    ///
    /// The required table is created automatically if it does not exist.
    /// Pass `":memory:"` for an ephemeral in-process store.
    pub fn new(path: &str, pool_size: u32) -> Result<Self> {
        let engine = StorageEngine::new(path, INIT_SQL, pool_size)?;
        Ok(Self {
            engine: Arc::new(engine),
        })
    }

    /// Store metadata for a new shard and return its generated UUIDv7.
    ///
    /// `path` is the filesystem location of the shard data.
    /// `start_time` must be strictly before `end_time`.
    pub fn add_shard(
        &self,
        path: &str,
        start_time: SystemTime,
        end_time: SystemTime,
    ) -> Result<Uuid> {
        if start_time >= end_time {
            return Err(err_msg("start_time must be strictly before end_time"));
        }
        let id = generate_v7();
        let start_ts = to_unix_secs(start_time)?;
        let end_ts = to_unix_secs(end_time)?;
        self.engine.execute(&format!(
            "INSERT INTO shards VALUES ('{id}', '{}', {start_ts}, {end_ts})",
            sql_escape(path),
        ))?;
        Ok(id)
    }

    /// Return all shards whose interval `[start_time, end_time)` contains `timestamp`.
    ///
    /// Results are ordered by `start_time` ascending. Returns an empty `Vec`
    /// when no shard covers `timestamp`.
    pub fn shards_at(&self, timestamp: SystemTime) -> Result<Vec<ShardInfo>> {
        let ts = to_unix_secs(timestamp)?;
        let rows = self.engine.select_all(&format!(
            "SELECT shard_id, path, start_ts, end_ts \
             FROM shards \
             WHERE start_ts <= {ts} AND end_ts > {ts} \
             ORDER BY start_ts ASC"
        ))?;
        rows.into_iter().map(row_to_shard_info).collect()
    }

    /// Return all registered shards ordered by `start_time` ascending.
    pub fn list_all(&self) -> Result<Vec<ShardInfo>> {
        let rows = self.engine.select_all(
            "SELECT shard_id, path, start_ts, end_ts FROM shards ORDER BY start_ts ASC",
        )?;
        rows.into_iter().map(row_to_shard_info).collect()
    }

    /// Return all shards whose interval overlaps the half-open window `[start, end)`.
    ///
    /// A shard overlaps the window when `shard.end_ts > start AND shard.start_ts < end`.
    /// Results are ordered by `start_time` ascending.
    pub fn shards_in_range(
        &self,
        start: SystemTime,
        end: SystemTime,
    ) -> Result<Vec<ShardInfo>> {
        let start_ts = to_unix_secs(start)?;
        let end_ts = to_unix_secs(end)?;
        let rows = self.engine.select_all(&format!(
            "SELECT shard_id, path, start_ts, end_ts \
             FROM shards \
             WHERE end_ts > {start_ts} AND start_ts < {end_ts} \
             ORDER BY start_ts ASC"
        ))?;
        rows.into_iter().map(row_to_shard_info).collect()
    }

    /// Return `true` if at least one shard covers `timestamp`.
    pub fn shard_exists_at(&self, timestamp: SystemTime) -> Result<bool> {
        let ts = to_unix_secs(timestamp)?;
        let rows = self.engine.select_all(&format!(
            "SELECT COUNT(*) FROM shards WHERE start_ts <= {ts} AND end_ts > {ts}"
        ))?;
        let count = rows
            .first()
            .and_then(|r| r.first())
            .ok_or_else(|| err_msg("COUNT query returned no rows"))?
            .cast_int()
            .map_err(|e| err_msg(e.to_string()))?;
        Ok(count > 0)
    }
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn row_to_shard_info(row: Vec<DynamicValue>) -> Result<ShardInfo> {
    let shard_id_str = row[0].cast_string().map_err(|e| err_msg(e.to_string()))?;
    let shard_id = Uuid::parse_str(&shard_id_str)
        .map_err(|e| err_msg(format!("invalid UUID in shards table: {e}")))?;
    let path = row[1].cast_string().map_err(|e| err_msg(e.to_string()))?;
    let start_ts = row[2].cast_int().map_err(|e| err_msg(e.to_string()))?;
    let end_ts = row[3].cast_int().map_err(|e| err_msg(e.to_string()))?;
    Ok(ShardInfo {
        shard_id,
        path,
        start_time: UNIX_EPOCH + Duration::from_secs(start_ts as u64),
        end_time: UNIX_EPOCH + Duration::from_secs(end_ts as u64),
    })
}
