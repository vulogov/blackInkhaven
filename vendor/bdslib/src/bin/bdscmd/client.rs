use anyhow::{bail, Context, Result};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

pub fn call(url: &str, method: &str, params: Value) -> Result<Value> {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id
    });
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("failed to build HTTP client")?;
    let body_str = serde_json::to_string(&body).context("failed to serialise request")?;
    let resp = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(body_str)
        .send()
        .with_context(|| format!("failed to connect to {url}"))?;
    let text = resp.text().context("failed to read response body")?;
    let v: Value = serde_json::from_str(&text).context("invalid JSON response")?;
    if let Some(e) = v.get("error") {
        bail!("server error: {e}");
    }
    Ok(v["result"].clone())
}

pub fn check_server(url: &str) -> Result<()> {
    call(url, "v2/status", serde_json::json!({}))
        .with_context(|| format!("bdsnode not reachable at {url}"))?;
    Ok(())
}
