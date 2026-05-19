use super::params::{rpc_err, TimeWindow, TimeWindowParams};
use jsonrpsee::RpcModule;

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/count", |params, _ctx, _| async move {
            log::debug!("v2/count: start");
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

                let mut total: u64 = 0;
                for si in shard_infos {
                    let obs = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
                    let n = match &window {
                        TimeWindow::All => obs.observability().count_all(),
                        TimeWindow::Range(s, e) => obs.observability().count_in_range(*s, *e),
                    }
                    .map_err(|e| rpc_err(-32004, e))?;
                    total += n;
                }

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(serde_json::json!({ "count": total }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/count: done");
            result
        })
        .unwrap();
}
