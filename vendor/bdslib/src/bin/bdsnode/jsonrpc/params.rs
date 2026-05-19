use jsonrpsee::types::ErrorObject;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

/// Fetch exact-match duplication timestamps for `id` from `obs` and convert
/// them to a sorted list of Unix seconds.  Returns an empty vec on any error
/// so callers never need to propagate this as a fatal failure.
pub fn duplication_timestamps(obs: &bdslib::ObservabilityStorage, id: Uuid) -> Vec<u64> {
    obs.get_duplicate_timestamps_by_id(id)
        .unwrap_or_default()
        .iter()
        .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs())
        .collect()
}

pub fn rpc_err(code: i32, msg: impl std::fmt::Display) -> ErrorObject<'static> {
    ErrorObject::owned(code, msg.to_string(), None::<()>)
}

/// Map a `bdslib::pipe::send` / `pipe::send_many` failure to a JSON-RPC
/// error.  "channel full" maps to `-32099` (server overloaded — the
/// client should back off and retry); every other failure (registry not
/// initialised, channel disconnected) maps to `-32001` (database
/// unavailable, treated as fatal).
pub fn pipe_err(msg: impl std::fmt::Display) -> ErrorObject<'static> {
    let s = msg.to_string();
    if s.contains("is full") {
        rpc_err(-32099, format!("ingest channel overloaded: {s}"))
    } else {
        rpc_err(-32001, s)
    }
}

/// Optional time-window parameters accepted by several methods.
#[derive(serde::Deserialize, Default)]
pub struct TimeWindowParams {
    /// Lookback window, e.g. `"1h"`, `"30min"`.
    pub duration: Option<String>,
    /// Range start as Unix seconds. Requires `end_ts`.
    pub start_ts: Option<i64>,
    /// Range end as Unix seconds. Requires `start_ts`.
    pub end_ts: Option<i64>,
}

pub enum TimeWindow {
    All,
    Range(SystemTime, SystemTime),
}

/// Find the [`bdslib::Shard`] that contains `uuid`.
///
/// Fast path: derive a `SystemTime` from the UUID v7 timestamp and call
/// `shards_at()`.  If that shard doesn't contain the record (e.g. the UUID
/// was generated with wall-clock time rather than the event time), fall back
/// to a linear scan across all shards.
pub fn find_shard_for_uuid(
    uuid: Uuid,
    db: &bdslib::ShardsManager,
) -> Result<bdslib::Shard, ErrorObject<'static>> {
    let cache = db.cache();
    let info = cache.info();

    // fast path
    if let Some(ts) = bdslib::timestamp_from_v7(uuid) {
        if let Ok(infos) = info.shards_at(ts) {
            for si in infos {
                if let Ok(shard) = cache.shard(si.start_time) {
                    if shard.observability().get_by_id(uuid).ok().flatten().is_some() {
                        return Ok(shard);
                    }
                }
            }
        }
    }

    // fallback: scan every shard
    let all = info.list_all().map_err(|e| rpc_err(-32002, e))?;
    for si in all {
        let shard = cache.shard(si.start_time).map_err(|e| rpc_err(-32003, e))?;
        if shard.observability().get_by_id(uuid).ok().flatten().is_some() {
            return Ok(shard);
        }
    }

    Err(rpc_err(-32404, format!("primary {uuid} not found")))
}

impl TimeWindowParams {
    pub fn resolve(self) -> Result<TimeWindow, ErrorObject<'static>> {
        if let Some(ref d) = self.duration {
            let secs = humantime::parse_duration(d)
                .map_err(|e| rpc_err(-32600, format!("invalid duration {d:?}: {e}")))?
                .as_secs();
            let end = SystemTime::now();
            let start = end - Duration::from_secs(secs);
            Ok(TimeWindow::Range(start, end))
        } else if let (Some(s), Some(e)) = (self.start_ts, self.end_ts) {
            let start = UNIX_EPOCH + Duration::from_secs(s as u64);
            let end = UNIX_EPOCH + Duration::from_secs(e as u64);
            Ok(TimeWindow::Range(start, end))
        } else {
            Ok(TimeWindow::All)
        }
    }
}
