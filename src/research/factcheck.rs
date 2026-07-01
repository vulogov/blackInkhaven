//! RESRCH-1 — `/factcheck`: a post-hoc audit of the whole Facts corpus. Two
//! phases (multiple LLM calls, by design):
//!   1. **truth** — every declared fact assessed for factual accuracy against
//!      the model's general knowledge (batched, a chunk per call);
//!   2. **consistency** — the full set checked for facts that contradict each
//!      other.
//!
//! Read-only: it reports findings into the chat; it never edits the corpus.

use uuid::Uuid;

use crate::store::NodeKind;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

/// One fact to audit: its node id, a readable location, and its prose.
pub(super) struct FactEntry {
    pub id: Uuid,
    pub location: String,
    pub text: String,
}

/// How many facts go into one truth-check LLM call.
pub(super) const TRUTH_CHUNK: usize = 8;

/// Whether a node carries the authorial `fact:undisputed` tag (RESRCH-UNDISPUTED).
fn is_undisputed(node: &crate::store::node::Node) -> bool {
    node.tags.iter().any(|t| t == super::UNDISPUTED_TAG)
}

/// Gather Facts-book paragraphs as `FactEntry`s (non-empty). `undisputed`
/// selects which set: `false` → the **disputed** facts (what `/factcheck`
/// checks); `true` → the **undisputed** authorial facts (what `/undisputed`
/// checks). RESRCH-UNDISPUTED — undisputed facts are excluded from `/factcheck`.
fn gather(store: &Store, h: &Hierarchy, book_id: Uuid, undisputed: bool) -> Vec<FactEntry> {
    let mut out = Vec::new();
    for id in h.collect_subtree(book_id) {
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph || is_undisputed(node) != undisputed {
            continue;
        }
        let text = match store.get_content(id) {
            Ok(Some(bytes)) => String::from_utf8_lossy(&bytes).trim().to_string(),
            _ => String::new(),
        };
        if text.is_empty() {
            continue;
        }
        out.push(FactEntry { id, location: h.slug_path(node), text });
    }
    out
}

/// The disputed facts — what `/factcheck` audits (undisputed ones are excluded).
pub(super) fn gather_facts(store: &Store, h: &Hierarchy, book_id: Uuid) -> Vec<FactEntry> {
    gather(store, h, book_id, false)
}

/// The undisputed (authorial) facts — what `/undisputed` checks for common sense.
pub(super) fn gather_undisputed(store: &Store, h: &Hierarchy, book_id: Uuid) -> Vec<FactEntry> {
    gather(store, h, book_id, true)
}

/// RESRCH-UNDISPUTED — the `/undisputed` common-sense prompt. Frames the facts as
/// deliberate fiction and checks only INTERNAL coherence, never real-world truth,
/// and never proposes rewrites.
pub(super) fn undisputed_system(language: &str) -> String {
    format!(
        "The following are statements from a WORK OF FICTION — the author's deliberate creative \
         invention. Do NOT check them against real-world knowledge; they are not meant to be real. \
         For EACH numbered statement, judge ONLY its internal common sense: is it self-consistent and \
         plausible WITHIN its own fictional frame, free of obvious contradiction or nonsense? Respond \
         with one line per statement, in this exact shape:\n\
         <number>. PLAUSIBLE | ODD | INCOHERENT — <short reason>\n\
         Never propose rewrites and never change the statements. Write the reasons in {language}."
    )
}

/// The truth-check system prompt (per-statement accuracy verdicts).
pub(super) fn truth_system(language: &str) -> String {
    format!(
        "You are fact-checking statements from a writer's reference database against your \
         general knowledge. For EACH numbered statement, judge its real-world factual accuracy. \
         Respond with one line per statement, in this exact shape:\n\
         <number>. ACCURATE | DUBIOUS | INACCURATE — <short reason>\n\
         Be concise. Do not add commentary outside the per-statement lines. \
         Write the reasons in {language}."
    )
}

/// The user message for a truth-check chunk: the numbered statements.
pub(super) fn truth_user(chunk: &[&FactEntry], base: usize) -> String {
    let mut s = String::from("Statements:\n");
    for (i, f) in chunk.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", base + i + 1, f.text));
    }
    s
}

/// The consistency-check system prompt (mutual contradictions).
pub(super) fn consistency_system(language: &str) -> String {
    format!(
        "You are checking a writer's reference database for internal consistency. Below are \
         numbered facts. Identify every PAIR of facts that CONTRADICT each other. Respond with \
         one line per contradicting pair, in this exact shape:\n\
         <a> ⇄ <b> — <what conflicts>\n\
         If there are no contradictions, reply exactly: No contradictions found. \
         Write the explanations in {language}."
    )
}

/// The user message for the consistency check: the group's facts, numbered.
pub(super) fn consistency_user(facts: &[&FactEntry]) -> String {
    let mut s = String::from("Facts:\n");
    for (i, f) in facts.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, f.text));
    }
    s
}

/// R2-E — the largest number of facts in one consistency call. Branches bigger
/// than this are split into bounded sub-chunks so context never grows unbounded.
pub(super) const CONSIST_MAX: usize = 16;

/// One bounded consistency call: a human label + indices into the fact list.
pub(super) struct ConsistGroup {
    pub label: String,
    pub idxs: Vec<usize>,
}

/// The Facts-tree branch a fact belongs to — the first path segment after the
/// Facts book root (`facts/<branch>/…`). Facts directly under the root, or one
/// level down, share `"(root)"` (they have no distinguishing branch).
pub(super) fn branch_label(location: &str) -> String {
    let segs: Vec<&str> = location.split('/').filter(|s| !s.is_empty()).collect();
    if segs.len() > 2 { segs[1].to_string() } else { "(root)".to_string() }
}

/// R2-E — partition the corpus into **bounded** consistency calls instead of one
/// unbounded prompt: a within-branch pass per Facts chapter (large branches split
/// into `CONSIST_MAX` sub-chunks), plus a final **cross-branch** pass over a few
/// representatives per branch so cross-cutting contradictions still surface.
pub(super) fn consistency_groups(facts: &[FactEntry], max_group: usize) -> Vec<ConsistGroup> {
    use std::collections::HashMap;
    let mut order: Vec<String> = Vec::new();
    let mut by_branch: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, f) in facts.iter().enumerate() {
        let b = branch_label(&f.location);
        if !by_branch.contains_key(&b) {
            order.push(b.clone());
        }
        by_branch.entry(b).or_default().push(i);
    }

    let mut groups: Vec<ConsistGroup> = Vec::new();
    let mut any_split = false;
    for b in &order {
        let idxs = &by_branch[b];
        if idxs.len() < 2 {
            continue; // a lone fact can't contradict within its own branch
        }
        if idxs.len() <= max_group {
            groups.push(ConsistGroup { label: b.clone(), idxs: idxs.clone() });
        } else {
            any_split = true;
            let k = idxs.len().div_ceil(max_group);
            for (ci, chunk) in idxs.chunks(max_group).enumerate() {
                groups.push(ConsistGroup { label: format!("{b} ({}/{k})", ci + 1), idxs: chunk.to_vec() });
            }
        }
    }

    // Cross pass — when more than one branch exists, or a branch was split (so
    // contradictions spanning sub-chunks of the same branch can still be caught).
    if order.len() > 1 || any_split {
        let mut reps: Vec<usize> = Vec::new();
        'outer: for b in &order {
            for &i in by_branch[b].iter().take(2) {
                reps.push(i);
                if reps.len() >= max_group * 2 {
                    break 'outer;
                }
            }
        }
        if reps.len() >= 2 {
            groups.push(ConsistGroup { label: "cross-branch".to_string(), idxs: reps });
        }
    }

    // Fallback — everything in one (still bounded) pass when nothing grouped
    // (e.g. a handful of lone facts all under the root).
    if groups.is_empty() && facts.len() >= 2 {
        groups.push(ConsistGroup { label: "all".to_string(), idxs: (0..facts.len()).collect() });
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fe(text: &str) -> FactEntry {
        FactEntry { id: Uuid::nil(), location: "facts/x".into(), text: text.into() }
    }

    fn fe_at(location: &str, text: &str) -> FactEntry {
        FactEntry { id: Uuid::nil(), location: location.into(), text: text.into() }
    }

    #[test]
    fn truth_user_numbers_from_base() {
        let a = fe("A"); let b = fe("B");
        let refs = vec![&a, &b];
        let u = truth_user(&refs, 8);
        assert!(u.contains("9. A"));
        assert!(u.contains("10. B"));
    }

    #[test]
    fn prompts_carry_language() {
        assert!(truth_system("Russian").contains("Russian"));
        assert!(consistency_system("Russian").contains("Russian"));
    }

    #[test]
    fn consistency_user_numbers_from_one() {
        let a = fe("A"); let b = fe("B");
        let refs = vec![&a, &b];
        let u = consistency_user(&refs);
        assert!(u.contains("1. A"));
        assert!(u.contains("2. B"));
    }

    #[test]
    fn branch_label_extracts_chapter() {
        assert_eq!(branch_label("facts/rome/engineering/aqueduct"), "rome");
        assert_eq!(branch_label("facts/rome/legion"), "rome"); // paragraph under chapter rome
        assert_eq!(branch_label("facts/loose"), "(root)"); // paragraph directly under the book
    }

    #[test]
    fn consistency_groups_cluster_by_branch_and_add_cross_pass() {
        let facts = vec![
            fe_at("facts/rome/a/1", "r1"),
            fe_at("facts/rome/a/2", "r2"),
            fe_at("facts/egypt/b/1", "e1"),
            fe_at("facts/egypt/b/2", "e2"),
        ];
        let groups = consistency_groups(&facts, CONSIST_MAX);
        // Two within-branch groups + one cross-branch pass.
        assert_eq!(groups.len(), 3);
        assert!(groups.iter().any(|g| g.label == "rome" && g.idxs.len() == 2));
        assert!(groups.iter().any(|g| g.label == "egypt"));
        assert_eq!(groups.last().unwrap().label, "cross-branch");
    }

    #[test]
    fn consistency_groups_split_large_branches() {
        // 20 facts in one branch, CONSIST_MAX 8 → 3 sub-chunks + a cross pass.
        let facts: Vec<FactEntry> =
            (0..20).map(|i| fe_at("facts/big/x/p", &format!("f{i}"))).collect();
        let groups = consistency_groups(&facts, 8);
        let subchunks = groups.iter().filter(|g| g.label.starts_with("big")).count();
        assert_eq!(subchunks, 3);
        assert!(groups.iter().any(|g| g.label == "cross-branch"));
        assert!(groups.iter().all(|g| g.idxs.len() <= 16)); // bounded
    }

    #[test]
    fn consistency_groups_root_facts_form_one_group() {
        // Two facts directly under the book share the "(root)" branch → one group.
        let facts = vec![fe_at("facts/p1", "a"), fe_at("facts/p2", "b")];
        let groups = consistency_groups(&facts, CONSIST_MAX);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "(root)");
    }

    #[test]
    fn consistency_groups_fallback_when_all_lone_branches() {
        // Each fact is the only one in its own branch → no within-branch call, but
        // the cross-branch pass still pairs them.
        let facts = vec![fe_at("facts/a/x/p", "a"), fe_at("facts/b/y/p", "b")];
        let groups = consistency_groups(&facts, CONSIST_MAX);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].label, "cross-branch");
        assert_eq!(groups[0].idxs.len(), 2);
    }
}
