use askama::Template;
use axum::{
    extract::State,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Redirect, Response},
    Form,
};
use serde::Deserialize;
use serde_json::json;

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

const CHAT_COOKIE: &str = "bds-chat-session";

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "chat.html")]
struct ChatPage {
    model: String,
}

pub async fn page(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    Ok(Html(ChatPage { model: (*state.ollama_model).clone() }.render()?))
}

// ── Session reset ─────────────────────────────────────────────────────────────

pub async fn reset() -> Response {
    let mut resp = Redirect::to("/chat").into_response();
    if let Ok(val) = HeaderValue::from_str(
        &format!("{CHAT_COOKIE}=; Path=/; Max-Age=0; SameSite=Strict; HttpOnly"),
    ) {
        resp.headers_mut().insert(header::SET_COOKIE, val);
    }
    resp
}

// ── Query POST ────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct QueryForm {
    #[serde(default = "default_duration")]
    duration: String,
    #[serde(default)]
    query: String,
}
fn default_duration() -> String { "1h".to_owned() }

const INIT_QUERY: &str = "A new analysis session has just started and telemetry key inventory has been \
    loaded as context. In 2–3 sentences, summarise: which keys or services appear most active \
    (highest record counts), any keys whose names suggest errors or anomalies, and what the \
    operator should investigate first. Be concise and direct — this is an opening briefing.";

#[derive(Template)]
#[template(path = "partials/chat_message.html")]
struct ChatMessage {
    user_query:       String,
    response:         String,
    error_msg:        String,
    has_error:        bool,
    has_context_note: bool,
    context_note:     String,
}

// ── New session POST ──────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct NewSessionForm {
    #[serde(default = "default_duration")]
    duration: String,
}

pub async fn new_session(
    State(state): State<AppState>,
    Form(form): Form<NewSessionForm>,
) -> Result<Response, AppError> {
    // Step 1: fetch key inventory from bdsnode.
    let explore_params = json!({
        "session":  SESSION,
        "duration": form.duration,
    });
    let explore_result = rpc(&state, "v2/primaries.explore", explore_params).await;

    // Build a plain-text context string from the key inventory.
    // Format: "Available telemetry keys (last <duration>):\n  <key>: <count> records\n  ..."
    let (context_str, n_keys) = match explore_result {
        Ok(ref arr) if arr.is_array() => {
            let items = arr.as_array().unwrap();
            let mut lines: Vec<String> = Vec::with_capacity(items.len());
            for item in items.iter().take(100) {
                let key   = item["key"].as_str().unwrap_or("?");
                let count = item["count"].as_u64().unwrap_or(0);
                lines.push(format!("  {key}: {count} records"));
            }
            let n = items.len();
            let body = if lines.is_empty() {
                "  (no telemetry data in the selected time window)".to_owned()
            } else {
                lines.join("\n")
            };
            (format!("Available telemetry keys (last {}):\n{}", form.duration, body), n)
        }
        _ => (
            format!("(telemetry key inventory unavailable for the last {})", form.duration),
            0,
        ),
    };

    // Step 2: open a new chat session and send the briefing prompt with context.
    let chat_params = json!({
        "session":  SESSION,
        "chat_id":  null,
        "duration": form.duration,
        "query":    INIT_QUERY,
        "context":  context_str,
    });

    match rpc(&state, "v2/chat.ollama", chat_params).await {
        Ok(v) => {
            let chat_id  = v["chat_id"].as_str().unwrap_or("").to_owned();
            let response = v["response"].as_str().unwrap_or("").to_owned();

            let context_note = format!(
                "New session started. Found {} key{} in the last {}.",
                n_keys,
                if n_keys == 1 { "" } else { "s" },
                form.duration,
            );

            let html = ChatMessage {
                user_query:       String::new(),
                response,
                error_msg:        String::new(),
                has_error:        false,
                has_context_note: true,
                context_note,
            }.render()?;

            let mut resp = Html(html).into_response();
            if !chat_id.is_empty() {
                if let Ok(val) = HeaderValue::from_str(&format!(
                    "{CHAT_COOKIE}={chat_id}; Path=/; SameSite=Strict; HttpOnly"
                )) {
                    resp.headers_mut().insert(header::SET_COOKIE, val);
                }
            }
            Ok(resp)
        }
        Err(AppError::Rpc(msg)) => {
            let html = ChatMessage {
                user_query:       String::new(),
                response:         String::new(),
                error_msg:        msg,
                has_error:        true,
                has_context_note: false,
                context_note:     String::new(),
            }.render()?;
            Ok(Html(html).into_response())
        }
        Err(e) => Err(e),
    }
}

fn extract_chat_cookie(headers: &HeaderMap) -> Option<String> {
    headers.get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| {
            s.split(';')
             .map(|p| p.trim())
             .find(|p| p.starts_with(&format!("{CHAT_COOKIE}=")))
             .map(|p| p[CHAT_COOKIE.len() + 1..].to_string())
        })
}

pub async fn query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<QueryForm>,
) -> Result<Response, AppError> {
    if form.query.trim().is_empty() {
        return Ok((
            StatusCode::OK,
            Html(ChatMessage {
                user_query:       form.query,
                response:         String::new(),
                error_msg:        String::new(),
                has_error:        false,
                has_context_note: false,
                context_note:     String::new(),
            }.render()?),
        ).into_response());
    }

    let existing_id = extract_chat_cookie(&headers);

    let params = json!({
        "session":  SESSION,
        "chat_id":  existing_id,
        "duration": form.duration,
        "query":    form.query,
    });

    match rpc(&state, "v2/chat.ollama", params).await {
        Ok(v) => {
            let chat_id     = v["chat_id"].as_str().unwrap_or("").to_owned();
            let response    = v["response"].as_str().unwrap_or("").to_owned();
            let n_telemetry = v["telemetry_count"].as_u64().unwrap_or(0);
            let n_docs      = v["document_count"].as_u64().unwrap_or(0);

            let context_note = format!(
                "Retrieved {} telemetry event{} and {} document{} from the last {}.",
                n_telemetry, if n_telemetry == 1 { "" } else { "s" },
                n_docs,      if n_docs      == 1 { "" } else { "s" },
                form.duration,
            );

            let html = ChatMessage {
                user_query:       form.query,
                response,
                error_msg:        String::new(),
                has_error:        false,
                has_context_note: true,
                context_note,
            }.render()?;

            let mut resp = Html(html).into_response();

            // Set cookie when a new session is created or the ID changes.
            if !chat_id.is_empty() && existing_id.as_deref() != Some(&chat_id) {
                if let Ok(val) = HeaderValue::from_str(&format!(
                    "{CHAT_COOKIE}={chat_id}; Path=/; SameSite=Strict; HttpOnly"
                )) {
                    resp.headers_mut().insert(header::SET_COOKIE, val);
                }
            }

            Ok(resp)
        }
        Err(AppError::Rpc(msg)) => {
            let html = ChatMessage {
                user_query:       form.query,
                response:         String::new(),
                error_msg:        msg,
                has_error:        true,
                has_context_note: false,
                context_note:     String::new(),
            }.render()?;
            Ok(Html(html).into_response())
        }
        Err(e) => Err(e),
    }
}
