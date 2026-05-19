use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_duration() -> String { "1h".to_owned() }

#[derive(serde::Deserialize)]
struct TplTemplatesRecentParams {
    #[allow(dead_code)]
    session: String,
    #[serde(default = "default_duration")]
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.templates_recent", |params, _ctx, _| async move {
            log::info!("v2/tpl.templates_recent: start");
            let p: TplTemplatesRecentParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/tpl.templates_recent: duration={}", p.duration);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let templates = db
                    .templates_recent(&p.duration)
                    .map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.templates_recent: {} templates", templates.len());
                Ok::<serde_json::Value, ErrorObject>(
                    serde_json::json!({ "templates": templates }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
