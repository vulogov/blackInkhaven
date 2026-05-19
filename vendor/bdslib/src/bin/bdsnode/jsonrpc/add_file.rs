use super::params::{pipe_err, rpc_err};
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct AddFileParams {
    #[allow(dead_code)]
    session: String,
    path: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/add.file", |params, _ctx, _| async move {
            log::debug!("v2/add.file: start");
            let p: AddFileParams = params.parse()?;

            let meta = std::fs::metadata(&p.path).map_err(|e| {
                rpc_err(-32600, format!("cannot access {:?}: {e}", p.path))
            })?;

            if !meta.is_file() {
                return Err(rpc_err(-32600, format!("{:?} is not a regular file", p.path)));
            }
            if meta.len() == 0 {
                return Err(rpc_err(-32600, format!("{:?} is empty", p.path)));
            }

            // Verify read access by opening the file.
            std::fs::File::open(&p.path).map_err(|e| {
                rpc_err(-32600, format!("cannot open {:?}: {e}", p.path))
            })?;

            bdslib::pipe::send("ingest_file", serde_json::json!(p.path))
                .map_err(pipe_err)?;

            log::debug!("v2/add.file: queued {:?}", p.path);
            Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "queued": p.path }))
        })
        .unwrap();
}
