use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TrendsParams {
    #[allow(dead_code)]
    session: String,
    key: String,
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/trends", |params, _ctx, _| async move {
            log::debug!("v2/trends: start");
            let p: TrendsParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/trends: session={} key={:?} duration={}",
                    p.session, p.key, p.duration
                );

                humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let trend = bdslib::TelemetryTrend::query_window(&p.key, &p.duration)
                    .map_err(|e| rpc_err(-32004, e))?;

                let value = serde_json::to_value(&trend)
                    .map_err(|e| rpc_err(-32004, format!("serialisation error: {e}")))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/trends: done");
            result
        })
        .unwrap();
}
