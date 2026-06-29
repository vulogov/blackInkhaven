//! WORLD-6 (W-P4) — Stage 3: entailment scan. For each chapter, the prose is
//! checked against the premise group's *elimination inventory*: does the
//! chapter use, depend on, or assume something the world declared eliminated?
//! Runs chapter by chapter with per-chapter caching (hash of chapter text +
//! inventory). Skips short chapters; respects a per-pass batch size.
//!
//! Stage 3 does NOT depend on Stage 2 — the inventory comes from Stage 1 claims.

use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::llm::{extract_json_array, utopia_llm_call};
use super::store::UtopiaStore;
use super::{FindingDomain, FindingType, UtopiaClaim, UtopiaFinding};

const ENTAILMENT_SYSTEM: &str = "You are checking a fiction manuscript for consistency with its \
declared world rules. The author has declared that certain things do NOT exist, are forbidden, or \
have been transformed in this fictional world.\n\
IMPORTANT CONSTRAINTS:\n\
- Flag ONLY clear, specific uses of eliminated elements — not metaphorical or historical \
references.\n\
- Do NOT flag a character merely thinking about an eliminated element — only USING one.\n\
- Do NOT evaluate whether the elimination is wise, just, or realistic.\n\
- Do NOT use real-world history or sociology to judge plausibility. Accept the world as declared.\n\
- If you find no violations, return an empty array.\n\
Return ONLY a JSON array (empty if none): \
[{\"para_id\":\"...|null\",\"eliminated_element\":\"...\",\"evidence_text\":\"...\",\"description\":\"...\"}]";

/// The elimination inventory (texts) for a group's claims.
pub(super) fn elimination_inventory(claims: &[UtopiaClaim]) -> Vec<String> {
    claims
        .iter()
        .filter(|c| c.claim_type == super::ClaimType::Elimination)
        .map(|c| c.claim_text.clone())
        .collect()
}

fn inventory_hash(inventory: &[String]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for e in inventory {
        e.hash(&mut h);
    }
    h.finish()
}

/// Per-chapter Stage 3 cache key: chapter text + the elimination inventory.
fn chapter_scan_hash(chapter_text: &str, inventory: &[String]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    chapter_text.hash(&mut h);
    inventory_hash(inventory).hash(&mut h);
    h.finish()
}

/// Build the entailment-scan user prompt.
pub(super) fn build_entailment_prompt(inventory: &[String], chapter_text: &str) -> String {
    let inv = inventory
        .iter()
        .map(|e| format!("- {e}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!("ELIMINATION INVENTORY:\n{inv}\n\nChapter text:\n{chapter_text}")
}

/// Parse the violation array into `EntailmentViolation` findings for a chapter.
pub(super) fn parse_violations(
    raw: &str,
    group: &str,
    chapter_ord: u32,
) -> Vec<UtopiaFinding> {
    let json = extract_json_array(raw);
    let arr: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    let mut out = Vec::new();
    for v in arr {
        let element = v
            .get("eliminated_element")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim();
        if element.is_empty() {
            continue;
        }
        let evidence = v.get("evidence_text").and_then(|x| x.as_str()).unwrap_or("").trim();
        let desc = v.get("description").and_then(|x| x.as_str()).unwrap_or("").trim();
        let para_id = v
            .get("para_id")
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty() && *s != "null")
            .map(str::to_string);
        let description = if desc.is_empty() {
            format!("\"{element}\" appears to be used; the elimination inventory forbids it")
        } else {
            format!("\"{element}\" used — {desc}")
        };
        out.push(UtopiaFinding {
            finding_id: deterministic_id(group, chapter_ord, element, evidence),
            premise_group: group.to_string(),
            finding_type: FindingType::EntailmentViolation,
            finding_domain: FindingDomain::Logical,
            description,
            evidence: (!evidence.is_empty()).then(|| evidence.to_string()),
            chapter_ord: Some(chapter_ord),
            para_id,
            suppressed: false,
            grounded_by_research: false,
        });
    }
    out
}

fn deterministic_id(group: &str, chapter: u32, element: &str, evidence: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    group.hash(&mut h);
    chapter.hash(&mut h);
    element.to_lowercase().hash(&mut h);
    evidence.hash(&mut h);
    format!("entail-{:016x}", h.finish())
}

/// A chapter's prose as one stripped blob (excludes Jinja; strips Typst).
pub(super) fn chapter_text(layout: &ProjectLayout, h: &Hierarchy, chapter_id: uuid::Uuid) -> String {
    let mut out = String::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        if let Some(rel) = p.file.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                out.push_str(&crate::audiobook::typst_to_plain(&raw));
                out.push('\n');
            }
        }
    }
    out
}

/// Run Stage 3 for one premise group across the book's chapters (up to
/// `batch_size` stale chapters this pass). Skips chapters below `min_words` and
/// chapters whose scan hash is unchanged. Returns (violations found, chapters
/// scanned this pass, chapters still stale).
pub(super) fn run_stage3_group(
    store: &UtopiaStore,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    group: &str,
    min_words: usize,
    batch_size: usize,
) -> Result<(usize, usize, usize)> {
    let claims = store.claims_for_group(&book.slug, group)?;
    let inventory = elimination_inventory(&claims);
    if inventory.is_empty() {
        return Ok((0, 0, 0));
    }
    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();
    let now = chrono::Utc::now().to_rfc3339();
    let (mut found, mut scanned, mut stale) = (0usize, 0usize, 0usize);
    for (idx, ch) in chapters.iter().enumerate() {
        let ord = (idx + 1) as u32;
        let text = chapter_text(layout, h, ch.id);
        if text.split_whitespace().count() < min_words {
            continue;
        }
        let hash = chapter_scan_hash(&text, &inventory);
        if store.chapter_scan_hash(&book.slug, group, ord)? == Some(hash) {
            continue; // unchanged
        }
        if scanned >= batch_size {
            stale += 1;
            continue; // deferred to a later pass
        }
        let user = build_entailment_prompt(&inventory, &text);
        let raw = utopia_llm_call(cfg, ENTAILMENT_SYSTEM, &user)?;
        let violations = parse_violations(&raw, group, ord);
        store.clear_chapter_entailment(&book.slug, group, ord)?;
        for f in &violations {
            store.upsert_finding(&book.slug, f, &now, None)?;
        }
        found += violations.len();
        store.set_chapter_scanned(&book.slug, group, ord, hash, &now)?;
        scanned += 1;
    }
    Ok((found, scanned, stale))
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::ClaimType;

    fn claim(ct: ClaimType, text: &str) -> UtopiaClaim {
        UtopiaClaim {
            claim_id: text.into(),
            premise_group: "g".into(),
            claim_type: ct,
            claim_text: text.into(),
            source_para_id: "p".into(),
        }
    }

    #[test]
    fn inventory_is_eliminations_only() {
        let claims = vec![
            claim(ClaimType::Premise, "P"),
            claim(ClaimType::Elimination, "no money"),
            claim(ClaimType::Elimination, "no privacy"),
            claim(ClaimType::Mechanism, "M"),
        ];
        let inv = elimination_inventory(&claims);
        assert_eq!(inv, vec!["no money".to_string(), "no privacy".to_string()]);
    }

    #[test]
    fn entailment_prompt_lists_inventory() {
        let p = build_entailment_prompt(&["no money".into()], "He paid in gold.");
        assert!(p.contains("ELIMINATION INVENTORY"));
        assert!(p.contains("- no money"));
        assert!(p.contains("He paid in gold."));
    }

    #[test]
    fn parse_violations_typed_and_deterministic() {
        let raw = "[{\"para_id\":\"p47\",\"eliminated_element\":\"private correspondence\",\
            \"evidence_text\":\"a sealed letter\",\"description\":\"a private letter\"},\
            {\"eliminated_element\":\"\",\"evidence_text\":\"x\"}]";
        let v = parse_violations(raw, "g", 12);
        assert_eq!(v.len(), 1); // empty element dropped
        assert_eq!(v[0].finding_type, FindingType::EntailmentViolation);
        assert_eq!(v[0].finding_domain, FindingDomain::Logical);
        assert_eq!(v[0].chapter_ord, Some(12));
        assert_eq!(v[0].para_id.as_deref(), Some("p47"));
        assert!(v[0].description.contains("private correspondence"));
        // Idempotent ids.
        assert_eq!(parse_violations(raw, "g", 12)[0].finding_id, v[0].finding_id);
    }

    #[test]
    fn empty_violations_when_no_array() {
        assert!(parse_violations("[]", "g", 1).is_empty());
        assert!(parse_violations("none found", "g", 1).is_empty());
    }

    #[test]
    fn null_para_id_handled() {
        let raw = "[{\"para_id\":null,\"eliminated_element\":\"gold\",\"evidence_text\":\"a coin\",\"description\":\"\"}]";
        let v = parse_violations(raw, "g", 3);
        assert_eq!(v.len(), 1);
        assert!(v[0].para_id.is_none());
    }
}
