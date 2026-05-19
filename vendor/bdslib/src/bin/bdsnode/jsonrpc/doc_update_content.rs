use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct DocUpdateContentParams {
    #[allow(dead_code)]
    session: String,
    id: String,
    content: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.update.content", |params, _ctx, _| async move {
            log::debug!("v2/doc.update.content: start");
            let p: DocUpdateContentParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.update.content: session={} id={}", p.session, id);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.doc_update_content(id, p.content.as_bytes())
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "updated": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.update.content: done");
            result
        })
        .unwrap();
}
