use askama::Template;
use axum::{
    extract::{Path, Query, State},
    response::Html,
    Form,
};
use serde::Deserialize;
use serde_json::json;
use std::time::{Duration, Instant};

use crate::{client::{rpc, SESSION}, error::AppError, state::AppState};

// ── Page (full shell) ─────────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "scripts.html")]
struct ScriptsPage {}

pub async fn page() -> Result<Html<String>, AppError> {
    Ok(Html(ScriptsPage {}.render()?))
}

// ── List (left column) ────────────────────────────────────────────────────────

struct ScriptListItem {
    id:       String,
    name:     String,
    schedule: String,
}

#[derive(Template)]
#[template(path = "partials/scripts_list.html")]
struct ScriptList {
    scripts: Vec<ScriptListItem>,
    is_empty: bool,
}

pub async fn list(State(state): State<AppState>) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/scripts", json!({ "session": SESSION })).await?;

    let mut items: Vec<ScriptListItem> = Vec::new();
    if let Some(arr) = resp.get("scripts").and_then(|v| v.as_array()) {
        for v in arr {
            items.push(ScriptListItem {
                id:       v.get("id").and_then(|s| s.as_str()).unwrap_or("").to_owned(),
                name:     v.get("name").and_then(|s| s.as_str()).unwrap_or("").to_owned(),
                schedule: v.get("schedule").and_then(|s| s.as_str()).unwrap_or("").to_owned(),
            });
        }
    }
    items.sort_by(|a, b| a.name.cmp(&b.name));
    let is_empty = items.is_empty();

    Ok(Html(ScriptList { scripts: items, is_empty }.render()?))
}

// ── Editor (right column) ─────────────────────────────────────────────────────

#[derive(Template)]
#[template(path = "partials/scripts_editor.html")]
struct ScriptEditor {
    /// Script id ("" when creating a new one).
    id:       String,
    /// Editable fields.
    name:     String,
    schedule: String,
    body:     String,
    /// Banner shown after save / delete.
    flash:    String,
    has_flash: bool,
    is_new:   bool,
}

#[derive(Deserialize)]
pub struct EditorParams {
    #[serde(default)]
    flash: String,
}

/// Empty editor for creating a new script.
pub async fn editor_new(Query(p): Query<EditorParams>) -> Result<Html<String>, AppError> {
    Ok(Html(ScriptEditor {
        id:       String::new(),
        name:     String::new(),
        schedule: String::new(),
        body:     String::new(),
        flash:    p.flash.clone(),
        has_flash: !p.flash.is_empty(),
        is_new:   true,
    }.render()?))
}

/// Editor pre-populated with an existing script.
pub async fn editor_get(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(p): Query<EditorParams>,
) -> Result<Html<String>, AppError> {
    let resp = rpc(&state, "v2/script", json!({
        "session": SESSION,
        "id":      id,
    })).await?;

    let body = resp.get("script").and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let meta = resp.get("metadata").cloned().unwrap_or(json!({}));
    let name = meta.get("name").and_then(|v| v.as_str()).unwrap_or("").to_owned();
    let schedule = meta.get("schedule").and_then(|v| v.as_str()).unwrap_or("").to_owned();

    Ok(Html(ScriptEditor {
        id,
        name,
        schedule,
        body,
        flash: p.flash.clone(),
        has_flash: !p.flash.is_empty(),
        is_new: false,
    }.render()?))
}

// ── Save (POST) — create or update ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct SaveForm {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub schedule: String,
    #[serde(default)]
    pub script: String,
}

pub async fn save(
    State(state): State<AppState>,
    Form(form): Form<SaveForm>,
) -> Result<Html<String>, AppError> {
    let metadata = json!({
        "name":     form.name,
        "schedule": form.schedule,
    });

    let (saved_id, flash) = if form.id.is_empty() {
        let resp = rpc(&state, "v2/script_add", json!({
            "session":  SESSION,
            "metadata": metadata,
            "script":   form.script,
        })).await?;
        let id = resp.get("id").and_then(|v| v.as_str()).unwrap_or("").to_owned();
        (id, "Created.".to_owned())
    } else {
        rpc(&state, "v2/script_update", json!({
            "session":  SESSION,
            "id":       form.id.clone(),
            "metadata": metadata,
            "script":   form.script,
        })).await?;
        (form.id.clone(), "Saved.".to_owned())
    };

    // Re-render the editor populated with saved fields (so the user keeps
    // editing) and refresh the list pane via HX-Trigger on the response.
    let editor = ScriptEditor {
        id:       saved_id,
        name:     form.name,
        schedule: form.schedule,
        body:     form.script,
        flash,
        has_flash: true,
        is_new: false,
    };
    Ok(Html(editor.render()?))
}

// ── Run (eval.queued + poll results) ──────────────────────────────────────────

#[derive(Deserialize)]
pub struct RunForm {
    #[serde(default)]
    pub script: String,
}

/// Total budget for waiting on the result queue to populate. Most BUND scripts
/// finish in well under a second; 30 s is a generous ceiling.
const RUN_POLL_BUDGET: Duration = Duration::from_secs(30);
/// Interval between `v2/results.empty` polls. Short enough to feel snappy,
/// long enough to avoid hammering the RPC server.
const RUN_POLL_INTERVAL: Duration = Duration::from_millis(250);

#[derive(Template)]
#[template(path = "partials/scripts_run_result.html")]
struct ScriptRunResult {
    /// Result-queue id returned by `v2/eval.queued`.
    queue_id: String,
    /// One pretty-printed JSON string per pulled value.
    values:   Vec<String>,
    /// True once at least one value was pulled.
    has_values: bool,
    /// True when the poll timed out without any values appearing.
    timed_out:  bool,
    /// Wall-clock seconds spent polling — informational.
    elapsed_ms: u128,
    /// Optional error message (RPC failure, empty script, …).
    error:    String,
    has_error: bool,
}

pub async fn run(
    State(state): State<AppState>,
    Form(form): Form<RunForm>,
) -> Result<Html<String>, AppError> {
    let script = form.script;
    let started = Instant::now();

    if script.trim().is_empty() {
        return Ok(Html(ScriptRunResult {
            queue_id: String::new(),
            values: vec![],
            has_values: false,
            timed_out: false,
            elapsed_ms: 0,
            error: "Script body is empty.".to_owned(),
            has_error: true,
        }.render()?));
    }

    // 1. Submit the script to the worker pool.
    let queue_id = match rpc(&state, "v2/eval.queued", json!({
        "session": SESSION,
        "script":  script,
    })).await {
        Ok(v) => v.get("id").and_then(|x| x.as_str()).unwrap_or("").to_owned(),
        Err(e) => return Ok(Html(ScriptRunResult {
            queue_id: String::new(),
            values: vec![],
            has_values: false,
            timed_out: false,
            elapsed_ms: started.elapsed().as_millis(),
            error: format!("v2/eval.queued failed: {e}"),
            has_error: true,
        }.render()?)),
    };

    if queue_id.is_empty() {
        return Ok(Html(ScriptRunResult {
            queue_id,
            values: vec![],
            has_values: false,
            timed_out: false,
            elapsed_ms: started.elapsed().as_millis(),
            error: "v2/eval.queued returned no id".to_owned(),
            has_error: true,
        }.render()?));
    }

    // 2. Poll v2/results.empty until the queue holds at least one value, or
    //    the budget elapses.
    let mut timed_out = true;
    while started.elapsed() < RUN_POLL_BUDGET {
        match rpc(&state, "v2/results.empty", json!({
            "session": SESSION,
            "id":      &queue_id,
        })).await {
            Ok(v) => {
                let count = v.get("count").and_then(|x| x.as_u64()).unwrap_or(0);
                if count > 0 {
                    timed_out = false;
                    break;
                }
            }
            Err(e) => return Ok(Html(ScriptRunResult {
                queue_id,
                values: vec![],
                has_values: false,
                timed_out: false,
                elapsed_ms: started.elapsed().as_millis(),
                error: format!("v2/results.empty failed: {e}"),
                has_error: true,
            }.render()?)),
        }
        tokio::time::sleep(RUN_POLL_INTERVAL).await;
    }

    if timed_out {
        return Ok(Html(ScriptRunResult {
            queue_id,
            values: vec![],
            has_values: false,
            timed_out: true,
            elapsed_ms: started.elapsed().as_millis(),
            error: String::new(),
            has_error: false,
        }.render()?));
    }

    // 3. Drain the queue: pull until remaining == 0.
    let mut values: Vec<String> = Vec::new();
    loop {
        let resp = rpc(&state, "v2/results.pull", json!({
            "session": SESSION,
            "id":      &queue_id,
        })).await;
        let v = match resp {
            Ok(v) => v,
            Err(e) => {
                return Ok(Html(ScriptRunResult {
                    queue_id,
                    values,
                    has_values: false,
                    timed_out: false,
                    elapsed_ms: started.elapsed().as_millis(),
                    error: format!("v2/results.pull failed: {e}"),
                    has_error: true,
                }.render()?));
            }
        };

        let value = v.get("value").cloned().unwrap_or(serde_json::Value::Null);
        let remaining = v.get("remaining").and_then(|x| x.as_u64()).unwrap_or(0);

        // Skip nulls returned for already-empty queues — defensive only.
        if !value.is_null() {
            let pretty = serde_json::to_string_pretty(&value)
                .unwrap_or_else(|_| value.to_string());
            values.push(pretty);
        }

        if remaining == 0 {
            break;
        }
    }

    let has_values = !values.is_empty();
    Ok(Html(ScriptRunResult {
        queue_id,
        values,
        has_values,
        timed_out: false,
        elapsed_ms: started.elapsed().as_millis(),
        error: String::new(),
        has_error: false,
    }.render()?))
}

// ── Delete ────────────────────────────────────────────────────────────────────

pub async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Html<String>, AppError> {
    rpc(&state, "v2/script_delete", json!({
        "session": SESSION,
        "id":      id,
    })).await?;

    // Return an empty editor with a flash message.
    Ok(Html(ScriptEditor {
        id:       String::new(),
        name:     String::new(),
        schedule: String::new(),
        body:     String::new(),
        flash:    "Deleted.".to_owned(),
        has_flash: true,
        is_new: true,
    }.render()?))
}
