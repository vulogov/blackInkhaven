//! 3.0.4 Phase-1 — `ink.goals.*` Bund stdlib: writing-goal progress, read-only.
//! Phase 1 exposes the streak, which needs only the writing-day history (no live
//! word total), so it stays a cheap deterministic read. The full pacing snapshot
//! (`ink.goals.snapshot`) — which requires the hierarchy word-count walk — lands
//! in a later phase.
//!
//! - `ink.goals.streak` ( -- dict )  the writing streak {days, best, grace_used,
//!   grace_per_week}, computed exactly as the goals dashboard does.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::progress::store::ProgressStore;

/// All recorded history, matching `aggregates::build_snapshot`'s window so
/// `best` is a genuine all-time longest streak.
const ALL_HISTORY_DAYS: i64 = 365 * 200;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.goals.streak", w_streak)];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    for (name, _) in words {
        if let Some(short) = name.strip_prefix("ink.") {
            let _ = vm.register_alias(short.to_string(), name.to_string());
        }
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

/// ( -- dict ) the writing streak {days, best, grace_used, grace_per_week}.
fn w_streak(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_streak(vm).map_err(to_bund_err)
}
fn do_streak(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.goals.streak";
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    // Same day-boundary the CLI honours, so "today" agrees with `inkhaven goals`.
    crate::dayclock::set_boundary(cfg.goals.day_boundary);

    let progress = ProgressStore::open(&store.project_root().join("progress.db"))
        .map_err(|e| anyhow!("{tag}: open progress.db: {e}"))?;
    let days = progress
        .writing_days_recent(ALL_HISTORY_DAYS)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let grace = cfg.goals.streak_grace_per_week;

    // `compute_streak` leaves `best` at 0; `build_snapshot` fills it via
    // `longest_streak` — we mirror that so the dict carries the all-time best.
    let mut streak = crate::progress::aggregates::compute_streak(
        &days,
        crate::dayclock::today_days(),
        grace,
    );
    streak.best = crate::progress::aggregates::longest_streak(&days, grace);

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("days".into(), Value::from_int(streak.days));
    m.insert("best".into(), Value::from_int(streak.best));
    m.insert("grace_used".into(), Value::from_int(streak.grace_used));
    m.insert("grace_per_week".into(), Value::from_int(streak.grace_per_week));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
