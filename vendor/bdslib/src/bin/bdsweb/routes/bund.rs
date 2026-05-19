use askama::Template;
use axum::{extract::State, response::Html, Form};
use serde::Deserialize;
use serde_json::json;

use crate::{client::rpc, error::AppError, state::AppState};

// ── Full page ─────────────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "bund.html")]
struct BundPage {}

pub async fn page() -> Result<Html<String>, AppError> {
    Ok(Html(BundPage {}.render()?))
}

// ── Eval POST ─────────────────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct EvalForm {
    #[serde(default)]
    context: String,
    #[serde(default)]
    script: String,
}

#[derive(Template)]
#[template(path = "partials/bund_result.html")]
struct BundResult {
    results:   Vec<String>,
    error_msg: String,
    has_error: bool,
}

pub async fn eval(
    State(state): State<AppState>,
    Form(form): Form<EvalForm>,
) -> Result<Html<String>, AppError> {
    if form.script.trim().is_empty() {
        return Ok(Html(BundResult {
            results: vec![],
            error_msg: String::new(),
            has_error: false,
        }.render()?));
    }

    let ctx = if form.context.trim().is_empty() {
        "default".to_owned()
    } else {
        form.context.clone()
    };

    match rpc(&state, "v2/eval", json!({ "context": ctx, "script": form.script })).await {
        Ok(v) => {
            let results = match v.get("result") {
                None | Some(serde_json::Value::Null) => vec![],
                Some(r) => vec![
                    serde_json::to_string_pretty(r).unwrap_or_else(|_| r.to_string())
                ],
            };
            Ok(Html(BundResult { results, error_msg: String::new(), has_error: false }.render()?))
        }
        Err(AppError::Rpc(msg)) => {
            Ok(Html(BundResult { results: vec![], error_msg: msg, has_error: true }.render()?))
        }
        Err(e) => Err(e),
    }
}
