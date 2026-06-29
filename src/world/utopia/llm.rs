//! WORLD-6 — the shared blocking LLM call for the three coherence stages.
//! Mirrors `inner_editor::engage` / `socratic_llm_call`: resolve the provider
//! from config, call `ai::stream::collect_blocking`, retry on transient errors
//! with backoff. Caps inform, never block — the caller emits the cost note.

use anyhow::{Result, anyhow};

use crate::config::Config;

/// Run one blocking LLM completion with a system + user prompt. Errors if no
/// provider is configured or all retries fail.
pub(super) fn utopia_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("no LLM provider for utopia coherence: {e}"))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| anyhow!("resolving provider: {e}"))?;

    let max_attempts = 3u32;
    let mut last_err = String::new();
    for attempt in 0..max_attempts {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            user.to_string(),
        ) {
            Ok(r) => return Ok(r),
            Err(e) => {
                last_err = e.to_string();
                if attempt + 1 < max_attempts
                    && crate::world::fact_check_slow::is_transient(&last_err)
                {
                    std::thread::sleep(crate::world::fact_check_slow::backoff_delay(attempt));
                    continue;
                }
                break;
            }
        }
    }
    Err(anyhow!("utopia LLM error: {last_err}"))
}

/// Extract the first top-level JSON array from an LLM response (models often
/// wrap JSON in prose or code fences). Returns the `[...]` slice, or the whole
/// trimmed string if no array bracket pair is found.
pub(super) fn extract_json_array(raw: &str) -> &str {
    match (raw.find('['), raw.rfind(']')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => raw.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_array_from_fenced_prose() {
        let raw = "Here you go:\n```json\n[{\"a\":1}]\n```\nDone.";
        assert_eq!(extract_json_array(raw), "[{\"a\":1}]");
    }
    #[test]
    fn empty_array_passthrough() {
        assert_eq!(extract_json_array("[]"), "[]");
    }
}
