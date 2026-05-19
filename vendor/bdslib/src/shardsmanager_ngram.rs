//! N-gram intelligence over recent primary records.
//!
//! [`ShardsManager::ngram_anomaly_recent`] and
//! [`ShardsManager::ngram_denoise_recent`] are thin shard-aware wrappers
//! around [`crate::analysis::ngram::ngram_anomaly_with`] and
//! [`crate::analysis::ngram::ngram_remove_noise_with`].
//!
//! Both walk every shard that overlaps the lookback window
//! `[now − lookback, now)`, fingerprint each primary record (key +
//! `json_fingerprint(data)` — the same recipe used by the LDA pipeline,
//! see [`crate::analysis::latentdirichletallocation`]), and feed the
//! resulting strings to the chosen n-gram endpoint. The JSON returned
//! by the analysis function is passed through verbatim.
//!
//! Why fingerprint instead of extracting a text body? The n-gram
//! anomaly / noise endpoints derive their signal from **phrase
//! structure**, and `json_fingerprint` exposes that structure directly:
//! every JSON leaf becomes a `"field: value"` token pair, so the n-gram
//! analyser sees both schema (field names) and payload (values) without
//! losing the relationship between them.

use crate::analysis::knn::{knn_summary_with, KnnConfig};
use crate::analysis::ngram::{
    ngram_anomaly_with, ngram_remove_noise_with, NgramAnomalyConfig, NgramNoiseConfig,
};
use crate::common::error::Result;
use crate::common::jsonfingerprint::json_fingerprint;
use crate::shard::Shard;
use crate::shardsmanager::ShardsManager;
use rayon::prelude::*;
use serde_json::Value as JsonValue;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

impl ShardsManager {
    /// Fetch every primary record observed in `[now − lookback, now)`,
    /// fingerprint each as `"<key>  <json_fingerprint(data)>"`, and run
    /// n-gram anomaly detection on the resulting strings.
    ///
    /// Returns the JSON value produced by [`ngram_anomaly_with`] verbatim
    /// — see `Documentation/Algorithm/NGRAM_ANOMALY.md` for the output
    /// shape.
    ///
    /// # Parameters
    ///
    /// - `_transaction_id` — UUIDv7 of the calling transaction; accepted
    ///   for parity with other v2 methods, not consulted internally.
    /// - `lookback` — how far back to look. Convert humantime strings
    ///   such as `"1h"` via [`humantime::parse_duration`].
    /// - `cfg` — n-gram-anomaly tuning knobs.
    pub fn ngram_anomaly_recent(
        &self,
        _transaction_id: Uuid,
        lookback: Duration,
        cfg: &NgramAnomalyConfig,
    ) -> Result<JsonValue> {
        let fingerprints = self.collect_fingerprints_in_recent(lookback)?;
        Ok(ngram_anomaly_with(&fingerprints, cfg))
    }

    /// Fetch every primary record observed in `[now − lookback, now)`,
    /// fingerprint each, and run n-gram noise removal on the resulting
    /// strings.
    ///
    /// Returns the JSON value produced by [`ngram_remove_noise_with`]
    /// verbatim — see `Documentation/Algorithm/NGRAM_NOISE.md` for the
    /// output shape.
    pub fn ngram_denoise_recent(
        &self,
        _transaction_id: Uuid,
        lookback: Duration,
        cfg: &NgramNoiseConfig,
    ) -> Result<JsonValue> {
        let fingerprints = self.collect_fingerprints_in_recent(lookback)?;
        Ok(ngram_remove_noise_with(&fingerprints, cfg))
    }

    /// Fetch every primary record observed in `[now − lookback, now)`,
    /// fingerprint each as `"<key>  <json_fingerprint(data)>"`, and run
    /// k-NN intelligence on the resulting strings.
    ///
    /// Reuses the same fingerprinting pipeline as
    /// [`ngram_anomaly_recent`](Self::ngram_anomaly_recent) and
    /// [`ngram_denoise_recent`](Self::ngram_denoise_recent), so the three
    /// endpoints operate on identical input — only the analysis algorithm
    /// differs.  Returns the JSON value produced by [`knn_summary_with`]
    /// verbatim — see `Documentation/Algorithm/KNN.md` for the output
    /// shape (clusters, anomalies, density-ranked representatives).
    ///
    /// # Parameters
    ///
    /// - `_transaction_id` — UUIDv7 of the calling transaction; accepted
    ///   for parity with other v2 methods, not consulted internally.
    /// - `lookback` — how far back to look. Convert humantime strings
    ///   such as `"1h"` via [`humantime::parse_duration`].
    /// - `cfg` — k-NN tuning knobs.
    pub fn knn_recent(
        &self,
        _transaction_id: Uuid,
        lookback: Duration,
        cfg: &KnnConfig,
    ) -> Result<JsonValue> {
        let fingerprints = self.collect_fingerprints_in_recent(lookback)?;
        Ok(knn_summary_with(&fingerprints, cfg))
    }

    /// Walk every shard that overlaps the lookback window and return
    /// one fingerprint string per primary record found there.
    fn collect_fingerprints_in_recent(&self, lookback: Duration) -> Result<Vec<String>> {
        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let start_secs = now_secs.saturating_sub(lookback.as_secs());

        let start_st = UNIX_EPOCH + Duration::from_secs(start_secs);
        let end_st = UNIX_EPOCH + Duration::from_secs(now_secs);

        let infos = self.cache.info().shards_in_range(start_st, end_st)?;
        let mut shards: Vec<Shard> = Vec::with_capacity(infos.len());
        for info in infos {
            shards.push(self.cache.shard(info.start_time)?);
        }

        // Per-shard scan — parallel for multi-shard windows, serial for one.
        let fingerprints: Vec<String> = if shards.len() <= 1 {
            let mut acc = Vec::new();
            for s in &shards {
                acc.extend(collect_fingerprints_in_range(s, start_st, end_st)?);
            }
            acc
        } else {
            let per_shard: Vec<Vec<String>> = shards
                .par_iter()
                .map(|s| collect_fingerprints_in_range(s, start_st, end_st))
                .collect::<Result<Vec<_>>>()?;
            let total: usize = per_shard.iter().map(|v| v.len()).sum();
            let mut acc = Vec::with_capacity(total);
            for v in per_shard {
                acc.extend(v);
            }
            acc
        };

        Ok(fingerprints)
    }
}

/// Collect a fingerprint for every primary record in the shard whose
/// `ts` falls in `[start, end)`.
fn collect_fingerprints_in_range(
    shard: &Shard,
    start: SystemTime,
    end: SystemTime,
) -> Result<Vec<String>> {
    let rows = shard
        .observability()
        .list_primaries_with_data_in_range(start, end)?;
    let mut out = Vec::with_capacity(rows.len());
    for (_id, key, data) in rows {
        let fp = record_to_fingerprint(&key, &data);
        if !fp.trim().is_empty() {
            out.push(fp);
        }
    }
    Ok(out)
}

/// Build a single fingerprint string combining the record's key with the
/// JSON fingerprint of its `data` payload.
///
/// The key has its `.`/`_`/`-` separators replaced by spaces so the
/// n-gram tokeniser sees its components as separate tokens
/// (`cpu.usage` → `"cpu usage"` → tokens `cpu`, `usage`). The
/// `json_fingerprint(data)` flattening exposes every JSON leaf as a
/// `"field: value"` pair, giving the n-gram analyser both schema and
/// content signal in one string.
///
/// Identical recipe to the LDA pipeline's `doc_to_text`, kept consistent
/// so the two analysis families operate on comparable input.
fn record_to_fingerprint(key: &str, data: &JsonValue) -> String {
    let key_part = key.replace(['.', '_', '-'], " ");
    let data_fp = json_fingerprint(data);
    match (key_part.is_empty(), data_fp.is_empty()) {
        (true, _) => data_fp,
        (_, true) => key_part,
        _ => format!("{key_part}  {data_fp}"),
    }
}
