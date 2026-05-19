//! Template-storage helpers on [`ShardsManager`].
//!
//! Templates are stored inside the per-shard [`DocumentStorage`] at
//! `{shard_path}/tplstorage`, making them time-partitioned the same way
//! telemetry records are.
//!
//! ## Routing
//!
//! **Writes** (`tpl_add`, `tpl_update_*`) require the template metadata to
//! contain a numeric `"timestamp"` field (Unix seconds).  The timestamp is
//! used to route the record to the correct [`Shard`] via [`ShardsCache`],
//! matching the behaviour of [`ShardsManager::add`].
//!
//! **Point reads / deletes** (`tpl_get_*`, `tpl_delete`) first try a direct
//! shard lookup derived from the UUIDv7 creation timestamp (O(1)), then fall
//! back to a full catalog scan if the direct hit misses (e.g. backdated
//! ingestion where the document timestamp differs from wall-clock time).
//!
//! **Range queries** (`tpl_list`, `tpl_search_text`, `tpl_search_json`,
//! `tpl_reindex`) accept a `duration` lookback window and query only the
//! shards that overlap `[now − duration, now]`, mirroring
//! [`ShardsManager::search_fts`] and related methods.
//!
//! **Frequency-tracking queries** (`templates_recent`, `templates_by_timestamp`)
//! only scan shards that overlap the requested time window for the observation
//! phase, since `frequencytracking_observe` always writes to the current-time
//! shard.

use crate::common::error::{err_msg, Result};
use crate::common::time::{extract_timestamp, lookback_window};
use crate::shard::Shard;
use crate::shardsmanager::ShardsManager;
use rayon::prelude::*;
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::time::{Duration, UNIX_EPOCH};
use uuid::Uuid;

// ── ShardsManager impl ────────────────────────────────────────────────────────

impl ShardsManager {
    // ── writes ────────────────────────────────────────────────────────────────

    /// Store a template in the shard that covers its `"timestamp"` field.
    ///
    /// `metadata` must contain a numeric `"timestamp"` field (Unix seconds).
    /// Both metadata and body are automatically embedded and indexed in the
    /// shard's tplstorage vector index.  Returns the UUIDv7 of the stored
    /// template.
    pub fn tpl_add(&self, metadata: JsonValue, body: &[u8]) -> Result<Uuid> {
        let ts = extract_timestamp(&metadata)?;
        let shard = self.cache.shard(ts)?;
        shard.tpl_add(metadata, body)
    }

    /// Replace the metadata for template `id` and re-embed it.
    ///
    /// Tries a direct shard lookup via the UUIDv7 creation timestamp before
    /// falling back to a full catalog scan.  Returns an error if the template
    /// is not found in any shard.
    pub fn tpl_update_metadata(&self, id: Uuid, metadata: JsonValue) -> Result<()> {
        if let Some(shard) = self.shard_for_uuid(id) {
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_update_metadata(id, metadata);
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_update_metadata(id, metadata);
            }
        }
        Err(err_msg(format!("template {id} not found in any shard")))
    }

    /// Replace the body of template `id` and re-embed it.
    ///
    /// Tries a direct shard lookup before falling back to a full catalog scan.
    pub fn tpl_update_body(&self, id: Uuid, body: &[u8]) -> Result<()> {
        if let Some(shard) = self.shard_for_uuid(id) {
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_update_body(id, body);
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_update_body(id, body);
            }
        }
        Err(err_msg(format!("template {id} not found in any shard")))
    }

    /// Remove template `id` from whichever shard contains it.
    ///
    /// Returns `Ok(())` if no shard contains the record.
    pub fn tpl_delete(&self, id: Uuid) -> Result<()> {
        if let Some(shard) = self.shard_for_uuid(id) {
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_delete(id);
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_delete(id);
            }
        }
        Ok(())
    }

    // ── reads ─────────────────────────────────────────────────────────────────

    /// Return the JSON metadata for template `id`.
    ///
    /// Tries a direct shard lookup before falling back to a full catalog scan.
    /// Returns `None` if no shard contains a template with that UUID.
    pub fn tpl_get_metadata(&self, id: Uuid) -> Result<Option<JsonValue>> {
        if let Some(shard) = self.shard_for_uuid(id) {
            if let Some(meta) = shard.tpl_get_metadata(id)? {
                return Ok(Some(meta));
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if let Some(meta) = shard.tpl_get_metadata(id)? {
                return Ok(Some(meta));
            }
        }
        Ok(None)
    }

    /// Return the raw body bytes for template `id`.
    ///
    /// Tries a direct shard lookup before falling back to a full catalog scan.
    /// Returns `None` if no shard contains a template with that UUID.
    pub fn tpl_get_body(&self, id: Uuid) -> Result<Option<Vec<u8>>> {
        if let Some(shard) = self.shard_for_uuid(id) {
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_get_body(id);
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if shard.tpl_get_metadata(id)?.is_some() {
                return shard.tpl_get_body(id);
            }
        }
        Ok(None)
    }

    // ── range queries ─────────────────────────────────────────────────────────

    /// Return all templates stored in shards that overlap
    /// `[now − duration, now]`, as `(id, metadata)` pairs.
    ///
    /// Results are merged from all matching shards.  When the lookback window
    /// overlaps more than one shard, the per-shard reads run in parallel via
    /// rayon; single-shard windows take the serial path to avoid the work-pool
    /// overhead.
    pub fn tpl_list(&self, duration: &str) -> Result<Vec<(Uuid, JsonValue)>> {
        let (start, end) = lookback_window(duration)?;
        let shards = self.resolve_shards_in_range(start, end)?;
        if shards.len() <= 1 {
            let mut out = Vec::new();
            for shard in shards {
                out.extend(shard.tpl_list()?);
            }
            return Ok(out);
        }
        let per_shard: Vec<Vec<(Uuid, JsonValue)>> = shards
            .par_iter()
            .map(|s| s.tpl_list())
            .collect::<Result<Vec<_>>>()?;
        let total: usize = per_shard.iter().map(|v| v.len()).sum();
        let mut out = Vec::with_capacity(total);
        for v in per_shard { out.extend(v); }
        Ok(out)
    }

    /// Semantic search over templates in shards overlapping
    /// `[now − duration, now]`, using a plain-text query.
    ///
    /// Results from all matching shards are merged and sorted by score
    /// descending, then truncated to `limit`.  Multi-shard windows fan out
    /// to a parallel rayon scan; single-shard windows stay serial.
    pub fn tpl_search_text(
        &self,
        duration: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        let (start, end) = lookback_window(duration)?;
        let shards = self.resolve_shards_in_range(start, end)?;
        let mut results: Vec<JsonValue> = if shards.len() <= 1 {
            let mut acc = Vec::new();
            for shard in shards {
                acc.extend(shard.tpl_search_text(query, limit)?);
            }
            acc
        } else {
            let per_shard: Vec<Vec<JsonValue>> = shards
                .par_iter()
                .map(|s| s.tpl_search_text(query, limit))
                .collect::<Result<Vec<_>>>()?;
            let total: usize = per_shard.iter().map(|v| v.len()).sum();
            let mut acc = Vec::with_capacity(total);
            for v in per_shard { acc.extend(v); }
            acc
        };
        results.sort_by(|a, b| {
            let sa = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let sb = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Semantic search over templates in shards overlapping
    /// `[now − duration, now]`, using a JSON query object.
    ///
    /// Results are merged, sorted by score descending, and truncated to `limit`.
    /// Multi-shard windows fan out to a parallel rayon scan; single-shard
    /// windows stay serial.
    pub fn tpl_search_json(
        &self,
        duration: &str,
        query: &JsonValue,
        limit: usize,
    ) -> Result<Vec<JsonValue>> {
        let (start, end) = lookback_window(duration)?;
        let shards = self.resolve_shards_in_range(start, end)?;
        let mut results: Vec<JsonValue> = if shards.len() <= 1 {
            let mut acc = Vec::new();
            for shard in shards {
                acc.extend(shard.tpl_search_json(query, limit)?);
            }
            acc
        } else {
            let per_shard: Vec<Vec<JsonValue>> = shards
                .par_iter()
                .map(|s| s.tpl_search_json(query, limit))
                .collect::<Result<Vec<_>>>()?;
            let total: usize = per_shard.iter().map(|v| v.len()).sum();
            let mut acc = Vec::with_capacity(total);
            for v in per_shard { acc.extend(v); }
            acc
        };
        results.sort_by(|a, b| {
            let sa = a.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let sb = b.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);
            sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(limit);
        Ok(results)
    }

    /// Rebuild the tplstorage vector index for every shard overlapping
    /// `[now − duration, now]`.
    ///
    /// Returns the total number of templates re-indexed across all shards.
    /// Multi-shard windows rebuild in parallel via rayon; single-shard
    /// windows stay serial.
    pub fn tpl_reindex(&self, duration: &str) -> Result<usize> {
        let (start, end) = lookback_window(duration)?;
        let shards = self.resolve_shards_in_range(start, end)?;
        if shards.len() <= 1 {
            let mut total = 0usize;
            for shard in shards {
                total += shard.tpl_reindex()?;
            }
            return Ok(total);
        }
        let counts: Vec<usize> = shards
            .par_iter()
            .map(|s| s.tpl_reindex())
            .collect::<Result<Vec<_>>>()?;
        Ok(counts.into_iter().sum())
    }

    /// Resolve every shard whose `[start, end)` interval overlaps
    /// `[start_ts, end_ts]`, in catalog order.
    ///
    /// `cache.shard()` is taken serially since it briefly holds the LRU mutex —
    /// running cache lookups in parallel just contends on that mutex.  The
    /// per-shard work that the caller does is the part worth parallelising.
    fn resolve_shards_in_range(
        &self,
        start: std::time::SystemTime,
        end: std::time::SystemTime,
    ) -> Result<Vec<Shard>> {
        let infos = self.cache.info().shards_in_range(start, end)?;
        let mut out = Vec::with_capacity(infos.len());
        for info in infos {
            out.push(self.cache.shard(info.start_time)?);
        }
        Ok(out)
    }

    // ── frequency-tracking queries ────────────────────────────────────────────

    /// Return the full template record for `id`, scanning all registered shards.
    ///
    /// The returned JSON object has three keys:
    /// - `"id"`: the UUID string
    /// - `"metadata"`: the stored template metadata
    /// - `"body"`: the template content decoded as UTF-8
    ///
    /// Returns `None` if no shard contains a template with that UUID.
    /// Returns `Err` if `id` is not a valid UUID string.
    pub fn template_by_id(&self, id: &str) -> Result<Option<JsonValue>> {
        let uuid = Uuid::parse_str(id)
            .map_err(|e| err_msg(format!("invalid template id '{id}': {e}")))?;
        if let Some(shard) = self.shard_for_uuid(uuid) {
            if let Some(metadata) = shard.tpl_get_metadata(uuid)? {
                let body = shard.tpl_get_body(uuid)?.unwrap_or_default();
                return Ok(Some(serde_json::json!({
                    "id":       id,
                    "metadata": metadata,
                    "body":     String::from_utf8_lossy(&body).into_owned(),
                })));
            }
        }
        for info in self.cache.info().list_all()? {
            let shard = self.cache.shard(info.start_time)?;
            if let Some(metadata) = shard.tpl_get_metadata(uuid)? {
                let body = shard.tpl_get_body(uuid)?.unwrap_or_default();
                return Ok(Some(serde_json::json!({
                    "id":       id,
                    "metadata": metadata,
                    "body":     String::from_utf8_lossy(&body).into_owned(),
                })));
            }
        }
        Ok(None)
    }

    /// Return all templates whose FrequencyTracking observation falls in the
    /// inclusive interval `[start, end]` (Unix seconds).
    ///
    /// Only queries shards that overlap `[start, end]` — since observations
    /// are always written to the current-time shard, old shards cannot contain
    /// observations in a future time window.
    pub fn templates_by_timestamp(&self, start: u64, end: u64) -> Result<Vec<JsonValue>> {
        let start_st = UNIX_EPOCH + Duration::from_secs(start);
        let end_st   = UNIX_EPOCH + Duration::from_secs(end);
        let mut ids: HashSet<String> = HashSet::new();
        for info in self.cache.info().shards_in_range(start_st, end_st)? {
            let shard = self.cache.shard(info.start_time)?;
            for id_str in shard.tplstorage.frequencytracking_time_range(start, end)? {
                ids.insert(id_str);
            }
        }
        self.resolve_template_ids(ids)
    }

    /// Return all templates observed within the humantime `duration` window.
    ///
    /// Only queries shards that overlap `[now - duration, now]` for the
    /// frequency-tracking phase — observations are written to the current-time
    /// shard, so old shards can never contain recent observations.  Template
    /// metadata is then resolved via the UUIDv7 creation timestamp (O(1) per
    /// template) with a full-catalog fallback for backdated ingestion.
    pub fn templates_recent(&self, duration: &str) -> Result<Vec<JsonValue>> {
        let (start, end) = lookback_window(duration)?;
        let s = start.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let e = end.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs();
        let mut ids: HashSet<String> = HashSet::new();
        for info in self.cache.info().shards_in_range(start, end)? {
            let shard = self.cache.shard(info.start_time)?;
            for id_str in shard.tplstorage.frequencytracking_time_range(s, e)? {
                ids.insert(id_str);
            }
        }
        self.resolve_template_ids(ids)
    }

    /// Resolve a set of template UUID strings to full `{id, metadata, body}` records.
    ///
    /// For each UUID, first attempts a direct shard lookup using the creation
    /// timestamp embedded in the UUIDv7 (O(1)).  Any IDs not found that way
    /// (e.g. from backdated ingestion where wall-clock ≠ document timestamp)
    /// are resolved via a full catalog scan with early termination.
    fn resolve_template_ids(&self, ids: HashSet<String>) -> Result<Vec<JsonValue>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut out = Vec::new();
        let mut not_found: HashSet<String> = HashSet::new();

        for id_str in &ids {
            let uuid = match Uuid::parse_str(id_str) {
                Ok(u) => u,
                Err(_) => { not_found.insert(id_str.clone()); continue; }
            };
            let found = if let Some(shard) = self.shard_for_uuid(uuid) {
                match shard.tpl_get_metadata(uuid)? {
                    Some(metadata) => {
                        let body = shard.tpl_get_body(uuid)?.unwrap_or_default();
                        out.push(serde_json::json!({
                            "id":       id_str,
                            "metadata": metadata,
                            "body":     String::from_utf8_lossy(&body).into_owned(),
                        }));
                        true
                    }
                    None => false,
                }
            } else {
                false
            };
            if !found {
                not_found.insert(id_str.clone());
            }
        }

        if !not_found.is_empty() {
            for info in self.cache.info().list_all()? {
                if not_found.is_empty() { break; }
                let shard = self.cache.shard(info.start_time)?;
                let candidates: Vec<String> = not_found.iter().cloned().collect();
                for id_str in candidates {
                    if let Ok(uuid) = Uuid::parse_str(&id_str) {
                        if let Some(metadata) = shard.tpl_get_metadata(uuid)? {
                            let body = shard.tpl_get_body(uuid)?.unwrap_or_default();
                            out.push(serde_json::json!({
                                "id":       id_str,
                                "metadata": metadata,
                                "body":     String::from_utf8_lossy(&body).into_owned(),
                            }));
                            not_found.remove(&uuid.to_string());
                        }
                    }
                }
            }
        }

        Ok(out)
    }

    /// Attempt a direct shard lookup for a UUIDv7 using its embedded creation timestamp.
    ///
    /// Returns `None` if the UUID has no v7 timestamp or if `cache.shard()` fails
    /// (e.g. the computed shard interval is not in the catalog).
    fn shard_for_uuid(&self, id: Uuid) -> Option<crate::shard::Shard> {
        let ts = id.get_timestamp()?;
        let (secs, _) = ts.to_unix();
        let t = UNIX_EPOCH + Duration::from_secs(secs);
        self.cache.shard(t).ok()
    }
}

