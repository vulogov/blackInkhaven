use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplDeleteParams {
    #[allow(dead_code)]
    session: String,
    id:      String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.delete", |params, _ctx, _| async move {
            log::info!("v2/tpl.delete: start");
            let p: TplDeleteParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.tpl_delete(id).map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.delete: deleted id={id}");
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "deleted": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
