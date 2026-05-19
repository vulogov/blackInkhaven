//! LSA summarisation over primary records.
//!
//! [`ShardsManager::summary_lsa_for_recent`] and
//! [`ShardsManager::summary_lsa_for_query`] mirror the TextRank equivalents in
//! [`crate::shardsmanager_primary_textrank`] but use Latent Semantic Analysis
//! (Steinberger & Ježek 2004) as the ranking algorithm.
//!
//! Body extraction rule (identical to the TextRank sibling):
//!
//! - If `data` is a bare number — **skip**, this is a numeric measurement.
//! - If `data["value"]` is a number — **skip**, same reason.
//! - Else read `data["value"]` if it is a non-empty string, falling back to
//!   `data["raw"]` if `value` is missing or non-string.
//! - Records that yield no body are excluded from the LSA input.

use crate::analysis::lsa::{lsa_summary_with, LsaConfig};
use crate::common::error::Result;
use crate::shard::Shard;
use crate::shardsmanager::ShardsManager;
use crate::shardsmanager_primary_textrank::extract_body;
use rayon::prelude::*;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

impl ShardsManager {
    /// Summarise text-bearing primary records observed in the last `lookback`
    /// window using LSA.
    ///
    /// All primary records whose `ts` falls in `[now − lookback, now)` are
    /// scanned across every shard that overlaps the window.  For each record,
    /// a body string is extracted via [`extract_body`]; numeric measurements
    /// (`data` is a bare number, or `data["value"]` is a number) are silently
    /// dropped.  The resulting list of bodies is fed to [`lsa_summary_with`].
    ///
    /// # Parameters
    /// - `_transaction_id` — UUIDv7 of the calling transaction; accepted for
    ///   parity with other query methods, not consulted internally.
    /// - `lookback` — how far back to look.  Convert humantime strings such
    ///   as `"1h"` via [`humantime::parse_duration`].
    /// - `cfg` — LSA tuning knobs.
    ///
    /// # Returns
    /// The LSA summary string.  Empty when the window contained no
    /// text-bearing primaries.
    pub fn summary_lsa_for_recent(
        &self,
        _transaction_id: Uuid,
        lookback: Duration,
        cfg: &LsaConfig,
    ) -> Result<String> {
        let now_secs = now_unix_secs();
        let start_secs = now_secs.saturating_sub(lookback.as_secs());

        let start_st = UNIX_EPOCH + Duration::from_secs(start_secs);
        let end_st = UNIX_EPOCH + Duration::from_secs(now_secs);

        let infos = self.cache.info().shards_in_range(start_st, end_st)?;
        let mut shards: Vec<Shard> = Vec::with_capacity(infos.len());
        for info in infos {
            shards.push(self.cache.shard(info.start_time)?);
        }

        let bodies: Vec<String> = if shards.len() <= 1 {
            let mut acc = Vec::new();
            for s in &shards {
                acc.extend(collect_bodies_in_range(s, start_st, end_st)?);
            }
            acc
        } else {
            let per_shard: Vec<Vec<String>> = shards
                .par_iter()
                .map(|s| collect_bodies_in_range(s, start_st, end_st))
                .collect::<Result<Vec<_>>>()?;
            let total: usize = per_shard.iter().map(|v| v.len()).sum();
            let mut acc = Vec::with_capacity(total);
            for v in per_shard {
                acc.extend(v);
            }
            acc
        };

        if bodies.is_empty() {
            return Ok(String::new());
        }
        Ok(lsa_summary_with(&bodies, cfg))
    }

    /// Summarise primary records that match a vector query using LSA.
    ///
    /// Runs a semantic vector search across every shard for `query`, then
    /// extracts text bodies from the matching records via the same rule as
    /// [`summary_lsa_for_recent`].
    ///
    /// Because the function takes no time window, it scans a generous default
    /// lookback (`365days`) — long enough to cover any realistic operational
    /// window while keeping the query bounded by the catalog.
    ///
    /// # Parameters
    /// - `_transaction_id` — UUIDv7 of the calling transaction; accepted for
    ///   parity with other query methods, not consulted internally.
    /// - `query` — plain-text vector query.
    /// - `cfg` — LSA tuning knobs.
    ///
    /// # Returns
    /// The LSA summary string.  Empty when no matching records had a text body.
    pub fn summary_lsa_for_query(
        &self,
        _transaction_id: Uuid,
        query: &str,
        cfg: &LsaConfig,
    ) -> Result<String> {
        let docs = self.search_vector(SUMMARY_LSA_QUERY_LOOKBACK, &json!(query))?;

        let bodies: Vec<String> = docs
            .iter()
            .filter_map(|doc| extract_body(doc.get("data").unwrap_or(&serde_json::Value::Null)))
            .collect();

        if bodies.is_empty() {
            return Ok(String::new());
        }
        Ok(lsa_summary_with(&bodies, cfg))
    }
}

const SUMMARY_LSA_QUERY_LOOKBACK: &str = "365days";

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn collect_bodies_in_range(
    shard: &Shard,
    start: SystemTime,
    end: SystemTime,
) -> Result<Vec<String>> {
    let rows = shard.observability().list_primaries_with_data_in_range(start, end)?;
    let mut out = Vec::with_capacity(rows.len());
    for (_id, _key, data) in rows {
        if let Some(body) = extract_body(&data) {
            out.push(body);
        }
    }
    Ok(out)
}
