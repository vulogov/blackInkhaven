use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct EvalParams {
    context: String,
    script: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/eval", |params, _ctx, _| async move {
            log::debug!("v2/eval: start");
            let p: EvalParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                // Acquire (or lazily create) the named BUND VM instance.
                let mut guard = bdslib::context::get(&p.context)
                    .map_err(|e| rpc_err(-32001, e))?;

                // Run the script against the VM.
                bdslib::vm::helpers::eval::bund_compile_and_eval(
                    &mut guard.vm,
                    p.script,
                )
                .map_err(|e| rpc_err(-32002, e))?;

                // Pull the last value pushed to the workbench.
                let result: serde_json::Value = guard
                    .vm
                    .stack
                    .workbench
                    .stack
                    .pop_back()
                    .map(bdslib::vm::helpers::eval::dynamic_to_json)
                    .unwrap_or(serde_json::Value::Null);

                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "result": result }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/eval: done");
            result
        })
        .unwrap();
}
