use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct DocDeleteParams {
    #[allow(dead_code)]
    session: String,
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.delete", |params, _ctx, _| async move {
            log::debug!("v2/doc.delete: start");
            let p: DocDeleteParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.delete: session={} id={}", p.session, id);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.doc_delete(id).map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "deleted": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.delete: done");
            result
        })
        .unwrap();
}
