use std::io::Write;
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

/// A prior turn in an ongoing chat, replayed back to the model so it has
/// context for follow-up questions. The TUI accumulates these in
/// `App::chat_history`; F9 clears them. Serde derives let the
/// `Ctrl+B K` exit hook persist the history into the project
/// directory and re-load it on the next entry.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ChatTurn {
    User(String),
    Assistant(String),
}

/// Spawn a background task that runs `Client::exec_chat_stream` and pushes
/// each text chunk onto an mpsc channel. The caller drains the receiver from
/// the sync event loop via `try_recv`. The task ends after sending either
/// `StreamMsg::Done` or `StreamMsg::Error`.
///
/// `history` is replayed in order before `user_prompt` so the assistant
/// sees prior turns. Pass an empty Vec for one-shot inferences (Help RAG).
pub fn spawn_chat_stream(
    client: Arc<Client>,
    model: String,
    system_prompt: Option<String>,
    history: Vec<ChatTurn>,
    user_prompt: String,
    category: &'static str,
) -> mpsc::UnboundedReceiver<StreamMsg> {
    // Road to 1.4.0 — record the inference to the AI cost dashboard under its
    // category (chat / grammar / explain / …). An empty category opts out (used by
    // `collect_blocking`, whose slow-track callers are counted in their own stores).
    // No-op headless.
    if !category.is_empty() {
        crate::ai::usage::record(category);
    }
    let (tx, rx) = mpsc::unbounded_channel();
    tokio::spawn(async move {
        let mut messages: Vec<ChatMessage> = Vec::new();
        if let Some(s) = system_prompt {
            if !s.trim().is_empty() {
                messages.push(ChatMessage::system(s));
            }
        }
        for turn in history {
            match turn {
                ChatTurn::User(t) => messages.push(ChatMessage::user(t)),
                ChatTurn::Assistant(t) => messages.push(ChatMessage::assistant(t)),
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

/// Run a one-shot chat completion to completion on the
/// calling (sync) thread, collecting every token into a
/// `String`.  Emits a `.` to stderr per token as a coarse
/// progress indicator.  This is what the bootstrap /
/// extract CLI commands use — they need the whole response
/// in hand before parsing it.
///
/// History is always empty (one-shot); pass the system
/// prompt the command wants.  `Err` carries the raw
/// inference-error message with no prefix — callers wrap
/// it in their own error type and decide how to report it.
pub fn collect_blocking(
    client: Arc<Client>,
    model: String,
    system_prompt: Option<String>,
    prompt: String,
) -> Result<String, String> {
    let mut rx = spawn_chat_stream(client, model, system_prompt, Vec::new(), prompt, "");
    let mut raw = String::new();
    while let Some(msg) = rx.blocking_recv() {
        match msg {
            StreamMsg::Token(t) => {
                raw.push_str(&t);
                let _ = std::io::stderr().write_all(b".");
                let _ = std::io::stderr().flush();
            }
            StreamMsg::Done => break,
            StreamMsg::Error(e) => return Err(e),
        }
    }
    Ok(raw)
}

#[cfg(test)]
mod runtime_context_tests {
    //! Regression for the INNER_EDITOR-1 engage crash: `collect_blocking`
    //! (`spawn_chat_stream` → `tokio::spawn`, then `blocking_recv`) panics with
    //! "there is no reactor running" when run on a plain OS thread with no Tokio
    //! runtime context. `App::start_bg_job` fixes that by capturing the main
    //! thread's runtime handle and entering it in the worker. This test pins the
    //! mechanism the fix relies on: an *entered* multi-thread handle lets a plain
    //! `std::thread` both `tokio::spawn` AND block on the result.

    #[test]
    fn entered_handle_lets_a_plain_thread_spawn_and_block() {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .build()
            .unwrap();
        let _guard = rt.enter();
        // Captured on a thread that holds the runtime context (as start_bg_job
        // does on the main thread).
        let handle = tokio::runtime::Handle::try_current().ok();
        assert!(handle.is_some(), "must capture a runtime handle inside the runtime");

        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let _g = handle.as_ref().map(|h| h.enter());
            // Without the entered handle this `tokio::spawn` would panic — the
            // same failure mode as collect_blocking.
            let (otx, orx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let _ = otx.send(7u8);
            });
            // The multi-thread runtime drives the task while we block here.
            let v = orx.blocking_recv().unwrap();
            tx.send(v).unwrap();
        })
        .join()
        .unwrap();

        assert_eq!(rx.recv().unwrap(), 7, "the spawned task ran and we received its result");
    }
}
