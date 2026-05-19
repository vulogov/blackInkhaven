pub mod prompts;
pub mod stream;

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

    /// Resolve a provider name to (model, env_var_or_none). Returns an
    /// error only when the provider declares an `api_key_env` and that env
    /// var is unset — local providers like Ollama omit `api_key_env` and
    /// bypass the check entirely.
    pub fn resolve_provider<'a>(
        &self,
        cfg: &'a LlmConfig,
        provider: Option<&str>,
    ) -> Result<(&'a str, Option<&'a str>)> {
        let name = provider.unwrap_or(&self.default_provider);
        let prov = cfg.providers.get(name).ok_or_else(|| {
            Error::Config(format!("unknown llm provider `{name}` — check inkhaven.hjson"))
        })?;
        if let Some(env_var) = prov.api_key_env.as_deref() {
            if std::env::var(env_var).is_err() {
                return Err(Error::Config(format!(
                    "{env_var} not set in environment — `export {env_var}=...`"
                )));
            }
            Ok((&prov.model, Some(env_var)))
        } else {
            Ok((&prov.model, None))
        }
    }
}
