//! 1.2.10+ — `Ctrl+B h` help pane.
//!
//! Parses `Documentation/CONFIGURATION.md` at runtime
//! into a `path → markdown body` map; serves slices to
//! the help pane.  Embedded via `include_str!` so the
//! binary stays self-contained.
//!
//! Phase 1: read-only floating pane that surfaces the
//! matched row.  No anchor-based deep linking yet —
//! that's a Phase 4 polish item.

use std::collections::HashMap;

/// Embedded copy of `CONFIGURATION.md`.  Re-included on
/// every build so the help pane never drifts from the
/// shipped docs.
const SOURCE: &str = include_str!("../../Documentation/CONFIGURATION.md");

/// In-memory index of `path → row text`.  Built lazily
/// on first call to `lookup`.
pub struct HelpIndex {
    entries: HashMap<String, String>,
}

impl HelpIndex {
    /// Parse the embedded CONFIGURATION.md and build
    /// the index.  Each row of the main markdown table
    /// is `| `path` | type | default | description |`;
    /// the path is wrapped in backticks (so the parser
    /// sees `` `editor.autosave_seconds` ``).  Sub-
    /// paths under a stanza heading inherit the stanza
    /// prefix — but Phase 1 keys on the exact field
    /// name only.
    pub fn build() -> Self {
        let mut entries: HashMap<String, String> = HashMap::new();
        for raw_line in SOURCE.lines() {
            let line = raw_line.trim_start();
            if !line.starts_with('|') {
                continue;
            }
            // Skip table headers + separators.
            if line.contains("---") {
                continue;
            }
            // Split on `|` and look at the first cell.
            let cells: Vec<&str> =
                line.split('|').map(str::trim).collect();
            if cells.len() < 4 {
                continue;
            }
            let key_cell = cells[1];
            // Pull a backtick-wrapped path out of the
            // first cell.  Skip rows whose first cell
            // doesn't start with a backtick (table
            // headers, prose interludes).
            let Some(path) = extract_path(key_cell) else {
                continue;
            };
            // The whole row, freshly built, becomes the
            // help body.  Wrap in a code block for the
            // markdown renderer.
            let body = format!(
                "**`{path}`**\n\n{}",
                cells[3..].join(" · "),
            );
            entries.insert(path.to_string(), body);
        }
        Self { entries }
    }

    /// Return the help body for `path`, walking up the
    /// dotted hierarchy if no exact match exists
    /// (`editor.style_warnings.show_dont_tell.enabled`
    /// → `style_warnings.show_dont_tell.enabled` →
    /// `show_dont_tell.enabled` → `enabled` →
    /// `None`).
    pub fn lookup(&self, path: &str) -> Option<&str> {
        if let Some(body) = self.entries.get(path) {
            return Some(body.as_str());
        }
        // Try progressively trimming leading segments.
        let mut remainder = path;
        while let Some(idx) = remainder.find('.') {
            remainder = &remainder[idx + 1..];
            if let Some(body) = self.entries.get(remainder) {
                return Some(body.as_str());
            }
        }
        // Try matching by the last segment alone.
        if let Some(last) = path.rsplit('.').next() {
            if let Some(body) = self.entries.get(last) {
                return Some(body.as_str());
            }
        }
        None
    }

    /// Test + diagnostic accessor; unused at runtime.
    #[allow(dead_code)]
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

/// Pull a backtick-wrapped path out of a table cell.
/// Accepts `` `a.b.c` `` and `` `a.b.c` (rest) ``.
fn extract_path(cell: &str) -> Option<&str> {
    let start = cell.find('`')?;
    let rest = &cell[start + 1..];
    let end = rest.find('`')?;
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_indexes_known_fields() {
        let idx = HelpIndex::build();
        // The README ships CONFIGURATION.md with at
        // least the headline knobs documented.  Spot-
        // check a few high-traffic ones.
        assert!(idx.entry_count() > 30, "got {}", idx.entry_count());
        // We expect at least one of these to land —
        // the lookup also walks up the dotted hierarchy.
        let probes = [
            "editor.autosave_seconds",
            "autosave_seconds",
            "language",
            "pov_chip_enabled",
        ];
        let hit_count = probes.iter().filter(|p| idx.lookup(p).is_some()).count();
        assert!(hit_count >= 1, "no doc rows matched any probe");
    }

    #[test]
    fn lookup_walks_up_dotted_path() {
        let mut e: HashMap<String, String> = HashMap::new();
        e.insert("wrap".to_string(), "wrap doc".into());
        let idx = HelpIndex { entries: e };
        assert_eq!(idx.lookup("editor.wrap"), Some("wrap doc"));
    }

    #[test]
    fn extract_path_handles_simple_row() {
        assert_eq!(
            extract_path("`editor.autosave_seconds`"),
            Some("editor.autosave_seconds")
        );
    }
}
