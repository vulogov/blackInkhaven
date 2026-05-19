use super::params::rpc_err;
use jsonrpsee::RpcModule;
use std::collections::BTreeSet;
use std::time::{Duration, SystemTime};

#[derive(serde::Deserialize)]
struct KeysParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/keys", |params, _ctx, _| async move {
            log::debug!("v2/keys: start");
            let p: KeysParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/keys: session={} duration={}",
                    p.session, p.duration
                );

                let secs = humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?
                    .as_secs();
                let end = SystemTime::now();
                let start = end - Duration::from_secs(secs);

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let cache = db.cache();

                let shard_infos = cache
                    .info()
                    .shards_in_range(start, end)
                    .map_err(|e| rpc_err(-32002, e))?;

                let mut keys: BTreeSet<String> = BTreeSet::new();
                for si in shard_infos {
                    let shard = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
                    let shard_keys = shard
                        .observability()
                        .list_primary_keys_in_range(start, end)
                        .map_err(|e| rpc_err(-32004, e))?;
                    keys.extend(shard_keys);
                }

                let keys: Vec<String> = keys.into_iter().collect();
                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "keys": keys }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/keys: done");
            result
        })
        .unwrap();
}
