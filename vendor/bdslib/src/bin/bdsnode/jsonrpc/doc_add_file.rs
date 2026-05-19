use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

fn default_slice() -> usize {
    512
}

fn default_overlap() -> f32 {
    20.0
}

#[derive(serde::Deserialize)]
struct DocAddFileParams {
    #[allow(dead_code)]
    session: String,
    path: String,
    name: String,
    #[serde(default = "default_slice")]
    slice: usize,
    #[serde(default = "default_overlap")]
    overlap: f32,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/doc.add.file", |params, _ctx, _| async move {
            log::debug!("v2/doc.add.file: start");
            let p: DocAddFileParams = params.parse()?;

            let meta = std::fs::metadata(&p.path)
                .map_err(|e| rpc_err(-32600, format!("cannot access {:?}: {e}", p.path)))?;
            if !meta.is_file() {
                return Err(rpc_err(-32600, format!("{:?} is not a regular file", p.path)));
            }
            std::fs::File::open(&p.path)
                .map_err(|e| rpc_err(-32600, format!("cannot open {:?}: {e}", p.path)))?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!("v2/doc.add.file: session={} path={:?}", p.session, p.path);
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                let doc_id = db
                    .doc_add_from_file(&p.path, &p.name, p.slice, p.overlap)
                    .map_err(|e| rpc_err(-32011, e))?;
                db.doc_sync().map_err(|e| rpc_err(-32011, e))?;
                let n_chunks_value = db
                    .doc_get_metadata(doc_id)
                    .map_err(|e| rpc_err(-32011, e))?
                    .map(|m| m.get("n_chunks").cloned().unwrap_or(serde_json::Value::Null))
                    .unwrap_or(serde_json::Value::Null);
                Ok::<serde_json::Value, ErrorObject>(serde_json::json!({
                    "id": doc_id.to_string(),
                    "n_chunks": n_chunks_value,
                }))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/doc.add.file: done");
            result
        })
        .unwrap();
}
