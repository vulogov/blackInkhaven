use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct ResultsEmptyParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
    /// UUIDv7 of the queue to inspect.
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/results.empty", |params, _ctx, _| async move {
            log::debug!("v2/results.empty: start");
            let p: ResultsEmptyParams = params.parse()?;
            let id = Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid id {:?}: {e}", p.id)))?;
            let queues = bdslib::vm::results();
            let count = queues.len(id);
            log::debug!("v2/results.empty: id={id} count={count}");
            Ok::<serde_json::Value, ErrorObject<'static>>(serde_json::json!({
                "id":    id.to_string(),
                "count": count,
                "empty": count == 0,
            }))
        })
        .unwrap();
}
