use super::params::rpc_err;
use jsonrpsee::types::ErrorObject;
use jsonrpsee::RpcModule;
use uuid::Uuid;

fn default_n()                 -> usize { 2 }
fn default_min_word_len()      -> usize { 2 }
fn default_anomaly_threshold() -> f32   { 0.7 }
fn default_max_anomalies()     -> usize { 20 }
fn default_max_novel_ngrams()  -> usize { 5 }

#[derive(serde::Deserialize)]
struct AnomalyRecentParams {
    /// Caller transaction UUIDv7.  Accepted for symmetry with other v2
    /// methods; not consulted internally.
    #[allow(dead_code)]
    #[serde(default)]
    session: String,

    /// Lookback window in humantime notation (e.g. `"1h"`, `"30min"`,
    /// `"7d"`).  Required.
    duration: String,

    #[serde(default = "default_n")]
    n: usize,
    #[serde(default = "default_min_word_len")]
    min_word_len: usize,
    #[serde(default = "default_anomaly_threshold")]
    anomaly_threshold: f32,
    #[serde(default = "default_max_anomalies")]
    max_anomalies: usize,
    #[serde(default = "default_max_novel_ngrams")]
    max_novel_ngrams: usize,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/anomaly.recent", |params, _ctx, _| async move {
            log::debug!("v2/anomaly.recent: start");
            let p: AnomalyRecentParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/anomaly.recent: duration={} n={} threshold={}",
                    p.duration, p.n, p.anomaly_threshold
                );

                let dur = humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let txn_id = Uuid::parse_str(&p.session).unwrap_or_else(|_| Uuid::nil());

                let cfg = bdslib::NgramAnomalyConfig {
                    n:                 p.n,
                    min_word_len:      p.min_word_len,
                    anomaly_threshold: p.anomaly_threshold,
                    max_anomalies:     p.max_anomalies,
                    max_novel_ngrams:  p.max_novel_ngrams,
                };

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                let value = db
                    .ngram_anomaly_recent(txn_id, dur, &cfg)
                    .map_err(|e| rpc_err(-32004, e))?;

                Ok::<serde_json::Value, ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/anomaly.recent: done");
            result
        })
        .unwrap();
}
