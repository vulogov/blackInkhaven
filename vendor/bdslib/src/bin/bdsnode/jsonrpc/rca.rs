use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_bucket_secs() -> u64   { 300 }
fn default_min_support()  -> usize { 2 }
fn default_jaccard()      -> f64   { 0.2 }
fn default_max_keys()     -> usize { 200 }

#[derive(serde::Deserialize)]
struct RcaParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    #[serde(default)]
    failure_key: Option<String>,
    #[serde(default = "default_bucket_secs")]
    bucket_secs: u64,
    #[serde(default = "default_min_support")]
    min_support: usize,
    #[serde(default = "default_jaccard")]
    jaccard_threshold: f64,
    #[serde(default = "default_max_keys")]
    max_keys: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/rca", |params, _ctx, _| async move {
            log::debug!("v2/rca: start");
            let p: RcaParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/rca: session={} duration={} failure_key={:?} bucket_secs={} jaccard_threshold={}",
                    p.session, p.duration, p.failure_key, p.bucket_secs, p.jaccard_threshold
                );

                humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let cfg = bdslib::RcaConfig {
                    bucket_secs:       p.bucket_secs,
                    min_support:       p.min_support,
                    jaccard_threshold: p.jaccard_threshold,
                    max_keys:          p.max_keys,
                };

                let rca_result = match &p.failure_key {
                    Some(fk) => bdslib::RcaResult::analyze_failure(fk, &p.duration, &cfg),
                    None     => bdslib::RcaResult::analyze(&p.duration, &cfg),
                }
                .map_err(|e| rpc_err(-32004, e))?;

                let value = serde_json::to_value(&rca_result)
                    .map_err(|e| rpc_err(-32004, format!("serialisation error: {e}")))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/rca: done");
            result
        })
        .unwrap();
}
