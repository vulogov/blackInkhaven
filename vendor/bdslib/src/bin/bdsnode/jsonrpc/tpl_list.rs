use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_duration() -> String { "1h".to_owned() }

#[derive(serde::Deserialize)]
struct TplListParams {
    #[allow(dead_code)]
    session:  String,
    /// Lookback window, e.g. "1h", "7days".  Defaults to "1h".
    #[serde(default = "default_duration")]
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.list", |params, _ctx, _| async move {
            log::info!("v2/tpl.list: start");
            let p: TplListParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/tpl.list: session={} duration={}", p.session, p.duration);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let all = db.tpl_list(&p.duration).map_err(|e| rpc_err(-32011, e))?;

                let templates: Vec<serde_json::Value> = all
                    .into_iter()
                    .map(|(id, metadata)| serde_json::json!({
                        "id":       id.to_string(),
                        "metadata": metadata,
                    }))
                    .collect();

                log::info!("v2/tpl.list: {} templates", templates.len());
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "templates": templates }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
