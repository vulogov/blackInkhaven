use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct SignalsParams {
    #[allow(dead_code)]
    session:  String,
    #[serde(default = "default_duration")]
    duration: String,
}

fn default_duration() -> String { "1h".to_owned() }

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/signals", |params, _ctx, _| async move {
            log::debug!("v2/signals: start");
            let p: SignalsParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let ids = db.signals_recent(&p.duration)
                    .map_err(|e| rpc_err(-32011, e))?;

                // Resolve each ID to its full metadata.
                let mut signals: Vec<serde_json::Value> = Vec::with_capacity(ids.len());
                for id_str in &ids {
                    if let Ok(uuid) = uuid::Uuid::parse_str(id_str) {
                        if let Ok(Some(meta)) = db.signal_get(uuid) {
                            signals.push(serde_json::json!({
                                "id":       id_str,
                                "metadata": meta,
                            }));
                            continue;
                        }
                    }
                    signals.push(serde_json::json!({ "id": id_str, "metadata": null }));
                }

                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "duration": p.duration,
                    "count":    signals.len(),
                    "signals":  signals,
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/signals: done");
            result
        })
        .unwrap();
}
