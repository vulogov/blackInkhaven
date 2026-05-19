use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_duration() -> String { "24h".to_owned() }

#[derive(serde::Deserialize)]
struct TplReindexParams {
    #[allow(dead_code)]
    session:  String,
    /// Lookback window for shards to reindex.  Defaults to "24h".
    #[serde(default = "default_duration")]
    duration: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/tpl.reindex", |params, _ctx, _| async move {
            log::info!("v2/tpl.reindex: start");
            let p: TplReindexParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::info!("v2/tpl.reindex: session={} duration={}", p.session, p.duration);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let indexed = db.tpl_reindex(&p.duration).map_err(|e| rpc_err(-32011, e))?;
                log::info!("v2/tpl.reindex: indexed={indexed}");
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "indexed": indexed }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            result
        })
        .unwrap();
}
