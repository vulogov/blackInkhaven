use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct KeysGetParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    key: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/keys.get", |params, _ctx, _| async move {
            log::debug!("v2/keys.get: start");
            let p: KeysGetParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/keys.get: session={} duration={} key={:?}",
                    p.session, p.duration, p.key
                );

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let entries = db
                    .keys_by_pattern(&p.duration, &p.key)
                    .map_err(|e| rpc_err(-32004, e))?;

                let results: Vec<serde_json::Value> = entries
                    .into_iter()
                    .map(|(primary_id, ts, secondary_ids)| {
                        serde_json::json!({
                            "timestamp": ts,
                            "primary_id": primary_id.to_string(),
                            "secondary_ids": secondary_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>(),
                        })
                    })
                    .collect();

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "results": results }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/keys.get: done");
            result
        })
        .unwrap();
}
