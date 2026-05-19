use super::params::rpc_err;
use jsonrpsee::RpcModule;
use uuid::Uuid;

fn default_max_sentences() -> usize { 0 }
fn default_ratio()         -> f32   { 0.3 }
fn default_min_word_len()  -> usize { 2 }
fn default_n_concepts()    -> usize { 3 }
fn default_power_iters()   -> usize { 50 }

#[derive(serde::Deserialize)]
struct SummaryLsaForQueryParams {
    #[allow(dead_code)]
    session: String,
    query: String,
    #[serde(default = "default_max_sentences")]
    max_sentences: usize,
    #[serde(default = "default_ratio")]
    ratio: f32,
    #[serde(default = "default_min_word_len")]
    min_word_len: usize,
    #[serde(default = "default_n_concepts")]
    n_concepts: usize,
    #[serde(default = "default_power_iters")]
    power_iters: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/summary_lsa_for_query", |params, _ctx, _| async move {
            log::debug!("v2/summary_lsa_for_query: start");
            let p: SummaryLsaForQueryParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/summary_lsa_for_query: query={:?} max_sentences={} ratio={}",
                    p.query, p.max_sentences, p.ratio
                );

                let txn_id = Uuid::parse_str(&p.session).unwrap_or_else(|_| Uuid::nil());

                let cfg = bdslib::LsaConfig {
                    max_sentences: p.max_sentences,
                    ratio:         p.ratio,
                    min_word_len:  p.min_word_len,
                    n_concepts:    p.n_concepts,
                    power_iters:   p.power_iters,
                };

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                let summary = db
                    .summary_lsa_for_query(txn_id, &p.query, &cfg)
                    .map_err(|e| rpc_err(-32004, e))?;

                let value = serde_json::json!({
                    "query":         p.query,
                    "max_sentences": p.max_sentences,
                    "ratio":         p.ratio,
                    "summary":       summary,
                });

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/summary_lsa_for_query: done");
            result
        })
        .unwrap();
}
