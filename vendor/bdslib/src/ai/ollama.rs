// High-level Ollama chat session manager backed by ShardsManager docstore.
//
// Each chat session is a docstore document whose *content* is a
// JSON-encoded array of Ollama message objects:
//   [{"role":"system","content":"..."},{"role":"user","content":"..."},...]
//
// The session UUID (returned by `new_chat_session`) is the stable handle
// callers must pass to `chat` on every subsequent turn.

use uuid::Uuid;
use serde_json::Value as JsonValue;
use crate::common::error::{err_msg, Result};
use crate::globals::get_db;
use crate::ai::clients::ollama::OllamaClient;

/// Create a new chat session document in the docstore.
///
/// Returns the UUID of the newly created document.  Pass this UUID to
/// [`chat`] to continue the conversation.
pub fn new_chat_session(model: &str, system_prompt: &str) -> Result<Uuid> {
    let db = get_db()?;
    let metadata = serde_json::json!({
        "type": "chat_session",
        "model": model,
        "system_prompt": system_prompt,
    });
    // Seed content with the system message so it is part of every future turn.
    let initial: Vec<JsonValue> = vec![
        serde_json::json!({"role": "system", "content": system_prompt}),
    ];
    let content = serde_json::to_vec(&initial)
        .map_err(|e| err_msg(format!("ollama: serialize initial history: {e}")))?;
    db.doc_add(metadata, &content)
}

/// Send `user_message` in an existing chat session, persist the updated
/// history, and return the assistant's reply.
///
/// # Parameters
/// - `chat_id`      — UUID returned by [`new_chat_session`]
/// - `ollama_url`   — base URL of the Ollama server, e.g. `"http://localhost:11434"`
/// - `model`        — model name, e.g. `"llama3"`
/// - `system_prompt`— if the stored history is empty/missing the system role
///                    this prompt is prepended; otherwise the stored history
///                    already contains it and this is ignored
/// - `user_message` — the new user turn to send
pub fn chat(
    chat_id: Uuid,
    ollama_url: &str,
    model: &str,
    system_prompt: &str,
    user_message: &str,
) -> Result<String> {
    let db = get_db()?;

    // Load existing history from docstore.
    let raw = db.doc_get_content(chat_id)?
        .ok_or_else(|| err_msg(format!("ollama: chat session {chat_id} not found in docstore")))?;

    let mut history: Vec<JsonValue> = if raw.is_empty() {
        vec![]
    } else {
        serde_json::from_slice(&raw)
            .map_err(|e| err_msg(format!("ollama: deserialize history: {e}")))?
    };

    // Ensure system prompt is the first message.
    if history.is_empty() || history[0].get("role").and_then(|r| r.as_str()) != Some("system") {
        history.insert(0, serde_json::json!({"role": "system", "content": system_prompt}));
    }

    // Append the new user turn.
    history.push(serde_json::json!({"role": "user", "content": user_message}));

    // Call Ollama.
    let client = OllamaClient::new(ollama_url, model);
    let reply = client.chat(model, &history)?;

    // Persist assistant reply.
    history.push(serde_json::json!({"role": "assistant", "content": reply}));
    let updated = serde_json::to_vec(&history)
        .map_err(|e| err_msg(format!("ollama: serialize updated history: {e}")))?;
    db.doc_update_content(chat_id, &updated)?;

    Ok(reply)
}
