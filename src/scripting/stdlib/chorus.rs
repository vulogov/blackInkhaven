//! CHORUS — `ink.chorus.*` Bund stdlib: voice & style at book scale from
//! scripts. All deterministic (zero-AI), over the single user book.
//!
//! - `ink.chorus.voices`    ( -- list )  per-character voice fingerprints.
//! - `ink.chorus.distinct`  ( -- dict )  the distinctiveness matrix.
//! - `ink.chorus.drift`     ( -- list )  per-character voice drift.
//! - `ink.chorus.headhops`  ( -- list )  POV / head-hop findings.
//! - `ink.chorus.tense`     ( -- dict )  the tense summary (or the honest reason).
//! - `ink.chorus.register`  ( -- dict )  per-chapter register + drifts.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, push};
use crate::chorus::voices::CharacterVoice;
use crate::config::Config;
use crate::dialogue::DialogueStore;
use crate::project::ProjectLayout;
use crate::prose::ProseStore;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.chorus.voices", w_voices),
        ("ink.chorus.distinct", w_distinct),
        ("ink.chorus.drift", w_drift),
        ("ink.chorus.headhops", w_headhops),
        ("ink.chorus.tense", w_tense),
        ("ink.chorus.register", w_register),
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

fn opt(o: Option<f32>) -> Value {
    match o {
        Some(v) => Value::from_float(v as f64),
        None => Value::nodata(),
    }
}

fn ctx(tag: &str) -> Result<(&'static Store, &'static Config, ProjectLayout, Hierarchy, Node)> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = crate::cli::resolve_user_book(&h, None, tag).map_err(|e| anyhow!("{tag}: {e}"))?.clone();
    Ok((store, cfg, layout, h, book))
}

/// Resolve the cast voices (refresh dialogue spans first, like the CLI).
fn cast(tag: &str) -> Result<(&'static Config, Vec<CharacterVoice>)> {
    let (store, cfg, layout, h, book) = ctx(tag)?;
    let now = chrono::Utc::now().to_rfc3339();
    let ds = DialogueStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::dialogue::refresh_book(&ds, &layout, &h, cfg, &book, None, &now)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let pstore = ProseStore::open(store.project_root()).map_err(|e| anyhow!("{tag}: {e}"))?;
    let voices = crate::chorus::voices::character_profiles(&pstore, &ds, cfg, &book, None, &now)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((cfg, voices))
}

macro_rules! word {
    ($w:ident, $do:ident) => {
        fn $w(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
            $do(vm).map_err(to_bund_err)
        }
    };
}

word!(w_voices, do_voices);
fn do_voices(vm: &mut VM) -> Result<&mut VM> {
    let (_cfg, voices) = cast("ink.chorus.voices")?;
    let items: Vec<Value> = voices
        .iter()
        .map(|v| {
            let p = &v.profile;
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("name".into(), Value::from_string(&v.name));
            m.insert("confidence".into(), Value::from_string(v.confidence.label()));
            m.insert("utterances".into(), Value::from_int(v.utterances as i64));
            m.insert("median_sentence_words".into(), Value::from_float(p.p50 as f64));
            m.insert("cv".into(), Value::from_float(p.cv as f64));
            m.insert("mattr".into(), Value::from_float(p.mattr as f64));
            m.insert("modal_density".into(), opt(p.modal_density));
            m.insert("interiority_ratio".into(), opt(p.interiority_ratio));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_distinct, do_distinct);
fn do_distinct(vm: &mut VM) -> Result<&mut VM> {
    let (cfg, voices) = cast("ink.chorus.distinct")?;
    let dm = crate::chorus::distinct::matrix(
        &voices,
        cfg.chorus.distinct_threshold,
        &cfg.chorus.distinct_ignore_pairs,
    );
    let pair = |p: &crate::chorus::distinct::VoicePair| {
        let mut m: HashMap<String, Value> = HashMap::new();
        m.insert("a".into(), Value::from_string(&p.a));
        m.insert("b".into(), Value::from_string(&p.b));
        m.insert("distance".into(), Value::from_float(p.distance as f64));
        Value::from_dict(m)
    };
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert(
        "compared".into(),
        Value::from_list(dm.names.iter().map(|n| Value::from_string(n)).collect()),
    );
    m.insert(
        "indistinguishable".into(),
        Value::from_list(dm.indistinguishable.iter().map(pair).collect()),
    );
    m.insert("closest".into(), dm.closest().map(pair).unwrap_or_else(Value::nodata));
    m.insert("most_distinct".into(), dm.most_distinct().map(pair).unwrap_or_else(Value::nodata));
    push(vm, Value::from_dict(m));
    Ok(vm)
}

word!(w_drift, do_drift);
fn do_drift(vm: &mut VM) -> Result<&mut VM> {
    let (cfg, voices) = cast("ink.chorus.drift")?;
    let items: Vec<Value> = voices
        .iter()
        .filter_map(|v| {
            let vs = crate::chorus::drift::character_drift(v, &cfg.prose.thresholds);
            if vs.is_empty() {
                return None;
            }
            let viols: Vec<Value> = vs
                .iter()
                .map(|x| {
                    let mut mm: HashMap<String, Value> = HashMap::new();
                    mm.insert("chapter".into(), Value::from_int(x.chapter as i64));
                    mm.insert("metric".into(), Value::from_string(x.metric));
                    mm.insert("delta".into(), Value::from_float(x.delta as f64));
                    Value::from_dict(mm)
                })
                .collect();
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("character".into(), Value::from_string(&v.name));
            m.insert("violations".into(), Value::from_list(viols));
            Some(Value::from_dict(m))
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_headhops, do_headhops);
fn do_headhops(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.chorus.headhops";
    let (_store, cfg, layout, h, book) = ctx(tag)?;
    let scenes = crate::chorus::pov::scan_head_hops(&layout, &h, cfg, &book);
    let mut items: Vec<Value> = Vec::new();
    for s in &scenes {
        for hh in &s.hops {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(s.chapter_ord as i64));
            m.insert("scene".into(), Value::from_int(s.scene_index as i64));
            m.insert("pov".into(), Value::from_string(&s.pov.describe()));
            m.insert("experiencer".into(), Value::from_string(&hh.experiencer));
            m.insert("count".into(), Value::from_int(hh.count as i64));
            items.push(Value::from_dict(m));
        }
    }
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_tense, do_tense);
fn do_tense(vm: &mut VM) -> Result<&mut VM> {
    use crate::chorus::tense::TenseSummary;
    let tag = "ink.chorus.tense";
    let (_store, cfg, layout, h, book) = ctx(tag)?;
    let mut m: HashMap<String, Value> = HashMap::new();
    match crate::chorus::tense::scan_tense(&layout, &h, cfg, &book) {
        TenseSummary::Unsupported(reason) => {
            m.insert("supported".into(), Value::from_bool(false));
            m.insert("reason".into(), Value::from_string(reason));
            m.insert("slips".into(), Value::from_list(Vec::new()));
        }
        TenseSummary::Scanned(scenes) => {
            m.insert("supported".into(), Value::from_bool(true));
            let slips: Vec<Value> = scenes
                .iter()
                .flat_map(|s| {
                    s.slips.iter().map(move |sl| {
                        let mut mm: HashMap<String, Value> = HashMap::new();
                        mm.insert("chapter".into(), Value::from_int(s.chapter_ord as i64));
                        mm.insert("scene".into(), Value::from_int(s.scene_index as i64));
                        mm.insert("dominant".into(), Value::from_string(s.dominant.label()));
                        mm.insert("tense".into(), Value::from_string(sl.tense.label()));
                        mm.insert("excerpt".into(), Value::from_string(&sl.excerpt));
                        Value::from_dict(mm)
                    })
                })
                .collect();
            m.insert("slips".into(), Value::from_list(slips));
        }
    }
    push(vm, Value::from_dict(m));
    Ok(vm)
}

word!(w_register, do_register);
fn do_register(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.chorus.register";
    let (_store, cfg, layout, h, book) = ctx(tag)?;
    let r = crate::chorus::register::scan_register(&layout, &h, cfg, &book);
    let chapters: Vec<Value> = r
        .chapters
        .iter()
        .map(|c| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(c.chapter_ord as i64));
            m.insert("contraction_rate".into(), Value::from_float(c.register.contraction_rate as f64));
            m.insert("archaism_density".into(), Value::from_float(c.register.archaism_density as f64));
            m.insert("formality".into(), Value::from_float(c.register.formality as f64));
            m.insert("latinate_density".into(), Value::from_float(c.register.latinate_density as f64));
            Value::from_dict(m)
        })
        .collect();
    let drifts: Vec<Value> = r
        .drifts
        .iter()
        .map(|d| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("chapter".into(), Value::from_int(d.chapter_ord as i64));
            m.insert("metric".into(), Value::from_string(d.metric));
            m.insert("delta".into(), Value::from_float(d.delta as f64));
            Value::from_dict(m)
        })
        .collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("chapters".into(), Value::from_list(chapters));
    m.insert("drifts".into(), Value::from_list(drifts));
    push(vm, Value::from_dict(m));
    Ok(vm)
}
