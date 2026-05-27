//! 1.2.10+ — sidecar annotation store.
//!
//! Annotations are free-text notes the user attaches
//! to individual config fields.  They live in
//! `<project>/.config-annotations.hjson` as a flat
//! `{ "path.to.field": "the note", … }` map — the
//! canonical store; HJSON file comments are
//! separately surfaced via the comment inspector
//! (`Ctrl+I`).
//!
//! Empty / whitespace-only annotations are erased
//! from the map on save so the file stays clean.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const SIDECAR_FILENAME: &str = ".config-annotations.hjson";

/// In-memory annotation store, keyed by dotted
/// config path.  `BTreeMap` so HJSON emission is
/// stable across saves.
#[derive(Debug, Clone, Default)]
pub struct Annotations {
    entries: BTreeMap<String, String>,
}

impl Annotations {
    /// Test/diagnostic constructor; runtime callers
    /// use `Annotations::load`.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Load `<project>/.config-annotations.hjson`.
    /// Missing file → empty store (silent).  Parse
    /// errors → empty store + tracing warning so the
    /// TUI doesn't hard-fail on a typo in the
    /// sidecar.
    pub fn load(project_root: &Path) -> Self {
        let path = Self::sidecar_path(project_root);
        if !path.exists() {
            return Self::default();
        }
        let raw = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::config_tui::annotations",
                    "read {} failed: {e}",
                    path.display()
                );
                return Self::default();
            }
        };
        match serde_hjson::from_str::<BTreeMap<String, String>>(&raw) {
            Ok(entries) => Self { entries },
            Err(e) => {
                tracing::warn!(
                    target: "inkhaven::config_tui::annotations",
                    "parse {} failed: {e}",
                    path.display()
                );
                Self::default()
            }
        }
    }

    /// Write the store to
    /// `<project>/.config-annotations.hjson`.  Atomic
    /// (`.tmp` + rename).  Empty store → file is
    /// deleted to keep the project root clean.
    pub fn save(&self, project_root: &Path) -> Result<()> {
        let path = Self::sidecar_path(project_root);
        if self.entries.is_empty() {
            if path.exists() {
                fs::remove_file(&path)
                    .with_context(|| format!("remove {}", path.display()))?;
            }
            return Ok(());
        }
        let body = serde_hjson::to_string(&self.entries)
            .context("serialise annotations to HJSON")?;
        let mut tmp = path.clone();
        let mut tmp_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        tmp_name.push_str(".tmp");
        tmp.set_file_name(&tmp_name);
        fs::write(&tmp, &body)
            .with_context(|| format!("write {}", tmp.display()))?;
        fs::rename(&tmp, &path).with_context(|| {
            format!("rename {} → {}", tmp.display(), path.display())
        })?;
        Ok(())
    }

    pub fn get(&self, path: &str) -> Option<&str> {
        self.entries.get(path).map(String::as_str)
    }

    pub fn set(&mut self, path: &str, text: &str) {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            self.entries.remove(path);
        } else {
            self.entries.insert(path.to_string(), trimmed.to_string());
        }
    }

    /// Surface the full annotation list — reserved
    /// for future bulk operations (CLI dump, "show
    /// all annotations" modal).  Phase 3 UI reads via
    /// `get` only.
    #[allow(dead_code)]
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Annotation count — used by tests + future
    /// status-bar chip.
    #[allow(dead_code)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(SIDECAR_FILENAME)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir_in_test() -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir()
            .join(format!("inkhaven_ann_test_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_sidecar_yields_empty_store() {
        let dir = tempdir_in_test();
        let store = Annotations::load(&dir);
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn round_trip_save_and_load() {
        let dir = tempdir_in_test();
        let mut store = Annotations::new();
        store.set("editor.autosave_seconds", "bumped from 10s — felt slow");
        store.set("language", "russian by default for this project");
        store.save(&dir).unwrap();
        let loaded = Annotations::load(&dir);
        assert_eq!(loaded.len(), 2);
        assert_eq!(
            loaded.get("language"),
            Some("russian by default for this project")
        );
    }

    #[test]
    fn empty_text_removes_entry() {
        let mut store = Annotations::new();
        store.set("a", "note");
        assert_eq!(store.len(), 1);
        store.set("a", "");
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn whitespace_only_text_removes_entry() {
        let mut store = Annotations::new();
        store.set("a", "note");
        store.set("a", "    \t  ");
        assert_eq!(store.len(), 0);
    }

    #[test]
    fn empty_store_save_removes_sidecar_file() {
        let dir = tempdir_in_test();
        let mut store = Annotations::new();
        store.set("a", "note");
        store.save(&dir).unwrap();
        assert!(dir.join(SIDECAR_FILENAME).exists());
        // Clear and save → file gone.
        store.set("a", "");
        store.save(&dir).unwrap();
        assert!(!dir.join(SIDECAR_FILENAME).exists());
    }
}
