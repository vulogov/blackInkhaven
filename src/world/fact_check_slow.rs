//! WORLD-4 Branch B — the fact-checker's **slow track** (P5). Where the fast
//! track is deterministic pattern matching, the slow track asks the configured
//! LLM to find the subtle / implicit world-contradictions the patterns missed:
//! an assumption buried in dialogue, a consequence two clauses deep. It runs on
//! demand (not as you type), is cost-capped, and never re-emits what the fast
//! track already found (the seam).
//!
//! The LLM call lives at the CLI/TUI boundary (it needs the AI client + config);
//! the pure pieces here — the world summary, the prompt, and the response parser
//! — are testable without a provider.

use crate::world::fact_check::Finding;
use crate::world::proposals::PlaceLink;
use crate::world::types::magic::MagicLedger;
use crate::world::types::WorldDefinition;

/// The system prompt: a careful, conservative world-consistency checker.
pub const SLOW_SYSTEM: &str = "You are a meticulous continuity editor for a work of fiction. \
You are given a summary of the story's world and a single paragraph of the manuscript. \
Identify only claims in the paragraph that CONTRADICT the established world — travel that is \
too fast, weather wrong for a place's climate, impossible astronomy, populations or resources \
that don't fit. Ignore anything the listed magic rules permit. Be conservative: if a claim is \
plausible, hypothetical, or about a character's feelings, do not flag it. Respond ONLY with a \
JSON array; each item is {\"category\": one of travel_time|climate|demographics|astronomy|economy|other, \
\"severity\": warning|contradiction, \"explanation\": a one-sentence reason}. Return [] if nothing contradicts.";

/// A compact prose summary of the world for the LLM. Built from the definition +
/// the world-linked places + the astronomy/geology facts already to hand.
pub fn world_summary(
    def: &WorldDefinition,
    places: &[PlaceLink],
    moons: &[String],
    minerals: &[String],
) -> String {
    let mut s = format!("World \"{}\".\n", def.name);
    if !moons.is_empty() {
        s.push_str(&format!("- Sky: {} moon(s): {}.\n", moons.len(), moons.join(", ")));
    }
    if !minerals.is_empty() {
        s.push_str(&format!("- The land's minerals: {}.\n", minerals.join(", ")));
    }
    if !places.is_empty() {
        s.push_str("- Notable places:\n");
        for p in places.iter().take(20) {
            s.push_str(&format!(
                "    {} — ~{} people, {} climate.\n",
                p.name,
                p.population,
                p.climate_zone.replace('_', " ")
            ));
        }
    }
    s
}

/// A compact summary of the magic ledger (the declared exceptions).
pub fn magic_summary(ledger: &MagicLedger) -> String {
    if !ledger.enabled || ledger.rules.is_empty() {
        return "None.".to_string();
    }
    ledger
        .rules
        .iter()
        .map(|r| format!("- {} (covers {}): {}", r.kind, r.covers.join(", "), r.description))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Build the user prompt: world + magic + the fast-track findings to skip + the
/// paragraph itself.
pub fn build_slow_prompt(
    paragraph: &str,
    world_summary: &str,
    magic_summary: &str,
    fast_findings: &[Finding],
) -> String {
    let already = if fast_findings.is_empty() {
        "(none)".to_string()
    } else {
        fast_findings.iter().map(|f| format!("- {}", f.body)).collect::<Vec<_>>().join("\n")
    };
    format!(
        "WORLD:\n{world_summary}\n\nMAGIC RULES (claims these permit are fine):\n{magic_summary}\n\n\
         ALREADY FOUND (do NOT repeat these):\n{already}\n\n\
         PARAGRAPH:\n{paragraph}\n\n\
         Return the JSON array of contradictions."
    )
}

// ── preflight (cost estimate + soft cap) ─────────────────────────────────────

/// Rough token estimate (~4 characters per token, the usual English heuristic).
/// Deliberately conservative and dependency-free — it only needs to be close
/// enough to warn an author before a large call.
pub fn estimate_tokens(text: &str) -> usize {
    (text.chars().count() + 3) / 4
}

/// The numbers shown before a slow-track call.
#[derive(Debug, Clone, PartialEq)]
pub struct SlowPreflight {
    /// Estimated input tokens (system + user prompt).
    pub est_prompt_tokens: usize,
    /// Estimated total tokens (input + a modest response allowance).
    pub est_total_tokens: usize,
    /// Slow-track LLM calls already made today.
    pub calls_used: i64,
    /// The daily hard cap.
    pub daily_cap: i64,
}

/// What the preflight decided.
#[derive(Debug, Clone, PartialEq)]
pub enum PreflightVerdict {
    /// Clear to call the model.
    Proceed,
    /// The daily hard cap is reached — refuse.
    DailyCapReached,
    /// The estimate exceeds the per-call soft cap — refuse unless forced.
    OverSoftCap { est_total_tokens: usize, soft_cap: usize },
}

/// Decide whether to run the slow track, and surface the cost numbers. The daily
/// hard cap wins over the soft cap; a `soft_cap_tokens` of 0 disables the soft
/// gate. Pure: no clock, no I/O.
pub fn slow_preflight(
    system: &str,
    prompt: &str,
    calls_used: i64,
    daily_cap: i64,
    soft_cap_tokens: usize,
) -> (SlowPreflight, PreflightVerdict) {
    let est_prompt = estimate_tokens(system) + estimate_tokens(prompt);
    // The model returns a short JSON array; budget ~25% + a small floor for it.
    let est_total = est_prompt + est_prompt / 4 + 64;
    let pf = SlowPreflight {
        est_prompt_tokens: est_prompt,
        est_total_tokens: est_total,
        calls_used,
        daily_cap,
    };
    let verdict = if calls_used >= daily_cap {
        PreflightVerdict::DailyCapReached
    } else if soft_cap_tokens > 0 && est_total > soft_cap_tokens {
        PreflightVerdict::OverSoftCap { est_total_tokens: est_total, soft_cap: soft_cap_tokens }
    } else {
        PreflightVerdict::Proceed
    };
    (pf, verdict)
}

/// Exponential backoff between retries of a transient LLM error: 0.5s, 1s, 2s,
/// 4s, capped at 8s.
pub fn backoff_delay(attempt: u32) -> std::time::Duration {
    let ms = 500u64.saturating_mul(1u64 << attempt.min(4));
    std::time::Duration::from_millis(ms.min(8_000))
}

/// Is an LLM error worth retrying — a rate limit, timeout, or upstream 5xx? A
/// hard error (bad request, auth) is not retried.
pub fn is_transient(err: &str) -> bool {
    let e = err.to_ascii_lowercase();
    [
        "429", "rate limit", "rate-limit", "timeout", "timed out", "502", "503", "504",
        "overloaded", "temporarily", "try again", "connection reset",
    ]
    .iter()
    .any(|p| e.contains(p))
}

/// Parse the LLM's JSON response into findings. Tolerant of markdown fences and
/// surrounding prose: it extracts the first `[ … ]` array.
pub fn parse_slow_findings(raw: &str) -> Vec<Finding> {
    let Some(json) = extract_json_array(raw) else {
        return Vec::new();
    };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| {
            let category = v.get("category").and_then(|c| c.as_str()).unwrap_or("other").to_string();
            let explanation = v
                .get("explanation")
                .and_then(|e| e.as_str())
                .or_else(|| v.get("claim").and_then(|c| c.as_str()))?
                .trim()
                .to_string();
            if explanation.is_empty() {
                return None;
            }
            let severity = match v.get("severity").and_then(|s| s.as_str()) {
                Some("contradiction") => "contradiction",
                _ => "warning",
            };
            Some(Finding {
                category,
                severity: severity.to_string(),
                body: explanation.clone(),
                body_en: explanation,
                suppressed_by: None,
            })
        })
        .collect()
}

/// Pull the first JSON array out of a possibly-fenced, possibly-chatty reply.
fn extract_json_array(raw: &str) -> Option<String> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    if end <= start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_paragraph_and_world() {
        let p = build_slow_prompt("The duke rode home.", "World \"X\".", "None.", &[]);
        assert!(p.contains("The duke rode home."));
        assert!(p.contains("World \"X\"."));
        assert!(p.contains("do NOT repeat"));
    }

    #[test]
    fn parses_a_fenced_json_reply() {
        let raw = "Sure! Here are the issues:\n```json\n[\n  {\"category\": \"climate\", \"severity\": \"warning\", \"explanation\": \"Snow in a tropical city.\"},\n  {\"category\": \"travel_time\", \"severity\": \"contradiction\", \"explanation\": \"Too fast.\"}\n]\n```\nHope that helps!";
        let f = parse_slow_findings(raw);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].category, "climate");
        assert_eq!(f[0].severity, "warning");
        assert_eq!(f[1].severity, "contradiction");
    }

    #[test]
    fn parses_empty_and_garbage() {
        assert!(parse_slow_findings("[]").is_empty());
        assert!(parse_slow_findings("no json here").is_empty());
        assert!(parse_slow_findings("").is_empty());
    }

    #[test]
    fn magic_summary_none_when_disabled() {
        let l = MagicLedger::default();
        assert_eq!(magic_summary(&l), "None.");
    }

    #[test]
    fn token_estimate_is_roughly_chars_over_four() {
        assert_eq!(estimate_tokens(""), 0);
        assert_eq!(estimate_tokens("abcd"), 1);
        assert_eq!(estimate_tokens("abcde"), 2);
    }

    #[test]
    fn preflight_proceeds_under_caps() {
        let (pf, v) = slow_preflight("sys", "a short prompt", 0, 200, 10_000);
        assert_eq!(v, PreflightVerdict::Proceed);
        assert!(pf.est_total_tokens >= pf.est_prompt_tokens);
        assert_eq!(pf.daily_cap, 200);
    }

    #[test]
    fn preflight_blocks_at_daily_cap() {
        let (_, v) = slow_preflight("sys", "p", 200, 200, 10_000);
        assert_eq!(v, PreflightVerdict::DailyCapReached);
    }

    #[test]
    fn preflight_flags_over_soft_cap() {
        let big = "word ".repeat(2_000); // ~10k chars → ~2.5k tokens
        let (_, v) = slow_preflight("sys", &big, 0, 200, 100);
        match v {
            PreflightVerdict::OverSoftCap { est_total_tokens, soft_cap } => {
                assert!(est_total_tokens > soft_cap);
                assert_eq!(soft_cap, 100);
            }
            other => panic!("expected OverSoftCap, got {other:?}"),
        }
    }

    #[test]
    fn soft_cap_zero_disables_gate() {
        let big = "word ".repeat(2_000);
        let (_, v) = slow_preflight("sys", &big, 0, 200, 0);
        assert_eq!(v, PreflightVerdict::Proceed);
    }

    #[test]
    fn backoff_grows_then_caps() {
        assert_eq!(backoff_delay(0).as_millis(), 500);
        assert_eq!(backoff_delay(1).as_millis(), 1_000);
        assert_eq!(backoff_delay(2).as_millis(), 2_000);
        assert_eq!(backoff_delay(20).as_millis(), 8_000); // capped
    }

    #[test]
    fn transient_errors_detected() {
        assert!(is_transient("HTTP 429 Too Many Requests"));
        assert!(is_transient("upstream timeout"));
        assert!(is_transient("service Overloaded, try again"));
        assert!(!is_transient("401 invalid api key"));
        assert!(!is_transient("malformed request"));
    }
}
