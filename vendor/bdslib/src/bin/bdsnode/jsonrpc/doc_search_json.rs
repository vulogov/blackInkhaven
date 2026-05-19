use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_limit() -> usize {
    10
}

#[derive(serde::Deserialize)]
struct DocSearchJsonParams {
    #[allow(dead_code)]
    session: String,
    query: serde_json::Value,
    #[serde(default = "default_limit")]
    limit: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.search.json", |params, _ctx, _| async move {
            log::debug!("v2/doc.search.json: start");
            let p: DocSearchJsonParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.search.json: session={}", p.session);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let results = db
                    .doc_search_json(&p.query, p.limit)
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "results": results }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.search.json: done");
            result
        })
        .unwrap();
}
