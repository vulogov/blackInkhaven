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
}
