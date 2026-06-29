//! MYTH-1 — the mythological & symbolic pattern library. A dedicated home for an
//! author's **declared** symbolic vocabulary (symbols), recurring patterns
//! (motifs), and archetypal character roles (archetypes), plus a lexicon
//! highlight colour, a deterministic density scan, explicit LLM consistency /
//! completeness checks, and grounding for the Inner family.
//!
//! It reads only what the author declares in the **Mythology** system book — it
//! never discovers symbols the author didn't name, never interprets them, and
//! never edits prose. Findings are advisory (`info`).
//!
//! M-P0 — the language model: the three `para:myth-*` declaration tags (a
//! parallel registry, like WORLD-6's `UTOPIA_TYPES` — STRUCTURAL_TYPES is
//! seed-only), the valence / archetype-role / finding taxonomies, and the entry
//! records the store (M-P2) and parsers (M-P3) build on.

use crate::store::node::Node;

mod checks;
mod parse;
mod pipeline;
mod store;

pub(crate) use checks::run_deterministic_checks;
pub(crate) use parse::{read_archetypes, read_motifs, read_symbols};
pub(crate) use pipeline::{collect_explicit_motifs, refresh_inventory, run_density_scan};
pub(crate) use store::MythStore;

/// The registered `para:myth-*` tag values, their tree glyphs, and labels. Valid
/// only inside the Mythology system book (the `myth` reader filters by ancestor).
/// Parallel to WORLD-6's `UTOPIA_TYPES`: declarations with no seeded boilerplate,
/// so they own their registry + glyph lookup rather than riding the structural
/// paragraph picker.
pub(crate) const MYTH_TYPES: &[(&str, &str, &str)] = &[
    ("para:myth-symbol", "⊛", "Symbol"),
    ("para:myth-motif", "∿", "Motif"),
    ("para:myth-archetype", "⍟", "Archetype"),
];

/// The tree glyph for a paragraph's `para:myth-*` tag, if it carries one
/// (`None` → not a myth declaration). Mirrors `utopia_glyph` / `structural_glyph`.
pub(crate) fn myth_glyph(node: &Node) -> Option<&'static str> {
    node.tags.iter().find_map(|t| {
        MYTH_TYPES.iter().find(|(tag, _, _)| *tag == t).map(|(_, glyph, _)| *glyph)
    })
}

/// Declared emotional / thematic valence of a symbol or motif.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MythValence {
    Positive,
    Negative,
    Ambiguous,
}

impl MythValence {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            MythValence::Positive => "positive",
            MythValence::Negative => "negative",
            MythValence::Ambiguous => "ambiguous",
        }
    }

    /// Parse a declared valence; unrecognised → `Ambiguous` (the safe default).
    pub(crate) fn from_code(s: &str) -> MythValence {
        match s.trim().to_lowercase().as_str() {
            "positive" => MythValence::Positive,
            "negative" => MythValence::Negative,
            _ => MythValence::Ambiguous,
        }
    }
}

/// Vogler's eight narrative archetypes, or an open string for a custom role.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchetypeRole {
    Hero,
    Mentor,
    ThresholdGuardian,
    Herald,
    Shapeshifter,
    Shadow,
    Ally,
    Trickster,
    Custom(String),
}

impl ArchetypeRole {
    pub(crate) fn as_code(&self) -> &str {
        match self {
            ArchetypeRole::Hero => "hero",
            ArchetypeRole::Mentor => "mentor",
            ArchetypeRole::ThresholdGuardian => "threshold_guardian",
            ArchetypeRole::Herald => "herald",
            ArchetypeRole::Shapeshifter => "shapeshifter",
            ArchetypeRole::Shadow => "shadow",
            ArchetypeRole::Ally => "ally",
            ArchetypeRole::Trickster => "trickster",
            ArchetypeRole::Custom(s) => s,
        }
    }

    pub(crate) fn from_code(s: &str) -> ArchetypeRole {
        match s.trim().to_lowercase().replace([' ', '-'], "_").as_str() {
            "hero" => ArchetypeRole::Hero,
            "mentor" => ArchetypeRole::Mentor,
            "threshold_guardian" => ArchetypeRole::ThresholdGuardian,
            "herald" => ArchetypeRole::Herald,
            "shapeshifter" => ArchetypeRole::Shapeshifter,
            "shadow" => ArchetypeRole::Shadow,
            "ally" => ArchetypeRole::Ally,
            "trickster" => ArchetypeRole::Trickster,
            other => ArchetypeRole::Custom(other.to_string()),
        }
    }

    /// Whether this is one of Vogler's eight (vs an open custom role) — the
    /// checker uses a role-specific prompt for the eight, a generic one for
    /// custom roles.
    pub(crate) fn is_vogler(&self) -> bool {
        !matches!(self, ArchetypeRole::Custom(_))
    }
}

/// LLM / deterministic check finding kinds (RFC §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindingType {
    /// A symbol used against its declared meaning / valence (LLM).
    SymbolInconsistency,
    /// A declared motif missing structural distribution (LLM).
    MotifIncomplete,
    /// A declared motif with no occurrence in the final act (deterministic).
    MotifAbsentFinalAct,
    /// An archetype role declared but no character assigned (deterministic).
    ArchetypeVacant,
    /// A mapped character absent from its expected structural zone (deterministic).
    ArchetypeAbsent,
}

impl FindingType {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            FindingType::SymbolInconsistency => "symbol_inconsistency",
            FindingType::MotifIncomplete => "motif_incomplete",
            FindingType::MotifAbsentFinalAct => "motif_absent_final_act",
            FindingType::ArchetypeVacant => "archetype_vacant",
            FindingType::ArchetypeAbsent => "archetype_absent",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<FindingType> {
        Some(match s {
            "symbol_inconsistency" => FindingType::SymbolInconsistency,
            "motif_incomplete" => FindingType::MotifIncomplete,
            "motif_absent_final_act" => FindingType::MotifAbsentFinalAct,
            "archetype_vacant" => FindingType::ArchetypeVacant,
            "archetype_absent" => FindingType::ArchetypeAbsent,
            _ => return None,
        })
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            FindingType::SymbolInconsistency => "symbol inconsistency",
            FindingType::MotifIncomplete => "motif incomplete",
            FindingType::MotifAbsentFinalAct => "motif absent from final act",
            FindingType::ArchetypeVacant => "archetype role vacant",
            FindingType::ArchetypeAbsent => "archetype character absent",
        }
    }
}

/// A declared symbol entry from a `para:myth-symbol` paragraph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MythSymbol {
    pub para_id: String,
    pub vocabulary: Vec<String>,
    pub meaning: String,
    pub valence: MythValence,
    pub traditions: Vec<String>,
}

/// A declared motif entry from a `para:myth-motif` paragraph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MythMotif {
    pub para_id: String,
    pub name: String,
    pub description: String,
    pub valence: MythValence,
}

/// A declared archetype entry from a `para:myth-archetype` paragraph.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MythArchetype {
    pub para_id: String,
    pub role: ArchetypeRole,
    pub character_name: String,
    pub function_desc: String,
}

/// One myth check finding (the store record).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MythFinding {
    pub finding_type: FindingType,
    pub description: String,
    pub evidence: Option<String>,
    pub entry_para_id: Option<String>,
    pub chapter_ord: Option<u32>,
    pub suppressed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valence_and_role_round_trip() {
        for v in [MythValence::Positive, MythValence::Negative, MythValence::Ambiguous] {
            assert_eq!(MythValence::from_code(v.as_code()), v);
        }
        assert_eq!(MythValence::from_code("nonsense"), MythValence::Ambiguous);

        for r in [
            ArchetypeRole::Hero,
            ArchetypeRole::Mentor,
            ArchetypeRole::ThresholdGuardian,
            ArchetypeRole::Herald,
            ArchetypeRole::Shapeshifter,
            ArchetypeRole::Shadow,
            ArchetypeRole::Ally,
            ArchetypeRole::Trickster,
        ] {
            assert_eq!(ArchetypeRole::from_code(r.as_code()), r);
            assert!(r.is_vogler());
        }
        assert_eq!(ArchetypeRole::from_code("Threshold Guardian"), ArchetypeRole::ThresholdGuardian);
        let c = ArchetypeRole::from_code("dark oracle");
        assert_eq!(c, ArchetypeRole::Custom("dark_oracle".into()));
        assert!(!c.is_vogler());
    }

    #[test]
    fn finding_type_codes_and_registry() {
        for f in [
            FindingType::SymbolInconsistency,
            FindingType::MotifIncomplete,
            FindingType::MotifAbsentFinalAct,
            FindingType::ArchetypeVacant,
            FindingType::ArchetypeAbsent,
        ] {
            assert_eq!(FindingType::from_code(f.as_code()), Some(f));
            assert!(!f.label().is_empty());
        }
        assert_eq!(MYTH_TYPES.len(), 3);
        // Glyphs distinct.
        let glyphs: Vec<&str> = MYTH_TYPES.iter().map(|(_, g, _)| *g).collect();
        assert_eq!(glyphs, vec!["⊛", "∿", "⍟"]);
    }
}
