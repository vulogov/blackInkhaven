//! CHAR-1 — the shared blocking LLM call for state extraction and arc checks.
//! Mirrors `world::utopia::llm` / `inner_editor::engage`: resolve the provider,
//! call `ai::stream::collect_blocking`, retry on transient errors. Caps inform,
//! never block.

use anyhow::{Result, anyhow};

use crate::config::Config;

pub(super) fn char_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("no LLM provider for character arc: {e}"))?;
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
    Err(anyhow!("character LLM error: {last_err}"))
}

/// Extract the first top-level JSON object from an LLM response (models often
/// wrap JSON in prose or fences).
pub(super) fn extract_json_object(raw: &str) -> &str {
    match (raw.find('{'), raw.rfind('}')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => raw.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_object_from_fenced_prose() {
        assert_eq!(extract_json_object("ok: ```json\n{\"a\":1}\n```"), "{\"a\":1}");
        assert_eq!(extract_json_object("{}"), "{}");
    }
}
