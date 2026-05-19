use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use rust_dynamic::value::Value;
use uuid::Uuid;

#[derive(serde::Deserialize)]
struct ResultsPullParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
    /// UUIDv7 of the queue.
    id: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/results.pull", |params, _ctx, _| async move {
            log::debug!("v2/results.pull: start");
            let p: ResultsPullParams = params.parse()?;
            let id = Uuid::parse_str(&p.id)
                .map_err(|e| rpc_err(-32600, format!("invalid id {:?}: {e}", p.id)))?;
            let queues = bdslib::vm::results();
            let popped = queues.pop(id);
            let remaining = queues.len(id);
            let value = popped.map(value_to_json).unwrap_or(serde_json::Value::Null);
            log::debug!("v2/results.pull: id={id} remaining={remaining}");
            Ok::<serde_json::Value, ErrorObject<'static>>(serde_json::json!({
                "id":        id.to_string(),
                "value":     value,
                "remaining": remaining,
            }))
        })
        .unwrap();
}

/// Convert any `rust_dynamic::Value` into a `serde_json::Value`.
///
/// JSON-typed Values (the shape produced by `v2/results.push`) round-trip
/// directly via `cast_json`.  Other types fall through to
/// `cast_value_to_json`, which serialises numbers, strings, lists and dicts.
/// Anything else becomes `null` so the response stays JSON-clean.
fn value_to_json(v: Value) -> serde_json::Value {
    if let Ok(j) = v.cast_json() {
        return j;
    }
    v.cast_value_to_json().unwrap_or(serde_json::Value::Null)
}
