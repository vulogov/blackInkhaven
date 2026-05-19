use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct TplTemplatesByTimestampParams {
    #[allow(dead_code)]
    session: String,
    start_ts: u64,
    end_ts: u64,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method(
            "v2/tpl.templates_by_timestamp",
            |params, _ctx, _| async move {
                log::info!("v2/tpl.templates_by_timestamp: start");
                let p: TplTemplatesByTimestampParams = params.parse()?;
                let result = tokio::task::spawn_blocking(move || {
                    log::info!(
                        "v2/tpl.templates_by_timestamp: start_ts={} end_ts={}",
                        p.start_ts, p.end_ts
                    );
                    let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                    let templates = db
                        .templates_by_timestamp(p.start_ts, p.end_ts)
                        .map_err(|e| rpc_err(-32011, e))?;
                    log::info!("v2/tpl.templates_by_timestamp: {} templates", templates.len());
                    Ok::<serde_json::Value, ErrorObject>(
                        serde_json::json!({ "templates": templates }),
                    )
                })
                .await
                .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
                result
            },
        )
        .unwrap();
}
