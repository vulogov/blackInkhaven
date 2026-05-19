use super::params::{rpc_err, TimeWindow, TimeWindowParams};
use jsonrpsee::RpcModule;
use std::time::UNIX_EPOCH;

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/shards", |params, _ctx, _| async move {
            log::debug!("v2/shards: start");
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

                let mut shards = Vec::new();
                for si in shard_infos {
                    let shard = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
                    let obs = shard.observability();

                    let (primary_count, secondary_count) = match &window {
                        TimeWindow::All => obs
                            .count_primaries_and_secondaries()
                            .map_err(|e| rpc_err(-32004, e))?,
                        TimeWindow::Range(s, e) => obs
                            .count_primaries_and_secondaries_in_range(*s, *e)
                            .map_err(|e| rpc_err(-32004, e))?,
                    };

                    let start_ts = si
                        .start_time
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let end_ts = si
                        .end_time
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();

                    shards.push(serde_json::json!({
                        "id": si.shard_id.to_string(),
                        "path": si.path,
                        "start_ts": start_ts,
                        "end_ts": end_ts,
                        "primary_count": primary_count,
                        "secondary_count": secondary_count,
                    }));
                }

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!(shards),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/shards: done");
            result
        })
        .unwrap();
}
