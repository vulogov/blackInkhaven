// AI endpoint clients.
//
// Each submodule wraps one provider's HTTP API.  All clients are expected to
// implement the `AiClient` trait defined here so they are interchangeable at
// the call site.

pub mod anthropic;
pub mod ollama;
pub mod openai;

use crate::common::error::Result;

/// Minimal contract every AI provider client must satisfy.
pub trait AiClient {
    /// Send a plain-text prompt and return the model's reply as a String.
    fn complete(&self, prompt: &str) -> Result<String>;
}
