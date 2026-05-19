use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct ResultsLenParams {
    /// Caller transaction UUIDv7. Accepted for symmetry with other v2 methods;
    /// not consulted internally.
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/results.len", |params, _ctx, _| async move {
            log::debug!("v2/results.len: start");
            let _p: ResultsLenParams = params.parse().unwrap_or(ResultsLenParams { session: String::new() });
            let queues = bdslib::vm::results();
            let count = queues.n_queues();
            let ids: Vec<String> = queues.ids().into_iter().map(|u| u.to_string()).collect();
            log::debug!("v2/results.len: {count} queue(s)");
            Ok::<serde_json::Value, ErrorObject<'static>>(serde_json::json!({
                "count": count,
                "ids":   ids,
            }))
        })
        .unwrap();
}
