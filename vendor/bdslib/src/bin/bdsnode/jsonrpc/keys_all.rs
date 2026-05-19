use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_key_pattern() -> String {
    "*".to_string()
}

#[derive(serde::Deserialize)]
struct KeysAllParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    #[serde(default = "default_key_pattern")]
    key: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/keys.all", |params, _ctx, _| async move {
            log::debug!("v2/keys.all: start");
            let p: KeysAllParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/keys.all: session={} duration={} key={}",
                    p.session, p.duration, p.key
                );

                humantime::parse_duration(&p.duration).map_err(|e| {
                    rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration))
                })?;

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let keys = db
                    .keys_all(&p.duration, &p.key)
                    .map_err(|e| rpc_err(-32004, e))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "keys": keys }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/keys.all: done");
            result
        })
        .unwrap();
}
