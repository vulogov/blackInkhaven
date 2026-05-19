use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct ScriptsParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/scripts", |params, _ctx, _| async move {
            log::debug!("v2/scripts: start");
            let _p: ScriptsParams = params.parse().unwrap_or(ScriptsParams { session: String::new() });
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let listed = db
                    .scripts_with_metadata()
                    .map_err(|e| rpc_err(-32004, e))?;
                let arr: Vec<serde_json::Value> = listed
                    .into_iter()
                    .map(|(id, meta)| {
                        let name = meta.get("name").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                        let schedule = meta.get("schedule").and_then(|v| v.as_str()).unwrap_or("").to_owned();
                        serde_json::json!({
                            "id":       id.to_string(),
                            "name":     name,
                            "schedule": schedule,
                            "metadata": meta,
                        })
                    })
                    .collect();
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "scripts": arr }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/scripts: done");
            result
        })
        .unwrap();
}
