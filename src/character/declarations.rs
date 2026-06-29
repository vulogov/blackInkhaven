//! CHAR-1 (C-P2) — read the optional `character_arc` HJSON block from each
//! Characters-book entry. A character entry is a paragraph under the Characters
//! system book; the character name is its title, and its HJSON body may carry a
//! `character_arc` block (RFC §5). Omitting the block is valid — the entry just
//! has no declared arc to check against.

use std::hash::{Hash, Hasher};

use serde::Deserialize;

use crate::project::ProjectLayout;
use crate::store::SYSTEM_TAG_CHARACTERS;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use super::{ArcDeclaration, ArcType};

/// The `character_arc` HJSON block.
#[derive(Debug, Default, Deserialize)]
struct CharacterArcBlock {
    #[serde(default)]
    arc_type: Option<String>,
    #[serde(default)]
    desired_state_start: Option<String>,
    #[serde(default)]
    desired_midpoint_state: Option<String>,
    #[serde(default)]
    desired_state_end: Option<String>,
}

/// A Characters-book entry's HJSON body (only the arc block is read).
#[derive(Debug, Default, Deserialize)]
struct CharacterEntry {
    #[serde(default)]
    character_arc: Option<CharacterArcBlock>,
}

/// Parse one entry body into an `ArcDeclaration` for `name`. Returns `None` when
/// the body isn't HJSON, has no `character_arc` block, or the block lacks the
/// required start/end states. Also returns the declaration hash (for cache
/// invalidation).
pub(super) fn parse_arc_block(name: &str, body: &str) -> Option<(ArcDeclaration, u64)> {
    let entry: CharacterEntry = serde_hjson::from_str(body).ok()?;
    let arc = entry.character_arc?;
    let start = arc.desired_state_start?;
    let end = arc.desired_state_end?;
    if start.trim().is_empty() || end.trim().is_empty() {
        return None;
    }
    let arc_type = arc
        .arc_type
        .as_deref()
        .map(ArcType::from_label)
        .unwrap_or(ArcType::Other("unspecified".into()));
    let mid = arc.desired_midpoint_state.filter(|m| !m.trim().is_empty());

    let mut h = std::collections::hash_map::DefaultHasher::new();
    arc_type.as_code().hash(&mut h);
    start.hash(&mut h);
    mid.hash(&mut h);
    end.hash(&mut h);
    let hash = h.finish();

    Some((
        ArcDeclaration {
            character_name: name.to_string(),
            arc_type,
            desired_state_start: start,
            desired_midpoint_state: mid,
            desired_state_end: end,
        },
        hash,
    ))
}

/// The Characters system book.
fn characters_book(h: &Hierarchy) -> Option<&Node> {
    h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_CHARACTERS)
    })
}

/// Read the body of a Characters-book paragraph (from its `file`).
fn entry_body(layout: &ProjectLayout, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    std::fs::read_to_string(layout.root.join(rel)).ok()
}

/// Read every declared arc from the Characters book as `(declaration, hash)`.
/// Entries without a `character_arc` block are skipped.
pub(crate) fn read_arc_declarations(
    h: &Hierarchy,
    layout: &ProjectLayout,
) -> Vec<(ArcDeclaration, u64)> {
    let Some(book) = characters_book(h) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for child in h.children_of(Some(book.id)) {
        if child.kind != NodeKind::Paragraph {
            continue;
        }
        let name = child.title.trim();
        if name.is_empty() {
            continue;
        }
        if let Some(body) = entry_body(layout, child) {
            if let Some(decl) = parse_arc_block(name, &body) {
                out.push(decl);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_full_arc_block() {
        let body = r#"{
            character_arc: {
                arc_type: "positive_change"
                desired_state_start: "Mara defers to her family."
                desired_midpoint_state: "Mara's first open defiance."
                desired_state_end: "Mara acts without permission."
            }
        }"#;
        let (d, hash) = parse_arc_block("Mara", body).unwrap();
        assert_eq!(d.arc_type, ArcType::PositiveChange);
        assert!(d.desired_state_start.contains("defers"));
        assert!(d.desired_midpoint_state.is_some());
        assert!(d.desired_state_end.contains("acts"));
        // Hash is stable + sensitive to content.
        assert_eq!(parse_arc_block("Mara", body).unwrap().1, hash);
    }

    #[test]
    fn midpoint_optional() {
        let body = r#"{ character_arc: {
            arc_type: "flat"
            desired_state_start: "holds the truth"
            desired_state_end: "still holds it"
        } }"#;
        let (d, _) = parse_arc_block("Atticus", body).unwrap();
        assert_eq!(d.arc_type, ArcType::Flat);
        assert!(d.desired_midpoint_state.is_none());
    }

    #[test]
    fn no_block_or_incomplete_is_none() {
        // No character_arc block.
        assert!(parse_arc_block("X", r#"{ name: "X", role: "hero" }"#).is_none());
        // Missing required end state.
        assert!(parse_arc_block("X", r#"{ character_arc: { desired_state_start: "a" } }"#).is_none());
        // Not HJSON at all (prose).
        assert!(parse_arc_block("X", "Just a prose description of the character.").is_none());
        // Empty start.
        assert!(
            parse_arc_block("X", r#"{ character_arc: { desired_state_start: "  ", desired_state_end: "z" } }"#)
                .is_none()
        );
    }

    #[test]
    fn unknown_arc_type_falls_back() {
        let body = r#"{ character_arc: {
            arc_type: "spiritual awakening"
            desired_state_start: "a"
            desired_state_end: "b"
        } }"#;
        let (d, _) = parse_arc_block("X", body).unwrap();
        assert!(matches!(d.arc_type, ArcType::Other(_)));
    }
}
