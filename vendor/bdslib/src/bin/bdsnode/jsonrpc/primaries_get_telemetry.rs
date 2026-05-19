use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct PrimariesGetTelemetryParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    key: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method(
            "v2/primaries.get.telemetry",
            |params, _ctx, _| async move {
                log::debug!("v2/primaries.get.telemetry: start");
                let p: PrimariesGetTelemetryParams = params.parse()?;

                let result = tokio::task::spawn_blocking(move || {
                    log::debug!(
                        "v2/primaries.get.telemetry: session={} duration={} key={}",
                        p.session, p.duration, p.key
                    );

                    humantime::parse_duration(&p.duration).map_err(|e| {
                        rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration))
                    })?;

                    let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                    let entries = db
                        .primaries_get_telemetry(&p.duration, &p.key)
                        .map_err(|e| rpc_err(-32004, e))?;

                    let items: Vec<serde_json::Value> = entries
                        .into_iter()
                        .map(|(id, ts, value)| {
                            serde_json::json!({
                                "id": id.to_string(),
                                "timestamp": ts,
                                "value": value,
                            })
                        })
                        .collect();

                    Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                        serde_json::json!({ "results": items }),
                    )
                })
                .await
                .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

                log::debug!("v2/primaries.get.telemetry: done");
                result
            },
        )
        .unwrap();
}
