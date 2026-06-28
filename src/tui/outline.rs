//! OUTLINE-1 — full-screen manuscript Outline pane.
//!
//! This module owns the pane's persisted view state (`OutlineState`) and, in
//! later phases, its renderer + key handling. The structural mutation
//! operations (reorder / promote / demote / copy / move) live in
//! `crate::outline` so the CLI and Bund share them.
//!
//! O-P0 — the state: expanded/collapsed flags, cursor, scroll, and the inline
//! filter string, persisted per-project to `.inkhaven/outline-state.json`
//! (ephemeral UI state; not backed up, not snapshot-versioned).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(super) struct OutlineState {
    /// Per-node expand flag. Absent → collapsed; `apply_default_view` seeds a
    /// chapter-level default (books expanded) when the map is empty.
    #[serde(default)]
    pub expanded: HashMap<Uuid, bool>,
    #[serde(default)]
    pub cursor_uuid: Option<Uuid>,
    #[serde(default)]
    pub scroll_offset: usize,
    /// Inline `/` filter (O-P5). Empty = no filter.
    #[serde(default)]
    pub filter_str: String,
}

impl OutlineState {
    pub(super) fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("outline-state.json")
    }

    /// Load the saved state. Absent **or malformed** → default (the pane is UI
    /// state — never fail the TUI over a corrupt sidecar).
    pub(super) fn load(project_root: &Path) -> OutlineState {
        match std::fs::read_to_string(Self::sidecar_path(project_root)) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => OutlineState::default(),
        }
    }

    /// Persist atomically (temp + rename via `io_atomic`).
    pub(super) fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }

    /// Whether a branch node is currently expanded (default: collapsed).
    pub(super) fn is_expanded(&self, id: &Uuid) -> bool {
        self.expanded.get(id).copied().unwrap_or(false)
    }

    pub(super) fn set_expanded(&mut self, id: Uuid, expanded: bool) {
        self.expanded.insert(id, expanded);
    }

    pub(super) fn toggle_expanded(&mut self, id: Uuid) {
        let now = self.is_expanded(&id);
        self.expanded.insert(id, !now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        let mut s = OutlineState::default();
        s.set_expanded(a, true);
        s.set_expanded(b, false);
        s.cursor_uuid = Some(a);
        s.scroll_offset = 12;
        s.filter_str = "harbour".into();
        s.save(root).unwrap();

        let back = OutlineState::load(root);
        assert_eq!(back, s);
        assert!(back.is_expanded(&a));
        assert!(!back.is_expanded(&b));
        assert_eq!(back.cursor_uuid, Some(a));
        assert_eq!(back.scroll_offset, 12);
        assert_eq!(back.filter_str, "harbour");
    }

    #[test]
    fn absent_and_malformed_default() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Absent → default.
        assert_eq!(OutlineState::load(root), OutlineState::default());
        // Malformed → default (never panics / errors).
        std::fs::create_dir_all(root.join(".inkhaven")).unwrap();
        std::fs::write(OutlineState::sidecar_path(root), b"{not json").unwrap();
        assert_eq!(OutlineState::load(root), OutlineState::default());
    }

    #[test]
    fn expand_helpers() {
        let mut s = OutlineState::default();
        let id = Uuid::new_v4();
        assert!(!s.is_expanded(&id)); // default collapsed
        s.toggle_expanded(id);
        assert!(s.is_expanded(&id));
        s.toggle_expanded(id);
        assert!(!s.is_expanded(&id));
    }
}
