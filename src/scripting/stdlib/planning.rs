//! 3.0.4 Phase-1 — `ink.planning.*` Bund stdlib: the story-structure frameworks,
//! read-only and pure (no store, no LLM). Bund reads the same canonical beat
//! tables `inkhaven plan` works from — the AI structural critique
//! (`plan analyze`) is deliberately NOT exposed (it costs), matching the
//! LECTOR / REDLINE / KEN precedent.
//!
//! - `ink.planning.frameworks` ( -- list )       every framework as {slug, label}.
//! - `ink.planning.beats`      ( framework -- list )  a framework's canonical beat
//!   table, each a dict {name, act, target_position, expected_tension}.
//! - `ink.planning.check`      ( -- dict )        the deterministic structural
//!   report for the project's book (`inkhaven plan check`): beats/gaps/acts/
//!   warnings/scenes/tension. The AI critique (`plan analyze`) stays CLI-only.
//! - `ink.planning.gaps`       ( -- list )        just the unmapped-beat names.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, pull, push, require_depth, value_to_string};
use crate::planning::{Framework, PlanReport};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::NodeKind;

/// The CLI's default beat-drift tolerance (`plan check` uses 0.10).
const DEFAULT_DRIFT: f32 = 0.10;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.planning.frameworks", w_frameworks),
        ("ink.planning.beats", w_beats),
        ("ink.planning.check", w_check),
        ("ink.planning.gaps", w_gaps),
    ];
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

/// ( -- list ) every framework as {slug, label}.
fn w_frameworks(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let items: Vec<Value> = Framework::ALL
        .iter()
        .map(|f| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("slug".into(), Value::from_string(f.slug()));
            m.insert("label".into(), Value::from_string(f.label()));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

/// ( framework -- list ) the canonical beat table for `framework` (accepts any
/// slug/alias `Framework::parse` recognises). Errors on an unknown framework.
fn w_beats(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_beats(vm).map_err(to_bund_err)
}
fn do_beats(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.planning.beats";
    require_depth(vm, 1, tag)?;
    let want = value_to_string(pull(vm, tag)?, "framework", tag)?;
    let fw = Framework::parse(&want).ok_or_else(|| {
        anyhow!(
            "{tag}: unknown framework `{want}` (try one of: {})",
            Framework::ALL.iter().map(|f| f.slug()).collect::<Vec<_>>().join(", ")
        )
    })?;
    let items: Vec<Value> = fw
        .beats()
        .iter()
        .map(|b| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(b.name));
            m.insert("act".into(), Value::from_int(b.act as i64));
            m.insert("target_position".into(), Value::from_float(b.target_position as f64));
            m.insert("expected_tension".into(), Value::from_float(b.expected_tension as f64));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

// ── check / gaps (the deterministic structural report) ──────────────────────

fn opt_f32(m: &mut HashMap<String, Value>, key: &str, v: Option<f32>) {
    // Skip NaN / inf — `Value::from_float` stores them verbatim, and a degenerate
    // position ratio on an edge-case book would otherwise leak NaN into the dict.
    // A non-finite derived value reads as "absent", the same as `None`.
    if let Some(v) = v {
        if v.is_finite() {
            m.insert(key.into(), Value::from_float(v as f64));
        }
    }
}

fn opt_str(m: &mut HashMap<String, Value>, key: &str, v: &Option<String>) {
    if let Some(s) = v {
        m.insert(key.into(), Value::from_string(s));
    }
}

fn strs(xs: &[String]) -> Value {
    Value::from_list(xs.iter().map(Value::from_string).collect())
}

/// Build the deterministic PlanReport for the project's first user book off the
/// active store (no LLM — that's `plan analyze`, kept CLI-only). Returns the
/// report and the book's title so a caller can report which book was analyzed.
///
/// (The CLI's `resolve_user_book(None)` errors on a multi-book project since it
/// can't disambiguate; a no-arg script word should still work, so we take the
/// first user book and surface its label rather than failing.)
fn plan_report(tag: &str) -> Result<(PlanReport, String)> {
    let store = active_store(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = h
        .children_of(None)
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .ok_or_else(|| anyhow!("{tag}: no user book to plan (create a book first)"))?;
    let label = book.title.clone();
    let (report, _framework, _chapters) =
        crate::cli::plan::build_report(store, &layout, &h, book, DEFAULT_DRIFT)
            .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((report, label))
}

fn beat_dict(b: &crate::planning::BeatStatus) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("beat".into(), Value::from_string(&b.beat));
    m.insert("act".into(), Value::from_int(b.act as i64));
    m.insert("target_position".into(), Value::from_float(b.target_position as f64));
    opt_str(&mut m, "mapped_chapter", &b.mapped_chapter);
    opt_f32(&mut m, "actual_position", b.actual_position);
    opt_f32(&mut m, "drift", b.drift);
    m.insert("threads".into(), strs(&b.threads));
    m.insert("unknown_threads".into(), strs(&b.unknown_threads));
    m.insert("notes".into(), Value::from_string(&b.notes));
    Value::from_dict(m)
}

fn tension_dict(t: &crate::planning::TensionCurve) -> Value {
    let points: Vec<Value> = t
        .points
        .iter()
        .map(|p| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("beat".into(), Value::from_string(&p.beat));
            opt_f32(&mut m, "position", p.position);
            m.insert("expected".into(), Value::from_float(p.expected as f64));
            opt_f32(&mut m, "actual", p.actual);
            opt_f32(&mut m, "gap", p.gap);
            opt_f32(&mut m, "ai", p.ai);
            Value::from_dict(m)
        })
        .collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("has_actual".into(), Value::from_bool(t.has_actual));
    m.insert("has_ai".into(), Value::from_bool(t.has_ai));
    m.insert("warnings".into(), strs(&t.warnings));
    m.insert("points".into(), Value::from_list(points));
    Value::from_dict(m)
}

/// ( -- dict ) the deterministic structural report.
fn w_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_check(vm).map_err(to_bund_err)
}
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let (report, book) = plan_report("ink.planning.check")?;

    let acts: Vec<Value> = report
        .acts
        .iter()
        .map(|a| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("act".into(), Value::from_int(a.act as i64));
            m.insert("expected".into(), Value::from_float(a.expected as f64));
            opt_f32(&mut m, "actual", a.actual);
            Value::from_dict(m)
        })
        .collect();
    let scenes: Vec<Value> = report
        .scenes
        .iter()
        .map(|s| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("title".into(), Value::from_string(&s.title));
            m.insert("chapter".into(), Value::from_string(&s.chapter));
            m.insert("kind".into(), Value::from_string(&s.kind));
            m.insert("weak".into(), Value::from_bool(s.weak));
            Value::from_dict(m)
        })
        .collect();

    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("book".into(), Value::from_string(&book));
    m.insert("beats".into(), Value::from_list(report.beats.iter().map(beat_dict).collect()));
    m.insert("gaps".into(), strs(&report.gaps));
    m.insert("acts".into(), Value::from_list(acts));
    m.insert("warnings".into(), strs(&report.warnings));
    m.insert("scenes".into(), Value::from_list(scenes));
    // A pass/fail gate for a structural-readiness script.
    m.insert("clean".into(), Value::from_bool(report.gaps.is_empty() && report.warnings.is_empty()));
    if let Some(t) = &report.tension {
        m.insert("tension".into(), tension_dict(t));
    }
    push(vm, Value::from_dict(m));
    Ok(vm)
}

/// ( -- list ) just the unmapped-beat names.
fn w_gaps(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_gaps(vm).map_err(to_bund_err)
}
fn do_gaps(vm: &mut VM) -> Result<&mut VM> {
    let (report, _book) = plan_report("ink.planning.gaps")?;
    push(vm, strs(&report.gaps));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use crate::scripting;

    #[test]
    fn frameworks_lists_every_canon_framework() {
        // The pure words need no project store, so they run under bare `eval`.
        let out = scripting::eval("planning.frameworks").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert_eq!(list.len(), super::Framework::ALL.len());
        let first = list[0].cast_dict().expect("a dict");
        assert!(first.contains_key("slug") && first.contains_key("label"));
    }

    #[test]
    fn beats_returns_the_framework_canon_table() {
        let out = scripting::eval("\"three_act\" planning.beats").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert!(!list.is_empty(), "three-act has canonical beats");
        let beat = list[0].cast_dict().expect("a dict");
        for k in ["name", "act", "target_position", "expected_tension"] {
            assert!(beat.contains_key(k), "beat dict missing `{k}`");
        }
    }

    #[test]
    fn beats_rejects_an_unknown_framework() {
        // An unknown framework is a clean script error, not a panic.
        assert!(scripting::eval("\"no_such_framework\" planning.beats").is_err());
    }
}
