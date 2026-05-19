use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct DocAddParams {
    #[allow(dead_code)]
    session: String,
    metadata: serde_json::Value,
    content: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.add", |params, _ctx, _| async move {
            log::debug!("v2/doc.add: start");
            let p: DocAddParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.add: session={}", p.session);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let id = db
                    .doc_add(p.metadata, p.content.as_bytes())
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "id": id.to_string() }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.add: done");
            result
        })
        .unwrap();
}
