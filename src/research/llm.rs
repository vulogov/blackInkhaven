//! RESRCH-1 (R-P7) — the research LLM glue. Reuses the writing TUI's streaming
//! primitive (`ai::stream::spawn_chat_stream` → `UnboundedReceiver<StreamMsg>`
//! drained with `try_recv()`), so no new async machinery. This module builds the
//! research system prompt and the coarse session-cost estimate; the streaming
//! state lives on `ResearchApp` and is drained each tick.

use super::thread::RagMode;
use crate::ai::stream::TokenUsage;
use crate::config::CostConfig;

/// The streaming category recorded in the AI usage dashboard.
pub(super) const CATEGORY: &str = "research";

/// R2-E — model-aware turn cost in USD, plus whether it is **exact** (priced from
/// the provider's real token usage) or **estimated** (the ≈4-chars/token heuristic,
/// when the provider reported none). Input and output are priced separately from
/// `cost.pricing`. `session_budget_warn` still only *informs*, never blocks.
pub(super) fn cost_for(
    cost_cfg: &CostConfig,
    model: &str,
    usage: Option<TokenUsage>,
    prompt: &str,
    response: &str,
) -> (f64, bool) {
    let price = cost_cfg.price_for(model);
    let per_million = |tokens: f64, rate: f64| (tokens / 1_000_000.0) * rate;
    match usage {
        Some(u) if u.is_reported() => {
            let c = per_million(u.prompt as f64, price.input_per_1m)
                + per_million(u.completion as f64, price.output_per_1m);
            (c, true)
        }
        _ => {
            // ≈4 chars/token, priced per side so input/output rates still apply.
            let in_tokens = prompt.len() as f64 / 4.0;
            let out_tokens = response.len() as f64 / 4.0;
            let c = per_million(in_tokens, price.input_per_1m)
                + per_million(out_tokens, price.output_per_1m);
            (c, false)
        }
    }
}

#[cfg(test)]
mod cost_tests {
    use super::*;

    #[test]
    fn exact_prices_input_and_output_separately() {
        let cfg = CostConfig::default();
        // gemini-2.5-pro: $1.25 in / $10 out per 1M.
        let usage = Some(TokenUsage { prompt: 1_000_000, completion: 1_000_000 });
        let (c, exact) = cost_for(&cfg, "gemini-2.5-pro", usage, "", "");
        assert!(exact);
        assert!((c - 11.25).abs() < 1e-9, "got {c}");
    }

    #[test]
    fn falls_back_to_heuristic_without_usage() {
        let cfg = CostConfig::default();
        let (_, exact) = cost_for(&cfg, "claude-sonnet-4-5", None, "abcd", "efgh");
        assert!(!exact);
    }

    #[test]
    fn unknown_model_uses_default_rate() {
        let cfg = CostConfig::default();
        let usage = Some(TokenUsage { prompt: 1_000_000, completion: 0 });
        let (c, _) = cost_for(&cfg, "some-unlisted-model", usage, "", "");
        assert!((c - cfg.default_input_per_1m).abs() < 1e-9, "got {c}");
    }
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
        let cfg = crate::config::CostConfig::default();
        let (small, _) = cost_for(&cfg, "gemini-2.5-pro", None, "hi", "there");
        let (big, _) = cost_for(&cfg, "gemini-2.5-pro", None, &"x".repeat(4000), &"y".repeat(4000));
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
