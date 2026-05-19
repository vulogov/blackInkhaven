use super::params::{pipe_err, rpc_err};
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct AddParams {
    doc: serde_json::Value,
    /// When `true`, bypass the ingest queue and call `ShardsManager::add`
    /// directly on the calling thread.  The response carries the
    /// assigned UUIDv7 instead of the bare queue acknowledgement.
    /// Default `false` (queued, fire-and-forget — historical behaviour).
    #[serde(default)]
    sync: bool,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/add", |params, _ctx, _| async move {
            log::debug!("v2/add: start");
            let p: AddParams = params.parse()?;

            if p.sync {
                // Synchronous path — the call returns once the record has
                // been classified, indexed, and assigned a UUID. Useful
                // for callers that need the id in the same response (e.g.
                // CLI scripts that pipe the id into the next command).
                //
                // ShardsManager::add does sync DuckDB + Tantivy + ONNX
                // work, so we move it to a blocking thread to avoid
                // tying up the tokio runtime.
                let result = tokio::task::spawn_blocking(move || {
                    let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                    let id = db.add(p.doc).map_err(|e| rpc_err(-32004, e))?;
                    Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                        "id":     id.to_string(),
                        "synced": true,
                    }))
                })
                .await
                .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
                log::debug!("v2/add: done (sync)");
                return result;
            }

            bdslib::pipe::send("ingest", p.doc).map_err(pipe_err)?;
            log::debug!("v2/add: done (queued)");
            Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "queued": 1 }))
        })
        .unwrap();
}
