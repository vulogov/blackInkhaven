pub mod prompts;
pub mod stream;
pub mod usage;

use std::collections::HashMap;
use std::sync::Arc;

use genai::adapter::AdapterKind;
use genai::resolver::{AuthData, Endpoint, ServiceTargetResolver};
use genai::{Client, ModelIden, ServiceTarget};

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
        // 3.21 — providers with a `base_url` are routed by a service-target
        // resolver keyed on the model string; without any, the stock client
        // (adapter and host from the model name) is unchanged.
        let targets = custom_targets(cfg)?;
        let client = if targets.is_empty() {
            Client::default()
        } else {
            let resolver = ServiceTargetResolver::from_resolver_fn(
                move |st: ServiceTarget| -> std::result::Result<ServiceTarget, genai::resolver::Error> {
                    Ok(resolve_custom_target(&targets, st))
                },
            );
            Client::builder().with_service_target_resolver(resolver).build()
        };
        Ok(Self {
            client: Arc::new(client),
            default_provider: cfg.default.clone(),
        })
    }

    /// 3.22 — where a provider's requests go, in words: its `base_url`
    /// through the OpenAI adapter, or genai's own choice of adapter and host
    /// from the model name. Pure; shared by `inkhaven ai --resolve` and the
    /// `Ctrl+B L` switch status.
    pub fn describe_target(prov: &crate::config::LlmProvider) -> String {
        match &prov.base_url {
            Some(url) => format!("{} (OpenAI-compatible)", url.trim().trim_end_matches('/')),
            None => "genai default (adapter and host from the model name)".to_string(),
        }
    }

    /// 3.22 — the key state of a provider, in words: `no key needed`,
    /// `KEY set` or `KEY unset`.
    pub fn describe_key(prov: &crate::config::LlmProvider) -> String {
        match prov.api_key_env.as_deref() {
            None => "no key needed".to_string(),
            Some(env) if std::env::var(env).is_ok() => format!("{env} set"),
            Some(env) => format!("{env} unset"),
        }
    }

    /// 3.22 — the provider name [`resolve_provider`](Self::resolve_provider)
    /// would land on for `provider` (the requested one, or the fallback it
    /// picks), with the model it sends.
    pub fn resolve_provider_name<'a>(
        &self,
        cfg: &'a LlmConfig,
        provider: Option<&str>,
    ) -> Result<(&'a str, &'a str)> {
        let (model, _) = self.resolve_provider(cfg, provider)?;
        let name = provider.unwrap_or(&self.default_provider);
        let chosen = cfg
            .providers
            .get(name)
            .filter(|p| p.model == model && provider_available(p))
            .map(|_| name)
            .or_else(|| {
                cfg.providers
                    .iter()
                    .find(|(n, p)| n.as_str() != name && p.model == model && provider_available(p))
                    .map(|(n, _)| n.as_str())
            })
            .unwrap_or(name);
        let (chosen, _) = cfg.providers.get_key_value(chosen).ok_or_else(|| {
            Error::Config(format!("unknown llm provider `{chosen}` — check inkhaven.hjson"))
        })?;
        Ok((chosen.as_str(), model))
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
/// One custom endpoint: the normalised base URL and the API-key env var.
type CustomTarget = (String, Option<String>);

/// The `base_url` providers keyed by model string, validated: a URL must be
/// http(s) (a trailing `/` is added — genai appends `chat/completions` to
/// it), and no model string may be claimed twice, since the model is all the
/// resolver sees at request time.
fn custom_targets(cfg: &LlmConfig) -> Result<HashMap<String, CustomTarget>> {
    let mut out: HashMap<String, CustomTarget> = HashMap::new();
    let mut owner: HashMap<&str, &str> = HashMap::new();
    for (name, p) in &cfg.providers {
        if let Some(prev) = owner.insert(p.model.as_str(), name.as_str()) {
            if p.base_url.is_some() || cfg.providers[prev].base_url.is_some() {
                return Err(Error::Config(format!(
                    "llm providers `{prev}` and `{name}` both list model `{}`; a `base_url` provider is \
                     recognised by its model string, so give it a distinct one (e.g. `gateway/{}`)",
                    p.model, p.model
                )));
            }
        }
        if let Some(url) = &p.base_url {
            out.insert(p.model.clone(), (normalise_base_url(name, url)?, p.api_key_env.clone()));
        }
    }
    Ok(out)
}

fn normalise_base_url(provider: &str, url: &str) -> Result<String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) || url.len() <= 8 {
        return Err(Error::Config(format!(
            "llm provider `{provider}`: base_url `{url}` must start with https:// or http://"
        )));
    }
    Ok(if url.ends_with('/') { url.to_string() } else { format!("{url}/") })
}

/// Send a `base_url` provider's model to its endpoint through the OpenAI
/// adapter, with its key; every other model keeps genai's own target.
fn resolve_custom_target(targets: &HashMap<String, CustomTarget>, st: ServiceTarget) -> ServiceTarget {
    let name = st.model.model_name.to_string();
    match targets.get(&name) {
        Some((url, key)) => ServiceTarget {
            endpoint: Endpoint::from_owned(url.clone()),
            auth: match key {
                Some(env) => AuthData::from_env(env.clone()),
                // A keyless local server; the adapter still sends a bearer.
                None => AuthData::from_single("none"),
            },
            model: ModelIden::new(AdapterKind::OpenAI, name),
        },
        None => st,
    }
}

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
                base_url: None,
            },
        );
        providers.insert(
            "local".into(),
            LlmProvider { model: "local-model".into(), api_key_env: None, base_url: None },
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
    fn base_url_routes_its_model_through_the_openai_adapter() {
        let mut c = cfg(true);
        c.providers.insert(
            "gateway".into(),
            LlmProvider {
                model: "org/some-open-model".into(),
                api_key_env: Some("GATEWAY_KEY".into()),
                base_url: Some("https://gateway.example/v1".into()),
            },
        );
        c.providers.insert(
            "lmstudio".into(),
            LlmProvider {
                model: "local-gguf".into(),
                api_key_env: None,
                base_url: Some("http://127.0.0.1:1234/v1/".into()),
            },
        );
        let targets = custom_targets(&c).unwrap();
        assert_eq!(targets.len(), 2);
        // The trailing slash genai needs is added; an existing one is kept.
        assert_eq!(targets["org/some-open-model"].0, "https://gateway.example/v1/");
        assert_eq!(targets["local-gguf"].0, "http://127.0.0.1:1234/v1/");
        let incoming = ServiceTarget {
            endpoint: Endpoint::from_static("http://localhost:11434/v1/"),
            auth: AuthData::from_single(""),
            model: ModelIden::new(AdapterKind::Ollama, "org/some-open-model"),
        };
        let out = resolve_custom_target(&targets, incoming);
        assert_eq!(out.endpoint.base_url(), "https://gateway.example/v1/");
        assert_eq!(out.model.adapter_kind, AdapterKind::OpenAI);
        assert_eq!(out.model.model_name.to_string(), "org/some-open-model");
        assert!(matches!(out.auth, AuthData::FromEnv(ref e) if e == "GATEWAY_KEY"));
        // A keyless local endpoint gets a placeholder bearer, not an env lookup.
        let local = resolve_custom_target(
            &targets,
            ServiceTarget {
                endpoint: Endpoint::from_static("http://localhost:11434/v1/"),
                auth: AuthData::from_single(""),
                model: ModelIden::new(AdapterKind::Ollama, "local-gguf"),
            },
        );
        assert!(matches!(local.auth, AuthData::Key(_)));
        // A model that belongs to no base_url provider is left to genai.
        let other = resolve_custom_target(
            &targets,
            ServiceTarget {
                endpoint: Endpoint::from_static("https://api.openai.com/v1/"),
                auth: AuthData::from_env("OPENAI_API_KEY"),
                model: ModelIden::new(AdapterKind::OpenAI, "gpt-4o"),
            },
        );
        assert_eq!(other.endpoint.base_url(), "https://api.openai.com/v1/");
        // The client builds, and the key gate still applies to the provider.
        let ai = AiClient::from_config(&c).unwrap();
        // With auto_fallback the unset GATEWAY_KEY falls through to a keyless
        // provider; strict mode refuses, like any other keyed provider.
        assert!(ai.resolve_provider(&c, Some("gateway")).is_ok());
        let mut strict = c.clone();
        strict.auto_fallback = false;
        assert!(ai.resolve_provider(&strict, Some("gateway")).is_err(), "GATEWAY_KEY is unset");
        assert_eq!(ai.resolve_provider(&c, Some("lmstudio")).unwrap().0, "local-gguf");
    }

    #[test]
    fn base_url_rejects_a_bad_scheme_and_a_shared_model_string() {
        let mut c = cfg(true);
        c.providers.insert(
            "gateway".into(),
            LlmProvider {
                model: "m".into(),
                api_key_env: None,
                base_url: Some("gateway.example/v1".into()),
            },
        );
        let err = AiClient::from_config(&c).err().map(|e| e.to_string()).expect("a config error");
        assert!(err.contains("must start with https:// or http://"), "{err}");
        // Same model string as the keyless `local` provider → refused.
        c.providers.get_mut("gateway").unwrap().base_url = Some("https://gateway.example/v1".into());
        c.providers.get_mut("gateway").unwrap().model = "local-model".into();
        let err = AiClient::from_config(&c).err().map(|e| e.to_string()).expect("a config error");
        assert!(err.contains("both list model `local-model`"), "{err}");
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
