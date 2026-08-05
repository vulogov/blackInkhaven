//! CHRONICLE-1 (CH-P5) — `ink.chronicle.*` Bund stdlib: the draft-history
//! intelligence, read-only. `marks` lists the captured milestones; `trend` and
//! `check` diff the live state against the most recent mark. Pure measurement —
//! CHRONICLE never edits the manuscript, and **marking is not exposed** (it writes
//! a milestone): scripts read the history, they don't stamp it.
//!
//! - `ink.chronicle.marks` ( -- list )  the milestones as dicts
//!   {label, ts, book, findings, errors, warnings, infos}.
//! - `ink.chronicle.trend` ( -- dict )  the live-vs-latest trend {marked, since,
//!   headline, categories, cleared, introduced, persisted}.
//! - `ink.chronicle.check` ( -- dict )  the gate {baseline, cleared, introduced,
//!   introduced_errors, clean} — `clean` = no error-severity finding introduced
//!   since the last mark.

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, push};
use crate::chronicle::store::ChronicleStore;
use crate::chronicle::{diff_findings, diff_vectors, Direction, MetricVector, TrendDelta};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.chronicle.marks", w_marks),
        ("ink.chronicle.trend", w_trend),
        ("ink.chronicle.check", w_check),
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

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

/// The trend direction word — matches [`Direction`]'s serialised form.
fn dir_word(d: Direction) -> &'static str {
    match d {
        Direction::Better => "better",
        Direction::Worse => "worse",
        Direction::Same => "same",
    }
}

fn delta_dict(d: &TrendDelta) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("key".into(), Value::from_string(&d.key));
    m.insert("old".into(), Value::from_int(d.old));
    m.insert("new".into(), Value::from_int(d.new));
    m.insert("direction".into(), Value::from_string(dir_word(d.direction)));
    Value::from_dict(m)
}

fn metrics_into(m: &mut HashMap<String, Value>, mv: &MetricVector) {
    m.insert("findings".into(), Value::from_int(mv.total as i64));
    m.insert("errors".into(), Value::from_int(mv.errors as i64));
    m.insert("warnings".into(), Value::from_int(mv.warnings as i64));
    m.insert("infos".into(), Value::from_int(mv.infos as i64));
}

/// The active project root + its chronicle store (whole-project scope).
fn project_store(tag: &str) -> Result<(PathBuf, ChronicleStore)> {
    let store = active_store(tag)?;
    let root = store.project_root().to_path_buf();
    let cstore = ChronicleStore::open_for_project(&root).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((root, cstore))
}

word!(w_marks, do_marks);
fn do_marks(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.chronicle.marks";
    let (_root, cstore) = project_store(tag)?;
    let marks = cstore.list_milestones(None).map_err(|e| anyhow!("{tag}: {e}"))?;
    let list: Vec<Value> = marks
        .iter()
        .map(|m| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("label".into(), Value::from_string(&m.label));
            d.insert("ts".into(), Value::from_int(m.ts));
            d.insert(
                "book".into(),
                m.book_slug.as_deref().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            metrics_into(&mut d, &m.metrics);
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(list));
    Ok(vm)
}

word!(w_trend, do_trend);
fn do_trend(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.chronicle.trend";
    let (root, cstore) = project_store(tag)?;
    let mut out: HashMap<String, Value> = HashMap::new();
    match cstore.latest(None).map_err(|e| anyhow!("{tag}: {e}"))? {
        None => {
            out.insert("marked".into(), Value::from_bool(false));
        }
        Some(base) => {
            let (current, current_refs) =
                crate::chronicle::capture(&root, None).map_err(|e| anyhow!("{tag}: {e}"))?;
            let base_refs = cstore.findings_for(base.id).map_err(|e| anyhow!("{tag}: {e}"))?;
            let t = diff_vectors(&base.metrics, &current);
            let fd = diff_findings(&base_refs, &current_refs);
            out.insert("marked".into(), Value::from_bool(true));
            out.insert("since".into(), Value::from_string(&base.label));
            out.insert(
                "headline".into(),
                Value::from_list(t.headline.iter().map(delta_dict).collect()),
            );
            out.insert(
                "categories".into(),
                Value::from_list(t.categories.iter().map(delta_dict).collect()),
            );
            out.insert("cleared".into(), Value::from_int(fd.cleared.len() as i64));
            out.insert("introduced".into(), Value::from_int(fd.introduced.len() as i64));
            out.insert("persisted".into(), Value::from_int(fd.persisted.len() as i64));
        }
    }
    push(vm, Value::from_dict(out));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.chronicle.check";
    let (root, cstore) = project_store(tag)?;
    let mut out: HashMap<String, Value> = HashMap::new();
    match cstore.latest(None).map_err(|e| anyhow!("{tag}: {e}"))? {
        None => {
            // Nothing to compare against — vacuously clean.
            out.insert("baseline".into(), Value::nodata());
            out.insert("cleared".into(), Value::from_int(0));
            out.insert("introduced".into(), Value::from_int(0));
            out.insert("introduced_errors".into(), Value::from_int(0));
            out.insert("clean".into(), Value::from_bool(true));
        }
        Some(base) => {
            let (_current, current_refs) =
                crate::chronicle::capture(&root, None).map_err(|e| anyhow!("{tag}: {e}"))?;
            let base_refs = cstore.findings_for(base.id).map_err(|e| anyhow!("{tag}: {e}"))?;
            let fd = diff_findings(&base_refs, &current_refs);
            let introduced_errors = fd.introduced.iter().filter(|f| f.is_error()).count();
            out.insert("baseline".into(), Value::from_string(&base.label));
            out.insert("cleared".into(), Value::from_int(fd.cleared.len() as i64));
            out.insert("introduced".into(), Value::from_int(fd.introduced.len() as i64));
            out.insert("introduced_errors".into(), Value::from_int(introduced_errors as i64));
            out.insert("clean".into(), Value::from_bool(introduced_errors == 0));
        }
    }
    push(vm, Value::from_dict(out));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direction_words_match_the_serialised_form() {
        assert_eq!(dir_word(Direction::Better), "better");
        assert_eq!(dir_word(Direction::Worse), "worse");
        assert_eq!(dir_word(Direction::Same), "same");
    }

    #[test]
    fn delta_dict_carries_the_documented_keys() {
        let d = TrendDelta { key: "findings".into(), old: 4, new: 2, direction: Direction::Better };
        let m = delta_dict(&d).cast_dict().expect("a dict");
        assert_eq!(m.get("key").and_then(|x| x.cast_string().ok()).as_deref(), Some("findings"));
        assert_eq!(m.get("direction").and_then(|x| x.cast_string().ok()).as_deref(), Some("better"));
        assert_eq!(m.get("new").and_then(|x| x.clone().cast_int().ok()), Some(2));
    }
}
