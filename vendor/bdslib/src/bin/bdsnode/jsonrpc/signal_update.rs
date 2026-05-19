use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct SignalUpdateParams {
    #[allow(dead_code)]
    session:  String,
    id:       String,
    metadata: serde_json::Value,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/signal.update", |params, _ctx, _| async move {
            log::debug!("v2/signal.update: start");
            let p: SignalUpdateParams = params.parse()?;
            let id = Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32602, format!("invalid UUID: {e}")))?;
            let result = tokio::task::spawn_blocking(move || {
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.signal_update(id, p.metadata)
                    .map_err(|e| rpc_err(-32011, e))?;
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "ok": true }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/signal.update: done");
            result
        })
        .unwrap();
}
