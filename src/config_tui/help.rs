//! 1.2.10+ — `Ctrl+H` help pane.
//!
//! Two data sources, merged at startup:
//!
//!   1. **Build-time doc-comment extraction** —
//!      `build.rs` parses `src/config.rs` with `syn`,
//!      walks the type graph rooted at `Config`, and
//!      emits a `(path, doc_comment)` table at
//!      `$OUT_DIR/config_help.rs`.  Always-fresh: any
//!      `///` line you write on a `Config` field
//!      lands here automatically.
//!
//!   2. **CONFIGURATION.md table rows** — manually
//!      curated, richer prose, may include rationale +
//!      examples.  Read at runtime from the embedded
//!      `include_str!` copy.
//!
//! Lookup precedence: CONFIGURATION.md row wins when
//! present (it's the curated form); doc-comment is the
//! fallback (always available).  This way the help
//! pane is informative for every field, but a hand-
//! written row still gets to override.
//!
//! Map-path lookup: when the focused path lives under
//! a known map stanza (e.g.
//! `llm.providers.gemini.model`), the lookup also
//! tries the `<entry>`-placeholder variant
//! (`llm.providers.<entry>.model`) — that's the form
//! the doc-comment extractor emits.

use std::collections::HashMap;

use super::schema;

/// Embedded copy of `CONFIGURATION.md`.  Re-included on
/// every build so the help pane never drifts from the
/// shipped docs.
const SOURCE: &str = include_str!("../../Documentation/CONFIGURATION.md");

// The build script emits this table — see
// `build.rs` + `config_help_extract.rs` at the
// crate root.
include!(concat!(env!("OUT_DIR"), "/config_help.rs"));

/// In-memory help index.  Stores two parallel maps:
///   * `curated` — CONFIGURATION.md row bodies.
///     Higher priority on lookup.
///   * `doc_comments` — build-time-extracted `///`
///     blocks from `src/config.rs`.  Always-fresh
///     fallback.
pub struct HelpIndex {
    curated: HashMap<String, String>,
    doc_comments: HashMap<String, String>,
}

impl HelpIndex {
    /// Build the index: parse CONFIGURATION.md and
    /// inhale the generated FIELD_DOCS table.  Both
    /// runs are one-shot at config-TUI launch.
    pub fn build() -> Self {
        // ── CONFIGURATION.md (curated rows).
        let mut curated: HashMap<String, String> = HashMap::new();
        for raw_line in SOURCE.lines() {
            let line = raw_line.trim_start();
            if !line.starts_with('|') {
                continue;
            }
            if line.contains("---") {
                continue;
            }
            let cells: Vec<&str> =
                line.split('|').map(str::trim).collect();
            if cells.len() < 4 {
                continue;
            }
            let key_cell = cells[1];
            let Some(path) = extract_path(key_cell) else {
                continue;
            };
            let body = format!(
                "**`{path}`**\n\n{}",
                cells[3..].join(" · "),
            );
            curated.insert(path.to_string(), body);
        }
        // ── Build-time doc-comments.
        let mut doc_comments: HashMap<String, String> = HashMap::new();
        for (path, doc) in FIELD_DOCS {
            doc_comments.insert((*path).to_string(), (*doc).to_string());
        }
        Self {
            curated,
            doc_comments,
        }
    }

    /// Return the help body for `path`.  Lookup
    /// precedence:
    ///
    ///   1. Curated CONFIGURATION.md row at the
    ///      exact path.
    ///   2. Build-time `///` doc-comment at the
    ///      exact path — including the
    ///      `<entry>`-placeholder variant for
    ///      paths that live under a known map
    ///      stanza (`llm.providers.gemini.model`
    ///      → `llm.providers.<entry>.model`).
    ///   3. Walk up dotted segments and retry the
    ///      curated map first, then doc-comments.
    pub fn lookup(&self, path: &str) -> Option<&str> {
        if let Some(body) = self.curated.get(path) {
            return Some(body.as_str());
        }
        if let Some(body) = self.doc_comments.get(path) {
            return Some(body.as_str());
        }
        // Map-path placeholder: if any prefix of the
        // path is a known map stanza, substitute the
        // segment immediately following with `<entry>`
        // and retry.
        if let Some(canonical) = canonicalise_map_path(path) {
            if let Some(body) = self.doc_comments.get(&canonical) {
                return Some(body.as_str());
            }
            if let Some(body) = self.curated.get(&canonical) {
                return Some(body.as_str());
            }
        }
        // Walk up dotted segments — useful for fields
        // that aren't documented under their full
        // stanza chain.
        let mut remainder = path;
        while let Some(idx) = remainder.find('.') {
            remainder = &remainder[idx + 1..];
            if let Some(body) = self.curated.get(remainder) {
                return Some(body.as_str());
            }
            if let Some(body) = self.doc_comments.get(remainder) {
                return Some(body.as_str());
            }
        }
        if let Some(last) = path.rsplit('.').next() {
            if let Some(body) = self.curated.get(last) {
                return Some(body.as_str());
            }
            if let Some(body) = self.doc_comments.get(last) {
                return Some(body.as_str());
            }
        }
        None
    }

    /// Test + diagnostic accessor; unused at runtime.
    /// Sum of curated + doc-comment entries.
    #[allow(dead_code)]
    pub fn entry_count(&self) -> usize {
        self.curated.len() + self.doc_comments.len()
    }
}

/// Substitute the segment immediately following any
/// known map prefix with `<entry>` — used by lookup
/// to find docs that the build-time extractor emits
/// with the `<entry>` placeholder for
/// `HashMap<String, T>` fields.
fn canonicalise_map_path(path: &str) -> Option<String> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    // Longest matching prefix wins (so nested map
    // paths route correctly when more land).
    for end in (1..parts.len()).rev() {
        let prefix = parts[..end].join(".");
        if schema::is_known_map_path(&prefix) {
            let mut out_parts: Vec<String> =
                parts[..end].iter().map(|s| s.to_string()).collect();
            out_parts.push("<entry>".to_string());
            for p in &parts[end + 1..] {
                out_parts.push(p.to_string());
            }
            return Some(out_parts.join("."));
        }
    }
    None
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
        let mut curated: HashMap<String, String> = HashMap::new();
        curated.insert("wrap".to_string(), "wrap doc".into());
        let idx = HelpIndex {
            curated,
            doc_comments: HashMap::new(),
        };
        assert_eq!(idx.lookup("editor.wrap"), Some("wrap doc"));
    }

    #[test]
    fn extract_path_handles_simple_row() {
        assert_eq!(
            extract_path("`editor.autosave_seconds`"),
            Some("editor.autosave_seconds")
        );
    }

    #[test]
    fn doc_comment_extraction_covers_undocumented_field() {
        // The build script pulls every field's `///`
        // comment from src/config.rs.  Spot-check
        // a known field that's NOT in CONFIGURATION.md
        // (e.g. `ai.diff_review_on_apply` — the very
        // field that showed "no row matched" in the
        // bug report).
        let idx = HelpIndex::build();
        let hit = idx.lookup("ai.diff_review_on_apply");
        assert!(hit.is_some(), "expected build-time doc-comment fallback for ai.diff_review_on_apply");
        let body = hit.unwrap();
        // Confirm the body looks like the actual doc
        // text, not a placeholder.
        assert!(body.contains("AI rewrite") || body.contains("diff"));
    }

    #[test]
    fn canonicalise_map_path_substitutes_entry_name() {
        // The build script emits docs keyed by
        // `llm.providers.<entry>.model`; a runtime
        // query for `llm.providers.gemini.model`
        // must canonicalise to find them.
        let canon = canonicalise_map_path("llm.providers.gemini.model");
        assert_eq!(canon, Some("llm.providers.<entry>.model".to_string()));
    }

    #[test]
    fn canonicalise_returns_none_outside_map_paths() {
        // Non-map paths return None (lookup falls
        // back to the segment walk).
        assert!(canonicalise_map_path("editor.autosave_seconds").is_none());
        assert!(canonicalise_map_path("language").is_none());
    }
}
