use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_limit() -> usize {
    10
}

#[derive(serde::Deserialize)]
struct DocSearchStringsParams {
    #[allow(dead_code)]
    session: String,
    query: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.search.strings", |params, _ctx, _| async move {
            log::debug!("v2/doc.search.strings: start");
            let p: DocSearchStringsParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/doc.search.strings: session={} query={:?}",
                    p.session,
                    p.query
                );
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let results = db
                    .doc_search_text_strings(&p.query, p.limit)
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "results": results }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.search.strings: done");
            result
        })
        .unwrap();
}
