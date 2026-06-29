//! WORLD-6 — utopian/dystopian coherence checker. Reads author-declared
//! `para:utopia-*` paragraphs from the World system book and checks the logical
//! chain (premise → mechanism → consequence) plus the elimination inventory for
//! coherence, via a three-stage LLM pipeline. Read-only / advisory; caps inform,
//! never block. Moral/theological coherence is explicitly out of scope (reserved
//! for a future Inner Theologian).
//!
//! W-P0 — the language model: the `para:utopia-*` tag/glyph registry, the core
//! claim/finding types, and premise-group detection over the World book. The
//! store (W-P1) and the LLM stages (W-P2…W-P4) build on these.

use crate::project::ProjectLayout;
use crate::store::SYSTEM_TAG_WORLD;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

mod store;

pub(crate) use store::UtopiaStore;

/// The four kinds of declared claim that make up a premise set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ClaimType {
    /// A foundational social/systemic claim — what differs from the real world.
    Premise,
    /// How the premise is enforced, maintained, or propagated.
    Mechanism,
    /// What the premise produces as social outcomes.
    Consequence,
    /// What the premise logically eliminates, forbids, or transforms.
    Elimination,
}

impl ClaimType {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            ClaimType::Premise => "premise",
            ClaimType::Mechanism => "mechanism",
            ClaimType::Consequence => "consequence",
            ClaimType::Elimination => "elimination",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<ClaimType> {
        match s {
            "premise" => Some(ClaimType::Premise),
            "mechanism" => Some(ClaimType::Mechanism),
            "consequence" => Some(ClaimType::Consequence),
            "elimination" => Some(ClaimType::Elimination),
            _ => None,
        }
    }

    /// The tree glyph for this declaration type.
    pub(crate) fn glyph(&self) -> &'static str {
        match self {
            ClaimType::Premise => "⊢ ",
            ClaimType::Mechanism => "⚙ ",
            ClaimType::Consequence => "⇒ ",
            ClaimType::Elimination => "∅ ",
        }
    }

    /// Uppercase display label for the model table.
    pub(crate) fn label(&self) -> &'static str {
        match self {
            ClaimType::Premise => "PREMISE",
            ClaimType::Mechanism => "MECHANISM",
            ClaimType::Consequence => "CONSEQUENCE",
            ClaimType::Elimination => "ELIMINATION",
        }
    }
}

/// The registered `para:utopia-*` tag values, their glyphs, and the claim type
/// each marks. Parallel to STRUCT-2's `STRUCTURAL_TYPES`, but these are *World-
/// book-only declarations* with no seeded boilerplate — so they have their own
/// registry and glyph lookup rather than riding the structural-paragraph picker.
pub(crate) const UTOPIA_TYPES: &[(&str, ClaimType)] = &[
    ("para:utopia-premise", ClaimType::Premise),
    ("para:utopia-mechanism", ClaimType::Mechanism),
    ("para:utopia-consequence", ClaimType::Consequence),
    ("para:utopia-elimination", ClaimType::Elimination),
];

/// The claim type a node's `para:utopia-*` tag marks, if it carries one.
pub(crate) fn claim_type_of(node: &Node) -> Option<ClaimType> {
    node.tags.iter().find_map(|t| {
        UTOPIA_TYPES
            .iter()
            .find(|(tag, _)| *tag == t)
            .map(|(_, ct)| *ct)
    })
}

/// The tree glyph for a paragraph's `para:utopia-*` tag, if it carries one
/// (`None` → not a utopia declaration). Mirrors `structural_glyph`.
pub(crate) fn utopia_glyph(node: &Node) -> Option<&'static str> {
    claim_type_of(node).map(|ct| ct.glyph())
}

/// Stage-2 finding kinds (chain logic) and the Stage-3 entailment kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindingType {
    /// Stage 2: a mechanism does not follow from / enforce a premise.
    ChainBreak,
    /// Stage 2: a consequence does not follow from a mechanism.
    ConsequenceGap,
    /// Stage 2: two consequences contradict each other.
    InternalConflict,
    /// Stage 3: prose uses something the premise declared eliminated.
    EntailmentViolation,
}

impl FindingType {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            FindingType::ChainBreak => "chain_break",
            FindingType::ConsequenceGap => "consequence_gap",
            FindingType::InternalConflict => "internal_conflict",
            FindingType::EntailmentViolation => "entailment_violation",
        }
    }

    pub(crate) fn from_code(s: &str) -> Option<FindingType> {
        match s {
            "chain_break" => Some(FindingType::ChainBreak),
            "consequence_gap" => Some(FindingType::ConsequenceGap),
            "internal_conflict" => Some(FindingType::InternalConflict),
            "entailment_violation" => Some(FindingType::EntailmentViolation),
            _ => None,
        }
    }

    /// `coherence` (Stage 2 chain logic) vs `prose` (Stage 3 entailment).
    pub(crate) fn stage_label(&self) -> &'static str {
        match self {
            FindingType::EntailmentViolation => "prose",
            _ => "coherence",
        }
    }
}

/// Which reasoning domain wrote a finding. WORLD-6 writes the first three;
/// `Theological` is reserved for a future Inner Theologian (never written here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FindingDomain {
    Systemic,
    Logical,
    Factual,
    Theological,
}

impl FindingDomain {
    pub(crate) fn as_code(&self) -> &'static str {
        match self {
            FindingDomain::Systemic => "systemic",
            FindingDomain::Logical => "logical",
            FindingDomain::Factual => "factual",
            FindingDomain::Theological => "theological",
        }
    }

    pub(crate) fn from_code(s: &str) -> FindingDomain {
        match s {
            "logical" => FindingDomain::Logical,
            "factual" => FindingDomain::Factual,
            "theological" => FindingDomain::Theological,
            _ => FindingDomain::Systemic,
        }
    }
}

/// One extracted logical claim (Stage 1 output).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UtopiaClaim {
    pub claim_id: String,
    pub premise_group: String,
    pub claim_type: ClaimType,
    pub claim_text: String,
    pub source_para_id: String,
}

/// One coherence finding (Stage 2 chain logic or Stage 3 entailment).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct UtopiaFinding {
    pub finding_id: String,
    pub premise_group: String,
    pub finding_type: FindingType,
    pub finding_domain: FindingDomain,
    pub description: String,
    pub evidence: Option<String>,
    pub chapter_ord: Option<u32>,
    pub para_id: Option<String>,
    pub suppressed: bool,
    pub grounded_by_research: bool,
}

/// One tagged declaration paragraph from the World book.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct TaggedPara {
    pub para_id: String,
    pub claim_type: ClaimType,
    pub text: String,
}

/// A contiguous run of tagged declaration paragraphs = one premise set.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PremiseGroup {
    pub name: String,
    pub claims: Vec<TaggedPara>,
}

/// The role a World-book paragraph plays in grouping.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ParaRole {
    /// A `para:utopia-*` declaration.
    Claim(ClaimType),
    /// A `// world:group:<name>` override comment — names the *next* group.
    GroupComment(String),
    /// Any other paragraph (a gap that can break a group).
    Other,
}

/// The World system book, if present.
pub(crate) fn world_book(h: &Hierarchy) -> Option<&Node> {
    h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_WORLD)
    })
}

/// Parse a `// world:group:<name>` override out of a comment paragraph's text.
pub(crate) fn parse_group_comment(text: &str) -> Option<String> {
    let t = text.trim();
    let rest = t.strip_prefix("//")?.trim();
    let name = rest.strip_prefix("world:group:")?.trim();
    (!name.is_empty()).then(|| name.to_string())
}

/// Group ordered World-book paragraphs into premise sets. A gap of
/// `gap_threshold` consecutive non-claim paragraphs breaks the current group
/// (default 1 → any gap breaks; 0 → never break, one group). A
/// `// world:group:<name>` comment names the next group and does not count as a
/// gap. Pure over the role list so it's unit-testable without disk.
pub(crate) fn group_paragraphs(
    items: &[(String, ParaRole, String)],
    gap_threshold: usize,
) -> Vec<PremiseGroup> {
    let mut groups: Vec<PremiseGroup> = Vec::new();
    let mut current: Vec<TaggedPara> = Vec::new();
    let mut pending_name: Option<String> = None;
    let mut auto = 0usize;
    let mut gap = 0usize;

    let mut flush = |current: &mut Vec<TaggedPara>, pending: &mut Option<String>| {
        if current.is_empty() {
            return;
        }
        auto += 1;
        let name = pending.take().unwrap_or_else(|| format!("group_{auto}"));
        groups.push(PremiseGroup {
            name,
            claims: std::mem::take(current),
        });
    };

    for (para_id, role, text) in items {
        match role {
            ParaRole::GroupComment(name) => {
                // Names the next group; only takes effect at a fresh group start.
                if current.is_empty() {
                    pending_name = Some(name.clone());
                }
                // A comment is not a gap.
            }
            ParaRole::Claim(ct) => {
                current.push(TaggedPara {
                    para_id: para_id.clone(),
                    claim_type: *ct,
                    text: text.clone(),
                });
                gap = 0;
            }
            ParaRole::Other => {
                if gap_threshold == 0 {
                    continue; // never break
                }
                gap += 1;
                if gap >= gap_threshold {
                    flush(&mut current, &mut pending_name);
                }
            }
        }
    }
    flush(&mut current, &mut pending_name);
    groups
}

/// Read a World-book paragraph's plain text (Typst stripped). Empty when the
/// file is missing.
pub(crate) fn para_text(layout: &ProjectLayout, node: &Node) -> String {
    node.file
        .as_ref()
        .and_then(|rel| std::fs::read_to_string(layout.root.join(rel)).ok())
        .map(|raw| crate::audiobook::typst_to_plain(&raw))
        .unwrap_or_default()
}

/// Build the role list for the World book (in document order) and group it.
pub(crate) fn detect_premise_groups(
    h: &Hierarchy,
    layout: &ProjectLayout,
    gap_threshold: usize,
) -> Vec<PremiseGroup> {
    let Some(book) = world_book(h) else {
        return Vec::new();
    };
    let mut items: Vec<(String, ParaRole, String)> = Vec::new();
    for id in h.collect_subtree(book.id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        let text = para_text(layout, p);
        let role = if let Some(ct) = claim_type_of(p) {
            ParaRole::Claim(ct)
        } else if let Some(name) = parse_group_comment(&text) {
            ParaRole::GroupComment(name)
        } else {
            ParaRole::Other
        };
        items.push((id.to_string(), role, text));
    }
    group_paragraphs(&items, gap_threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(id: &str, ct: ClaimType) -> (String, ParaRole, String) {
        (id.into(), ParaRole::Claim(ct), format!("text of {id}"))
    }
    fn other(id: &str) -> (String, ParaRole, String) {
        (id.into(), ParaRole::Other, String::new())
    }
    fn comment(id: &str, name: &str) -> (String, ParaRole, String) {
        (id.into(), ParaRole::GroupComment(name.into()), String::new())
    }

    #[test]
    fn claim_type_codes_round_trip() {
        for ct in [
            ClaimType::Premise,
            ClaimType::Mechanism,
            ClaimType::Consequence,
            ClaimType::Elimination,
        ] {
            assert_eq!(ClaimType::from_code(ct.as_code()), Some(ct));
        }
        assert_eq!(UTOPIA_TYPES.len(), 4);
    }

    #[test]
    fn single_group_when_contiguous() {
        let items = vec![
            claim("a", ClaimType::Premise),
            claim("b", ClaimType::Mechanism),
            claim("c", ClaimType::Consequence),
        ];
        let g = group_paragraphs(&items, 1);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "group_1");
        assert_eq!(g[0].claims.len(), 3);
    }

    #[test]
    fn gap_breaks_into_two_groups() {
        let items = vec![
            claim("a", ClaimType::Premise),
            claim("b", ClaimType::Mechanism),
            other("gap"),
            claim("c", ClaimType::Premise),
            claim("d", ClaimType::Elimination),
        ];
        let g = group_paragraphs(&items, 1);
        assert_eq!(g.len(), 2);
        assert_eq!(g[0].claims.len(), 2);
        assert_eq!(g[1].claims.len(), 2);
        assert_eq!(g[1].name, "group_2");
    }

    #[test]
    fn gap_threshold_zero_never_breaks() {
        let items = vec![
            claim("a", ClaimType::Premise),
            other("gap"),
            claim("b", ClaimType::Mechanism),
        ];
        let g = group_paragraphs(&items, 0);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].claims.len(), 2);
    }

    #[test]
    fn group_comment_names_next_group_and_is_not_a_gap() {
        let items = vec![
            comment("c1", "dominant_society"),
            claim("a", ClaimType::Premise),
            claim("b", ClaimType::Mechanism),
        ];
        let g = group_paragraphs(&items, 1);
        assert_eq!(g.len(), 1);
        assert_eq!(g[0].name, "dominant_society");
    }

    #[test]
    fn parse_group_comment_forms() {
        assert_eq!(
            parse_group_comment("// world:group:resistance"),
            Some("resistance".to_string())
        );
        assert_eq!(parse_group_comment("//world:group: rebels "), Some("rebels".to_string()));
        assert_eq!(parse_group_comment("just a comment"), None);
        assert_eq!(parse_group_comment("// world:group:"), None);
    }
}
