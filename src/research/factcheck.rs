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

/// Gather every paragraph under the Facts book as a `FactEntry` (non-empty).
pub(super) fn gather_facts(store: &Store, h: &Hierarchy, book_id: Uuid) -> Vec<FactEntry> {
    let mut out = Vec::new();
    for id in h.collect_subtree(book_id) {
        let Some(node) = h.get(id) else { continue };
        if node.kind != NodeKind::Paragraph {
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

/// The user message for the consistency check: every fact, numbered.
pub(super) fn consistency_user(facts: &[FactEntry]) -> String {
    let mut s = String::from("Facts:\n");
    for (i, f) in facts.iter().enumerate() {
        s.push_str(&format!("{}. {}\n", i + 1, f.text));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fe(text: &str) -> FactEntry {
        FactEntry { id: Uuid::nil(), location: "facts/x".into(), text: text.into() }
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
        let facts = vec![fe("A"), fe("B")];
        let u = consistency_user(&facts);
        assert!(u.contains("1. A"));
        assert!(u.contains("2. B"));
    }
}
