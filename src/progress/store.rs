//! DuckDB-backed writing-progress store.
//!
//! Holds the append-only `writing_events` table plus
//! `writing_baselines` (one row per day per book + project-wide).
//! Connection pool size is small — progress writes are infrequent
//! (one per save), and readers are limited to the status-bar
//! redraw + the Ctrl+V G modal which open one snapshot at a time.

use std::path::Path;

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
        let engine = StorageEngine::new_versioned(path, INIT_SQL, 2, 1)?;
        Ok(Self {
            engine: std::sync::Arc::new(engine),
        })
    }

    /// Test-only: an event stamped `ts` instead of now, so a probe can seed
    /// writing days in the past and drive the streak hooks.
    #[cfg(test)]
    pub fn record_event_at(&self, ts: i64, kind: &str, node_id: Uuid, book_id: Option<Uuid>, word_delta: i64, total_words: i64) -> Result<()> {
        let node_str = node_id.to_string();
        let book_str = book_id.map(|b| b.to_string());
        let extra: Option<&str> = None;
        self.engine.execute_with(
            "INSERT INTO writing_events
             (ts, node_id, book_id, kind, word_delta, total_words, extra_json)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[&ts, &node_str, &book_str, &kind, &word_delta, &total_words, &extra],
        )?;
        Ok(())
    }

    /// Test-only: a baseline row for an arbitrary day (the heatmap and the
    /// sparkline derive a day's words from consecutive baselines).
    #[cfg(test)]
    pub fn record_baseline_at(&self, day: i64, book_id: Uuid, total_words: i64) -> Result<()> {
        let book_str = book_id.to_string();
        self.engine.execute_with(
            "INSERT INTO writing_baselines (day, book_id, total_words)
             VALUES (?, ?, ?)
             ON CONFLICT (day, book_id) DO UPDATE SET total_words = excluded.total_words",
            &[&day, &book_str, &total_words],
        )?;
        Ok(())
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
        let day = crate::dayclock::today_days();
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
        let rows = self.engine.select_all_with(
            "SELECT total_words FROM writing_baselines
             WHERE day = ? AND book_id = ?",
            &[&day, &book_str],
        )?;
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
        let day = crate::dayclock::today_days();
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
        let today = crate::dayclock::today_days();
        let mut out = Vec::with_capacity(n);
        // Pre-fetch every baseline in the range so we don't do
        // N round-trips. DuckDB handles small range scans
        // efficiently.
        let book_str = book_id.to_string();
        let earliest = today - n as i64;
        let rows = self.engine.select_all_with(
            "SELECT day, total_words FROM writing_baselines
             WHERE book_id = ? AND day >= ?
             ORDER BY day ASC",
            &[&book_str, &earliest],
        )?;
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
            // 3.22 — a day with no baseline of its own reads 0 (the project
            // wasn't open), even when the next day has one: diffing the
            // next baseline against an imagined 0 credited the whole
            // manuscript to the first day before the history began.
            let written = match bl.get(&d) {
                Some(this_d) => (next_d_total - this_d).max(0),
                None => 0,
            };
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
        let rows = self.engine.select_all_with(
            "SELECT
                 json_extract_string(extra_json, '$.to') AS to_status,
                 COUNT(*) AS n
             FROM writing_events
             WHERE kind = 'status_change' AND ts >= ?
             GROUP BY to_status",
            &[&cutoff_secs],
        )?;
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
        // Bin events into days under the active boundary (offset 0 = UTC)
        // so the streak agrees with `today_days` (1.3.37 day_boundary).
        // 3.22 — `/` is float division in DuckDB, so the old `(ts + ?) / 86400`
        // came back as DOUBLE rows, which the integer-only reader below
        // dropped: every project reported a 0-day streak, an empty heatmap
        // and no `hook.on_streak_milestone`. Floor-divide like
        // `dayclock::today_days` does and read the row as an integer.
        let offset = crate::dayclock::offset_secs();
        let rows = self.engine.select_all_with(
            "SELECT DISTINCT CAST(floor((ts + ?) / 86400) AS BIGINT) AS day
             FROM writing_events
             WHERE kind = 'save' AND word_delta > 0 AND ts >= ?
             ORDER BY day DESC",
            &[&offset, &cutoff_secs],
        )?;
        let mut out = Vec::new();
        for row in rows {
            match row.into_iter().next() {
                Some(DuckValue::Int(i)) => out.push(i as i64),
                Some(DuckValue::BigInt(i)) => out.push(i),
                Some(DuckValue::HugeInt(i)) => out.push(i as i64),
                Some(DuckValue::Double(f)) => out.push(f.floor() as i64),
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
        let rows = self.engine.select_all_with(
            "WITH saves AS (
                 SELECT ts FROM writing_events
                 WHERE kind = 'save' AND ts >= ? AND ts < ?
                 ORDER BY ts
             ),
             gaps AS (
                 SELECT ts - LAG(ts) OVER (ORDER BY ts) AS gap FROM saves
             )
             SELECT COALESCE(SUM(LEAST(gap, ?)), 0)
             FROM gaps WHERE gap IS NOT NULL",
            &[&from_secs, &until_secs, &cap_seconds],
        )?;
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

/// The clock every event is stamped with — `dayclock`'s, so the bins agree
/// with `today_days` and a probe's shifted clock moves the events too.
fn now_unix_secs() -> i64 {
    crate::dayclock::now_secs()
}


#[cfg(test)]
mod tests {
    use super::*;

    /// 3.22 regression — the writing-day query must come back as integer
    /// day numbers (DuckDB's `/` is float division), one per day with a
    /// positive save, newest first, and never count an empty save.
    #[test]
    fn writing_days_recent_bins_positive_saves_by_day() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ProgressStore::open(&tmp.path().join("progress.duckdb")).unwrap();
        let now = now_unix_secs();
        let today = (now + crate::dayclock::offset_secs()).div_euclid(86_400);
        let pid = Uuid::new_v4();
        store.record_event_at(now, "save", pid, None, 10, 100).unwrap();
        store.record_event_at(now - 3 * 86_400, "save", pid, None, 5, 90).unwrap();
        store.record_event_at(now - 3 * 86_400 + 60, "save", pid, None, 2, 92).unwrap();
        // Empty saves and status flips don't tick the streak.
        store.record_event_at(now - 86_400, "save", pid, None, 0, 90).unwrap();
        store.record_event_at(now - 2 * 86_400, "status_change", pid, None, 7, 90).unwrap();
        let days = store.writing_days_recent(30).unwrap();
        assert_eq!(days, vec![today, today - 3]);
    }

    /// The other aggregate readers, pinned for the same DuckDB typing reason:
    /// `SUM` over BIGINT comes back as HUGEINT, `COUNT(*)` as BIGINT.
    #[test]
    fn active_seconds_sums_capped_gaps_between_saves() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ProgressStore::open(&tmp.path().join("progress.duckdb")).unwrap();
        let pid = Uuid::new_v4();
        let t0 = 1_700_000_000;
        // 60 s, 120 s, then a 2 h break (capped to 300), then 30 s.
        for (i, ts) in [t0, t0 + 60, t0 + 180, t0 + 180 + 7_200, t0 + 180 + 7_230].iter().enumerate() {
            store.record_event_at(*ts, "save", pid, None, i as i64, 10).unwrap();
        }
        let secs = store.active_seconds_in_range(t0 - 1, t0 + 10_000, 300).unwrap();
        assert_eq!(secs, 60 + 120 + 300 + 30);
        assert_eq!(store.active_seconds_in_range(t0 + 10_000, t0 + 20_000, 300).unwrap(), 0);
    }

    #[test]
    fn status_promotions_group_by_target_status() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ProgressStore::open(&tmp.path().join("progress.duckdb")).unwrap();
        let pid = Uuid::new_v4();
        for to in ["ready", "final", "ready"] {
            let extra = serde_json::json!({ "from": "draft", "to": to }).to_string();
            store.record_event("status_change", pid, None, 0, 10, Some(&extra)).unwrap();
        }
        let mut rows = store.status_promotions_recent(7).unwrap();
        rows.sort();
        assert_eq!(rows, vec![("final".to_string(), 1), ("ready".to_string(), 2)]);
    }

    #[test]
    fn daily_words_come_from_consecutive_baselines() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ProgressStore::open(&tmp.path().join("progress.duckdb")).unwrap();
        let today = crate::dayclock::today_days();
        let book = Uuid::new_v4();
        // Baselines: start of day-2 = 100, day-1 = 130, today = 130 (nothing
        // written yesterday), live total now 145.
        store.record_baseline_at(today - 2, book, 100).unwrap();
        store.record_baseline_at(today - 1, book, 130).unwrap();
        store.record_baseline_at(today, book, 130).unwrap();
        assert_eq!(store.last_n_daily(book, 145, 3).unwrap(), vec![30, 0, 15]);
        // The day before the first baseline has no baseline of its own: it
        // reads 0, not "everything up to the first baseline".
        assert_eq!(store.last_n_daily(book, 145, 4).unwrap(), vec![0, 30, 0, 15]);
        assert_eq!(store.today_words(book, 145).unwrap(), 15);
        // No baseline for a day → that day reads as 0, not as the whole total.
        assert_eq!(store.today_words(Uuid::new_v4(), 145).unwrap(), 0);
    }
}
