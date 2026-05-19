//! Event-frequency tracking backed by DuckDB via [`StorageEngine`].
//!
//! Each observation is a `(timestamp, id)` pair where `id` is an arbitrary
//! string label (metric name, user identifier, cluster id, IP address, …)
//! and `timestamp` is a Unix-seconds integer.  The same pair can be recorded
//! many times, so the store faithfully captures actual event rates rather than
//! merely tracking existence.
//!
//! # Typical use-cases
//!
//! - Count how often a drain3 template cluster appears over time.
//! - Track per-user or per-IP access frequency.
//! - Record every firing of an alerting rule so its cadence can be analysed.
//!
//! # Threading
//!
//! `FrequencyTracking` is `Clone`; all clones share the same underlying
//! DuckDB connection pool.  Any number of threads may read and write
//! concurrently.

use crate::common::error::{err_msg, Result};
use crate::common::sql::sql_escape;
use crate::common::time::{lookback_window, now_secs};
use crate::StorageEngine;
use rust_dynamic::value::Value as DynamicValue;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

// ── schema ────────────────────────────────────────────────────────────────────

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS frequency_tracking (
        ts BIGINT NOT NULL,
        id TEXT   NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_ft_id    ON frequency_tracking (id);
    CREATE INDEX IF NOT EXISTS idx_ft_ts    ON frequency_tracking (ts);
    CREATE INDEX IF NOT EXISTS idx_ft_id_ts ON frequency_tracking (id, ts);
";

// ── FrequencyTracking ─────────────────────────────────────────────────────────

/// Thread-safe event-frequency store.
///
/// Records `(timestamp, id)` observations and supports querying by id, by
/// exact timestamp, by time range, and by lookback duration.
///
/// Open a persistent store with [`new`](FrequencyTracking::new) or pass
/// `":memory:"` for an ephemeral in-process instance.
///
/// # Example
///
/// ```rust,no_run
/// use bdslib::FrequencyTracking;
///
/// let ft = FrequencyTracking::new(":memory:", 4).unwrap();
///
/// // Record observations.
/// ft.add("api.login").unwrap();
/// ft.add("api.search").unwrap();
/// ft.add("api.login").unwrap();
///
/// // List every timestamp at which "api.login" fired.
/// let ts = ft.by_id("api.login").unwrap();
/// assert_eq!(ts.len(), 2);
///
/// // Which IDs fired in the last minute?
/// let ids = ft.recent("1min").unwrap();
/// ```
#[derive(Clone)]
pub struct FrequencyTracking {
    engine: Arc<StorageEngine>,
}

impl FrequencyTracking {
    // ── construction ──────────────────────────────────────────────────────────

    /// Open or create a frequency-tracking store at `path`.
    ///
    /// The required table and indices are created automatically if they do not
    /// exist.  Pass `":memory:"` for an ephemeral in-process store.
    ///
    /// `pool_size` controls the maximum number of concurrent DuckDB
    /// connections; `4` is a reasonable default for most workloads.
    pub fn new(path: &str, pool_size: u32) -> Result<Self> {
        let engine = StorageEngine::new(path, INIT_SQL, pool_size)?;
        Ok(Self { engine: Arc::new(engine) })
    }

    // ── writes ────────────────────────────────────────────────────────────────

    /// Record one observation of `id` at an explicit Unix-seconds `timestamp`.
    ///
    /// The same `(timestamp, id)` pair may be inserted more than once; each
    /// insertion produces a separate row so that event counts are preserved.
    pub fn add_with_timestamp(&self, timestamp: u64, id: &str) -> Result<()> {
        self.engine.execute(&format!(
            "INSERT INTO frequency_tracking (ts, id) VALUES ({timestamp}, '{}')",
            sql_escape(id),
        ))
    }

    /// Record one observation of `id` at the current wall-clock time.
    ///
    /// Equivalent to `add_with_timestamp(now_secs(), id)`.
    pub fn add(&self, id: &str) -> Result<()> {
        self.add_with_timestamp(now_secs(), id)
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return all timestamps (Unix seconds, ascending) at which `id` was
    /// observed.
    ///
    /// Multiple events at the same second each produce a separate entry, so
    /// `by_id("x").len()` reflects the total number of times `"x"` was
    /// recorded.
    ///
    /// Returns an empty `Vec` if `id` has never been recorded.
    pub fn by_id(&self, id: &str) -> Result<Vec<u64>> {
        let rows = self.engine.select_all(&format!(
            "SELECT ts FROM frequency_tracking \
             WHERE id = '{}' ORDER BY ts ASC",
            sql_escape(id),
        ))?;
        rows.into_iter()
            .map(|row| {
                let ts = row.into_iter()
                    .next()
                    .ok_or_else(|| err_msg("frequency_tracking row missing ts column"))?
                    .cast_int()
                    .map_err(|e| err_msg(e.to_string()))?;
                Ok(ts as u64)
            })
            .collect()
    }

    /// Return the distinct IDs observed at the given exact Unix-seconds
    /// `timestamp`, sorted alphabetically.
    ///
    /// Returns an empty `Vec` if nothing was recorded at that second.
    pub fn by_timestamp(&self, timestamp: u64) -> Result<Vec<String>> {
        let rows = self.engine.select_all(&format!(
            "SELECT DISTINCT id FROM frequency_tracking \
             WHERE ts = {timestamp} ORDER BY id ASC",
        ))?;
        Self::rows_to_ids(rows)
    }

    /// Return the distinct IDs that have at least one observation in the
    /// inclusive time interval `[start, end]` (both Unix seconds).
    ///
    /// Results are sorted alphabetically.  Returns an empty `Vec` if no
    /// records fall in the range.
    pub fn time_range(&self, start: u64, end: u64) -> Result<Vec<String>> {
        let rows = self.engine.select_all(&format!(
            "SELECT DISTINCT id FROM frequency_tracking \
             WHERE ts >= {start} AND ts <= {end} ORDER BY id ASC",
        ))?;
        Self::rows_to_ids(rows)
    }

    /// Return the distinct IDs observed in the window `[now − duration, now]`.
    ///
    /// `duration` is a human-readable string accepted by
    /// [`humantime::parse_duration`], such as `"30s"`, `"5min"`, `"1h"`,
    /// `"7days"`.  The window end is `now + 1 s` so that events written at
    /// exactly `now` are always included.
    ///
    /// Results are sorted alphabetically.
    pub fn recent(&self, duration: &str) -> Result<Vec<String>> {
        let (start, end) = lookback_window(duration)?;
        let s = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let e = end.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        self.time_range(s, e)
    }

    // ── deletes ───────────────────────────────────────────────────────────────

    /// Remove all tracking rows for `id`.
    ///
    /// Returns `Ok(())` even if `id` has never been recorded.
    pub fn delete(&self, id: &str) -> Result<()> {
        self.engine.execute(&format!(
            "DELETE FROM frequency_tracking WHERE id = '{}'",
            sql_escape(id),
        ))
    }

    // ── maintenance ───────────────────────────────────────────────────────────

    /// Flush the WAL to disk via a DuckDB `CHECKPOINT`.
    ///
    /// Call periodically for long-running processes or when the store must be
    /// readable from another process.
    pub fn sync(&self) -> Result<()> {
        self.engine.sync()
    }

    // ── helpers ───────────────────────────────────────────────────────────────

    fn rows_to_ids(rows: Vec<Vec<DynamicValue>>) -> Result<Vec<String>> {
        rows.into_iter()
            .map(|row| {
                row.into_iter()
                    .next()
                    .ok_or_else(|| err_msg("frequency_tracking row missing id column"))?
                    .cast_string()
                    .map_err(|e| err_msg(e.to_string()))
            })
            .collect()
    }
}
