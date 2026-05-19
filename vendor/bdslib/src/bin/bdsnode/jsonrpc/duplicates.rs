use super::params::{rpc_err, TimeWindow, TimeWindowParams};
use jsonrpsee::RpcModule;
use std::time::UNIX_EPOCH;

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/duplicates", |params, _ctx, _| async move {
            log::debug!("v2/duplicates: start");
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

                let mut result = serde_json::Map::new();

                for si in shard_infos {
                    let shard = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
                    let obs = shard.observability();

                    let entries = match &window {
                        TimeWindow::All => obs.list_all_dedup_entries(),
                        TimeWindow::Range(s, e) => obs.list_dedup_entries_in_range(*s, *e),
                    }
                    .map_err(|e| rpc_err(-32004, e))?;

                    for (id, _key, times) in entries {
                        if times.is_empty() {
                            continue;
                        }
                        let ts_list: Vec<u64> = times
                            .into_iter()
                            .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
                            .collect();
                        let key = id.to_string();
                        result
                            .entry(key)
                            .or_insert_with(|| serde_json::json!(ts_list));
                    }
                }

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "duplicates": result }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/duplicates: done");
            result
        })
        .unwrap();
}
