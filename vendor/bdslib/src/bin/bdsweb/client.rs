use serde_json::{json, Value};
use crate::{error::AppError, state::AppState};

pub const SESSION: &str = "bdsweb-ui-session";

pub async fn rpc(state: &AppState, method: &str, params: Value) -> Result<Value, AppError> {
    let payload = json!({
        "jsonrpc": "2.0",
        "method":  method,
        "params":  params,
        "id":      1
    });

    let body = serde_json::to_string(&payload)?;
    let resp  = state.http
        .post(state.node_url.as_str())
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await?;

    let text: String = resp.text().await?;
    let envelope: Value = serde_json::from_str(&text)?;

    if let Some(err) = envelope.get("error") {
        let msg = err["message"].as_str().unwrap_or("unknown RPC error").to_owned();
        return Err(AppError::Rpc(msg));
    }

    Ok(envelope["result"].clone())
}

// ── Small helpers to pull typed scalars out of JSON safely ────────────────────

pub fn str_val(v: &Value, key: &str) -> String {
    v.get(key)
     .and_then(|x| x.as_str())
     .unwrap_or("—")
     .to_owned()
}

pub fn u64_val(v: &Value, key: &str) -> u64 {
    v.get(key).and_then(|x| x.as_u64()).unwrap_or(0)
}

pub fn fmt_ts(unix_secs: u64) -> String {
    use chrono::{TimeZone, Utc};
    if unix_secs == 0 { return "—".to_owned(); }
    Utc.timestamp_opt(unix_secs as i64, 0)
       .single()
       .map(|dt| dt.format("%Y-%m-%d %H:%M UTC").to_string())
       .unwrap_or_else(|| "—".to_owned())
}
