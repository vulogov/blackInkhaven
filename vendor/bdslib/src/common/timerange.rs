use crate::common::error::{err_msg, Result};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Convert `t` to whole Unix seconds elapsed since the epoch.
///
/// Returns `Err` if `t` predates the Unix epoch.
pub fn to_unix_secs(t: SystemTime) -> Result<i64> {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| err_msg(format!("timestamp predates Unix epoch: {e}")))
}

/// Return the current time as whole Unix seconds.
pub fn now_unix_secs() -> Result<i64> {
    to_unix_secs(SystemTime::now())
}

/// A half-open time interval `[start, end)`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimeRange {
    pub start: SystemTime,
    pub end: SystemTime,
}

impl TimeRange {
    fn new(start: SystemTime, duration_secs: u64) -> Self {
        Self {
            start,
            end: start + Duration::from_secs(duration_secs),
        }
    }
}

fn floor_to(time: SystemTime, interval_secs: u64) -> Result<SystemTime> {
    let secs = time
        .duration_since(UNIX_EPOCH)
        .map_err(|e| err_msg(format!("timestamp predates Unix epoch: {e}")))?
        .as_secs();
    Ok(UNIX_EPOCH + Duration::from_secs((secs / interval_secs) * interval_secs))
}

/// Return the `[start, end)` interval of `n` whole minutes that contains `time`.
///
/// `n` must be a divisor of 60 (1, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30, 60) so
/// that boundaries align to the hour. Returns `Err` for `n == 0` or a
/// non-divisor, and for timestamps that predate the Unix epoch.
pub fn minute_range(time: SystemTime, n: u64) -> Result<TimeRange> {
    if n == 0 || 60 % n != 0 {
        return Err(err_msg(format!(
            "n={n} is not a divisor of 60; valid values: 1,2,3,4,5,6,10,12,15,20,30,60"
        )));
    }
    let interval = n * 60;
    Ok(TimeRange::new(floor_to(time, interval)?, interval))
}

/// Return the `[start, end)` hour interval that contains `time`.
pub fn hour_range(time: SystemTime) -> Result<TimeRange> {
    Ok(TimeRange::new(floor_to(time, 3_600)?, 3_600))
}

/// Return the `[start, end)` UTC day interval that contains `time`.
pub fn day_range(time: SystemTime) -> Result<TimeRange> {
    Ok(TimeRange::new(floor_to(time, 86_400)?, 86_400))
}

/// Align `timestamp` down to the nearest `duration` boundary and return
/// the half-open interval `[start, end)` of length `duration` that contains it.
///
/// Boundaries are computed relative to the Unix epoch, so all intervals of the
/// same duration are non-overlapping and contiguous.
///
/// Returns `Err` if `timestamp` predates the Unix epoch or `duration` is zero.
pub fn align_to_duration(
    timestamp: SystemTime,
    duration: Duration,
) -> Result<(SystemTime, SystemTime)> {
    if duration.is_zero() {
        return Err(err_msg("duration must be non-zero"));
    }
    let secs = timestamp
        .duration_since(UNIX_EPOCH)
        .map_err(|e| err_msg(format!("timestamp predates Unix epoch: {e}")))?
        .as_secs();
    let dur_secs = duration.as_secs().max(1);
    let start_secs = (secs / dur_secs) * dur_secs;
    let start = UNIX_EPOCH + Duration::from_secs(start_secs);
    let end = start + duration;
    Ok((start, end))
}
