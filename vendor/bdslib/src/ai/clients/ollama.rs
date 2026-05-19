// Ollama Chat API client.
//
// Wraps the Ollama `/api/chat` HTTP endpoint (non-streaming).
// Docs: https://github.com/ollama/ollama/blob/main/docs/api.md#generate-a-chat-completion

use crate::common::error::{err_msg, Result};
use crate::ai::clients::AiClient;
use serde_json::Value as JsonValue;

/// Blocking HTTP client for a single Ollama instance.
///
/// Construct once and reuse across calls; the underlying `reqwest` connection
/// pool is shared.
pub struct OllamaClient {
    /// Base URL of the Ollama server, e.g. `"http://localhost:11434"`.
    base_url: String,
    /// Default model name used by [`AiClient::complete`].
    model: String,
    http: reqwest::blocking::Client,
}

impl OllamaClient {
    /// Create a new client pointed at `base_url` (trailing slash is stripped).
    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_owned(),
            model:    model.to_owned(),
            http:     reqwest::blocking::Client::new(),
        }
    }

    /// POST a chat request to `/api/chat` with `stream: false`.
    ///
    /// `messages` must be an ordered slice of Ollama message objects:
    /// `{"role": "system"|"user"|"assistant", "content": "…"}`.
    ///
    /// Returns the assistant's reply string on success.
    pub fn chat(&self, model: &str, messages: &[JsonValue]) -> Result<String> {
        let body = serde_json::json!({
            "model":    model,
            "messages": messages,
            "stream":   false,
        });
        let body_str = serde_json::to_string(&body)
            .map_err(|e| err_msg(format!("ollama: request serialize failed: {e}")))?;

        let resp = self.http
            .post(format!("{}/api/chat", self.base_url))
            .header("Content-Type", "application/json")
            .body(body_str)
            .send()
            .map_err(|e| err_msg(format!("ollama: HTTP request failed: {e}")))?;

        let status = resp.status();
        let text = resp.text()
            .map_err(|e| err_msg(format!("ollama: response read failed: {e}")))?;

        if !status.is_success() {
            return Err(err_msg(format!("ollama: HTTP {status}: {text}")));
        }

        let json: JsonValue = serde_json::from_str(&text)
            .map_err(|e| err_msg(format!("ollama: response parse failed: {e}")))?;

        json.get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(str::to_owned)
            .ok_or_else(|| err_msg(format!("ollama: response missing message.content — got: {text}")))
    }
}

impl AiClient for OllamaClient {
    /// Single-turn completion using the client's default model.
    fn complete(&self, prompt: &str) -> Result<String> {
        let messages = vec![serde_json::json!({"role": "user", "content": prompt})];
        self.chat(&self.model, &messages)
    }
}
