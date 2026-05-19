use super::params::{find_shard_for_uuid, rpc_err};
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct SecondariesParams {
    primary_id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/secondaries", |params, _ctx, _| async move {
            log::debug!("v2/secondaries: start");
            let p: SecondariesParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                let uuid = Uuid::parse_str(&p.primary_id)
                    .map_err(|e| rpc_err(-32600, format!("invalid UUID {:?}: {e}", p.primary_id)))?;

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let shard = find_shard_for_uuid(uuid, db)?;

                let ids: Vec<String> = shard
                    .observability()
                    .list_secondaries(uuid)
                    .map_err(|e| rpc_err(-32004, e))?
                    .into_iter()
                    .map(|u| u.to_string())
                    .collect();

                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "ids": ids }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/secondaries: done");
            result
        })
        .unwrap();
}
