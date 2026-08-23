//! BONDS-1 (BD-P5) — `ink.bonds.*` Bund stdlib: the relationship check (are the
//! bonds between characters earned on the page?), read-only. `ties` reads the
//! declared bond ledger, `findings` the deterministic breaks, `check` a pass/fail
//! gate. The opt-in `--deep` LLM `implied_cooling` pass is **not** exposed (it
//! costs); Bund reads only the always-free deterministic check. KEN's sibling —
//! mirrors `ink.knowledge.*`.
//!
//! - `ink.bonds.ties`     ( -- list )  {a, b, kind, chapter}
//! - `ink.bonds.findings` ( -- list )  {kind, severity, chapter, a, b, message}
//! - `ink.bonds.check`    ( -- dict )  {unwritten, unearned, dropped, clean}

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::bonds::{self, BondFinding, Declared};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.bonds.ties", w_ties),
        ("ink.bonds.findings", w_findings),
        ("ink.bonds.check", w_check),
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

fn tie_dict(d: &Declared) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("a".into(), Value::from_string(&d.a));
    m.insert("b".into(), Value::from_string(&d.b));
    m.insert("kind".into(), Value::from_string(&d.kind));
    m.insert("chapter".into(), Value::from_int(d.at.chapter_ord as i64));
    Value::from_dict(m)
}

fn finding_dict(f: &BondFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("kind".into(), Value::from_string(f.kind));
    m.insert("severity".into(), Value::from_string(f.severity.label()));
    m.insert("chapter".into(), Value::from_int(f.chapter as i64));
    m.insert("a".into(), Value::from_string(&f.a));
    m.insert("b".into(), Value::from_string(&f.b));
    m.insert("message".into(), Value::from_string(&f.message));
    Value::from_dict(m)
}

/// Resolve the active project's declared bond ledger.
fn active_ties(tag: &str) -> Result<Vec<Declared>> {
    let store = active_store(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(bonds::ties(&layout, &h, book))
}

/// Resolve the active project's deterministic relationship findings.
fn active_findings(tag: &str) -> Result<Vec<BondFinding>> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(bonds::check::run(&layout, &h, cfg, book))
}

word!(w_ties, do_ties);
fn do_ties(vm: &mut VM) -> Result<&mut VM> {
    let ties = active_ties("ink.bonds.ties")?;
    push(vm, Value::from_list(ties.iter().map(tie_dict).collect()));
    Ok(vm)
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let findings = active_findings("ink.bonds.findings")?;
    push(vm, Value::from_list(findings.iter().map(finding_dict).collect()));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let findings = active_findings("ink.bonds.check")?;
    let (mut unwritten, mut unearned, mut dropped) = (0i64, 0i64, 0i64);
    for f in &findings {
        match f.kind {
            "unwritten_bond" => unwritten += 1,
            "unearned_shift" => unearned += 1,
            "dropped_bond" => dropped += 1,
            _ => {}
        }
    }
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("unwritten".into(), Value::from_int(unwritten));
    m.insert("unearned".into(), Value::from_int(unearned));
    m.insert("dropped".into(), Value::from_int(dropped));
    // `clean` = no hard relationship break (an unearned shift). The advisory
    // Notices (unwritten / dropped) don't fail the gate.
    m.insert("clean".into(), Value::from_bool(unearned == 0));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bonds::ScenePos;
    use uuid::Uuid;

    #[test]
    fn tie_dict_carries_pair_kind_and_chapter() {
        let d = Declared::new(
            "ally",
            "Mara",
            "Kell",
            ScenePos { chapter_ord: 7, scene_index: 1 },
            Uuid::from_u128(1),
        );
        let m = tie_dict(&d).cast_dict().expect("a dict");
        assert_eq!(m.get("kind").and_then(|v| v.cast_string().ok()).as_deref(), Some("ally"));
        assert_eq!(m.get("chapter").and_then(|v| v.clone().cast_int().ok()), Some(7));
        // Declared canonicalizes the pair (Kell < Mara).
        assert_eq!(m.get("a").and_then(|v| v.cast_string().ok()).as_deref(), Some("Kell"));
        assert_eq!(m.get("b").and_then(|v| v.cast_string().ok()).as_deref(), Some("Mara"));
    }

    #[test]
    fn finding_dict_carries_severity_and_pair() {
        let f = BondFinding {
            kind: "unearned_shift",
            severity: crate::bonds::Severity::Break,
            chapter: 9,
            anchor: Some(Uuid::from_u128(2)),
            a: "Kell".into(),
            b: "Mara".into(),
            message: "…".into(),
        };
        let m = finding_dict(&f).cast_dict().expect("a dict");
        assert_eq!(m.get("severity").and_then(|v| v.cast_string().ok()).as_deref(), Some("break"));
        assert_eq!(m.get("a").and_then(|v| v.cast_string().ok()).as_deref(), Some("Kell"));
        assert_eq!(m.get("chapter").and_then(|v| v.clone().cast_int().ok()), Some(9));
    }
}
