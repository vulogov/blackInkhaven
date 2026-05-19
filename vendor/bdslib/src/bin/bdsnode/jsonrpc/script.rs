use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct ScriptParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
    /// UUIDv7 string of the script.
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/script", |params, _ctx, _| async move {
            log::debug!("v2/script: start");
            let p: ScriptParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let id = Uuid::parse_str(&p.id)
                    .map_err(|e| rpc_err(-32600, format!("invalid id {:?}: {e}", p.id)))?;
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let body = db.script(id).map_err(|e| rpc_err(-32004, e))?;
                let meta = db.script_metadata(id).map_err(|e| rpc_err(-32004, e))?;
                let body = body.ok_or_else(|| rpc_err(-32404, format!("script {} not found", p.id)))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "id":       p.id,
                    "script":   body,
                    "metadata": meta.unwrap_or(serde_json::json!({})),
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/script: done");
            result
        })
        .unwrap();
}
