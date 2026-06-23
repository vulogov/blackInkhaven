//! WORLD-5 — the `ink.world.fact_check.timeline.*` Bund stdlib. Read-only helpers
//! for the fact-checker's perspective on the timeline: events near a point, events
//! for a character / place, the season of a point, and a paragraph's effective
//! date. All `store_read` (default-allowed); the timeline's own data is exposed
//! elsewhere via `ink.event.*`.

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;
use uuid::Uuid;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::store::hierarchy::Hierarchy;
use crate::timeline::calendar::{Calendar, TimelinePoint};
use crate::world::timeline_context as tc;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.world.fact_check.timeline.events_near", w_events_near),
        ("ink.world.fact_check.timeline.events_for_character", w_events_for_character),
        ("ink.world.fact_check.timeline.events_for_place", w_events_for_place),
        ("ink.world.fact_check.timeline.season_for", w_season_for),
        ("ink.world.fact_check.timeline.effective_date", w_effective_date),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(format!("{e}"))
}

/// Load the project's events + calendar (the timeline data the words operate on).
fn timeline_data(tag: &str) -> Result<(Vec<tc::TlEvent>, Calendar)> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let events = tc::gather_events(&hierarchy);
    let cfg = Config::load_layered(&ProjectLayout::new(store.project_root()).config_path())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((events, Calendar::from_config(cfg.timeline.calendar)))
}

fn event_dict(e: &tc::TlEvent) -> Value {
    let mut h = std::collections::HashMap::new();
    h.insert("id".to_string(), Value::from_string(e.id.to_string()));
    h.insert("title".to_string(), Value::from_string(e.title.clone()));
    h.insert("start_ticks".to_string(), Value::from_int(e.start_ticks));
    Value::from_dict(h)
}

fn pull_int(vm: &mut VM, tag: &str) -> Result<i64> {
    let v = pull(vm, tag)?;
    v.cast_int().map_err(|e| anyhow!("{tag}: expected an integer ({e})"))
}

fn pull_uuid(vm: &mut VM, field: &str, tag: &str) -> Result<Uuid> {
    let s = value_to_string(pull(vm, tag)?, field, tag)?;
    Uuid::parse_str(s.trim()).map_err(|e| anyhow!("{tag}: bad {field} uuid `{s}`: {e}"))
}

// ( point window -- list )  events whose start is within `window` ticks of `point`.
fn w_events_near(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_events_near(vm).map_err(to_bund_err)
}
fn do_events_near(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.world.fact_check.timeline.events_near";
    require_depth(vm, 2, tag)?;
    let window = pull_int(vm, tag)?;
    let point = pull_int(vm, tag)?;
    let (events, _) = timeline_data(tag)?;
    let near: Vec<Value> = tc::events_near(&events, point, window).iter().map(|e| event_dict(e)).collect();
    push(vm, Value::from_list(near));
    Ok(vm)
}

// ( character-uuid -- list )  events involving the character.
fn w_events_for_character(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_events_for_character(vm).map_err(to_bund_err)
}
fn do_events_for_character(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.world.fact_check.timeline.events_for_character";
    require_depth(vm, 1, tag)?;
    let ch = pull_uuid(vm, "character", tag)?;
    let (events, _) = timeline_data(tag)?;
    let items: Vec<Value> = tc::events_for_character(&events, ch).iter().map(|e| event_dict(e)).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( place-uuid -- list )  events at the place.
fn w_events_for_place(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_events_for_place(vm).map_err(to_bund_err)
}
fn do_events_for_place(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.world.fact_check.timeline.events_for_place";
    require_depth(vm, 1, tag)?;
    let place = pull_uuid(vm, "place", tag)?;
    let (events, _) = timeline_data(tag)?;
    let items: Vec<Value> = tc::events_for_place(&events, place).iter().map(|e| event_dict(e)).collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ( point -- season )  the calendar season covering a point ("" if none).
fn w_season_for(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_season_for(vm).map_err(to_bund_err)
}
fn do_season_for(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.world.fact_check.timeline.season_for";
    require_depth(vm, 1, tag)?;
    let point = pull_int(vm, tag)?;
    let (_, calendar) = timeline_data(tag)?;
    let season = calendar.season_for(TimelinePoint::from_ticks(point)).unwrap_or_default();
    push(vm, Value::from_string(season));
    Ok(vm)
}

// ( paragraph-uuid -- date )  the paragraph's effective world-time in ticks (-1
// if none could be established).
fn w_effective_date(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_effective_date(vm).map_err(to_bund_err)
}
fn do_effective_date(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.world.fact_check.timeline.effective_date";
    require_depth(vm, 1, tag)?;
    let para = pull_uuid(vm, "paragraph", tag)?;
    let (events, calendar) = timeline_data(tag)?;
    let day = calendar.ticks_per("day").unwrap_or(1);
    let ctx = tc::build_context(para, &events, &calendar, 90 * day);
    push(vm, Value::from_int(ctx.effective_date.unwrap_or(-1)));
    Ok(vm)
}
