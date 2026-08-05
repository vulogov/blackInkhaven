//! KEN-1 (KEN-P7) — `ink.knowledge.*` Bund stdlib: the epistemic check (who knows
//! what, when), read-only. `grants` reads the who-could-know-what ledger,
//! `findings` the deterministic breaks, `check` a pass/fail gate. The opt-in
//! `--deep` LLM `implied_irony` pass is **not** exposed (it costs); Bund reads only
//! the always-free deterministic check.
//!
//! - `ink.knowledge.grants`   ( -- list )  {character, topic, chapter, source}
//! - `ink.knowledge.findings` ( -- list )  {kind, severity, chapter, character, topic, message}
//! - `ink.knowledge.check`    ( -- dict )  {premature, leaked, dropped, clean}

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::ken::{self, GrantSource, Grant, KnowledgeFinding};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.knowledge.grants", w_grants),
        ("ink.knowledge.findings", w_findings),
        ("ink.knowledge.check", w_check),
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

fn source_word(s: GrantSource) -> &'static str {
    match s {
        GrantSource::Presence => "presence",
        GrantSource::Declared => "declared",
    }
}

fn grant_dict(g: &Grant) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("character".into(), Value::from_string(&g.character));
    m.insert("topic".into(), Value::from_string(&g.topic));
    m.insert("chapter".into(), Value::from_int(g.at.chapter_ord as i64));
    m.insert("source".into(), Value::from_string(source_word(g.source)));
    Value::from_dict(m)
}

fn finding_dict(f: &KnowledgeFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("kind".into(), Value::from_string(f.kind));
    m.insert("severity".into(), Value::from_string(f.severity.label()));
    m.insert("chapter".into(), Value::from_int(f.chapter as i64));
    m.insert("character".into(), Value::from_string(&f.character));
    m.insert("topic".into(), Value::from_string(&f.topic));
    m.insert("message".into(), Value::from_string(&f.message));
    Value::from_dict(m)
}

/// Resolve the active project's grants (the who-could-know-what ledger).
fn active_grants(tag: &str) -> Result<Vec<Grant>> {
    let store = active_store(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?;
    let (grants, _items, _paras) = ken::grants::build_grants(&layout, &h, book);
    Ok(grants)
}

/// Resolve the active project's deterministic knowledge findings.
fn active_findings(tag: &str) -> Result<Vec<KnowledgeFinding>> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(ken::check::run(&layout, &h, cfg, book))
}

word!(w_grants, do_grants);
fn do_grants(vm: &mut VM) -> Result<&mut VM> {
    let grants = active_grants("ink.knowledge.grants")?;
    push(vm, Value::from_list(grants.iter().map(grant_dict).collect()));
    Ok(vm)
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let findings = active_findings("ink.knowledge.findings")?;
    push(vm, Value::from_list(findings.iter().map(finding_dict).collect()));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let findings = active_findings("ink.knowledge.check")?;
    let (mut premature, mut leaked, mut dropped) = (0i64, 0i64, 0i64);
    for f in &findings {
        match f.kind {
            "premature_knowledge" => premature += 1,
            "leaked_secret" => leaked += 1,
            "dropped_reveal" => dropped += 1,
            _ => {}
        }
    }
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("premature".into(), Value::from_int(premature));
    m.insert("leaked".into(), Value::from_int(leaked));
    m.insert("dropped".into(), Value::from_int(dropped));
    // `clean` = no hard epistemic break (a premature reference or a leaked secret).
    m.insert("clean".into(), Value::from_bool(premature + leaked == 0));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ken::ScenePos;
    use uuid::Uuid;

    #[test]
    fn grant_dict_carries_source_and_chapter() {
        let g = Grant {
            character: "Mara".into(),
            topic: "the betrayal".into(),
            at: ScenePos { chapter_ord: 7, scene_index: 1 },
            source: GrantSource::Declared,
            anchor: Some(Uuid::from_u128(1)),
        };
        let m = grant_dict(&g).cast_dict().expect("a dict");
        assert_eq!(m.get("source").and_then(|v| v.cast_string().ok()).as_deref(), Some("declared"));
        assert_eq!(m.get("chapter").and_then(|v| v.clone().cast_int().ok()), Some(7));
        assert_eq!(m.get("character").and_then(|v| v.cast_string().ok()).as_deref(), Some("Mara"));
    }

    #[test]
    fn source_word_matches_the_variants() {
        assert_eq!(source_word(GrantSource::Presence), "presence");
        assert_eq!(source_word(GrantSource::Declared), "declared");
    }
}
