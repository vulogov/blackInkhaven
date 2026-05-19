use super::params::rpc_err;
use jsonrpsee::RpcModule;

#[derive(serde::Deserialize)]
struct AggregationSearchParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    query: String,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/aggregationsearch", |params, _ctx, _| async move {
            log::debug!("v2/aggregationsearch: start");
            let p: AggregationSearchParams = params.parse()?;
            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/aggregationsearch: session={} duration={:?} query={:?}",
                    p.session, p.duration, p.query
                );
                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;
                db.aggregationsearch(&p.duration, &p.query)
                    .map_err(|e| rpc_err(-32004, e))
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;
            log::debug!("v2/aggregationsearch: done");
            result
        })
        .unwrap();
}
