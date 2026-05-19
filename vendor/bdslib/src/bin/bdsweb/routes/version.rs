use axum::{extract::State, Json};
use serde_json::{json, Value};

use crate::{client::rpc, error::AppError, state::AppState};

/// Returns `{"bdsweb": "<ver>", "bdsnode": "<ver>", "bundcore": "<ver>"}`
/// for the footer to display.
///
/// - `bdsweb`   — compiled-in `CARGO_PKG_VERSION` of the bdsweb binary.
/// - `bdsnode`  — fetched live from `v2/status`; falls back to `"unknown"`
///   when the RPC fails so the footer still renders.
/// - `bundcore` — version of the BUND VM crate, exposed by bdslib via
///   `bdslib::bundcore_version()`.  Reports the version compiled into
///   bdsweb itself; bdsnode loads the same crate so they match in
///   normal deployments.
pub async fn version(State(state): State<AppState>) -> Result<Json<Value>, AppError> {
    let bdsnode_ver = match rpc(&state, "v2/status", json!({})).await {
        Ok(v) => v.get("version")
            .and_then(|x| x.as_str())
            .unwrap_or("unknown")
            .to_owned(),
        Err(_) => "unknown".to_owned(),
    };
    Ok(Json(json!({
        "bdsweb":   env!("CARGO_PKG_VERSION"),
        "bdsnode":  bdsnode_ver,
        "bundcore": bdslib::bundcore_version(),
    })))
}
