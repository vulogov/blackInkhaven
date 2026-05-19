use crate::common::error::{err_msg, Result};
use serde_json::Value as JsonValue;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Return the current time as whole Unix seconds (infallible).
pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Extract a `SystemTime` from the `"timestamp"` field of a JSON document.
///
/// The field must be a non-negative integer representing Unix seconds.
pub fn extract_timestamp(doc: &JsonValue) -> Result<SystemTime> {
    let secs = doc
        .get("timestamp")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| err_msg("document must contain a numeric 'timestamp' field"))?;
    Ok(UNIX_EPOCH + Duration::from_secs(secs))
}

/// Return `(start, end)` for a lookback window ending at the current time.
///
/// `duration` is a human-readable string such as `"1h"`, `"7days"`, etc.
/// `end` is `now + 1 s` so that records written at exactly `now` are included.
pub fn lookback_window(duration: &str) -> Result<(SystemTime, SystemTime)> {
    let dur = humantime::parse_duration(duration)
        .map_err(|e| err_msg(format!("invalid duration '{duration}': {e}")))?;
    let now = SystemTime::now();
    let start = now.checked_sub(dur).unwrap_or(UNIX_EPOCH);
    let end = now + Duration::from_secs(1);
    Ok((start, end))
}
