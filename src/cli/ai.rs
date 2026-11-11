use std::io::Write;
use std::path::Path;

use crate::ai::AiClient;
use crate::ai::stream::{StreamMsg, spawn_chat_stream};
use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;

/// `inkhaven ai --resolve [--provider X]` (3.22) — the dry run: every
/// configured provider with its model, target and key state, then the
/// provider a send would use. Nothing is sent.
pub fn resolve(project: &Path, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let ai = AiClient::from_config(&cfg.llm)?;
    println!("{}", resolve_report(&ai, &cfg.llm, provider));
    Ok(())
}

/// The text `--resolve` prints (pure, so the suite can pin it).
pub(crate) fn resolve_report(ai: &AiClient, llm: &crate::config::LlmConfig, provider: Option<&str>) -> String {
    let mut out = String::new();
    let width = llm.providers.keys().map(|k| k.len()).max().unwrap_or(8).max(8);
    out.push_str(&format!(
        "{:<width$}  {:<28}  {}
",
        "provider", "model", "target · key",
        width = width
    ));
    for (name, prov) in &llm.providers {
        let marker = if *name == llm.default { "*" } else { " " };
        out.push_str(&format!(
            "{marker}{:<width$} {:<28}  {} · {}
",
            name,
            prov.model,
            AiClient::describe_target(prov),
            AiClient::describe_key(prov),
            width = width
        ));
    }
    out.push_str(&format!(
        "
* = llm.default (`{}`) · auto_fallback {}
",
        llm.default,
        if llm.auto_fallback { "on" } else { "off" }
    ));
    let asked = provider.unwrap_or(&llm.default);
    match ai.resolve_provider_name(llm, provider) {
        Ok((chosen, model)) if chosen == asked => {
            out.push_str(&format!("→ `{asked}` would be used: model `{model}`
"));
        }
        Ok((chosen, model)) => {
            out.push_str(&format!(
                "→ `{asked}` is not usable (its key is unset); auto_fallback would use `{chosen}`: model `{model}`
"
            ));
        }
        Err(e) => out.push_str(&format!("→ nothing usable for `{asked}`: {e}
")),
    }
    out
}

pub fn run(project: &Path, prompt: &str, provider: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let cfg = Config::load_layered(&layout.config_path())?;
    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env_var) = ai.resolve_provider(&cfg.llm, provider)?;

    let mut rx = spawn_chat_stream(
        ai.client.clone(),
        model.to_string(),
        None,
        Vec::new(),
        prompt.to_string(),
        "ai-cli",
    );

    let mut stdout = std::io::stdout().lock();
    let mut wrote_anything = false;
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            StreamMsg::Token(t) => {
                let _ = stdout.write_all(t.as_bytes());
                let _ = stdout.flush();
                wrote_anything = true;
            }
            StreamMsg::Done(_) => break,
            StreamMsg::Error(e) => {
                let _ = stdout.write_all(b"\n");
                eprintln!("inference error: {e}");
                return Ok(());
            }
        }
    }
    if wrote_anything {
        let _ = stdout.write_all(b"\n");
    } else {
        eprintln!("(no tokens received)");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{LlmConfig, LlmProvider};

    fn cfg() -> LlmConfig {
        let mut c = LlmConfig::default();
        c.providers.clear();
        c.providers.insert(
            "cloud".into(),
            LlmProvider { model: "gpt-9-mini".into(), api_key_env: Some("INKHAVEN_TEST_NEVER_SET".into()), base_url: None },
        );
        c.providers.insert(
            "local".into(),
            LlmProvider { model: "llama-local".into(), api_key_env: None, base_url: Some("http://127.0.0.1:1234/v1/".into()) },
        );
        c.default = "cloud".into();
        c.auto_fallback = true;
        c
    }

    #[test]
    fn resolve_report_names_targets_keys_and_the_fallback() {
        let c = cfg();
        let ai = AiClient::from_config(&c).unwrap();
        let r = resolve_report(&ai, &c, None);
        assert!(r.contains("*cloud"), "{r}");
        assert!(r.contains("genai default"), "{r}");
        assert!(r.contains("INKHAVEN_TEST_NEVER_SET unset"), "{r}");
        assert!(r.contains("http://127.0.0.1:1234/v1 (OpenAI-compatible) · no key needed"), "{r}");
        assert!(r.contains("auto_fallback would use `local`: model `llama-local`"), "{r}");
        // Asking for the usable one directly.
        let r = resolve_report(&ai, &c, Some("local"));
        assert!(r.contains("→ `local` would be used: model `llama-local`"), "{r}");
    }

    #[test]
    fn resolve_report_strict_mode_reports_nothing_usable() {
        let mut c = cfg();
        c.auto_fallback = false;
        let ai = AiClient::from_config(&c).unwrap();
        let r = resolve_report(&ai, &c, None);
        assert!(r.contains("auto_fallback off"), "{r}");
        assert!(r.contains("→ nothing usable for `cloud`"), "{r}");
    }
}
