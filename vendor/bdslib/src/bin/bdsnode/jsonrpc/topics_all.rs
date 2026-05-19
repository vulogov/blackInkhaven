use super::params::rpc_err;
use jsonrpsee::RpcModule;

fn default_k() -> usize { 3 }
fn default_alpha() -> f64 { 0.1 }
fn default_beta() -> f64 { 0.01 }
fn default_seed() -> u64 { 42 }
fn default_iters() -> usize { 200 }
fn default_top_n() -> usize { 10 }

#[derive(serde::Deserialize)]
struct TopicsAllParams {
    #[allow(dead_code)]
    session: String,
    duration: String,
    #[serde(default = "default_k")]
    k: usize,
    #[serde(default = "default_alpha")]
    alpha: f64,
    #[serde(default = "default_beta")]
    beta: f64,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_iters")]
    iters: usize,
    #[serde(default = "default_top_n")]
    top_n: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/topics.all", |params, _ctx, _| async move {
            log::debug!("v2/topics.all: start");
            let p: TopicsAllParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/topics.all: session={} duration={} k={} iters={}",
                    p.session, p.duration, p.k, p.iters
                );

                humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let config = bdslib::LdaConfig {
                    k: p.k,
                    alpha: p.alpha,
                    beta: p.beta,
                    seed: p.seed,
                    iters: p.iters,
                    top_n: p.top_n,
                };

                let summaries = bdslib::TopicSummary::query_all_keys(&p.duration, config)
                    .map_err(|e| rpc_err(-32004, e))?;

                let topics: Vec<serde_json::Value> = summaries
                    .into_iter()
                    .map(|s| serde_json::to_value(&s).unwrap_or(serde_json::Value::Null))
                    .collect();

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(
                    serde_json::json!({ "topics": topics }),
                )
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/topics.all: done");
            result
        })
        .unwrap();
}
