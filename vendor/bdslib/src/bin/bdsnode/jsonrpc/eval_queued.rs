use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct EvalQueuedParams {
    #[allow(dead_code)]
    #[serde(default)]
    session: String,
    /// BUND script source to execute asynchronously in the worker pool.
    script: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/eval.queued", |params, _ctx, _| async move {
            log::debug!("v2/eval.queued: start");
            let p: EvalQueuedParams = params.parse()?;
            // submit_script is a non-blocking channel send — safe to call directly.
            let id = bdslib::submit_script(&p.script)
                .map_err(|e| rpc_err(-32002, e))?;
            log::debug!("v2/eval.queued: queued id={id}");
            Ok::<serde_json::Value, ErrorObject<'static>>(
                serde_json::json!({ "id": id.to_string() }),
            )
        })
        .unwrap();
}
