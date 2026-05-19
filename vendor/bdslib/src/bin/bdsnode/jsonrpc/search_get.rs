use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_limit() -> usize { 10 }

#[derive(serde::Deserialize)]
struct SearchGetParams {
    session: String,
    query: String,
    duration: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/search.get", |params, _ctx, _| async move {
            log::debug!("v2/search.get: start");
            let p: SearchGetParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/search.get: session={} query={:?} duration={} limit={}",
                    p.session, p.query, p.duration, p.limit
                );

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let query_json = serde_json::json!(p.query);
                let docs = db
                    .vectorsearch_recent(&p.duration, &query_json, p.limit)
                    .map_err(|e| rpc_err(-32004, e))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "results": docs }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/search.get: done");
            result
        })
        .unwrap();
}
