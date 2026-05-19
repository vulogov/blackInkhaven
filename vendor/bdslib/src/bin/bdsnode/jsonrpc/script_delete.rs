use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct ScriptDeleteParams {
    #[allow(dead_code)]
    session: String,
    /// UUIDv7 string of the script to delete.
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/script_delete", |params, _ctx, _| async move {
            log::debug!("v2/script_delete: start");
            let p: ScriptDeleteParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let id = Uuid::parse_str(&p.id)
                    .map_err(|e| rpc_err(-32600, format!("invalid id {:?}: {e}", p.id)))?;
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.script_delete(id).map_err(|e| rpc_err(-32004, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "id": p.id, "deleted": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/script_delete: done");
            result
        })
        .unwrap();
}
