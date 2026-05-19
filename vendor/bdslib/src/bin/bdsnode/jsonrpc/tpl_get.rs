use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplGetParams {
    #[allow(dead_code)]
    session: String,
    id:      String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.get", |params, _ctx, _| async move {
            log::info!("v2/tpl.get: start");
            let p: TplGetParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let id_str = p.id.clone();
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let metadata = db
                    .tpl_get_metadata(id)
                    .map_err(|e| rpc_err(-32011, e))?
                    .ok_or_else(|| rpc_err(-32010, format!("template {id} not found")))?;
                let body_bytes = db
                    .tpl_get_body(id)
                    .map_err(|e| rpc_err(-32011, e))?
                    .unwrap_or_default();
                let body = String::from_utf8_lossy(&body_bytes).into_owned();
                log::info!("v2/tpl.get: found id={id_str}");
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "id":       id_str,
                    "metadata": metadata,
                    "body":     body,
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
