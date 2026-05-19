use super::params::{duplication_timestamps, find_shard_for_uuid, rpc_err};
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct SecondaryParams {
    secondary_id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/secondary", |params, _ctx, _| async move {
            log::debug!("v2/secondary: start");
            let p: SecondaryParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                let uuid = Uuid::parse_str(&p.secondary_id)
                    .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.secondary_id)))?;

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let shard = find_shard_for_uuid(uuid, db)?;
                let obs = shard.observability();

                let mut doc = obs
                    .get_by_id(uuid)
                    .map_err(|e| rpc_err(-32004, e))?
                    .ok_or_else(|| rpc_err(-32404, format!("secondary {} not found", p.secondary_id)))?;

                let primary_id = obs
                    .primary_of(uuid)
                    .map_err(|e| rpc_err(-32005, e))?
                    .ok_or_else(|| rpc_err(-32404, format!("no primary found for secondary {}", p.secondary_id)))?;

                let duplications = duplication_timestamps(obs, uuid);

                if let Some(obj) = doc.as_object_mut() {
                    obj.insert("primary_id".to_string(), serde_json::json!(primary_id.to_string()));
                    obj.insert("duplications".to_string(), serde_json::json!(duplications));
                }

                Ok::<serde_json::Value, ErrorObject>(doc)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/secondary: done");
            result
        })
        .unwrap();
}
