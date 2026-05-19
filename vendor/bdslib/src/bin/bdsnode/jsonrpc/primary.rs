use super::params::{duplication_timestamps, find_shard_for_uuid, rpc_err};
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct PrimaryParams {
    primary_id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/primary", |params, _ctx, _| async move {
            log::debug!("v2/primary: start");
            let p: PrimaryParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                let uuid = Uuid::parse_str(&p.primary_id)
                    .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.primary_id)))?;

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let shard = find_shard_for_uuid(uuid, db)?;
                let obs = shard.observability();

                let mut doc = obs
                    .get_by_id(uuid)
                    .map_err(|e| rpc_err(-32004, e))?
                    .ok_or_else(|| rpc_err(-32404, format!("primary {} not found", p.primary_id)))?;

                let secondaries_count =
                    obs.list_secondaries(uuid).map(|v| v.len()).unwrap_or(0);
                let duplications = duplication_timestamps(obs, uuid);

                if let Some(obj) = doc.as_object_mut() {
                    obj.insert("secondaries_count".to_string(), serde_json::json!(secondaries_count));
                    obj.insert("duplications".to_string(), serde_json::json!(duplications));
                }

                Ok::<serde_json::Value, ErrorObject>(doc)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/primary: done");
            result
        })
        .unwrap();
}
