use super::params::pipe_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct AddBatchParams {
    docs: Vec<serde_json::Value>,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/add.batch", |params, _ctx, _| async move {
            log::debug!("v2/add.batch: start");
            let p: AddBatchParams = params.parse()?;
            let n = p.docs.len();
            // Bulk-push all docs in a single helper call: the channel mutex
            // is taken once per item (instead of once per call site) and the
            // tokio worker is freed up sooner.
            bdslib::pipe::send_many("ingest", p.docs).map_err(pipe_err)?;
            log::debug!("v2/add.batch: done");
            Ok::<serde_json::Value, ErrorObject>(serde_json::json!({ "queued": n }))
        })
        .unwrap();
}
