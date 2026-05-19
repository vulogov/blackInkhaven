use bdslib::common::uuid::{generate_v7, generate_v7_at, timestamp_from_v7};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// ── generate_v7 ───────────────────────────────────────────────────────────────

#[test]
fn test_generate_v7_returns_version_7() {
    let id = generate_v7();
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn test_generate_v7_successive_ids_are_monotonic() {
    let a = generate_v7();
    let b = generate_v7();
    assert!(b >= a, "successive UUIDv7s must be non-decreasing");
}

#[test]
fn test_generate_v7_ids_are_unique() {
    let ids: Vec<_> = (0..100).map(|_| generate_v7()).collect();
    let mut sorted = ids.clone();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len(), "all generated UUIDs must be unique");
}

#[test]
fn test_generate_v7_timestamp_is_recent() {
    let before = SystemTime::now();
    let id = generate_v7();
    let after = SystemTime::now();

    let ts = timestamp_from_v7(id).expect("must have timestamp");
    assert!(ts >= before.checked_sub(Duration::from_secs(1)).unwrap());
    assert!(ts <= after.checked_add(Duration::from_secs(1)).unwrap());
}

// ── generate_v7_at ────────────────────────────────────────────────────────────

#[test]
fn test_generate_v7_at_returns_version_7() {
    let id = generate_v7_at(SystemTime::now());
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn test_generate_v7_at_embeds_correct_timestamp() {
    let time = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let id = generate_v7_at(time);
    let extracted = timestamp_from_v7(id).expect("must have timestamp");

    // UUIDv7 has millisecond precision — allow ±1 ms
    let diff = if extracted >= time {
        extracted.duration_since(time).unwrap()
    } else {
        time.duration_since(extracted).unwrap()
    };
    assert!(diff <= Duration::from_millis(1), "timestamp mismatch: {diff:?}");
}

#[test]
fn test_generate_v7_at_past_time() {
    let past = UNIX_EPOCH + Duration::from_secs(1_000_000_000); // Sep 2001
    let id = generate_v7_at(past);
    assert_eq!(id.get_version_num(), 7);
    let ts = timestamp_from_v7(id).expect("must have timestamp");
    let diff = if ts >= past {
        ts.duration_since(past).unwrap()
    } else {
        past.duration_since(ts).unwrap()
    };
    assert!(diff <= Duration::from_millis(1));
}

#[test]
fn test_generate_v7_at_future_time() {
    let future = SystemTime::now() + Duration::from_secs(86_400 * 365);
    let id = generate_v7_at(future);
    assert_eq!(id.get_version_num(), 7);
    let ts = timestamp_from_v7(id).expect("must have timestamp");
    let diff = if ts >= future {
        ts.duration_since(future).unwrap()
    } else {
        future.duration_since(ts).unwrap()
    };
    assert!(diff <= Duration::from_millis(1));
}

#[test]
fn test_generate_v7_at_before_epoch_uses_epoch() {
    // SystemTime before UNIX_EPOCH — must not panic
    let pre_epoch = UNIX_EPOCH.checked_sub(Duration::from_secs(1)).unwrap();
    let id = generate_v7_at(pre_epoch);
    assert_eq!(id.get_version_num(), 7);
}

#[test]
fn test_generate_v7_at_ordering_matches_time_ordering() {
    let t1 = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
    let t2 = UNIX_EPOCH + Duration::from_secs(1_700_000_000);
    let t3 = UNIX_EPOCH + Duration::from_secs(1_800_000_000);

    let id1 = generate_v7_at(t1);
    let id2 = generate_v7_at(t2);
    let id3 = generate_v7_at(t3);

    assert!(id1 < id2, "earlier time must produce smaller UUID");
    assert!(id2 < id3, "earlier time must produce smaller UUID");
}

// ── timestamp_from_v7 ─────────────────────────────────────────────────────────

#[test]
fn test_timestamp_from_v7_round_trips() {
    let original = UNIX_EPOCH + Duration::from_millis(1_700_000_000_123);
    let id = generate_v7_at(original);
    let recovered = timestamp_from_v7(id).expect("must extract timestamp");

    let diff = if recovered >= original {
        recovered.duration_since(original).unwrap()
    } else {
        original.duration_since(recovered).unwrap()
    };
    assert!(diff <= Duration::from_millis(1));
}

#[test]
fn test_timestamp_from_v7_on_generate_v7() {
    let before = SystemTime::now();
    let id = generate_v7();
    let after = SystemTime::now();

    let ts = timestamp_from_v7(id).expect("generate_v7 must embed a timestamp");
    assert!(ts >= before.checked_sub(Duration::from_secs(1)).unwrap());
    assert!(ts <= after.checked_add(Duration::from_secs(1)).unwrap());
}

#[test]
fn test_timestamp_from_non_v7_uuid_returns_none() {
    let v4 = uuid::Uuid::new_v4();
    assert!(
        timestamp_from_v7(v4).is_none(),
        "non-v7 UUID must return None"
    );
}

#[test]
fn test_timestamp_advances_with_successive_ids() {
    let id1 = generate_v7();
    std::thread::sleep(Duration::from_millis(5));
    let id2 = generate_v7();

    let ts1 = timestamp_from_v7(id1).unwrap();
    let ts2 = timestamp_from_v7(id2).unwrap();
    assert!(ts2 >= ts1, "timestamp must advance between successive IDs");
}
