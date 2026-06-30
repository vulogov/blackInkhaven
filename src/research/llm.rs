//! RESRCH-1 (R-P7) — the research LLM glue. Reuses the writing TUI's streaming
//! primitive (`ai::stream::spawn_chat_stream` → `UnboundedReceiver<StreamMsg>`
//! drained with `try_recv()`), so no new async machinery. This module builds the
//! research system prompt and the coarse session-cost estimate; the streaming
//! state lives on `ResearchApp` and is drained each tick.

use super::thread::RagMode;

/// The streaming category recorded in the AI usage dashboard.
pub(super) const CATEGORY: &str = "research";

/// Coarse session-cost estimate. There is no per-model price table in the tree
/// yet, so this is an explicitly approximate mid-tier rate (the status bar marks
/// it `~`, and `session_budget_warn` only *informs*, never blocks — RFC §23).
const EST_USD_PER_1K_TOKENS: f64 = 0.003;

/// Rough token estimate (≈ 4 chars/token) → USD. Counts both sides of the turn.
pub(super) fn estimate_cost(prompt: &str, response: &str) -> f64 {
    let chars = (prompt.len() + response.len()) as f64;
    let tokens = chars / 4.0;
    (tokens / 1000.0) * EST_USD_PER_1K_TOKENS
}

/// The research-mode system prompt (RFC §8). `rag_context` is the assembled
/// Facts context (R-P8); `None`/empty when there is nothing to ground on.
pub(super) fn system_prompt(rag_mode: RagMode, rag_context: Option<&str>) -> String {
    let mut s = String::from(
        "You are a research assistant helping a writer populate their knowledge base. \
         Answer accurately. When uncertain about specific facts, say so explicitly. \
         Do NOT fabricate citations, authors, dates, or statistics.",
    );
    match rag_context {
        Some(ctx) if !ctx.trim().is_empty() => {
            s.push_str("\n\nThe author has these existing facts in their knowledge base:\n");
            s.push_str(ctx.trim());
        }
        _ => {}
    }
    if rag_mode == RagMode::FactsOnly {
        s.push_str(
            "\n\nAnswer using only the provided context. If the answer is not in the provided \
             context, say so.",
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cost_scales_with_length() {
        let small = estimate_cost("hi", "there");
        let big = estimate_cost(&"x".repeat(4000), &"y".repeat(4000));
        assert!(big > small);
        assert!(small >= 0.0);
    }

    #[test]
    fn facts_only_adds_closed_instruction() {
        let p = system_prompt(RagMode::FactsOnly, Some("a fact"));
        assert!(p.contains("only the provided context"));
        let p2 = system_prompt(RagMode::FactsPlusFull, Some("a fact"));
        assert!(!p2.contains("only the provided context"));
        assert!(p2.contains("a fact"));
    }

    #[test]
    fn no_context_section_when_empty() {
        let p = system_prompt(RagMode::FactsPlusFull, None);
        assert!(!p.contains("existing facts"));
    }
}
