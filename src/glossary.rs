//! TERMS-1 — the Glossary: canonical terms + banned synonyms.
//!
//! Glossary entries are authored as **HJSON paragraphs** in the `Glossary`
//! system book (`content_type: "hjson"` — no new content type, same as
//! SOURCES-1). Each carries a canonical `term`, a `definition`, and a list of
//! banned `synonyms` that the editor overlay and `inkhaven terms check` flag in
//! prose. This module is the pure schema + parse + a thin store reader; the
//! detector lives in `tui::style_warnings`.
//!
//! The module-level `dead_code` allow covers items consumed by later TERMS-1
//! phases; it tightens as they land.
#![allow(dead_code)]

use serde::Deserialize;

/// One glossary entry. Every field is `serde(default)` so a partial / in-progress
/// HJSON paragraph still parses; an entry with an empty `term` is skipped (it
/// can't be canonical). Unknown HJSON fields are ignored (forward-compatible).
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct GlossaryEntry {
    /// Canonical form — the term to use everywhere (e.g. `access token`).
    pub term: String,
    /// What the term means (terms viewer + Bund API).
    pub definition: String,
    /// Banned forms — flagged in prose (e.g. `["auth token", "authentication token"]`).
    pub synonyms: Vec<String>,
    /// `global` (default) or a book slug to scope enforcement to one book.
    pub scope: Option<String>,
    /// Authorial note — why this canonical form was chosen.
    pub note: Option<String>,
}

impl GlossaryEntry {
    /// Parse one HJSON paragraph body into an entry. Tolerant — a malformed body
    /// yields `None`; unknown fields are ignored.
    pub fn from_hjson(body: &str) -> Option<GlossaryEntry> {
        serde_hjson::from_str::<GlossaryEntry>(body).ok()
    }

    /// Whether this entry is usable (has a non-empty canonical term).
    pub fn is_valid(&self) -> bool {
        !self.term.trim().is_empty()
    }

    /// The banned forms, lowercased + trimmed, empties skipped — what the
    /// detector flags.
    pub fn banned_synonyms(&self) -> impl Iterator<Item = String> + '_ {
        self.synonyms
            .iter()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
    }

    /// Whether this entry is enforced given a book scope. A `global` entry
    /// (scope `None` / `""` / `"global"`) applies everywhere; a book-scoped
    /// entry applies for a whole-project scan (`book_scope == None`) or when the
    /// scope slug matches the book being checked.
    pub fn applies_to(&self, book_scope: Option<&str>) -> bool {
        match self.scope.as_deref().map(str::trim) {
            None | Some("") | Some("global") => true,
            Some(slug) => book_scope.is_none() || book_scope == Some(slug),
        }
    }
}

/// The HJSON template seeded into a freshly-created Glossary paragraph.
// NOTE (HJSON gotcha, learned in SOURCES-1): unquoted strings run to end-of-line,
// so an inline `// …` after a value becomes PART of the value. Keep every
// comment on its own line.
pub const GLOSSARY_TEMPLATE: &str = "{
  // Canonical term — the form to use everywhere.
  term: canonical-term-here
  definition: What this term means in this project.
  // Banned forms — flagged with a red underline in prose. One per line:
  synonyms: [
    // banned-form-1
    // banned-form-2
  ]
  // scope: global   // (own line) or a book slug to limit enforcement to one book
  // note: Why this canonical form was chosen.
}
";

/// Seed body for a new Glossary paragraph created in the TUI — the typed
/// paragraph title becomes the canonical `term`. Mirrors
/// `sources::seed_sources_body_for_tui`.
pub fn seed_glossary_body_for_tui(title: &str) -> String {
    let term = title.trim();
    let term = if term.is_empty() { "canonical-term-here" } else { term };
    GLOSSARY_TEMPLATE.replacen("canonical-term-here", term, 1)
}

/// Strip a leading `= Title` editor heading (defensive — seeded paragraphs are
/// pure HJSON, but a hand-created one may carry the chrome heading).
fn strip_heading(body: &str) -> &str {
    if body.trim_start().starts_with("= ") {
        body.splitn(2, '\n').nth(1).unwrap_or("")
    } else {
        body
    }
}

/// Collect every valid glossary entry under the **Glossary** system book,
/// filtered to those that apply given `book_scope` (the book being checked, or
/// `None` for the whole project). Reads paragraph bodies from the store.
pub fn glossary_entries_from_store(
    store: &crate::store::Store,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    book_scope: Option<&str>,
) -> Vec<GlossaryEntry> {
    use crate::store::NodeKind;
    let Some(glossary) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_GLOSSARY)
    }) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in hierarchy.collect_subtree(glossary.id) {
        if id == glossary.id {
            continue;
        }
        let Some(node) = hierarchy.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue };
        let Ok(text) = std::str::from_utf8(&bytes) else { continue };
        let body = strip_heading(text);
        if let Some(e) = GlossaryEntry::from_hjson(body) {
            if e.is_valid() && e.applies_to(book_scope) {
                out.push(e);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // HJSON entries are multi-line (one field per line); arrays one item per line.

    #[test]
    fn parses_a_full_entry() {
        let body = "{\n  term: access token\n  definition: A short-lived credential.\n  \
                     synonyms: [\n    auth token\n    authentication token\n  ]\n  \
                     scope: global\n  note: chosen for clarity\n}";
        let e = GlossaryEntry::from_hjson(body).expect("parses");
        assert!(e.is_valid());
        assert_eq!(e.term, "access token");
        assert_eq!(e.definition, "A short-lived credential.");
        let syn: Vec<String> = e.banned_synonyms().collect();
        assert_eq!(syn, vec!["auth token", "authentication token"]);
        assert_eq!(e.note.as_deref(), Some("chosen for clarity"));
    }

    #[test]
    fn entry_without_synonyms_is_valid() {
        // A definition-only glossary entry contributes no overlay but is valid.
        let e = GlossaryEntry::from_hjson("{\n  term: idempotent\n  definition: Same result on repeat.\n}")
            .unwrap();
        assert!(e.is_valid());
        assert_eq!(e.banned_synonyms().count(), 0);
    }

    #[test]
    fn empty_term_is_invalid() {
        let e = GlossaryEntry::from_hjson("{\n  definition: orphan\n}").unwrap();
        assert!(!e.is_valid());
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        let e = GlossaryEntry::from_hjson("{\n  term: t\n  some_future_field: x\n}").unwrap();
        assert_eq!(e.term, "t");
    }

    #[test]
    fn scope_filter() {
        let global = GlossaryEntry { term: "t".into(), ..Default::default() };
        assert!(global.applies_to(None));
        assert!(global.applies_to(Some("guide")));

        let scoped = GlossaryEntry {
            term: "t".into(),
            scope: Some("guide".into()),
            ..Default::default()
        };
        assert!(scoped.applies_to(None)); // whole-project scan includes it
        assert!(scoped.applies_to(Some("guide"))); // matching book
        assert!(!scoped.applies_to(Some("novel"))); // other book — excluded
    }

    #[test]
    fn cyrillic_synonyms_lowercase() {
        let e = GlossaryEntry::from_hjson("{\n  term: токен\n  synonyms: [\n    Аутентификатор\n  ]\n}")
            .unwrap();
        let syn: Vec<String> = e.banned_synonyms().collect();
        assert_eq!(syn, vec!["аутентификатор"]);
    }

    #[test]
    fn template_parses_and_seed_uses_title() {
        let e = GlossaryEntry::from_hjson(GLOSSARY_TEMPLATE).expect("template parses");
        assert_eq!(e.term, "canonical-term-here");
        // No synonyms (all commented out) — valid, contributes nothing.
        assert_eq!(e.banned_synonyms().count(), 0);

        let seeded = seed_glossary_body_for_tui("access token");
        let e2 = GlossaryEntry::from_hjson(&seeded).expect("seeded body parses");
        assert_eq!(e2.term, "access token");
    }
}
