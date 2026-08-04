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

fn build_prompt(category: &str, message: &str, language: &str) -> String {
    format!(
        "Manuscript language: {language}.\n\nThe issue (kind: {category}):\n{message}\n\n\
         Write the revision brief."
    )
}

/// Generate a revision brief for a finding. Self-contained (loads its own config +
/// provider), so it is safe to call from a background worker. Retries transient
/// errors, like the other slow tracks. Returns the brief text.
pub(crate) fn brief(project: &Path, category: &str, message: &str) -> Result<String, String> {
    let layout = ProjectLayout::new(project);
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| format!("no LLM provider for the revision brief: {e}"))?;
    let (model, _env) =
        ai.resolve_provider(&cfg.llm, None).map_err(|e| format!("resolving provider: {e}"))?;
    let language = if cfg.language.trim().is_empty() { "English" } else { &cfg.language };
    let prompt = build_prompt(category, message, language);

    let mut last_err = String::new();
    for attempt in 0..3u32 {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(BRIEF_SYSTEM.to_string()),
            prompt.clone(),
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
    Err(format!("revision brief LLM error: {last_err}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_carries_the_issue_and_forbids_rewriting() {
        let p = build_prompt("shape_sag", "Act 2 sags in ch. 5–7.", "English");
        assert!(p.contains("shape_sag"));
        assert!(p.contains("Act 2 sags"));
        assert!(BRIEF_SYSTEM.contains("do NOT rewrite"));
    }
}
