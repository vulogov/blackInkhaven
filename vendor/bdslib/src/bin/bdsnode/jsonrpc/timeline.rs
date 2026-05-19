use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn err(code: i32, msg: impl std::fmt::Display) -> ErrorObject<'static> {
    ErrorObject::owned(code, msg.to_string(), None::<()>)
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/timeline", |_params, _ctx, _| async move {
            log::debug!("v2/timeline: start");
            let result = tokio::task::spawn_blocking(|| {
                let db = bdslib::get_db().map_err(|e| err(-32001, e))?;

                // Shards are time-partitioned and non-overlapping, so list_all()
                // returns them sorted by start_ts ASC. The global min timestamp
                // is always in the first non-empty shard and the global max in
                // the last — query only those two instead of every shard.
                let shards = db
                    .cache()
                    .info()
                    .list_all()
                    .map_err(|e| err(-32002, e))?;

                let mut global_min: Option<i64> = None;
                let mut global_max: Option<i64> = None;

                // Find min from the oldest shard (first in list).
                for info in &shards {
                    let shard = db.cache().shard(info.start_time).map_err(|e| err(-32003, e))?;
                    let (smin, _) = shard.observability().timestamp_range().map_err(|e| err(-32004, e))?;
                    if smin.is_some() {
                        global_min = smin;
                        break;
                    }
                }

                // Find max from the newest shard (last in list).
                for info in shards.iter().rev() {
                    let shard = db.cache().shard(info.start_time).map_err(|e| err(-32003, e))?;
                    let (_, smax) = shard.observability().timestamp_range().map_err(|e| err(-32004, e))?;
                    if smax.is_some() {
                        global_max = smax;
                        break;
                    }
                }

                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "min_ts": global_min,
                    "max_ts": global_max,
                }))
            })
            .await
            .map_err(|e| err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/timeline: done");
            result
        })
        .unwrap();
}
