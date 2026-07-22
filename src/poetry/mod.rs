//! Poetry as a structural paragraph family (POEM-1, PO-P0).
//!
//! Poetry is not a separate project type or hierarchy — it is a family of
//! `para:verse-*` structural subtypes, exactly as STRUCT-2 modelled `para:code`
//! / `para:math` and WORLD-6 modelled `para:utopia-*`. A poem can therefore live
//! anywhere in a prose book (an epigraph, a recited stanza, song lyrics) without
//! changing the manuscript structure; a poetry-only project is just a book whose
//! paragraphs are all verse subtypes.
//!
//! Because every verse tag begins `para:`, the existing `is_structural_paragraph`
//! gate already excludes verse from the prose companions (Inner Editor / Socrates
//! / NARR-1 profiler) and from the prose word count — no extra wiring needed
//! there. This module adds the verse-specific registry, the tree glyph, and the
//! [`is_verse_paragraph`] predicate the Inner Poet (PO-P3) scopes on.

pub mod form;
pub mod metre;
pub mod rhyme;
pub mod syllabify;

use crate::store::node::Node;

/// The `para:verse-*` subtype registry — the poetry parallel to
/// `STRUCTURAL_TYPES` / `UTOPIA_TYPES`. Columns: `(tag, tree-glyph, picker-label,
/// seed-body)`. Freeform at the tag-parser level; verse-book scope is a display
/// concern, not enforced here (verse may appear in any book).
pub const VERSE_TYPES: &[(&str, &str, &str, &str)] = &[
    ("para:verse-line", "‖ ", "verse line", "\n"),
    ("para:verse-stanza", "♩ ", "verse stanza", "// stanza 1\n"),
    ("para:verse-couplet", "‗ ", "verse couplet", "\n\n"),
    ("para:verse-tercet", "⁚ ", "verse tercet", "\n\n\n"),
    ("para:verse-quatrain", "⁛ ", "verse quatrain", "\n\n\n\n"),
    ("para:verse-translation", "⇄ ", "verse translation", "\n"),
];

/// The tree glyph for a paragraph's `para:verse-*` subtype, if it carries one.
/// `None` → the caller falls through to the next glyph source (structural / prose).
pub fn verse_glyph(node: &Node) -> Option<&'static str> {
    let tag = node.tags.iter().find(|t| t.starts_with("para:verse-"))?;
    VERSE_TYPES.iter().find(|(t, ..)| *t == tag.as_str()).map(|(_, glyph, ..)| *glyph)
}

/// True when the paragraph carries any `para:verse-*` subtype tag. The Inner Poet
/// scopes on this; prose companions already skip it via `is_structural_paragraph`.
pub fn is_verse_paragraph(node: &Node) -> bool {
    node.tags.iter().any(|t| t.starts_with("para:verse-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn para(tags: &[&str]) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::nil(), "kind": "paragraph", "title": "p", "slug": "p",
            "path": [], "parent_id": null, "order": 1, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
            "tags": tags,
        }))
        .expect("test node")
    }

    #[test]
    fn glyph_resolves_every_registered_type() {
        for (tag, glyph, ..) in VERSE_TYPES {
            assert_eq!(verse_glyph(&para(&[tag])), Some(*glyph));
        }
    }

    #[test]
    fn is_verse_paragraph_gates_on_the_verse_prefix() {
        assert!(is_verse_paragraph(&para(&["para:verse-stanza"])));
        assert!(is_verse_paragraph(&para(&["draft", "para:verse-line"])));
        // A non-verse structural paragraph is not verse.
        assert!(!is_verse_paragraph(&para(&["para:code"])));
        assert!(!is_verse_paragraph(&para(&[])));
    }

    #[test]
    fn non_verse_gets_no_glyph() {
        assert_eq!(verse_glyph(&para(&["para:code"])), None);
        assert_eq!(verse_glyph(&para(&[])), None);
    }
}
