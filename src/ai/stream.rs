use std::sync::Arc;

use futures_util::StreamExt;
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest, ChatStreamEvent};
use tokio::sync::mpsc;

/// Streaming event we forward from the genai task back to the TUI event loop.
#[derive(Debug)]
pub enum StreamMsg {
    Token(String),
    Done,
    Error(String),
}

/// Spawn a background task that runs `Client::exec_chat_stream` and pushes
/// each text chunk onto an mpsc channel. The caller drains the receiver from
/// the sync event loop via `try_recv`. The task ends after sending either
/// `StreamMsg::Done` or `StreamMsg::Error`.
pub fn spawn_chat_stream(
    client: Arc<Client>,
    model: String,
    system_prompt: Option<String>,
    user_prompt: String,
) -> mpsc::UnboundedReceiver<StreamMsg> {
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut messages: Vec<ChatMessage> = Vec::new();
        if let Some(s) = system_prompt {
            if !s.trim().is_empty() {
                messages.push(ChatMessage::system(s));
            }
        }
        messages.push(ChatMessage::user(user_prompt));
        let req = ChatRequest::new(messages);

        let response = match client.exec_chat_stream(model.as_str(), req, None).await {
            Ok(r) => r,
            Err(e) => {
                let _ = tx.send(StreamMsg::Error(format!("exec_chat_stream: {e}")));
                return;
            }
        };

        let mut stream = response.stream;
        while let Some(event) = stream.next().await {
            match event {
                Ok(ChatStreamEvent::Chunk(chunk)) => {
                    if tx.send(StreamMsg::Token(chunk.content)).is_err() {
                        // Receiver dropped — abandon stream.
                        return;
                    }
                }
                Ok(ChatStreamEvent::ReasoningChunk(_))
                | Ok(ChatStreamEvent::ThoughtSignatureChunk(_))
                | Ok(ChatStreamEvent::ToolCallChunk(_))
                | Ok(ChatStreamEvent::Start)
                | Ok(ChatStreamEvent::End(_)) => {}
                Err(e) => {
                    let _ = tx.send(StreamMsg::Error(format!("stream event: {e}")));
                    return;
                }
            }
        }
        let _ = tx.send(StreamMsg::Done);
    });
    rx
}
