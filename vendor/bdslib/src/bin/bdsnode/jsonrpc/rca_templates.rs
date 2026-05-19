use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_bucket_secs() -> u64   { 300 }
fn default_min_support()  -> usize { 2 }
fn default_jaccard()      -> f64   { 0.2 }
fn default_max_keys()     -> usize { 200 }

#[derive(serde::Deserialize)]
struct RcaTemplatesParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    #[serde(default)]
    failure_body: Option<String>,
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
        .register_async_method("v2/rca.templates", |params, _ctx, _| async move {
            log::debug!("v2/rca.templates: start");
            let p: RcaTemplatesParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/rca.templates: duration={} failure_body={:?} bucket_secs={} jaccard={}",
                    p.duration, p.failure_body, p.bucket_secs, p.jaccard_threshold
                );

                humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let cfg = bdslib::RcaTemplatesConfig {
                    bucket_secs:       p.bucket_secs,
                    min_support:       p.min_support,
                    jaccard_threshold: p.jaccard_threshold,
                    max_keys:          p.max_keys,
                };

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                let rca_result = match &p.failure_body {
                    Some(fb) => bdslib::RcaTemplatesResult::analyze_failure(db, fb, &p.duration, &cfg),
                    None     => bdslib::RcaTemplatesResult::analyze(db, &p.duration, &cfg),
                }
                .map_err(|e| rpc_err(-32004, e))?;

                let value = serde_json::to_value(&rca_result)
                    .map_err(|e| rpc_err(-32004, format!("serialisation error: {e}")))?;

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/rca.templates: done");
            result
        })
        .unwrap();
}
