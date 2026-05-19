use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct PrimariesExploreParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/primaries.explore", |params, _ctx, _| async move {
            log::debug!("v2/primaries.explore: start");
            let p: PrimariesExploreParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/primaries.explore: session={} duration={}",
                    p.session, p.duration
                );

                humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let entries = db
                    .primaries_explore(&p.duration)
                    .map_err(|e| rpc_err(-32004, e))?;

                let items: Vec<serde_json::Value> = entries
                    .into_iter()
                    .map(|(key, count, ids)| {
                        serde_json::json!({
                            "key": key,
                            "count": count,
                            "primary_id": ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                        })
                    })
                    .collect();

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!(items),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/primaries.explore: done");
            result
        })
        .unwrap();
}
