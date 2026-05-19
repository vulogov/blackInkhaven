use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use rust_dynamic::value::Value;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct ResultsPushParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
    /// UUIDv7 of the queue.  Auto-created (with a fresh creation timestamp)
    /// when no queue exists yet for this id.
    id:    String,
    /// Arbitrary JSON value to enqueue.  Wrapped server-side as a
    /// `rust_dynamic::Value` of type `JSON` and stored verbatim.
    value: serde_json::Value,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/results.push", |params, _ctx, _| async move {
            log::debug!("v2/results.push: start");
            let p: ResultsPushParams = params.parse()?;
            let id = Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid id {:?}: {e}", p.id)))?;
            let queues = bdslib::vm::results();
            queues.push(id, Value::json(p.value));
            let count = queues.len(id);
            log::debug!("v2/results.push: id={id} new_len={count}");
            Ok::<serde_json::Value, ErrorObject<'static>>(serde_json::json!({
                "id":    id.to_string(),
                "count": count,
            }))
        })
        .unwrap();
}
