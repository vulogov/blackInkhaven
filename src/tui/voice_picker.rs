//! Pure state model for the TTS voice picker
//! (`Ctrl+B Shift+V`).
//!
//! The picker shows every voice known to the project —
//! catalog hits + already-downloaded voices the catalog
//! doesn't know about (e.g. when the catalog refresh
//! failed but the user has voices on disk).  Each entry
//! carries enough info to render its row + decide which
//! action keys make sense (download vs. select vs.
//! remove).
//!
//! ## Sort order
//!
//! Language code ascending (en_US before ru_RU), then
//! quality tier descending (high before medium), then
//! voice key ascending (stable lexicographic
//! tie-break).  This matches the
//! `Catalog::voices_for_language` order so users
//! browsing by language see the best quality first.
//!
//! ## Filter
//!
//! `filter` is a free-form needle; rows are kept when
//! the needle appears (case-insensitive) in the voice
//! key, language code, or English language name.  Empty
//! filter → all rows.

use std::collections::BTreeSet;
use std::path::Path;

use super::piper::catalog::{Catalog, VoiceMeta};
use super::piper::voice::voice_files_present;

/// One row in the picker.  Carries everything the
/// renderer + key handler need without re-reading the
/// catalog or the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VoiceListEntry {
    pub key: String,
    pub language_code: String,
    pub language_english: String,
    pub quality: String,
    pub downloaded: bool,
    pub size_bytes: u64,
    /// True when the catalog contained this voice (false
    /// when we picked it up from disk only — offline +
    /// no cache).  Drives the "no catalog metadata"
    /// fallback rendering for language/quality.
    pub from_catalog: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TtsVoicePickerState {
    pub entries: Vec<VoiceListEntry>,
    pub filter: String,
    /// Index into `filtered_indices()`.  Clamped on
    /// filter change.
    pub cursor: usize,
    /// True when the catalog was loaded from a stale
    /// cache (T.3's freshness-first policy).  Picker
    /// surfaces this with a chip in the header.
    pub catalog_stale: bool,
    /// Some(reason) when the catalog couldn't be loaded
    /// at all (network down + no cache).  Picker shows
    /// the reason in the header so the user knows why
    /// the list is dir-only.
    pub catalog_failed: Option<String>,
    /// Picker-local status (one-line message at the
    /// bottom of the modal).  Refreshed on key actions.
    pub status: String,
}

impl TtsVoicePickerState {
    /// Build from a loaded catalog + the project's
    /// voices directory.  Catalog entries are merged
    /// with any on-disk voices the catalog doesn't list
    /// (defensive — projects that swapped catalogs
    /// can still see their old voices).
    pub fn from_catalog(
        catalog: &Catalog,
        voices_dir: &Path,
    ) -> Self {
        let mut entries: Vec<VoiceListEntry> = catalog
            .voices
            .values()
            .map(|v| from_catalog_voice(v, voices_dir))
            .collect();

        // Add stragglers — on-disk voices not present
        // in the catalog.
        let catalog_keys: BTreeSet<&str> = catalog
            .voices
            .values()
            .map(|v| v.key.as_str())
            .collect();
        for key in scan_disk_voice_keys(voices_dir) {
            if !catalog_keys.contains(key.as_str()) {
                entries.push(from_disk_voice(&key, voices_dir));
            }
        }

        entries.sort_by(default_order);
        Self {
            entries,
            filter: String::new(),
            cursor: 0,
            catalog_stale: catalog.stale,
            catalog_failed: None,
            status: String::new(),
        }
    }

    /// Build from disk only — used when the catalog
    /// can't be loaded (network down + no cache).
    /// Shows just the voices the user already has.
    pub fn from_dir_only(voices_dir: &Path, reason: String) -> Self {
        let mut entries: Vec<VoiceListEntry> = scan_disk_voice_keys(voices_dir)
            .into_iter()
            .map(|key| from_disk_voice(&key, voices_dir))
            .collect();
        entries.sort_by(default_order);
        Self {
            entries,
            filter: String::new(),
            cursor: 0,
            catalog_stale: false,
            catalog_failed: Some(reason),
            status: String::new(),
        }
    }

    /// Indices into `entries` matching the current
    /// filter.  Order matches `entries`.
    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            return (0..self.entries.len()).collect();
        }
        let needle = self.filter.to_lowercase();
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if matches_needle(e, &needle) {
                    Some(i)
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn selected_entry(&self) -> Option<&VoiceListEntry> {
        let indices = self.filtered_indices();
        let i = indices.get(self.cursor)?;
        self.entries.get(*i)
    }

    pub fn move_down(&mut self) {
        let n = self.filtered_indices().len();
        if n > 0 && self.cursor + 1 < n {
            self.cursor += 1;
        }
    }

    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_to_top(&mut self) {
        self.cursor = 0;
    }

    pub fn move_to_bottom(&mut self) {
        let n = self.filtered_indices().len();
        if n > 0 {
            self.cursor = n - 1;
        }
    }

    pub fn push_filter_char(&mut self, c: char) {
        self.filter.push(c);
        self.cursor = 0;
    }

    pub fn pop_filter_char(&mut self) {
        self.filter.pop();
        self.cursor = 0;
    }

    #[allow(dead_code)]
    pub fn clear_filter(&mut self) {
        self.filter.clear();
        self.cursor = 0;
    }

    /// Refresh `downloaded` flag for every entry by
    /// re-checking the filesystem.  Called after
    /// download / remove actions so the picker reflects
    /// the new state without rebuilding the whole list.
    pub fn refresh_downloaded(&mut self, voices_dir: &Path) {
        for e in &mut self.entries {
            e.downloaded = voice_files_present(voices_dir, &e.key);
        }
    }
}

fn matches_needle(e: &VoiceListEntry, needle: &str) -> bool {
    e.key.to_lowercase().contains(needle)
        || e.language_code.to_lowercase().contains(needle)
        || e.language_english.to_lowercase().contains(needle)
}

/// Numeric quality rank (higher = better).  Mirrors
/// `catalog::quality_rank` but kept local so the picker
/// has no inter-module call for one sort.
fn quality_rank(q: &str) -> u8 {
    match q {
        "high" => 4,
        "medium" => 3,
        "low" => 2,
        "x_low" => 1,
        _ => 0,
    }
}

fn default_order(a: &VoiceListEntry, b: &VoiceListEntry) -> std::cmp::Ordering {
    a.language_code
        .cmp(&b.language_code)
        .then(quality_rank(&b.quality).cmp(&quality_rank(&a.quality)))
        .then(a.key.cmp(&b.key))
}

fn from_catalog_voice(v: &VoiceMeta, voices_dir: &Path) -> VoiceListEntry {
    VoiceListEntry {
        key: v.key.clone(),
        language_code: v.language_code.clone(),
        language_english: v.language_english.clone(),
        quality: v.quality.clone(),
        downloaded: voice_files_present(voices_dir, &v.key),
        size_bytes: v.synthesis_size_bytes(),
        from_catalog: true,
    }
}

fn from_disk_voice(key: &str, voices_dir: &Path) -> VoiceListEntry {
    // Extract language + quality from the canonical
    // key shape "lang-name-quality" (e.g.
    // "en_US-lessac-medium").  Fallback to "?" when
    // the key doesn't follow the convention.
    let (language_code, quality) = parse_canonical_key(key);
    let size_bytes = on_disk_size(voices_dir, key);
    VoiceListEntry {
        key: key.to_string(),
        language_code,
        language_english: String::new(),
        quality,
        downloaded: true,
        size_bytes,
        from_catalog: false,
    }
}

/// Parse a Piper voice key (`en_US-lessac-medium`) into
/// `(language_code, quality)`.  Returns `("?", "?")`
/// when the key doesn't follow the three-part shape.
fn parse_canonical_key(key: &str) -> (String, String) {
    let parts: Vec<&str> = key.splitn(3, '-').collect();
    if parts.len() == 3 {
        (parts[0].to_string(), parts[2].to_string())
    } else {
        ("?".into(), "?".into())
    }
}

fn on_disk_size(voices_dir: &Path, key: &str) -> u64 {
    let onnx = voices_dir.join(format!("{key}.onnx"));
    let json = voices_dir.join(format!("{key}.onnx.json"));
    let s1 = std::fs::metadata(&onnx).map(|m| m.len()).unwrap_or(0);
    let s2 = std::fs::metadata(&json).map(|m| m.len()).unwrap_or(0);
    s1 + s2
}

/// Scan `voices_dir` for filenames matching
/// `<key>.onnx` (excluding the `.onnx.json` config) and
/// return the inferred keys.  Tolerant of missing /
/// unreadable dirs (returns empty).  Skips entries
/// whose name doesn't look like a voice key.
fn scan_disk_voice_keys(voices_dir: &Path) -> Vec<String> {
    let entries = match std::fs::read_dir(voices_dir) {
        Ok(e) => e,
        Err(_) => return Vec::new(),
    };
    let mut keys: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.ends_with(".onnx") && !name.ends_with(".onnx.json") {
                Some(name.trim_end_matches(".onnx").to_string())
            } else {
                None
            }
        })
        .filter(|key| !key.is_empty() && !key.starts_with('.'))
        .collect();
    keys.sort();
    keys.dedup();
    keys
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::piper::catalog::parse_voice_catalog;

    const FIXTURE: &str = r#"
    {
      "en_US-lessac-medium": {
        "key": "en_US-lessac-medium",
        "name": "lessac",
        "language": {
          "code": "en_US", "family": "en",
          "name_native": "English", "name_english": "English"
        },
        "quality": "medium",
        "num_speakers": 1,
        "files": {
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx": {
            "size_bytes": 1000, "md5_digest": "a"
          },
          "en/en_US/lessac/medium/en_US-lessac-medium.onnx.json": {
            "size_bytes": 100, "md5_digest": "b"
          }
        },
        "aliases": []
      },
      "en_US-ryan-high": {
        "key": "en_US-ryan-high",
        "name": "ryan",
        "language": {
          "code": "en_US", "family": "en",
          "name_native": "English", "name_english": "English"
        },
        "quality": "high",
        "num_speakers": 1,
        "files": {
          "en/en_US/ryan/high/en_US-ryan-high.onnx": {
            "size_bytes": 2000, "md5_digest": "c"
          },
          "en/en_US/ryan/high/en_US-ryan-high.onnx.json": {
            "size_bytes": 100, "md5_digest": "d"
          }
        },
        "aliases": []
      },
      "ru_RU-irina-medium": {
        "key": "ru_RU-irina-medium",
        "name": "irina",
        "language": {
          "code": "ru_RU", "family": "ru",
          "name_native": "Russian", "name_english": "Russian"
        },
        "quality": "medium",
        "num_speakers": 1,
        "files": {
          "ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx": {
            "size_bytes": 1500, "md5_digest": "e"
          },
          "ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx.json": {
            "size_bytes": 80, "md5_digest": "f"
          }
        },
        "aliases": []
      }
    }
    "#;

    fn make_voice_files(dir: &Path, key: &str) {
        std::fs::write(dir.join(format!("{key}.onnx")), b"x").unwrap();
        std::fs::write(dir.join(format!("{key}.onnx.json")), b"y").unwrap();
    }

    // ── ordering ──────────────────────────────────────

    #[test]
    fn from_catalog_orders_by_lang_then_quality_desc() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let keys: Vec<&str> = state.entries.iter().map(|e| e.key.as_str()).collect();
        // en_US-ryan-high (high) before en_US-lessac-medium (medium); ru_RU after en_US.
        assert_eq!(
            keys,
            vec![
                "en_US-ryan-high",
                "en_US-lessac-medium",
                "ru_RU-irina-medium",
            ]
        );
    }

    #[test]
    fn from_catalog_marks_downloaded_when_files_present() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        make_voice_files(tmp.path(), "en_US-lessac-medium");
        let state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let lessac = state
            .entries
            .iter()
            .find(|e| e.key == "en_US-lessac-medium")
            .unwrap();
        assert!(lessac.downloaded);
        let ryan = state
            .entries
            .iter()
            .find(|e| e.key == "en_US-ryan-high")
            .unwrap();
        assert!(!ryan.downloaded);
    }

    #[test]
    fn from_catalog_includes_on_disk_voices_not_in_catalog() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        make_voice_files(tmp.path(), "es_ES-mystery-low");
        let state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let extra = state
            .entries
            .iter()
            .find(|e| e.key == "es_ES-mystery-low")
            .unwrap();
        assert!(extra.downloaded);
        assert!(!extra.from_catalog);
        assert_eq!(extra.language_code, "es_ES");
        assert_eq!(extra.quality, "low");
    }

    #[test]
    fn from_dir_only_lists_only_downloaded() {
        let tmp = tempfile::tempdir().unwrap();
        make_voice_files(tmp.path(), "fr_FR-tom-medium");
        make_voice_files(tmp.path(), "en_US-lessac-medium");
        let state =
            TtsVoicePickerState::from_dir_only(tmp.path(), "offline".into());
        assert_eq!(state.entries.len(), 2);
        for e in &state.entries {
            assert!(e.downloaded);
            assert!(!e.from_catalog);
        }
        assert_eq!(
            state.catalog_failed.as_deref().unwrap(),
            "offline",
        );
    }

    // ── filter ────────────────────────────────────────

    #[test]
    fn filter_matches_language_code() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.filter = "ru_RU".into();
        let filtered = state.filtered_indices();
        assert_eq!(filtered.len(), 1);
        assert_eq!(state.entries[filtered[0]].key, "ru_RU-irina-medium");
    }

    #[test]
    fn filter_matches_voice_name_substring() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.filter = "ryan".into();
        let filtered = state.filtered_indices();
        assert_eq!(filtered.len(), 1);
        assert_eq!(state.entries[filtered[0]].key, "en_US-ryan-high");
    }

    #[test]
    fn filter_is_case_insensitive() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.filter = "EN_US".into();
        let filtered = state.filtered_indices();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filter_empty_yields_all() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let filtered = state.filtered_indices();
        assert_eq!(filtered.len(), 3);
    }

    // ── cursor movement ───────────────────────────────

    #[test]
    fn move_down_clamps_at_bottom() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        // 3 entries; move down 10 times.
        for _ in 0..10 {
            state.move_down();
        }
        assert_eq!(state.cursor, 2);
    }

    #[test]
    fn move_up_clamps_at_top() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.cursor = 1;
        state.move_up();
        state.move_up();
        state.move_up();
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn push_filter_resets_cursor_and_selected_entry() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.cursor = 2;
        state.push_filter_char('r');
        // After 'r' filter, only en_US-ryan-high + ru_RU-irina-medium remain (both contain "r").
        // Wait: language_english contains "Russian" for ru, and key "ryan" + "irina" — both pass.
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn selected_entry_none_when_filter_excludes_all() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.filter = "no-such-language".into();
        assert!(state.selected_entry().is_none());
    }

    #[test]
    fn selected_entry_first_matching_after_filter_set() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.push_filter_char('r');
        state.push_filter_char('u');
        // "ru" matches "ru_RU" + "Russian" → ru_RU-irina-medium only.
        let sel = state.selected_entry().unwrap();
        assert_eq!(sel.key, "ru_RU-irina-medium");
    }

    #[test]
    fn clear_filter_restores_full_list() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.filter = "ryan".into();
        state.cursor = 0;
        state.clear_filter();
        assert_eq!(state.filtered_indices().len(), 3);
        assert_eq!(state.cursor, 0);
    }

    #[test]
    fn move_to_top_and_bottom() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        state.move_to_bottom();
        assert_eq!(state.cursor, 2);
        state.move_to_top();
        assert_eq!(state.cursor, 0);
    }

    // ── refresh_downloaded ────────────────────────────

    #[test]
    fn refresh_downloaded_flips_after_install() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let lessac_idx = state
            .entries
            .iter()
            .position(|e| e.key == "en_US-lessac-medium")
            .unwrap();
        assert!(!state.entries[lessac_idx].downloaded);
        make_voice_files(tmp.path(), "en_US-lessac-medium");
        state.refresh_downloaded(tmp.path());
        assert!(state.entries[lessac_idx].downloaded);
    }

    #[test]
    fn refresh_downloaded_flips_after_remove() {
        let cat = parse_voice_catalog(FIXTURE.as_bytes()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        make_voice_files(tmp.path(), "en_US-lessac-medium");
        let mut state = TtsVoicePickerState::from_catalog(&cat, tmp.path());
        let lessac_idx = state
            .entries
            .iter()
            .position(|e| e.key == "en_US-lessac-medium")
            .unwrap();
        assert!(state.entries[lessac_idx].downloaded);
        std::fs::remove_file(tmp.path().join("en_US-lessac-medium.onnx")).unwrap();
        std::fs::remove_file(tmp.path().join("en_US-lessac-medium.onnx.json")).unwrap();
        state.refresh_downloaded(tmp.path());
        assert!(!state.entries[lessac_idx].downloaded);
    }

    #[test]
    fn parse_canonical_key_round_trips() {
        let (lang, q) = parse_canonical_key("en_US-lessac-medium");
        assert_eq!(lang, "en_US");
        assert_eq!(q, "medium");
    }

    #[test]
    fn parse_canonical_key_handles_unknown_shape() {
        let (lang, q) = parse_canonical_key("strange");
        assert_eq!(lang, "?");
        assert_eq!(q, "?");
    }

    #[test]
    fn scan_disk_skips_dotfiles() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join(".lru.onnx"), b"x").unwrap();
        std::fs::write(tmp.path().join("en_US-x-low.onnx"), b"x").unwrap();
        let keys = scan_disk_voice_keys(tmp.path());
        assert_eq!(keys, vec!["en_US-x-low".to_string()]);
    }

    #[test]
    fn scan_disk_skips_onnx_json_files() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("v.onnx"), b"x").unwrap();
        std::fs::write(tmp.path().join("v.onnx.json"), b"y").unwrap();
        let keys = scan_disk_voice_keys(tmp.path());
        // Should list "v" once, not duplicated.
        assert_eq!(keys, vec!["v".to_string()]);
    }
}
