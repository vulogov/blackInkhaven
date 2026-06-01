//! Least-recently-used index for the project's voice
//! cache.
//!
//! Voice models are 25–100 MB each.  A project that ran
//! a French-novel pass last week + a Russian short-story
//! pass this morning + an English review just now could
//! easily accumulate 5–10 voices, blowing the
//! `cache_max_voices` cap.  The LRU index tracks "last
//! touched" (download OR use) per voice key + provides
//! eviction so a long-lived project doesn't grow without
//! bound.
//!
//! ## On-disk format
//!
//! `<voices_dir>/.lru` is a plain-text file, one entry
//! per line:
//!
//!   `<voice-key> <iso-8601-timestamp>`
//!
//! Example:
//!
//!   ```text
//!   en_US-lessac-medium 2026-06-01T14:23:00Z
//!   ru_RU-irina-medium 2026-06-01T14:22:00Z
//!   ```
//!
//! Plain-text on purpose: easy to inspect, easy to
//! truncate or delete by hand if a user wants to reset
//! the cache.  Malformed lines are silently dropped on
//! load (tolerance for hand-edits).
//!
//! ## Atomicity
//!
//! Writes go through `crate::io_atomic` — a corrupted
//! `.lru` is worse than a wrong one (it'd lose track of
//! every voice's usage), so we treat every save as a
//! critical-data write under the 1.2.15 standard.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::PiperUnavailable;

pub(crate) const LRU_FILENAME: &str = ".lru";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LruEntry {
    pub voice_key: String,
    /// Seconds since UNIX_EPOCH.  Stored as Duration so
    /// the round-trip through ISO 8601 doesn't lose
    /// precision (`SystemTime` round-trip via the
    /// formatter is messy on some platforms).
    pub last_touched: Duration,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LruIndex {
    /// Newest-first.  Touches move the touched entry
    /// to the front; eviction pops from the back.
    entries: Vec<LruEntry>,
}

impl LruIndex {
    /// Load the index from `<voices_dir>/.lru`.  Returns
    /// an empty index if the file is missing or any read
    /// I/O error fires — a missing or unreadable LRU
    /// file is not load-bearing for synthesis, and we
    /// regenerate on the next save.  Malformed lines are
    /// silently dropped.
    pub fn load(voices_dir: &Path) -> Self {
        let path = voices_dir.join(LRU_FILENAME);
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut entries = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some(entry) = parse_line(line) {
                entries.push(entry);
            }
        }
        Self { entries }
    }

    /// Persist the index atomically to
    /// `<voices_dir>/.lru`.
    pub fn save(&self, voices_dir: &Path) -> Result<(), PiperUnavailable> {
        let path = voices_dir.join(LRU_FILENAME);
        let body = self.serialise();
        std::fs::create_dir_all(voices_dir).map_err(|e| {
            PiperUnavailable::DownloadFailed(format!(
                "mkdir voices_dir for .lru: {e}",
            ))
        })?;
        crate::io_atomic::write(&path, body.as_bytes()).map_err(|e| {
            PiperUnavailable::DownloadFailed(format!(
                "atomic write .lru: {e}",
            ))
        })
    }

    /// Record a touch of `voice_key` at `now`.  Inserts
    /// a new entry at the front, or moves the existing
    /// entry to the front if `voice_key` was already
    /// tracked.
    pub fn touch(&mut self, voice_key: &str, now: SystemTime) {
        let when = now
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO);
        self.entries.retain(|e| e.voice_key != voice_key);
        self.entries.insert(
            0,
            LruEntry {
                voice_key: voice_key.to_string(),
                last_touched: when,
            },
        );
    }

    /// Trim the index to at most `max` entries.  Returns
    /// the voice keys that were evicted (oldest-first).
    /// `max == 0` is a documented no-op (caller's
    /// responsibility to set a sensible cap; we don't
    /// wipe the cache to zero on a config typo).
    pub fn evict_beyond(&mut self, max: usize) -> Vec<String> {
        if max == 0 || self.entries.len() <= max {
            return Vec::new();
        }
        let evicted: Vec<String> = self
            .entries
            .split_off(max)
            .into_iter()
            .map(|e| e.voice_key)
            .collect();
        evicted
    }

    /// Read-only view of the entries, newest-first.
    pub fn entries(&self) -> &[LruEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// True when `voice_key` is tracked at any
    /// position.  Convenience for tests + the picker.
    pub fn contains(&self, voice_key: &str) -> bool {
        self.entries.iter().any(|e| e.voice_key == voice_key)
    }

    fn serialise(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&entry.voice_key);
            out.push(' ');
            out.push_str(&format_iso8601(entry.last_touched));
            out.push('\n');
        }
        out
    }
}

/// Parse one `.lru` line.  Returns `None` on malformed
/// input so the loader can skip it without poisoning the
/// index.
fn parse_line(line: &str) -> Option<LruEntry> {
    let (voice_key, ts) = line.split_once(' ')?;
    if voice_key.is_empty() {
        return None;
    }
    let last_touched = parse_iso8601(ts)?;
    Some(LruEntry {
        voice_key: voice_key.to_string(),
        last_touched,
    })
}

/// Minimal ISO-8601 formatter — `YYYY-MM-DDTHH:MM:SSZ`,
/// UTC, second precision.  Inkhaven already uses chrono
/// extensively but adding a chrono dep to this module
/// for one timestamp is overkill; we round-trip via
/// `chrono::DateTime` here using the version that's
/// already in the workspace dep tree.
fn format_iso8601(d: Duration) -> String {
    let secs = d.as_secs() as i64;
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .unwrap_or_else(|| format!("@{secs}"))
}

fn parse_iso8601(ts: &str) -> Option<Duration> {
    if let Some(stripped) = ts.strip_prefix('@') {
        // Fallback shape from the formatter — raw secs.
        let secs: i64 = stripped.parse().ok()?;
        if secs < 0 {
            return None;
        }
        return Some(Duration::from_secs(secs as u64));
    }
    let dt = chrono::DateTime::parse_from_rfc3339(ts).ok()?;
    let secs = dt.timestamp();
    if secs < 0 {
        return None;
    }
    Some(Duration::from_secs(secs as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    // ── touch / evict semantics ───────────────────────

    #[test]
    fn touch_inserts_at_front() {
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.touch("b", t(200));
        let keys: Vec<&str> =
            idx.entries().iter().map(|e| e.voice_key.as_str()).collect();
        assert_eq!(keys, vec!["b", "a"]);
    }

    #[test]
    fn touch_moves_existing_to_front() {
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.touch("b", t(200));
        idx.touch("a", t(300));
        let keys: Vec<&str> =
            idx.entries().iter().map(|e| e.voice_key.as_str()).collect();
        assert_eq!(keys, vec!["a", "b"]);
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn touch_updates_timestamp() {
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.touch("a", t(200));
        assert_eq!(idx.entries()[0].last_touched.as_secs(), 200);
    }

    #[test]
    fn evict_beyond_keeps_newest() {
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.touch("b", t(200));
        idx.touch("c", t(300));
        idx.touch("d", t(400));
        let evicted = idx.evict_beyond(2);
        // Kept: d, c.  Evicted: b, a (oldest two,
        // returned in the order they sat in the
        // newest-first vec — so b before a).
        assert_eq!(evicted, vec!["b".to_string(), "a".to_string()]);
        let keys: Vec<&str> =
            idx.entries().iter().map(|e| e.voice_key.as_str()).collect();
        assert_eq!(keys, vec!["d", "c"]);
    }

    #[test]
    fn evict_beyond_noop_when_under_cap() {
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.touch("b", t(200));
        let evicted = idx.evict_beyond(5);
        assert!(evicted.is_empty());
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn evict_beyond_zero_is_noop() {
        // Defensive: don't wipe the cache to zero on a
        // misconfigured cap.
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        let evicted = idx.evict_beyond(0);
        assert!(evicted.is_empty());
        assert_eq!(idx.len(), 1);
    }

    // ── load / save round-trip ────────────────────────

    #[test]
    fn save_then_load_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let mut idx = LruIndex::default();
        idx.touch("en_US-lessac-medium", t(1_717_200_000));
        idx.touch("ru_RU-irina-medium", t(1_717_300_000));
        idx.save(tmp.path()).unwrap();
        let loaded = LruIndex::load(tmp.path());
        assert_eq!(loaded.len(), 2);
        let keys: Vec<&str> = loaded
            .entries()
            .iter()
            .map(|e| e.voice_key.as_str())
            .collect();
        // ru is newer → it lands first.
        assert_eq!(keys, vec!["ru_RU-irina-medium", "en_US-lessac-medium"]);
    }

    #[test]
    fn load_missing_file_yields_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let idx = LruIndex::load(tmp.path());
        assert!(idx.is_empty());
    }

    #[test]
    fn load_skips_malformed_lines() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join(LRU_FILENAME),
            "good_voice 2026-06-01T00:00:00Z\n\
             malformed_no_timestamp\n\
             \n\
             also_good 2026-06-02T00:00:00Z\n\
             garbage_ts not_a_timestamp\n",
        )
        .unwrap();
        let idx = LruIndex::load(tmp.path());
        assert_eq!(idx.len(), 2);
        assert!(idx.contains("good_voice"));
        assert!(idx.contains("also_good"));
        assert!(!idx.contains("malformed_no_timestamp"));
    }

    #[test]
    fn save_creates_voices_dir_if_missing() {
        let parent = tempfile::tempdir().unwrap();
        let voices_dir = parent.path().join("voices");
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.save(&voices_dir).unwrap();
        assert!(voices_dir.join(LRU_FILENAME).exists());
    }

    #[test]
    fn save_is_atomic() {
        // Verify save uses io_atomic — no leftover
        // temp-files in the dir after a successful
        // save.  Mirrors the catalog's atomic-write
        // assertion.
        let tmp = tempfile::tempdir().unwrap();
        let mut idx = LruIndex::default();
        idx.touch("a", t(100));
        idx.save(tmp.path()).unwrap();
        let stray: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                name.starts_with(".lru") && name != LRU_FILENAME
            })
            .collect();
        assert!(
            stray.is_empty(),
            "expected no leftover atomic-temp files, got: {stray:?}",
        );
    }

    // ── ISO 8601 round-trip ───────────────────────────

    #[test]
    fn iso8601_round_trips() {
        let d = Duration::from_secs(1_717_200_000);
        let s = format_iso8601(d);
        let parsed = parse_iso8601(&s).unwrap();
        assert_eq!(parsed, d);
    }

    #[test]
    fn parse_iso8601_rejects_garbage() {
        assert!(parse_iso8601("not_a_timestamp").is_none());
        assert!(parse_iso8601("").is_none());
    }

    #[test]
    fn parse_iso8601_accepts_at_fallback() {
        let parsed = parse_iso8601("@1717200000").unwrap();
        assert_eq!(parsed.as_secs(), 1_717_200_000);
    }

    #[test]
    fn lru_struct_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<LruIndex>();
        assert_send_sync::<LruEntry>();
    }
}
