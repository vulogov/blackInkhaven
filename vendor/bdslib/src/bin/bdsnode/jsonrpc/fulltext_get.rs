use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct FulltextGetParams {
    session: String,
    query: String,
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/fulltext.get", |params, _ctx, _| async move {
            log::debug!("v2/fulltext.get: start");
            let p: FulltextGetParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/fulltext.get: session={} query={:?} duration={}", p.session, p.query, p.duration);

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let docs = db
                    .search_fts(&p.duration, &p.query)
                    .map_err(|e| rpc_err(-32002, e))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "results": docs }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/fulltext.get: done");
            result
        })
        .unwrap();
}
