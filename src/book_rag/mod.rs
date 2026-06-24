//! BOOK_RAG-1 — Chat with Your Book.
//!
//! The AI pane's **Book scope** is retrieval-augmented: a prompt in Book
//! scope retrieves the semantically relevant paragraphs (via the existing
//! `Store::search_text` over the vecstore HNSW index), expands them with a
//! little surrounding context, composes a focused, token-budgeted context,
//! and grounds the LLM's answer in those passages with markdown citations
//! — instead of sending the entire book. It is the generalisation of the
//! shipped Facts semantic-search-grounds-a-chat pattern (`search_facts`)
//! from the Facts book to the manuscript.
//!
//! This module holds the pure pieces (passage type, token estimate,
//! context composition, system prompt). The retrieval itself lives on the
//! `App` (`book_rag_impl.rs`) because it needs the live store + hierarchy.

/// One retrieved paragraph, ready to compose into the LLM context.
#[derive(Debug, Clone)]
pub struct RetrievedPassage {
    /// Paragraph node id — also its citation anchor (`[id](#id)`).
    pub id: uuid::Uuid,
    /// `chapter-slug/paragraph-slug`-style breadcrumb for the heading.
    pub breadcrumb: String,
    /// The paragraph's prose (`.typ` body).
    pub body: String,
    /// Vecstore similarity score (0..1). Expansion paragraphs carry the
    /// score of the hit that pulled them in. Surfaced by the P3 transparency
    /// section.
    #[allow(dead_code)] // read by P3 "Retrieved passages" UI
    pub score: f64,
    /// True for a direct semantic hit; false for a context-expansion
    /// neighbour pulled in around a hit.
    pub is_hit: bool,
}

/// Rough token estimate (≈ chars / 4) — there is no tokenizer in-tree, and
/// the budget only needs to be approximately right.
pub fn estimate_tokens(s: &str) -> usize {
    s.chars().count() / 4
}

/// Compose the retrieved passages into the context block prepended to the
/// user's prompt. Each passage is labelled with its breadcrumb and citation
/// id so the LLM can cite it as `[id](#id)`.
pub fn compose_context_prefix(passages: &[RetrievedPassage]) -> String {
    if passages.is_empty() {
        return "── Retrieved passages ──\n(No passages in this book matched the \
                query semantically.)\n── end retrieved passages ──"
            .to_string();
    }
    let mut out = String::from("── Retrieved passages (grounding evidence) ──\n");
    for p in passages {
        let marker = if p.is_hit { " ★" } else { "" };
        out.push_str(&format!(
            "\n[{id}] {breadcrumb}{marker}\n{body}\n",
            id = p.id,
            breadcrumb = p.breadcrumb,
            marker = marker,
            body = p.body.trim(),
        ));
    }
    out.push_str("\n── end retrieved passages ──");
    out
}

/// The set of paragraph ids cited by the retrieval — used by the citation
/// validator to flag any cited id the LLM invented.
pub fn cited_ids(passages: &[RetrievedPassage]) -> std::collections::HashSet<String> {
    passages.iter().map(|p| p.id.to_string()).collect()
}

/// Flag hallucinated citations inline. Scans the LLM response for markdown
/// fragment links — `](#id)` — and, for any `id` NOT in `valid_ids` (the
/// retrieval set), appends a visible `[citation could not be validated: id]`
/// after the link so the author sees what's grounded vs. invented. A
/// structural commitment to grounding integrity (RFC §8.3).
pub fn validate_citations(
    response: &str,
    valid_ids: &std::collections::HashSet<String>,
) -> String {
    const OPEN: &str = "](#";
    let mut out = String::with_capacity(response.len() + 32);
    let mut rest = response;
    while let Some(pos) = rest.find(OPEN) {
        let frag_start = pos + OPEN.len();
        let after = &rest[frag_start..];
        let Some(end) = after.find(')') else {
            out.push_str(rest); // unterminated link — leave verbatim
            return out;
        };
        let id = &after[..end];
        // Copy through the closing ')'.
        out.push_str(&rest[..frag_start + end + 1]);
        if !id.is_empty() && !valid_ids.contains(id) {
            out.push_str(&format!(" [citation could not be validated: {id}]"));
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    out
}

/// The Book-RAG system prompt: ground answers in the retrieved passages,
/// cite with markdown links, and be honest when the passages don't address
/// the question. Per-language variants land in P6; English is the baseline.
pub fn system_prompt(lang: &str) -> &'static str {
    match lang {
        // P6 fills RU/ES/FR/DE; until then they share the English contract.
        _ => EN_SYSTEM_PROMPT,
    }
}

const EN_SYSTEM_PROMPT: &str = "\
You are helping the author of this book think about their own work. You have \
been given relevant passages from the book, retrieved by semantic similarity \
to the author's question and marked with a citation id like [ch07-p042]. The \
passages are the book's prose in Typst markup — `= heading`, `*strong*`, \
`_emphasis_`, `#footnote[…]` — read through the markup to the prose beneath it.

Answer the author's question using the retrieved passages as primary \
evidence. Every claim about the book MUST cite at least one retrieved \
passage as a markdown link: [ch07-p042](#ch07-p042). Cite multiple passages \
when a claim spans them. Never state something about the book without citing.

When the retrieved passages don't address the question, say so plainly — \
\"The retrieved passages don't address that directly\" — then either ask the \
author to refine the question or offer general knowledge clearly marked as \
not from the book (\"Setting the book aside, in general…\").

Tone: helpful, grounded, specific. The author is consulting their own work, \
not asking you to invent it. Answer in the language of the author's question.";

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn passage(body: &str, is_hit: bool) -> RetrievedPassage {
        RetrievedPassage {
            id: Uuid::new_v4(),
            breadcrumb: "ch1/opening".into(),
            body: body.into(),
            score: 0.8,
            is_hit,
        }
    }

    #[test]
    fn estimate_tokens_is_chars_over_four() {
        assert_eq!(estimate_tokens(&"a".repeat(40)), 10);
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn compose_labels_passages_with_id_and_hit_marker() {
        let ps = vec![passage("the road was long", true), passage("it rained", false)];
        let out = compose_context_prefix(&ps);
        assert!(out.contains("Retrieved passages"));
        assert!(out.contains(&format!("[{}]", ps[0].id)));
        assert!(out.contains("★"), "hit should be starred");
        assert!(out.contains("the road was long"));
        assert!(out.contains("it rained"));
    }

    #[test]
    fn empty_retrieval_composes_a_no_match_notice() {
        let out = compose_context_prefix(&[]);
        assert!(out.to_lowercase().contains("no passages"));
    }

    #[test]
    fn validate_flags_only_uncited_ids() {
        let mut valid = std::collections::HashSet::new();
        valid.insert("ch07-p042".to_string());
        let resp = "She returned [here](#ch07-p042) and again [later](#ch15-p103).";
        let out = validate_citations(resp, &valid);
        // The valid citation is untouched…
        assert!(out.contains("[here](#ch07-p042)"));
        assert!(!out.contains("ch07-p042]"), "valid id must not be flagged");
        // …the invented one is flagged inline.
        assert!(out.contains("[later](#ch15-p103) [citation could not be validated: ch15-p103]"));
    }

    #[test]
    fn validate_no_citations_is_unchanged() {
        let valid = std::collections::HashSet::new();
        assert_eq!(validate_citations("plain text, no links", &valid), "plain text, no links");
    }

    #[test]
    fn validate_unterminated_link_does_not_panic() {
        let valid = std::collections::HashSet::new();
        let out = validate_citations("oops [x](#unterminated", &valid);
        assert!(out.contains("#unterminated"));
    }

    #[test]
    fn cited_ids_collects_every_passage_id() {
        let ps = vec![passage("a", true), passage("b", false)];
        let ids = cited_ids(&ps);
        assert!(ids.contains(&ps[0].id.to_string()));
        assert!(ids.contains(&ps[1].id.to_string()));
    }
}
