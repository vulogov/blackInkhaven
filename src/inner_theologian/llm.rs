//! INNER-THEOLOGIAN-1 (IT-P5) — the blocking LLM call for slow-track sessions.
//! Mirrors `character::llm` / `world::utopia::llm`: resolve the provider, call
//! `ai::stream::collect_blocking`, retry on transient errors. Caps inform, never
//! block. The slow track is conversational — the model returns the persona's
//! questions as prose in the project language (no JSON parsing), to be seeded
//! into the AI pane (IT-P7).

use anyhow::{Result, anyhow};

use crate::config::Config;

pub(crate) fn theologian_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("no LLM provider for Inner Theologian: {e}"))?;
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
    Err(anyhow!("Inner Theologian LLM error: {last_err}"))
}
