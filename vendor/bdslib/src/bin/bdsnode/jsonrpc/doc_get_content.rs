use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct DocGetContentParams {
    #[allow(dead_code)]
    session: String,
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.get.content", |params, _ctx, _| async move {
            log::debug!("v2/doc.get.content: start");
            let p: DocGetContentParams = params.parse()?;
            let id = uuid::Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.id)))?;
            let id_str = p.id.clone();
            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.get.content: session={} id={}", p.session, id);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let content_bytes = db
                    .doc_get_content(id)
                    .map_err(|e| rpc_err(-32011, e))?
                    .ok_or_else(|| rpc_err(-32010, format!("document {id} not found")))?;
                let content_str = String::from_utf8_lossy(&content_bytes).into_owned();
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "id": id_str,
                    "content": content_str,
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.get.content: done");
            result
        })
        .unwrap();
}
