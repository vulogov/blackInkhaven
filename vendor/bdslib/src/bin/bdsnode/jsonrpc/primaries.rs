use super::params::{rpc_err, TimeWindow, TimeWindowParams};
use jsonrpsee::RpcModule;

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/primaries", |params, _ctx, _| async move {
            log::debug!("v2/primaries: start");
            let p: TimeWindowParams = params.parse().unwrap_or_default();
            let window = p.resolve()?;

            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let cache = db.cache();

                let shard_infos = match &window {
                    TimeWindow::All => cache.info().list_all(),
                    TimeWindow::Range(s, e) => cache.info().shards_in_range(*s, *e),
                }
                .map_err(|e| rpc_err(-32002, e))?;

                let mut ids: Vec<String> = Vec::new();
                for si in shard_infos {
                    let shard = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
                    let obs = shard.observability();
                    let uuids = match &window {
                        TimeWindow::All => obs.list_primaries(),
                        TimeWindow::Range(s, e) => obs.list_primaries_in_range(*s, *e),
                    }
                    .map_err(|e| rpc_err(-32004, e))?;

                    ids.extend(uuids.into_iter().map(|u| u.to_string()));
                }

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(serde_json::json!({ "ids": ids }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/primaries: done");
            result
        })
        .unwrap();
}
