use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct SignalsQueryParams {
    #[allow(dead_code)]
    session: String,
    query:   String,
    #[serde(default = "default_limit")]
    limit:   usize,
}

fn default_limit() -> usize { 20 }

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/signals_query", |params, _ctx, _| async move {
            log::debug!("v2/signals_query: start");
            let p: SignalsQueryParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let results = db.signals_query(&p.query, p.limit)
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "query":   p.query,
                    "count":   results.len(),
                    "results": results,
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/signals_query: done");
            result
        })
        .unwrap();
}
