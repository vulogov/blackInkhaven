use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_limit() -> usize { 10 }

#[derive(serde::Deserialize)]
struct FulltextParams {
    session: String,
    query: String,
    duration: String,
    #[serde(default = "default_limit")]
    limit: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/fulltext", |params, _ctx, _| async move {
            log::debug!("v2/fulltext: start");
            let p: FulltextParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/fulltext: session={} query={:?} duration={} limit={}", p.session, p.query, p.duration, p.limit);

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let hits = db
                    .fulltextsearch(&p.duration, &p.query, p.limit)
                    .map_err(|e| rpc_err(-32002, e))?;

                let results: Vec<serde_json::Value> = hits
                    .into_iter()
                    .map(|(id, score)| serde_json::json!({ "id": id.to_string(), "score": score }))
                    .collect();

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "results": results }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/fulltext: done");
            result
        })
        .unwrap();
}
