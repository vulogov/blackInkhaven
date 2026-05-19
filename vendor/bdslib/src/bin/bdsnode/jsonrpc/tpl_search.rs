use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_limit() -> usize { 10 }
fn default_duration() -> String { "1h".to_owned() }

#[derive(serde::Deserialize)]
struct TplSearchParams {
    #[allow(dead_code)]
    session:  String,
    /// Lookback window for shards to search, e.g. "1h", "7days".
    #[serde(default = "default_duration")]
    duration: String,
    query:    String,
    #[serde(default = "default_limit")]
    limit:    usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.search", |params, _ctx, _| async move {
            log::info!("v2/tpl.search: start");
            let p: TplSearchParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/tpl.search: duration={} query={:?} limit={}",
                    p.duration, p.query, p.limit);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let results = db
                    .tpl_search_text(&p.duration, &p.query, p.limit)
                    .map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.search: {} results", results.len());
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "results": results }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
