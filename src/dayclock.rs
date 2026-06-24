//! 1.3.37 — the process-global "what is today" clock.
//!
//! Streaks, daily word/active totals, AI-usage tallies, and the
//! slow-track daily caps all bin events into days. They must agree on
//! when a day rolls over, which is governed by `goals.day_boundary`
//! (`utc` | `local`). Rather than thread a boundary through every store
//! method, the boundary is collapsed to a single **offset in seconds**
//! installed once at startup (like `ai::usage`'s root): a day is
//! `(unix_secs + offset) / 86400`. Offset `0` = UTC, the default, so
//! nothing changes until a project opts into `local`.
//!
//! `local` uses the *current* UTC offset as a fixed shift. That's exact
//! except across a DST transition (a once-a-year ±1h edge), which is an
//! acceptable approximation for day-binning writing activity.

use std::sync::atomic::{AtomicI64, Ordering};

use crate::config::DayBoundary;

/// Seconds to add to a UTC timestamp before dividing by 86400 to get
/// the local day. `0` = UTC.
static OFFSET_SECS: AtomicI64 = AtomicI64::new(0);

/// Install the day boundary (call once at startup, after config loads).
pub fn set_boundary(boundary: DayBoundary) {
    let off = match boundary {
        DayBoundary::Utc => 0,
        DayBoundary::Local => {
            use chrono::Offset;
            chrono::Local::now().offset().fix().local_minus_utc() as i64
        }
    };
    OFFSET_SECS.store(off, Ordering::Relaxed);
}

pub fn offset_secs() -> i64 {
    OFFSET_SECS.load(Ordering::Relaxed)
}

/// Now, in Unix seconds (UTC).
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Today's day number (days since the epoch) under the active boundary.
pub fn today_days() -> i64 {
    (now_secs() + offset_secs()).div_euclid(86_400)
}

/// Unix seconds at the start of today under the active boundary (i.e.
/// local midnight when `local`, UTC midnight when `utc`).
pub fn today_start_secs() -> i64 {
    today_days() * 86_400 - offset_secs()
}

/// The `YYYY-MM-DD` key for today under the active boundary. Matches
/// `today_days` (formats the offset-shifted instant as a date).
pub fn today_key() -> String {
    chrono::DateTime::from_timestamp(now_secs() + offset_secs(), 0)
        .map(|dt| dt.format("%Y-%m-%d").to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utc_default_matches_plain_division() {
        set_boundary(DayBoundary::Utc);
        assert_eq!(offset_secs(), 0);
        assert_eq!(today_days(), now_secs().div_euclid(86_400));
        // today_start at UTC == day * 86400.
        assert_eq!(today_start_secs(), today_days() * 86_400);
    }

    #[test]
    fn today_key_is_consistent_with_today_days() {
        set_boundary(DayBoundary::Utc);
        // The key's date, parsed back to days, equals today_days.
        let key = today_key();
        let d = chrono::NaiveDate::parse_from_str(&key, "%Y-%m-%d").unwrap();
        let epoch = chrono::NaiveDate::from_ymd_opt(1970, 1, 1).unwrap();
        assert_eq!(d.signed_duration_since(epoch).num_days(), today_days());
    }

    #[test]
    fn local_offset_shifts_the_day_start_into_alignment() {
        set_boundary(DayBoundary::Local);
        // today_start + a full day brackets `now` regardless of tz, and
        // today_start lands on a boundary day edge (now − start in [0, 86400)).
        let elapsed = now_secs() - today_start_secs();
        assert!((0..86_400).contains(&elapsed), "elapsed since day start: {elapsed}");
        set_boundary(DayBoundary::Utc); // restore for other tests
    }
}
