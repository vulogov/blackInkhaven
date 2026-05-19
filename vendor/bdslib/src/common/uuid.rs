use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::{
    timestamp::{context::NoContext, Timestamp},
    Uuid,
};

/// Generate a UUIDv7 using the current system time.
pub fn generate_v7() -> Uuid {
    Uuid::now_v7()
}

/// Generate a UUIDv7 from an arbitrary [`SystemTime`].
///
/// If `time` predates the Unix epoch the epoch itself is used as the timestamp.
pub fn generate_v7_at(time: SystemTime) -> Uuid {
    let dur = time.duration_since(UNIX_EPOCH).unwrap_or(Duration::ZERO);
    let ts = Timestamp::from_unix(NoContext, dur.as_secs(), dur.subsec_nanos());
    Uuid::new_v7(ts)
}

/// Extract the embedded Unix timestamp from a UUIDv7.
///
/// Returns `None` if `id` is not a version-7 UUID or its timestamp cannot be
/// represented as a [`SystemTime`].
pub fn timestamp_from_v7(id: Uuid) -> Option<SystemTime> {
    let ts = id.get_timestamp()?;
    let (secs, nanos) = ts.to_unix();
    UNIX_EPOCH.checked_add(Duration::new(secs, nanos))
}
