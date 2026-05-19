use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct ScriptAddParams {
    #[allow(dead_code)]
    session: String,
    /// Metadata JSON object — must contain non-empty `name` and `schedule`.
    metadata: serde_json::Value,
    /// Raw BUND script body.
    script: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/script_add", |params, _ctx, _| async move {
            log::debug!("v2/script_add: start");
            let p: ScriptAddParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let id = db
                    .script_add(p.metadata, &p.script)
                    .map_err(|e| rpc_err(-32600, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "id": id.to_string() }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/script_add: done");
            result
        })
        .unwrap();
}
