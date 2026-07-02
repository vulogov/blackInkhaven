//! WORLD-6 (W-P5) — `utopian-architect` persona grounding. When that Inner
//! Socrates persona opens a Slow session, its *opening context* is grounded
//! from four sources in priority order (RFC §8): (1) active coherence findings
//! in `utopia.duckdb`, (2) the World book's tagged premise structure, (3) the
//! Facts book, (4 — MYTH-1) the declared motifs as thematic structure. If all
//! are empty the persona falls back to its default
//! questions (this returns `None`). The question bank itself is unchanged — only
//! the prose context the model reads is prefixed.

use std::path::Path;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

use super::store::UtopiaStore;
use super::{FindingType, PremiseGroup, UtopiaFinding, detect_premise_groups};

/// The persona id this grounding applies to.
pub(crate) const UTOPIAN_ARCHITECT: &str = "utopian-architect";

/// Build the grounding opening for the utopian-architect, or `None` to fall
/// back to the default questions. Opens the project store read-only.
pub(crate) fn build_grounding(project: &Path) -> Option<String> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path()).ok()?;
    let store = Store::open(layout.clone(), &cfg).ok()?;
    let h = Hierarchy::load(&store).ok()?;
    let book = crate::cli::resolve_user_book(&h, None, "utopia").ok()?.clone();
    let ustore = UtopiaStore::open(store.project_root()).ok()?;

    // Source 1 — active (unsuppressed) findings.
    if let Ok(findings) = ustore.findings(&book.slug, true) {
        if !findings.is_empty() {
            return Some(build_findings_text(&findings));
        }
    }
    // Source 2 — the World book's tagged premise structure. (The configurable
    // gap threshold is threaded through the callers in W-P9; default 1 here.)
    let groups = detect_premise_groups(&h, &layout, 1);
    if !groups.is_empty() {
        return Some(build_structure_text(&groups));
    }
    // Source 3 — the Facts book.
    let facts = super::stage1::facts_entities(&h);
    if !facts.is_empty() {
        return Some(build_facts_text(&facts));
    }
    // Source 4 (MYTH-1) — declared motifs as the book's thematic structure. When
    // the world has no coherence findings, premise tags, or facts yet, the
    // mythic layer still tells the architect which patterns the author cares
    // about, so the questions can probe whether the world serves them.
    if let Ok(ms) = crate::myth::MythStore::open(store.project_root()) {
        if let Ok(motifs) = ms.motifs(&book.slug) {
            if !motifs.is_empty() {
                return Some(build_motifs_text(&motifs));
            }
        }
    }
    None
}

/// Source 4 — the declared motifs when nothing else grounds the session.
pub(crate) fn build_motifs_text(motifs: &[crate::myth::MythMotif]) -> String {
    let list: Vec<String> = motifs.iter().take(8).map(|m| m.name.clone()).collect();
    format!(
        "Your Mythology book declares {} recurring motif(s) ({}). Let us consider whether your world's \
         logic serves these patterns or works against them.",
        motifs.len(),
        list.join(", ")
    )
}

/// Source 1 — a grounded opening from coherence findings (§8.2). Chain-logic
/// findings first, then entailment; capped so the prompt stays bounded.
pub(crate) fn build_findings_text(findings: &[UtopiaFinding]) -> String {
    const MAX: usize = 5;
    let shown: Vec<&UtopiaFinding> = findings.iter().take(MAX).collect();
    let mut s = format!(
        "Before I begin my questions, the coherence check has flagged {} unresolved tension(s) \
         in your declared world system:\n",
        findings.len()
    );
    for (i, f) in shown.iter().enumerate() {
        let loc = match (f.finding_type, f.chapter_ord, &f.para_id) {
            (FindingType::EntailmentViolation, Some(ch), Some(p)) => {
                // BUG-12 — truncate by chars, not bytes: `para_id` can be a
                // model-echoed value, and a byte slice at 8 could split a codepoint.
                let short: String = p.chars().take(8).collect();
                format!(" (chapter {ch}, para {short})")
            }
            _ => String::new(),
        };
        s.push_str(&format!("{}. {}{loc}\n", i + 1, f.description));
    }
    if findings.len() > MAX {
        s.push_str(&format!("…and {} more.\n", findings.len() - MAX));
    }
    s.push_str(
        "\nShall we begin by exploring one of these, or proceed to my questions about the \
         manuscript's thematic coherence?",
    );
    s
}

/// Source 2 — the premise structure when no findings exist yet.
pub(crate) fn build_structure_text(groups: &[PremiseGroup]) -> String {
    use super::ClaimType::*;
    let mut s = String::from(
        "The coherence check has not run yet, but your World book declares this premise structure:\n",
    );
    for g in groups {
        let count = |t: super::ClaimType| g.claims.iter().filter(|c| c.claim_type == t).count();
        s.push_str(&format!(
            "- {}: {} premise(s), {} mechanism(s), {} consequence(s), {} elimination(s)\n",
            g.name,
            count(Premise),
            count(Mechanism),
            count(Consequence),
            count(Elimination),
        ));
    }
    s.push_str("\nLet us examine whether these hold together.");
    s
}

/// Source 3 — the Facts book entities when no premise structure exists.
pub(crate) fn build_facts_text(facts: &[String]) -> String {
    let list: Vec<&str> = facts.iter().take(12).map(String::as_str).collect();
    format!(
        "Your Facts book documents: {}. Let us consider what world these facts imply.",
        list.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{FindingDomain, TaggedPara};

    fn finding(ft: FindingType, desc: &str, ch: Option<u32>, para: Option<&str>) -> UtopiaFinding {
        UtopiaFinding {
            finding_id: "f".into(),
            premise_group: "g".into(),
            finding_type: ft,
            finding_domain: FindingDomain::Systemic,
            description: desc.into(),
            evidence: None,
            chapter_ord: ch,
            para_id: para.map(str::to_string),
            suppressed: false,
            grounded_by_research: false,
        }
    }

    #[test]
    fn findings_text_numbers_and_locates() {
        let fs = vec![
            finding(FindingType::ChainBreak, "CHAIN BREAK — mechanism X", None, None),
            finding(
                FindingType::EntailmentViolation,
                "ENTAILMENT — private letter",
                Some(12),
                Some("para-47abcdef"),
            ),
        ];
        let t = build_findings_text(&fs);
        assert!(t.contains("2 unresolved tension"));
        assert!(t.contains("1. CHAIN BREAK"));
        assert!(t.contains("2. ENTAILMENT"));
        assert!(t.contains("chapter 12, para"));
        assert!(t.contains("proceed to my questions"));
    }

    #[test]
    fn structure_text_counts_claim_types() {
        let g = PremiseGroup {
            name: "dominant".into(),
            claims: vec![
                TaggedPara { para_id: "a".into(), claim_type: super::super::ClaimType::Premise, text: "p".into() },
                TaggedPara { para_id: "b".into(), claim_type: super::super::ClaimType::Elimination, text: "e".into() },
                TaggedPara { para_id: "c".into(), claim_type: super::super::ClaimType::Elimination, text: "e2".into() },
            ],
        };
        let t = build_structure_text(&[g]);
        assert!(t.contains("dominant: 1 premise(s), 0 mechanism(s), 0 consequence(s), 2 elimination(s)"));
    }

    #[test]
    fn facts_text_lists_entities() {
        let t = build_facts_text(&["Newspeak".into(), "Telescreen".into()]);
        assert!(t.contains("Newspeak"));
        assert!(t.contains("Telescreen"));
    }
}
