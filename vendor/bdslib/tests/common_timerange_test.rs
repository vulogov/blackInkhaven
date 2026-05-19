use bdslib::common::timerange::{day_range, hour_range, minute_range};
use std::time::{Duration, UNIX_EPOCH};

// convenience: build a SystemTime from a known Unix timestamp (seconds)
fn at(secs: u64) -> std::time::SystemTime {
    UNIX_EPOCH + Duration::from_secs(secs)
}

// ── minute_range ──────────────────────────────────────────────────────────────

#[test]
fn test_minute_range_1_aligns_to_minute() {
    // 2024-01-01 00:07:42 UTC  →  [00:07:00, 00:08:00)
    let time = at(1_704_067_662); // 00:07:42
    let r = minute_range(time, 1).unwrap();
    assert_eq!(r.start, at(1_704_067_620)); // 00:07:00
    assert_eq!(r.end,   at(1_704_067_680)); // 00:08:00
}

#[test]
fn test_minute_range_5_aligns_to_five_minute_block() {
    // 00:07:42  →  [00:05:00, 00:10:00)
    let time = at(1_704_067_662);
    let r = minute_range(time, 5).unwrap();
    assert_eq!(r.start, at(1_704_067_500)); // 00:05:00
    assert_eq!(r.end,   at(1_704_067_800)); // 00:10:00
}

#[test]
fn test_minute_range_15_aligns_to_quarter_hour() {
    // 00:07:42  →  [00:00:00, 00:15:00)
    let time = at(1_704_067_662);
    let r = minute_range(time, 15).unwrap();
    assert_eq!(r.start, at(1_704_067_200)); // 00:00:00
    assert_eq!(r.end,   at(1_704_068_100)); // 00:15:00
}

#[test]
fn test_minute_range_30_aligns_to_half_hour() {
    // 00:07:42  →  [00:00:00, 00:30:00)
    let time = at(1_704_067_662);
    let r = minute_range(time, 30).unwrap();
    assert_eq!(r.start, at(1_704_067_200)); // 00:00:00
    assert_eq!(r.end,   at(1_704_069_000)); // 00:30:00
}

#[test]
fn test_minute_range_60_equals_hour_range() {
    let time = at(1_704_067_662);
    let mr = minute_range(time, 60).unwrap();
    let hr = hour_range(time).unwrap();
    assert_eq!(mr, hr);
}

#[test]
fn test_minute_range_time_on_boundary_is_own_start() {
    // exactly on a 5-minute boundary
    let time = at(1_704_067_500); // 00:05:00
    let r = minute_range(time, 5).unwrap();
    assert_eq!(r.start, time);
    assert_eq!(r.end, at(1_704_067_800)); // 00:10:00
}

#[test]
fn test_minute_range_interval_duration_is_correct() {
    for n in [1u64, 2, 3, 4, 5, 6, 10, 12, 15, 20, 30, 60] {
        let r = minute_range(at(1_704_067_662), n).unwrap();
        let duration = r.end.duration_since(r.start).unwrap();
        assert_eq!(duration, Duration::from_secs(n * 60), "n={n}");
    }
}

#[test]
fn test_minute_range_zero_is_error() {
    assert!(minute_range(at(1_704_067_662), 0).is_err());
}

#[test]
fn test_minute_range_non_divisor_is_error() {
    for bad in [7u64, 8, 9, 11, 13, 14, 16, 17, 25, 45] {
        assert!(
            minute_range(at(1_704_067_662), bad).is_err(),
            "n={bad} should be rejected"
        );
    }
}

#[test]
fn test_minute_range_pre_epoch_is_error() {
    let pre = UNIX_EPOCH.checked_sub(Duration::from_secs(1)).unwrap();
    assert!(minute_range(pre, 5).is_err());
}

// ── hour_range ────────────────────────────────────────────────────────────────

#[test]
fn test_hour_range_aligns_to_hour() {
    // 2024-01-01 03:22:15 UTC  →  [03:00:00, 04:00:00)
    let time = at(1_704_079_335);
    let r = hour_range(time).unwrap();
    assert_eq!(r.start, at(1_704_078_000)); // 03:00:00
    assert_eq!(r.end,   at(1_704_081_600)); // 04:00:00
}

#[test]
fn test_hour_range_time_on_boundary_is_own_start() {
    let time = at(1_704_078_000); // exactly 03:00:00
    let r = hour_range(time).unwrap();
    assert_eq!(r.start, time);
    assert_eq!(r.end, at(1_704_081_600));
}

#[test]
fn test_hour_range_duration_is_3600_seconds() {
    let r = hour_range(at(1_704_079_335)).unwrap();
    assert_eq!(
        r.end.duration_since(r.start).unwrap(),
        Duration::from_secs(3_600)
    );
}

#[test]
fn test_hour_range_start_has_zero_sub_hour_seconds() {
    let r = hour_range(at(1_704_079_335)).unwrap();
    let start_secs = r.start.duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(start_secs % 3_600, 0);
}

#[test]
fn test_hour_range_pre_epoch_is_error() {
    let pre = UNIX_EPOCH.checked_sub(Duration::from_secs(1)).unwrap();
    assert!(hour_range(pre).is_err());
}

// ── day_range ─────────────────────────────────────────────────────────────────

#[test]
fn test_day_range_aligns_to_utc_day() {
    // 2024-01-15 14:30:00 UTC  →  [2024-01-15 00:00:00, 2024-01-16 00:00:00)
    let time = at(1_705_329_000);
    let r = day_range(time).unwrap();
    assert_eq!(r.start, at(1_705_276_800)); // 2024-01-15 00:00:00
    assert_eq!(r.end,   at(1_705_363_200)); // 2024-01-16 00:00:00
}

#[test]
fn test_day_range_time_on_boundary_is_own_start() {
    let midnight = at(1_705_276_800);
    let r = day_range(midnight).unwrap();
    assert_eq!(r.start, midnight);
    assert_eq!(r.end, at(1_705_363_200));
}

#[test]
fn test_day_range_duration_is_86400_seconds() {
    let r = day_range(at(1_705_329_000)).unwrap();
    assert_eq!(
        r.end.duration_since(r.start).unwrap(),
        Duration::from_secs(86_400)
    );
}

#[test]
fn test_day_range_start_has_zero_sub_day_seconds() {
    let r = day_range(at(1_705_329_000)).unwrap();
    let start_secs = r.start.duration_since(UNIX_EPOCH).unwrap().as_secs();
    assert_eq!(start_secs % 86_400, 0);
}

#[test]
fn test_day_range_pre_epoch_is_error() {
    let pre = UNIX_EPOCH.checked_sub(Duration::from_secs(1)).unwrap();
    assert!(day_range(pre).is_err());
}

// ── cross-function consistency ────────────────────────────────────────────────

#[test]
fn test_hour_range_nested_in_day_range() {
    let time = at(1_705_329_000);
    let hour = hour_range(time).unwrap();
    let day  = day_range(time).unwrap();
    assert!(hour.start >= day.start);
    assert!(hour.end   <= day.end);
}

#[test]
fn test_minute_range_nested_in_hour_range() {
    let time = at(1_705_329_000);
    let min  = minute_range(time, 15).unwrap();
    let hour = hour_range(time).unwrap();
    assert!(min.start >= hour.start);
    assert!(min.end   <= hour.end);
}

#[test]
fn test_successive_minute_ranges_are_contiguous() {
    let time = at(1_704_067_662);
    let r1 = minute_range(time, 5).unwrap();
    let r2 = minute_range(r1.end, 5).unwrap();
    assert_eq!(r1.end, r2.start);
}

#[test]
fn test_successive_hour_ranges_are_contiguous() {
    let time = at(1_704_067_662);
    let r1 = hour_range(time).unwrap();
    let r2 = hour_range(r1.end).unwrap();
    assert_eq!(r1.end, r2.start);
}

#[test]
fn test_successive_day_ranges_are_contiguous() {
    let time = at(1_705_329_000);
    let r1 = day_range(time).unwrap();
    let r2 = day_range(r1.end).unwrap();
    assert_eq!(r1.end, r2.start);
}
