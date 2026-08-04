//! REDLINE-1 (RD-P4) — the revision brief.
//!
//! For a structural or book-level finding a single-paragraph rewrite can't honestly
//! solve — a saggy act, a likely put-down point, two voices that read alike — the
//! AI can still *advise*. `brief` asks a developmental editor to write a short,
//! concrete, actionable revision plan. It is **explicit** (the author asks for it),
//! runs in the manuscript's language, and **never touches prose** — the brief lands
//! in the Thoughts pane; the writing stays the author's.

use std::path::Path;

use crate::config::Config;
use crate::project::ProjectLayout;

const BRIEF_SYSTEM: &str = "You are a seasoned developmental editor. You are given ONE structural or \
book-level issue with a manuscript. Write a SHORT, concrete, actionable revision brief: name the \
problem in one line, then give two or three SPECIFIC suggestions for how the author might address \
it — grounded in craft, tied to what the issue says, not generic advice. You ADVISE; you do NOT \
rewrite the prose. Keep it under a page, in the manuscript's language.";

const LETTER_SYSTEM: &str = "You are a developmental editor writing the opening of an editorial \
letter to an author. You are given a list of diagnostic findings about their manuscript. Do NOT \
list them back mechanically. SYNTHESISE: open with the BIG PICTURE — the one or two things a reader \
will feel first — then group the rest by theme (continuity, structure & pacing, voice & character, \
line & prose), most important first, saying briefly WHY each matters and roughly what to do about \
it. Warm, specific, honest — the way a good editor opens. A page or two, in the manuscript's \
language. You advise; you do not rewrite.";

fn build_brief_prompt(category: &str, message: &str, language: &str) -> String {
    format!(
        "Manuscript language: {language}.\n\nThe issue (kind: {category}):\n{message}\n\n\
         Write the revision brief."
    )
}

fn build_letter_prompt(findings_block: &str, language: &str) -> String {
    format!(
        "Manuscript language: {language}.\n\nThe diagnostic findings (severity · category · \
         location · how it can be acted on):\n{findings_block}\n\nWrite the editorial letter."
    )
}

/// The manuscript language name for the prompt (empty config → English).
fn language_of(cfg: &Config) -> String {
    if cfg.language.trim().is_empty() { "English".to_string() } else { cfg.language.clone() }
}

/// One cost-informed raw-text LLM call with transient-error retry (the other slow
/// tracks' pattern). Self-contained given a loaded config.
fn call(cfg: &Config, system: &str, prompt: &str) -> Result<String, String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| format!("no LLM provider: {e}"))?;
    let (model, _env) =
        ai.resolve_provider(&cfg.llm, None).map_err(|e| format!("resolving provider: {e}"))?;
    let mut last_err = String::new();
    for attempt in 0..3u32 {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            prompt.to_string(),
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
    Err(format!("revision LLM error: {last_err}"))
}

/// Generate a revision brief for one finding (RD-P4). Self-contained; bg-safe.
pub(crate) fn brief(project: &Path, category: &str, message: &str) -> Result<String, String> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    call(&cfg, BRIEF_SYSTEM, &build_brief_prompt(category, message, &language_of(&cfg)))
}

/// Synthesise the editorial letter over a pre-formatted findings block (RD-P5).
/// Self-contained; safe to call from a background worker.
pub(crate) fn letter(project: &Path, findings_block: &str) -> Result<String, String> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    call(&cfg, LETTER_SYSTEM, &build_letter_prompt(findings_block, &language_of(&cfg)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn brief_prompt_carries_the_issue_and_forbids_rewriting() {
        let p = build_brief_prompt("shape_sag", "Act 2 sags in ch. 5–7.", "English");
        assert!(p.contains("shape_sag") && p.contains("Act 2 sags"));
        assert!(BRIEF_SYSTEM.contains("do NOT rewrite"));
    }

    #[test]
    fn letter_prompt_carries_the_findings_and_synthesises() {
        let p = build_letter_prompt("- [high] co_location · ch. 3 (decision) — Mara in two places", "English");
        assert!(p.contains("Mara in two places"));
        assert!(LETTER_SYSTEM.contains("SYNTHESISE"));
    }
}
