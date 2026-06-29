//! MYTH-1 (M-P3) — read the declared symbol / motif / archetype entries from the
//! Mythology system book. Each entry is a paragraph tagged `para:myth-*` whose
//! HJSON body carries a `myth_symbol` / `myth_motif` / `myth_archetype` block.
//! Entries with a missing/empty required field are skipped (tolerant).

use serde::Deserialize;

use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use super::{ArchetypeRole, MythArchetype, MythMotif, MythSymbol, MythValence};

/// The Mythology system book's tag (= its `SYSTEM_BOOKS` slug, added in M-P1).
const MYTHOLOGY_TAG: &str = "mythology";

#[derive(Debug, Default, Deserialize)]
struct SymbolDoc {
    myth_symbol: Option<SymbolBody>,
}
#[derive(Debug, Default, Deserialize)]
struct SymbolBody {
    #[serde(default)]
    vocabulary: Vec<String>,
    #[serde(default)]
    meaning: String,
    #[serde(default)]
    valence: Option<String>,
    #[serde(default)]
    traditions: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MotifDoc {
    myth_motif: Option<MotifBody>,
}
#[derive(Debug, Default, Deserialize)]
struct MotifBody {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    valence: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct ArchetypeDoc {
    myth_archetype: Option<ArchetypeBody>,
}
#[derive(Debug, Default, Deserialize)]
struct ArchetypeBody {
    #[serde(default)]
    role: Option<String>,
    #[serde(default)]
    character: String,
    #[serde(default)]
    function: String,
}

pub(super) fn parse_symbol_block(para_id: &str, body: &str) -> Option<MythSymbol> {
    let s = serde_hjson::from_str::<SymbolDoc>(body).ok()?.myth_symbol?;
    let vocabulary: Vec<String> = s
        .vocabulary
        .into_iter()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .collect();
    if vocabulary.is_empty() {
        return None; // a symbol with no detectable vocabulary is unusable
    }
    Some(MythSymbol {
        para_id: para_id.to_string(),
        vocabulary,
        meaning: s.meaning.trim().to_string(),
        valence: MythValence::from_code(s.valence.as_deref().unwrap_or("ambiguous")),
        traditions: s.traditions.into_iter().map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect(),
    })
}

pub(super) fn parse_motif_block(para_id: &str, body: &str) -> Option<MythMotif> {
    let m = serde_hjson::from_str::<MotifDoc>(body).ok()?.myth_motif?;
    let name = m.name.trim().to_string();
    if name.is_empty() {
        return None;
    }
    Some(MythMotif {
        para_id: para_id.to_string(),
        name,
        description: m.description.trim().to_string(),
        valence: MythValence::from_code(m.valence.as_deref().unwrap_or("ambiguous")),
    })
}

pub(super) fn parse_archetype_block(para_id: &str, body: &str) -> Option<MythArchetype> {
    let a = serde_hjson::from_str::<ArchetypeDoc>(body).ok()?.myth_archetype?;
    let role = ArchetypeRole::from_code(a.role.as_deref().unwrap_or(""));
    // An archetype with no role string at all is unusable.
    if a.role.as_deref().map(str::trim).unwrap_or("").is_empty() {
        return None;
    }
    Some(MythArchetype {
        para_id: para_id.to_string(),
        role,
        character_name: a.character.trim().to_string(),
        function_desc: a.function.trim().to_string(),
    })
}

fn mythology_book(h: &Hierarchy) -> Option<&Node> {
    h.iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(MYTHOLOGY_TAG))
}

fn entry_body(layout: &ProjectLayout, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    std::fs::read_to_string(layout.root.join(rel)).ok()
}

fn has_tag(node: &Node, tag: &str) -> bool {
    node.tags.iter().any(|t| t == tag)
}

/// Walk the Mythology book's paragraph entries, returning the bodies of those
/// carrying `tag`, paired with their para id.
fn tagged_entries<'a>(h: &'a Hierarchy, layout: &ProjectLayout, tag: &str) -> Vec<(String, String)> {
    let Some(book) = mythology_book(h) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in h.collect_subtree(book.id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph || !has_tag(p, tag) {
            continue;
        }
        if let Some(body) = entry_body(layout, p) {
            out.push((id.to_string(), body));
        }
    }
    out
}

pub(crate) fn read_symbols(h: &Hierarchy, layout: &ProjectLayout) -> Vec<MythSymbol> {
    tagged_entries(h, layout, "para:myth-symbol")
        .iter()
        .filter_map(|(id, body)| parse_symbol_block(id, body))
        .collect()
}

pub(crate) fn read_motifs(h: &Hierarchy, layout: &ProjectLayout) -> Vec<MythMotif> {
    tagged_entries(h, layout, "para:myth-motif")
        .iter()
        .filter_map(|(id, body)| parse_motif_block(id, body))
        .collect()
}

pub(crate) fn read_archetypes(h: &Hierarchy, layout: &ProjectLayout) -> Vec<MythArchetype> {
    tagged_entries(h, layout, "para:myth-archetype")
        .iter()
        .filter_map(|(id, body)| parse_archetype_block(id, body))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_symbol_block() {
        let body = r#"{ myth_symbol: {
            vocabulary: ["raven", "ravens", "  "]
            meaning: "Precursor to betrayal."
            valence: "negative"
            traditions: ["Norse", "Celtic"]
        } }"#;
        let s = parse_symbol_block("p1", body).unwrap();
        assert_eq!(s.vocabulary, vec!["raven", "ravens"]); // blank dropped
        assert_eq!(s.valence, MythValence::Negative);
        assert_eq!(s.traditions, vec!["Norse", "Celtic"]);
        // No vocabulary → None.
        assert!(parse_symbol_block("p2", r#"{ myth_symbol: { vocabulary: [], meaning: "x" } }"#).is_none());
        // Not the right block / prose → None.
        assert!(parse_symbol_block("p3", "just prose").is_none());
        assert!(parse_symbol_block("p4", r#"{ name: "x" }"#).is_none());
    }

    #[test]
    fn parses_motif_block() {
        let body = r#"{ myth_motif: { name: "The locked door", description: "A threshold.", valence: "ambiguous" } }"#;
        let m = parse_motif_block("p1", body).unwrap();
        assert_eq!(m.name, "The locked door");
        assert_eq!(m.valence, MythValence::Ambiguous);
        // No name → None.
        assert!(parse_motif_block("p2", r#"{ myth_motif: { description: "d" } }"#).is_none());
    }

    #[test]
    fn parses_archetype_block_vogler_and_custom() {
        let body = r#"{ myth_archetype: { role: "herald", character: "Seren", function: "announces the rupture" } }"#;
        let a = parse_archetype_block("p1", body).unwrap();
        assert_eq!(a.role, ArchetypeRole::Herald);
        assert_eq!(a.character_name, "Seren");
        // Custom role accepted.
        let c = parse_archetype_block("p2", r#"{ myth_archetype: { role: "dark oracle", character: "V" } }"#).unwrap();
        assert!(matches!(c.role, ArchetypeRole::Custom(_)));
        // No role → None.
        assert!(parse_archetype_block("p3", r#"{ myth_archetype: { character: "X" } }"#).is_none());
    }
}
