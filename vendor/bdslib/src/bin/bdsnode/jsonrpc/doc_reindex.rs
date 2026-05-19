use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct DocReindexParams {
    #[allow(dead_code)]
    session: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.reindex", |params, _ctx, _| async move {
            log::info!("v2/doc.reindex: start");
            let p: DocReindexParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/doc.reindex: session={}", p.session);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let count = db.doc_reindex().map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/doc.reindex: indexed {} documents", count);
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "indexed": count }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::info!("v2/doc.reindex: done");
            result
        })
        .unwrap();
}
