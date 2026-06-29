//! WORLD-6 (W-P2) — Stage 1: extraction. For each premise group, an LLM call
//! extracts structured logical claims from the tagged World-book paragraphs;
//! the result is parsed, validated, and persisted with content-hash laziness.
//! A deterministic (no-LLM) Facts cross-reference pass runs afterward.
//!
//! The LLM prompt is constrained to read ONLY what the author declared — never
//! to infer from real-world history, political theory, or to evaluate morality
//! (RFC §2/§4/§7.2).

use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::SYSTEM_TAG_FACTS;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use super::llm::{extract_json_array, utopia_llm_call};
use super::store::UtopiaStore;
use super::{
    ClaimType, FindingDomain, FindingType, PremiseGroup, UtopiaClaim, UtopiaFinding,
    detect_premise_groups,
};

const EXTRACTION_SYSTEM: &str = "You are a logical analyst. The paragraphs are from a \
fiction author's world design notes describing a fictional society's social and systemic \
structure. Extract a structured list of logical claims.\n\
IMPORTANT CONSTRAINTS:\n\
- Extract ONLY what the author has explicitly stated. Do not infer additional claims from \
your knowledge of history, political theory, sociology, or real societies.\n\
- Do NOT evaluate whether the claims are morally just, spiritually sound, or psychologically \
realistic. Evaluate logical structure only.\n\
- Do NOT add claims the author has not written.\n\
For each paragraph extract 1-3 core propositions as short, precise declarative sentences, \
each labelled premise / mechanism / consequence / elimination. \
Return ONLY a JSON array: \
[{\"source_para_id\":\"...\",\"claim_type\":\"premise|mechanism|consequence|elimination\",\"claim_text\":\"...\"}]";

/// Build the user prompt for one premise group: the tagged paragraphs with their
/// declared type and `source_para_id`.
pub(super) fn build_extraction_prompt(group: &PremiseGroup) -> String {
    let mut s = String::from("Paragraphs:\n");
    for c in &group.claims {
        s.push_str(&format!(
            "[{} · source_para_id={}]\n{}\n\n",
            c.claim_type.as_code(),
            c.para_id,
            c.text.trim()
        ));
    }
    s
}

/// Parse the LLM's claim array into `UtopiaClaim`s for `group`. Tolerant: skips
/// malformed entries; derives a deterministic `claim_id` from the source para +
/// claim text so re-extraction is idempotent.
pub(super) fn parse_claims(raw: &str, group: &str) -> Vec<UtopiaClaim> {
    let json = extract_json_array(raw);
    let arr: Vec<serde_json::Value> = serde_json::from_str(json).unwrap_or_default();
    let mut out = Vec::new();
    for v in arr {
        let Some(ct) = v.get("claim_type").and_then(|x| x.as_str()).and_then(ClaimType::from_code)
        else {
            continue;
        };
        let text = v.get("claim_text").and_then(|x| x.as_str()).unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let para = v
            .get("source_para_id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        out.push(UtopiaClaim {
            claim_id: deterministic_id("claim", group, &para, text),
            premise_group: group.to_string(),
            claim_type: ct,
            claim_text: text.to_string(),
            source_para_id: para,
        });
    }
    out
}

fn deterministic_id(prefix: &str, a: &str, b: &str, c: &str) -> String {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut h);
    b.hash(&mut h);
    c.hash(&mut h);
    format!("{prefix}-{:016x}", h.finish())
}

/// Hash of a group's tagged paragraph content — the Stage 1 cache key.
fn group_content_hash(group: &PremiseGroup) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for c in &group.claims {
        c.claim_type.as_code().hash(&mut h);
        c.text.hash(&mut h);
    }
    h.finish()
}

/// Extract one group (lazy on the content hash). Returns the claim count.
pub(super) fn extract_group(
    store: &UtopiaStore,
    cfg: &Config,
    book_slug: &str,
    group: &PremiseGroup,
) -> Result<usize> {
    let hash = group_content_hash(group);
    if store.stage1_hash(book_slug, &group.name)? == Some(hash) {
        return Ok(store.claims_for_group(book_slug, &group.name)?.len());
    }
    let user = build_extraction_prompt(group);
    let raw = utopia_llm_call(cfg, EXTRACTION_SYSTEM, &user)?;
    let claims = parse_claims(&raw, &group.name);
    store.clear_group_claims(book_slug, &group.name)?;
    let now = chrono::Utc::now().to_rfc3339();
    for c in &claims {
        store.upsert_claim(book_slug, c, &now, hash)?;
    }
    store.set_stage1(book_slug, &group.name, hash, &now)?;
    Ok(claims.len())
}

/// Run Stage 1 for the whole book: detect groups, extract each (lazy), then the
/// deterministic Facts cross-reference. Returns total claims extracted.
pub(super) fn run_stage1(
    store: &UtopiaStore,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    gap_threshold: usize,
) -> Result<usize> {
    let groups = detect_premise_groups(h, layout, gap_threshold);
    let mut total = 0;
    for g in &groups {
        total += extract_group(store, cfg, &book.slug, g)?;
    }
    facts_cross_reference(store, h, &book.slug, &groups)?;
    Ok(total)
}

/// Entity names from the Facts system book (titles of its paragraphs).
pub(super) fn facts_entities(h: &Hierarchy) -> Vec<String> {
    let Some(book) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_FACTS)
    }) else {
        return Vec::new();
    };
    let mut out: Vec<String> = h
        .collect_subtree(book.id)
        .into_iter()
        .filter_map(|id| h.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Deterministic cross-reference: if a Facts-book entity appears (whole-word,
/// case-insensitive) in an elimination claim, raise a `Factual` `ChainBreak`.
/// No LLM. Idempotent (deterministic finding ids; clears its own domain first).
pub(super) fn facts_cross_reference(
    store: &UtopiaStore,
    h: &Hierarchy,
    book_slug: &str,
    groups: &[PremiseGroup],
) -> Result<()> {
    let entities = facts_entities(h);
    let now = chrono::Utc::now().to_rfc3339();
    for g in groups {
        store.clear_group_findings_by_domain(book_slug, &g.name, FindingDomain::Factual)?;
        if entities.is_empty() {
            continue;
        }
        let claims = store.claims_for_group(book_slug, &g.name)?;
        for c in claims.iter().filter(|c| c.claim_type == ClaimType::Elimination) {
            let lc = c.claim_text.to_lowercase();
            for ent in &entities {
                if contains_word(&lc, &ent.to_lowercase()) {
                    let f = UtopiaFinding {
                        finding_id: deterministic_id("factual", &g.name, ent, &c.source_para_id),
                        premise_group: g.name.clone(),
                        finding_type: FindingType::ChainBreak,
                        finding_domain: FindingDomain::Factual,
                        description: format!(
                            "Fact '{ent}' is documented in the Facts system book but appears in \
                             the elimination inventory of premise group '{}'. Verify the fact is \
                             not used in the prose.",
                            g.name
                        ),
                        evidence: Some(c.claim_text.clone()),
                        chapter_ord: None,
                        para_id: Some(c.source_para_id.clone()),
                        suppressed: false,
                        grounded_by_research: false,
                    };
                    store.upsert_finding(book_slug, &f, &now, None)?;
                }
            }
        }
    }
    Ok(())
}

/// Whole-word (token) containment, both args already lowercase.
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    // Multi-word entity: substring match is acceptable; single word: token match.
    if needle.contains(' ') {
        return haystack.contains(needle);
    }
    haystack
        .split(|c: char| !c.is_alphanumeric())
        .any(|tok| tok == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group() -> PremiseGroup {
        PremiseGroup {
            name: "dominant".into(),
            claims: vec![
                super::super::TaggedPara {
                    para_id: "p1".into(),
                    claim_type: ClaimType::Premise,
                    text: "Truth is whatever the Party declares.".into(),
                },
                super::super::TaggedPara {
                    para_id: "p2".into(),
                    claim_type: ClaimType::Elimination,
                    text: "No independent journalism.".into(),
                },
            ],
        }
    }

    #[test]
    fn extraction_prompt_lists_paragraphs_with_ids() {
        let p = build_extraction_prompt(&group());
        assert!(p.contains("source_para_id=p1"));
        assert!(p.contains("premise"));
        assert!(p.contains("Truth is whatever"));
    }

    #[test]
    fn parse_claims_tolerant_and_typed() {
        let raw = "sure:\n[\
            {\"source_para_id\":\"p1\",\"claim_type\":\"premise\",\"claim_text\":\"X holds\"},\
            {\"source_para_id\":\"p2\",\"claim_type\":\"elimination\",\"claim_text\":\"No money\"},\
            {\"claim_type\":\"bogus\",\"claim_text\":\"skip\"},\
            {\"source_para_id\":\"p3\",\"claim_type\":\"mechanism\",\"claim_text\":\"  \"}\
        ]\nend";
        let claims = parse_claims(raw, "g");
        assert_eq!(claims.len(), 2); // bogus type + empty text dropped
        assert_eq!(claims[0].claim_type, ClaimType::Premise);
        assert_eq!(claims[1].claim_type, ClaimType::Elimination);
        // Deterministic ids → re-parse yields identical ids.
        let again = parse_claims(raw, "g");
        assert_eq!(claims[0].claim_id, again[0].claim_id);
    }

    #[test]
    fn parse_empty_array() {
        assert!(parse_claims("[]", "g").is_empty());
        assert!(parse_claims("no json here", "g").is_empty());
    }

    #[test]
    fn facts_cross_reference_flags_eliminated_entity() {
        let dir = tempfile::tempdir().unwrap();
        let store = UtopiaStore::open(dir.path()).unwrap();
        // A stored elimination claim mentioning "Newspeak".
        let claim = UtopiaClaim {
            claim_id: "c1".into(),
            premise_group: "dominant".into(),
            claim_type: ClaimType::Elimination,
            claim_text: "Newspeak abolishes the old vocabulary entirely.".into(),
            source_para_id: "p9".into(),
        };
        store.upsert_claim("b", &claim, "now", 1).unwrap();
        // Build a hierarchy with a Facts book holding the entity "Newspeak".
        let h = facts_hierarchy(&["Newspeak", "Telescreen"]);
        let g = PremiseGroup { name: "dominant".into(), claims: vec![] };
        facts_cross_reference(&store, &h, "b", std::slice::from_ref(&g)).unwrap();
        let findings = store.findings("b", true).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].finding_domain, FindingDomain::Factual);
        assert!(findings[0].description.contains("Newspeak"));
        // Idempotent: re-run does not duplicate.
        facts_cross_reference(&store, &h, "b", std::slice::from_ref(&g)).unwrap();
        assert_eq!(store.findings("b", true).unwrap().len(), 1);
    }

    fn facts_hierarchy(entities: &[&str]) -> Hierarchy {
        use serde_json::json;
        let fbook: Node = serde_json::from_value(json!({
            "id": uuid::Uuid::now_v7(), "kind": "book", "title": "Facts", "slug": "facts",
            "path": [], "parent_id": null, "order": 80, "file": null,
            "modified_at": "2026-01-01T00:00:00Z", "system_tag": "facts",
        })).unwrap();
        let mut nodes = vec![fbook.clone()];
        for e in entities {
            nodes.push(serde_json::from_value(json!({
                "id": uuid::Uuid::now_v7(), "kind": "paragraph", "title": e, "slug": e,
                "path": [], "parent_id": fbook.id, "order": 1, "file": null,
                "modified_at": "2026-01-01T00:00:00Z",
            })).unwrap());
        }
        Hierarchy::from_nodes_for_test(nodes)
    }

    #[test]
    fn contains_word_token_vs_substring() {
        assert!(contains_word("no money here", "money"));
        assert!(!contains_word("moneymaker", "money")); // token, not substring
        assert!(contains_word("the secret police force", "secret police")); // multi-word
    }
}
