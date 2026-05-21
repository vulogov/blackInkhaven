//! DuckDB-backed writing-progress store.
//!
//! Holds the append-only `writing_events` table plus
//! `writing_baselines` (one row per day per book + project-wide).
//! Connection pool size is small — progress writes are infrequent
//! (one per save), and readers are limited to the status-bar
//! redraw + the Ctrl+V G modal which open one snapshot at a time.

use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Result;
use uuid::Uuid;

use crate::storage::engine::StorageEngine;
use duckdb::types::Value as DuckValue;

/// Sentinel UUID used as the `book_id` for project-wide baselines
/// and aggregates ("words across all user books today"). Chosen
/// deterministically so the value is stable across processes.
pub const PROJECT_SCOPE_BOOK_ID: Uuid =
    Uuid::from_u128(0x10001000_1000_1000_1000_100010001000);

const INIT_SQL: &str = "
    CREATE SEQUENCE IF NOT EXISTS writing_event_id_seq START 1;

    CREATE TABLE IF NOT EXISTS writing_events (
        id          BIGINT  PRIMARY KEY DEFAULT nextval('writing_event_id_seq'),
        ts          BIGINT  NOT NULL,        -- unix-seconds
        node_id     TEXT,                    -- nullable for project-wide events
        book_id     TEXT,                    -- nullable for project-wide events
        kind        TEXT    NOT NULL,        -- save | status_change | snapshot | delete
        word_delta  INTEGER NOT NULL DEFAULT 0,
        total_words INTEGER NOT NULL DEFAULT 0,
        extra_json  TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_events_ts   ON writing_events(ts);
    CREATE INDEX IF NOT EXISTS idx_events_book ON writing_events(book_id);
    CREATE INDEX IF NOT EXISTS idx_events_node ON writing_events(node_id);

    CREATE TABLE IF NOT EXISTS writing_baselines (
        day         INTEGER NOT NULL,        -- days since epoch (UTC)
        book_id     TEXT    NOT NULL,        -- includes PROJECT_SCOPE_BOOK_ID for project-wide
        total_words INTEGER NOT NULL,
        PRIMARY KEY (day, book_id)
    );
";

/// Public store handle. Cloneable; clones share the connection
/// pool — but only one handle is installed via `progress::install`
/// at a time, so concurrent access is by design rare.
#[derive(Clone)]
pub struct ProgressStore {
    engine: std::sync::Arc<StorageEngine>,
}

impl ProgressStore {
    pub fn open(path: &Path) -> Result<Self> {
        let engine = StorageEngine::new(path, INIT_SQL, 2)?;
        Ok(Self {
            engine: std::sync::Arc::new(engine),
        })
    }

    /// Append one event row. `node_id` / `book_id` may be `None`
    /// for project-wide events. `extra` is a JSON string the
    /// status-ladder analyser parses — pass `None` when the
    /// event has no payload.
    pub fn record_event(
        &self,
        kind: &str,
        node_id: Uuid,
        book_id: Option<Uuid>,
        word_delta: i64,
        total_words: i64,
        extra: Option<&str>,
    ) -> Result<()> {
        let ts = now_unix_secs();
        let node_str = node_id.to_string();
        let book_str = book_id.map(|b| b.to_string());
        // duckdb-rs ToSql for Option<String> wants us to pass
        // the inner Option<&str>; pass via params_from_iter.
        self.engine.execute_with(
            "INSERT INTO writing_events
             (ts, node_id, book_id, kind, word_delta, total_words, extra_json)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[
                &ts,
                &node_str,
                &book_str,
                &kind,
                &word_delta,
                &total_words,
                &extra,
            ],
        )?;
        Ok(())
    }

    /// Snapshot per-book + project-wide baselines for today.
    /// Idempotent — the second call inside the same UTC day is
    /// a silent no-op (DuckDB's `INSERT OR IGNORE` equivalent
    /// is `INSERT ... ON CONFLICT DO NOTHING`).
    pub fn capture_baselines_today(
        &self,
        per_book: &[(Uuid, i64)],
        project_total: i64,
    ) -> Result<()> {
        let day = today_utc_days();
        for (book, total) in per_book {
            let book_str = book.to_string();
            self.engine.execute_with(
                "INSERT INTO writing_baselines (day, book_id, total_words)
                 VALUES (?, ?, ?)
                 ON CONFLICT (day, book_id) DO NOTHING",
                &[&day, &book_str, total],
            )?;
        }
        let project_str = PROJECT_SCOPE_BOOK_ID.to_string();
        self.engine.execute_with(
            "INSERT INTO writing_baselines (day, book_id, total_words)
             VALUES (?, ?, ?)
             ON CONFLICT (day, book_id) DO NOTHING",
            &[&day, &project_str, &project_total],
        )?;
        Ok(())
    }

    /// Fetch the baseline for `(day, book_id)`. Returns None when
    /// no baseline exists — the project wasn't open that day.
    pub fn baseline_for(&self, day: i64, book_id: Uuid) -> Result<Option<i64>> {
        let book_str = book_id.to_string();
        let rows = self.engine.select_all(&format!(
            "SELECT total_words FROM writing_baselines
             WHERE day = {day} AND book_id = '{book}'",
            book = sql_escape(&book_str),
        ))?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => match row.into_iter().next() {
                Some(DuckValue::Int(i)) => Ok(Some(i as i64)),
                Some(DuckValue::BigInt(i)) => Ok(Some(i)),
                _ => Ok(None),
            },
        }
    }

    /// Today's net delta for `book_id` = (current total) − (today's
    /// baseline). `current_total` is supplied by the caller because
    /// it has cheap access to the live hierarchy + paragraph word
    /// counts; the store doesn't shadow that data.
    pub fn today_words(&self, book_id: Uuid, current_total: i64) -> Result<i64> {
        let day = today_utc_days();
        match self.baseline_for(day, book_id)? {
            Some(base) => Ok(current_total - base),
            None => Ok(0),
        }
    }

    /// Vector of `(days-ago, words_written_that_day)` for the last
    /// `n` days, oldest first. A day with no baseline maps to 0
    /// — the project simply wasn't open that day.
    ///
    /// `words_written_that_day = baseline(d+1) − baseline(d)` for
    /// past days; for today, it's `current_total − baseline(today)`.
    pub fn last_n_daily(
        &self,
        book_id: Uuid,
        current_total: i64,
        n: usize,
    ) -> Result<Vec<i64>> {
        let today = today_utc_days();
        let mut out = Vec::with_capacity(n);
        // Pre-fetch every baseline in the range so we don't do
        // N round-trips. DuckDB handles small range scans
        // efficiently.
        let book_str = book_id.to_string();
        let earliest = today - n as i64;
        let rows = self.engine.select_all(&format!(
            "SELECT day, total_words FROM writing_baselines
             WHERE book_id = '{book}' AND day >= {earliest}
             ORDER BY day ASC",
            book = sql_escape(&book_str),
        ))?;
        let mut bl: std::collections::HashMap<i64, i64> =
            std::collections::HashMap::new();
        for row in rows {
            let mut it = row.into_iter();
            let day = match it.next() {
                Some(DuckValue::Int(i)) => i as i64,
                Some(DuckValue::BigInt(i)) => i,
                _ => continue,
            };
            let tw = match it.next() {
                Some(DuckValue::Int(i)) => i as i64,
                Some(DuckValue::BigInt(i)) => i,
                _ => continue,
            };
            bl.insert(day, tw);
        }
        for i in 0..n {
            let d = today - (n as i64 - 1) + i as i64;
            let next_d_total = if d == today {
                current_total
            } else {
                // Use the next day's baseline as the "end" of
                // day `d`. If the next-day baseline is missing
                // (project wasn't open), fall back to this
                // day's baseline (zero delta).
                *bl.get(&(d + 1)).unwrap_or_else(|| {
                    bl.get(&d).unwrap_or(&0)
                })
            };
            let this_d = *bl.get(&d).unwrap_or(&0);
            let written = (next_d_total - this_d).max(0);
            out.push(written);
        }
        Ok(out)
    }

    /// Count status-promotion events in the trailing `days_back`
    /// days, grouped by the `to` status (e.g.
    /// `[("ready", 2), ("final", 5)]`).
    pub fn status_promotions_recent(
        &self,
        days_back: i64,
    ) -> Result<Vec<(String, i64)>> {
        let cutoff_secs = now_unix_secs() - days_back * 86_400;
        let rows = self.engine.select_all(&format!(
            "SELECT
                 json_extract_string(extra_json, '$.to') AS to_status,
                 COUNT(*) AS n
             FROM writing_events
             WHERE kind = 'status_change' AND ts >= {cutoff_secs}
             GROUP BY to_status",
        ))?;
        let mut out: Vec<(String, i64)> = Vec::new();
        for row in rows {
            let mut it = row.into_iter();
            let status = match it.next() {
                Some(DuckValue::Text(s)) => s,
                _ => continue,
            };
            let n = match it.next() {
                Some(DuckValue::Int(i)) => i as i64,
                Some(DuckValue::BigInt(i)) => i,
                _ => 0,
            };
            if !status.is_empty() {
                out.push((status, n));
            }
        }
        Ok(out)
    }

    /// Days (UTC-day numbers) with at least one positive
    /// `word_delta` save event, in descending order. Used by the
    /// streak computation.
    pub fn writing_days_recent(&self, days_back: i64) -> Result<Vec<i64>> {
        let cutoff_secs = now_unix_secs() - days_back * 86_400;
        let rows = self.engine.select_all(&format!(
            "SELECT DISTINCT (ts / 86400) AS day
             FROM writing_events
             WHERE kind = 'save' AND word_delta > 0 AND ts >= {cutoff_secs}
             ORDER BY day DESC",
        ))?;
        let mut out = Vec::new();
        for row in rows {
            match row.into_iter().next() {
                Some(DuckValue::Int(i)) => out.push(i as i64),
                Some(DuckValue::BigInt(i)) => out.push(i),
                _ => {}
            }
        }
        Ok(out)
    }

    /// Active writing time inside the window `[from_secs, until_secs)`
    /// — sum of gaps between consecutive `save` events, with each
    /// gap capped at `cap_seconds` so AFK time doesn't inflate the
    /// total. Per the user spec: 300 s (5 min) is the default cap.
    ///
    /// Returns total seconds of active time. The first event of
    /// the window contributes zero (no prior gap to measure
    /// against); a single save in isolation = 0 active seconds.
    /// Saves with word_delta == 0 still count — opening a file
    /// and re-saving a metadata change is still "time at the
    /// keyboard" in this model.
    pub fn active_seconds_in_range(
        &self,
        from_secs: i64,
        until_secs: i64,
        cap_seconds: i64,
    ) -> Result<i64> {
        // DuckDB's LAG inside a CTE gives us the prior save's
        // timestamp; LEAST clamps each gap to the cap. We coalesce
        // the SUM so a no-events window returns 0 instead of NULL.
        let rows = self.engine.select_all(&format!(
            "WITH saves AS (
                 SELECT ts FROM writing_events
                 WHERE kind = 'save' AND ts >= {from_secs} AND ts < {until_secs}
                 ORDER BY ts
             ),
             gaps AS (
                 SELECT ts - LAG(ts) OVER (ORDER BY ts) AS gap FROM saves
             )
             SELECT COALESCE(SUM(LEAST(gap, {cap_seconds})), 0)
             FROM gaps WHERE gap IS NOT NULL",
        ))?;
        let value = rows
            .into_iter()
            .next()
            .and_then(|row| row.into_iter().next());
        Ok(match value {
            Some(DuckValue::Int(i)) => i as i64,
            Some(DuckValue::BigInt(i)) => i,
            Some(DuckValue::HugeInt(i)) => i as i64,
            Some(DuckValue::Double(f)) => f as i64,
            _ => 0,
        })
    }
}

/// Days since the Unix epoch, UTC.
pub fn today_utc_days() -> i64 {
    let secs = now_unix_secs();
    secs / 86_400
}

fn now_unix_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn sql_escape(s: &str) -> String {
    s.replace('\'', "''")
}

