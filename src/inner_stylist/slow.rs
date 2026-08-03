//! Inner Stylist slow track (CH-P7) — LLM coaching over the measured findings.
//! Non-prescriptive and grounded: the model is given the deterministic
//! measurements and turns them into a few observations in the Inner-family voice
//! ("I notice…"), never a rewrite, never an invented finding.

use crate::config::Config;
use crate::prose::ProseLanguage;

use super::Finding;

pub(crate) const STYLIST_SYSTEM: &str = "You are the Inner Stylist, a perceptive reader of a \
book's VOICE at scale. You are given DETERMINISTIC measurements of the manuscript — the \
distinctiveness of its characters' voices, per-character voice drift, point-of-view \
discipline, tense consistency, and register. Turn them into a few concise, grounded \
observations in a non-prescriptive voice — \"I notice…\", \"you might consider…\" — never \
\"should\" or \"must\". Ground EVERY observation in the measurements you were given; never \
invent a finding. You NEVER rewrite the prose or propose replacement text. Praise must be \
earned; if the voice reads clean, say so briefly.";

/// Compose the coaching prompt from the synthesised findings.
pub(crate) fn build_coach_prompt(findings: &[Finding], lang: &ProseLanguage) -> String {
    let language = language_name(lang);
    let body = if findings.is_empty() {
        "(no issues detected — the measurements read clean)".to_string()
    } else {
        findings
            .iter()
            .map(|f| format!("- [{}] {}", f.severity.label(), f.message))
            .collect::<Vec<_>>()
            .join("\n")
    };
    format!(
        "Here are the measured voice-and-style findings for the whole manuscript:\n\n{body}\n\n\
         Offer the author a few grounded observations (in {language}) about what these say \
         about the book's voice at scale — what is working, and what to watch. Observe, do \
         not prescribe; do not rewrite; do not add findings that aren't in the list."
    )
}

/// Run the Inner Stylist's LLM call (blocking, with transient-error retry) —
/// mirrors the other Inner-family slow tracks.
pub(crate) fn stylist_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String, String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| format!("no LLM provider for the Inner Stylist: {e}"))?;
    let (model, _env) =
        ai.resolve_provider(&cfg.llm, None).map_err(|e| format!("resolving provider: {e}"))?;
    let mut last_err = String::new();
    for attempt in 0..3u32 {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            user.to_string(),
        ) {
            Ok(r) => return Ok(r),
            Err(e) => {
                last_err = e;
                if attempt + 1 < 3 && crate::world::fact_check_slow::is_transient(&last_err) {
                    std::thread::sleep(crate::world::fact_check_slow::backoff_delay(attempt));
                    continue;
                }
                break;
            }
        }
    }
    Err(format!("Inner Stylist LLM error: {last_err}"))
}

fn language_name(lang: &ProseLanguage) -> &'static str {
    match lang {
        ProseLanguage::En => "English",
        ProseLanguage::Ru => "Russian",
        ProseLanguage::De => "German",
        ProseLanguage::Fr => "French",
        ProseLanguage::Es => "Spanish",
        ProseLanguage::Other(_) => "the language of the manuscript",
    }
}
