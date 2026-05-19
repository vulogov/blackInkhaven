use super::params::rpc_err;
use jsonrpsee::RpcModule;
use uuid::Uuid;

fn default_max_sentences() -> usize { 0 }    // 0 → use ratio
fn default_ratio()         -> f32   { 0.3 }
fn default_min_word_len()  -> usize { 2 }
fn default_damping()       -> f32   { 0.85 }
fn default_iters()         -> usize { 30 }
fn default_tolerance()     -> f32   { 1e-4 }

#[derive(serde::Deserialize)]
struct SummaryForRecentParams {
    /// Caller transaction UUIDv7.  Accepted for symmetry with other v2
    /// methods; not consulted internally.
    #[allow(dead_code)]
    session: String,

    /// Lookback window in humantime notation (e.g. `"1h"`, `"30min"`, `"7d"`).
    duration: String,

    #[serde(default = "default_max_sentences")]
    max_sentences: usize,
    #[serde(default = "default_ratio")]
    ratio: f32,
    #[serde(default = "default_min_word_len")]
    min_word_len: usize,
    #[serde(default = "default_damping")]
    damping: f32,
    #[serde(default = "default_iters")]
    iters: usize,
    #[serde(default = "default_tolerance")]
    tolerance: f32,
}

pub fn register(module: &mut RpcModule<()>) {
    module
        .register_async_method("v2/summary_for_recent", |params, _ctx, _| async move {
            log::debug!("v2/summary_for_recent: start");
            let p: SummaryForRecentParams = params.parse()?;

            let result = tokio::task::spawn_blocking(move || {
                log::debug!(
                    "v2/summary_for_recent: duration={} max_sentences={} ratio={}",
                    p.duration, p.max_sentences, p.ratio
                );

                let dur = humantime::parse_duration(&p.duration)
                    .map_err(|e| rpc_err(-32600, format!("invalid duration {:?}: {e}", p.duration)))?;

                let txn_id = Uuid::parse_str(&p.session).unwrap_or_else(|_| Uuid::nil());

                let cfg = bdslib::TextRankConfig {
                    max_sentences: p.max_sentences,
                    ratio:         p.ratio,
                    min_word_len:  p.min_word_len,
                    damping:       p.damping,
                    iters:         p.iters,
                    tolerance:     p.tolerance,
                };

                let db = bdslib::get_db().map_err(|e| rpc_err(-32001, e))?;

                let summary = db
                    .summary_for_recent(txn_id, dur, &cfg)
                    .map_err(|e| rpc_err(-32004, e))?;

                let value = serde_json::json!({
                    "duration":      p.duration,
                    "max_sentences": p.max_sentences,
                    "ratio":         p.ratio,
                    "summary":       summary,
                });

                Ok::<serde_json::Value, jsonrpsee::types::ErrorObject>(value)
            })
            .await
            .map_err(|e| rpc_err(-32000, format!("task panicked: {e}")))?;

            log::debug!("v2/summary_for_recent: done");
            result
        })
        .unwrap();
}
