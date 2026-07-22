//! Inner Poet slow track (PO-P6) — LLM observations on a stanza, in the Thoughts
//! pane. Non-prescriptive: it *observes* what the verse is doing — enjambment,
//! sound texture, caesura, and (for a sonnet) the turn — and never rewrites a
//! line or proposes a replacement word. The register is the Inner Editor's: "I
//! notice…", never "should".

use crate::config::Config;
use crate::poetry::form::PoemForm;
use crate::prose::ProseLanguage;

/// The Inner Poet's system prompt.
pub const POET_SYSTEM: &str = "You are the Inner Poet, a perceptive reader of verse. \
You observe what a poem is doing prosodically and share a few concise observations in a \
non-prescriptive voice — \"I notice…\", \"you might consider…\" — never \"should\" or \
\"must\". You NEVER rewrite the poem, propose replacement words, or generate verse. Praise \
must be earned; silence on a clean stanza is fine.";

/// Build the observation prompt for a stanza against its declared form. The
/// volta question is included only for sonnet forms.
pub fn build_observation_prompt(stanza: &str, form: &PoemForm, lang: &ProseLanguage) -> String {
    let language = language_name(lang);
    let mut ask = String::from(
        "its enjambment (do the line breaks fall at phrase boundaries, or cut across the \
         syntax — and to what effect?), its sound texture (alliteration, assonance, \
         consonance), and its caesura (where the natural breath-pause falls in each line)",
    );
    if form.form.contains("sonnet") {
        ask.push_str(
            ", and — this is a sonnet — whether a real turn (volta) is present at the \
             expected position (after line 8 for a Petrarchan sonnet, after line 12 for a \
             Shakespearean one): note whether the turn is substantive, partial, or absent, \
             without prescribing what it should be",
        );
    }
    let declared = if form.form.is_empty() {
        String::new()
    } else {
        format!("\n\nThe declared form is {} ({} {}).", form.form, form.metre, form.feet)
    };
    format!(
        "Read this stanza (in {language}) and share a few observations on: {ask}. Observe, \
         do not prescribe; do not rewrite. Stanza:\n\n{stanza}{declared}"
    )
}

/// Run the Inner Poet's LLM call (blocking, with transient-error retry) — mirrors
/// the other Inner-family slow tracks.
pub fn poet_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String, String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| format!("no LLM provider for the Inner Poet: {e}"))?;
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
    Err(format!("Inner Poet LLM error: {last_err}"))
}

fn language_name(lang: &ProseLanguage) -> &'static str {
    use ProseLanguage::*;
    match lang {
        En => "English",
        Ru => "Russian",
        De => "German",
        Fr => "French",
        Es => "Spanish",
        Other(_) => "this language",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poetry::form::PoemForm;

    #[test]
    fn prompt_mentions_volta_only_for_sonnets() {
        let sonnet = PoemForm { form: "shakespearean_sonnet".into(), ..Default::default() };
        let p = build_observation_prompt("a line\nanother", &sonnet, &ProseLanguage::En);
        assert!(p.contains("volta"));
        let haiku = PoemForm { form: "haiku".into(), ..Default::default() };
        let q = build_observation_prompt("a line\nanother", &haiku, &ProseLanguage::En);
        assert!(!q.contains("volta"));
    }

    #[test]
    fn prompt_carries_the_stanza_and_language() {
        let form = PoemForm { form: "blank_verse".into(), ..Default::default() };
        let p = build_observation_prompt("тёмный лес", &form, &ProseLanguage::Ru);
        assert!(p.contains("тёмный лес"));
        assert!(p.contains("Russian"));
    }
}
