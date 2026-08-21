//! 3.0.4 — `ink.goals.*` Bund stdlib: writing-goal progress, read-only. `streak`
//! needs only the writing-day history; `snapshot` is the full pacing dashboard
//! (`inkhaven goals`) — the per-book word-count walk (filesystem reads over the
//! active store's hierarchy) plus the streak/status/sparkline aggregation. Both
//! open `progress.db` fresh (a separate DB file, safe alongside the live store).
//!
//! - `ink.goals.streak`   ( -- dict )  the writing streak {days, best, grace_used,
//!   grace_per_week}.
//! - `ink.goals.snapshot` ( -- dict )  the full progress snapshot {project, books,
//!   status, streak, sparkline, active_seconds_today, active_seconds_week}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{active_config, active_store, push};
use crate::progress::aggregates::{BookProgress, ProgressSnapshot};
use crate::progress::store::ProgressStore;
use crate::progress::{count_words, LiveTotals};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, Store};

/// All recorded history, matching `aggregates::build_snapshot`'s window so
/// `best` is a genuine all-time longest streak.
const ALL_HISTORY_DAYS: i64 = 365 * 200;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.goals.streak", w_streak), ("ink.goals.snapshot", w_snapshot)];
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

/// Per-book live word totals — the same filesystem walk the goals dashboard uses
/// (`cli::goals::live_totals`), replicated here since that helper is private.
fn live_totals(layout: &ProjectLayout, h: &Hierarchy) -> LiveTotals {
    let mut per_book: HashMap<Uuid, i64> = HashMap::new();
    let mut book_titles: HashMap<Uuid, String> = HashMap::new();
    let mut book_slugs: HashMap<Uuid, String> = HashMap::new();
    for (node, _) in h.flatten() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let book = h
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == NodeKind::Book)
            .filter(|b| b.system_tag.is_none());
        let Some(book) = book else { continue };
        let Some(rel) = node.file.as_ref() else { continue };
        let body = std::fs::read_to_string(layout.root.join(rel)).unwrap_or_default();
        *per_book.entry(book.id).or_insert(0) += count_words(&body);
        book_titles.entry(book.id).or_insert_with(|| book.title.clone());
        book_slugs.entry(book.id).or_insert_with(|| book.slug.clone());
    }
    let project_total = per_book.values().sum();
    LiveTotals { per_book, project_total, book_titles, book_slugs }
}

fn opt_int(m: &mut HashMap<String, Value>, key: &str, v: Option<i64>) {
    if let Some(v) = v {
        m.insert(key.into(), Value::from_int(v));
    }
}

fn book_progress_dict(b: &BookProgress) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("label".into(), Value::from_string(&b.label));
    m.insert("today_words".into(), Value::from_int(b.today_words));
    m.insert("total_words".into(), Value::from_int(b.total_words));
    opt_int(&mut m, "daily_goal", b.daily_goal);
    opt_int(&mut m, "target_words", b.target_words);
    opt_int(&mut m, "required_pace", b.required_pace);
    opt_int(&mut m, "days_to_deadline", b.days_to_deadline);
    Value::from_dict(m)
}

/// A `Vec<(String, i64)>` status ladder → a list of {name, count} dicts.
fn ladder(rows: &[(String, i64)]) -> Value {
    Value::from_list(
        rows.iter()
            .map(|(name, count)| {
                let mut m: HashMap<String, Value> = HashMap::new();
                m.insert("name".into(), Value::from_string(name));
                m.insert("count".into(), Value::from_int(*count));
                Value::from_dict(m)
            })
            .collect(),
    )
}

/// ( -- dict ) the full progress snapshot.
fn w_snapshot(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_snapshot(vm).map_err(to_bund_err)
}
fn do_snapshot(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.goals.snapshot";
    let store: &Store = active_store(tag)?;
    let cfg = active_config(tag)?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);

    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let live = live_totals(&layout, &h);
    let progress = ProgressStore::open(&store.project_root().join("progress.db"))
        .map_err(|e| anyhow!("{tag}: open progress.db: {e}"))?;
    let snap: ProgressSnapshot = crate::progress::aggregates::build_snapshot(&progress, &cfg.goals, &live)
        .map_err(|e| anyhow!("{tag}: {e}"))?;

    let mut streak: HashMap<String, Value> = HashMap::new();
    streak.insert("days".into(), Value::from_int(snap.streak.days));
    streak.insert("best".into(), Value::from_int(snap.streak.best));
    streak.insert("grace_used".into(), Value::from_int(snap.streak.grace_used));
    streak.insert("grace_per_week".into(), Value::from_int(snap.streak.grace_per_week));

    let mut status: HashMap<String, Value> = HashMap::new();
    status.insert("recent".into(), ladder(&snap.status.recent));
    status.insert("goals".into(), ladder(&snap.status.goals));

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("project".into(), book_progress_dict(&snap.project));
    m.insert(
        "books".into(),
        Value::from_list(snap.books.iter().map(book_progress_dict).collect()),
    );
    m.insert("status".into(), Value::from_dict(status));
    m.insert("streak".into(), Value::from_dict(streak));
    m.insert(
        "sparkline".into(),
        Value::from_list(snap.sparkline.iter().map(|v| Value::from_int(*v)).collect()),
    );
    m.insert("active_seconds_today".into(), Value::from_int(snap.active_seconds_today));
    m.insert("active_seconds_week".into(), Value::from_int(snap.active_seconds_week));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
