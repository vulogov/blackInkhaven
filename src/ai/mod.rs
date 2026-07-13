pub mod prompts;
pub mod stream;
pub mod usage;

use std::sync::Arc;

use genai::Client;

use crate::config::LlmConfig;
use crate::error::{Error, Result};

/// Resolved AI runtime: a shared genai client + the user's default provider.
/// genai picks the adapter (Gemini, DeepSeek, OpenAI, …) from the model string
/// passed to `exec_chat_stream`, and reads API keys from the env vars named in
/// `~/.config/inkhaven/inkhaven.hjson` (e.g. `GEMINI_API_KEY`, `DEEPSEEK_API_KEY`).
#[derive(Clone)]
pub struct AiClient {
    pub client: Arc<Client>,
    pub default_provider: String,
}

impl AiClient {
    pub fn from_config(cfg: &LlmConfig) -> Result<Self> {
        if !cfg.providers.contains_key(&cfg.default) {
            return Err(Error::Config(format!(
                "default provider `{}` is not in providers map",
                cfg.default
            )));
        }
        Ok(Self {
            client: Arc::new(Client::default()),
            default_provider: cfg.default.clone(),
        })
    }

    /// Resolve a provider name to `(model, env_var_or_none)`. Prefers the
    /// requested (or default) provider, but **falls back to any configured
    /// provider whose key is actually available** when the requested one's key is
    /// unset — so a project configured for one provider still runs against
    /// whichever key the user has set (or a local, keyless provider like Ollama).
    /// Errors only when nothing is usable. Providers omitting `api_key_env` are
    /// always available (no auth).
    pub fn resolve_provider<'a>(
        &self,
        cfg: &'a LlmConfig,
        provider: Option<&str>,
    ) -> Result<(&'a str, Option<&'a str>)> {
        let name = provider.unwrap_or(&self.default_provider);
        let requested = cfg.providers.get(name).ok_or_else(|| {
            Error::Config(format!("unknown llm provider `{name}` — check inkhaven.hjson"))
        })?;
        // The requested/default provider is usable → prefer it.
        if provider_available(requested) {
            return Ok((&requested.model, requested.api_key_env.as_deref()));
        }
        let missing = requested.api_key_env.as_deref().unwrap_or("a provider key");
        // Auto-fallback (opt-out via `llm.auto_fallback = false`): use any other
        // configured provider whose key is available. Deterministic order (the
        // providers map is sorted by name).
        if cfg.auto_fallback {
            if let Some((fallback, prov)) = cfg
                .providers
                .iter()
                .find(|(n, p)| n.as_str() != name && provider_available(p))
            {
                tracing::info!(
                    "llm: `{name}`'s key is unset; falling back to available provider `{fallback}`"
                );
                return Ok((&prov.model, prov.api_key_env.as_deref()));
            }
            // Auto-fallback on, but nothing is usable.
            return Err(Error::Config(format!(
                "{missing} not set, and no other configured provider has an available key — \
                 `export {missing}=...` (or set a key for one of the other providers in inkhaven.hjson)"
            )));
        }
        // Auto-fallback disabled — respect the configured provider strictly.
        Err(Error::Config(format!(
            "{missing} not set — `export {missing}=...` (or enable `llm.auto_fallback` to use another provider)"
        )))
    }
}

/// Whether a provider can be used right now: it needs no key (local, e.g. Ollama),
/// or its `api_key_env` variable is set.
fn provider_available(prov: &crate::config::LlmProvider) -> bool {
    match prov.api_key_env.as_deref() {
        None => true,
        Some(env) => std::env::var(env).is_ok(),
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;
    use crate::config::{LlmConfig, LlmProvider};
    use std::collections::BTreeMap;

    // A config whose default provider ("cloud") needs a key that is never set,
    // plus a keyless "local" provider that is always available.
    fn cfg(auto_fallback: bool) -> LlmConfig {
        let mut providers = BTreeMap::new();
        providers.insert(
            "cloud".into(),
            LlmProvider {
                model: "cloud-model".into(),
                api_key_env: Some("INKHAVEN_TEST_UNSET_KEY_ZZZ".into()),
            },
        );
        providers.insert(
            "local".into(),
            LlmProvider { model: "local-model".into(), api_key_env: None },
        );
        LlmConfig { default: "cloud".into(), providers, auto_fallback }
    }

    #[test]
    fn falls_back_to_available_provider_when_enabled() {
        let cfg = cfg(true);
        let ai = AiClient::from_config(&cfg).unwrap();
        let (model, env) = ai.resolve_provider(&cfg, None).unwrap();
        assert_eq!(model, "local-model"); // cloud key unset → keyless local
        assert!(env.is_none());
    }

    #[test]
    fn strict_error_when_fallback_disabled() {
        let cfg = cfg(false);
        let ai = AiClient::from_config(&cfg).unwrap();
        assert!(ai.resolve_provider(&cfg, None).is_err());
    }

    #[test]
    fn requested_available_provider_is_used_directly() {
        let cfg = cfg(true);
        let ai = AiClient::from_config(&cfg).unwrap();
        let (model, _) = ai.resolve_provider(&cfg, Some("local")).unwrap();
        assert_eq!(model, "local-model");
    }
}
