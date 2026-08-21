//! 3.0.4 Phase-1 — `ink.rigor.*` Bund stdlib: the Inner Rigor reasoning reader,
//! read-only. Bund reads the same deterministic, zero-AI argument-rigor signals
//! (false dichotomy / question-begging / straw man / overgeneralization /
//! non-sequitur / equivocation) that `inkhaven rigor scan` and the in-editor
//! Ctrl+B J→(rigor) check surface. Nothing here calls the LLM or mutates the
//! store — the reader is pure over the manuscript's files.
//!
//! - `ink.rigor.scan`      ( -- list )  every signal across the user books, each a
//!   dict {signal, label, chapter, para_id, description}.
//! - `ink.rigor.check`     ( -- dict )  summary counts by signal + a `clean` gate.
//! - `ink.rigor.paragraph` ( text -- list )  scan one passed-in paragraph string,
//!   each finding a dict {signal, label, description}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, require_depth, value_to_string};
use crate::inner_rigor::{self, RigorFinding};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, Store};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.rigor.scan", w_scan),
        ("ink.rigor.check", w_check),
        ("ink.rigor.paragraph", w_paragraph),
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

fn finding_to_dict(f: &RigorFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("signal".into(), Value::from_string(f.signal.as_code()));
    m.insert("label".into(), Value::from_string(f.signal.label()));
    m.insert("chapter".into(), Value::from_int(f.chapter_ord as i64));
    m.insert("para_id".into(), Value::from_string(&f.para_id));
    m.insert("description".into(), Value::from_string(&f.description));
    Value::from_dict(m)
}

/// Scan every user book (system books excluded — the reader is for prose), the
/// same sweep the whole-project `inkhaven rigor scan` does book-by-book. Opens
/// its own read handle off the active store, like the other reader words.
fn scan_all(tag: &str) -> Result<Vec<RigorFinding>> {
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;

    let mut out: Vec<RigorFinding> = Vec::new();
    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        let entries =
            crate::glossary::glossary_entries_from_store(store, &h, Some(&book.slug));
        let watched = inner_rigor::watched_terms_from_glossary(&entries);
        out.extend(inner_rigor::scan_book(&layout, &h, cfg, book, &watched));
    }
    Ok(out)
}

word!(w_scan, do_scan);
fn do_scan(vm: &mut VM) -> Result<&mut VM> {
    let findings = scan_all("ink.rigor.scan")?;
    push(vm, Value::from_list(findings.iter().map(finding_to_dict).collect()));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let findings = scan_all("ink.rigor.check")?;
    let mut by_signal: HashMap<String, i64> = HashMap::new();
    for f in &findings {
        *by_signal.entry(f.signal.as_code().to_string()).or_insert(0) += 1;
    }
    let mut out: HashMap<String, Value> = HashMap::new();
    out.insert("findings".into(), Value::from_int(findings.len() as i64));
    // `clean` = a simple pass/fail gate: no argument-rigor signals at all.
    out.insert("clean".into(), Value::from_bool(findings.is_empty()));
    out.insert(
        "by_signal".into(),
        Value::from_dict(by_signal.into_iter().map(|(k, v)| (k, Value::from_int(v))).collect()),
    );
    push(vm, Value::from_dict(out));
    Ok(vm)
}

word!(w_paragraph, do_paragraph);
fn do_paragraph(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.rigor.paragraph";
    require_depth(vm, 1, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let cfg = active_config(tag)?;
    let store: &Store = active_store(tag)?;
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;

    // Same glue as the in-editor rigor check: resolve prose language, read the
    // rigor category toggles, and gather the glossary's equivocation-watched
    // terms so the single-paragraph pass matches the whole-book pass.
    let (lang, _note) =
        crate::prose::resolve_prose_language(cfg.rigor.language.as_deref(), &cfg.language);
    let cats = inner_rigor::RigorCats::from_config(&cfg.rigor);
    let entries = crate::glossary::glossary_entries_from_store(store, &h, None);
    let watched = inner_rigor::watched_terms_from_glossary(&entries);

    let plain = crate::audiobook::typst_to_plain(&text);
    let findings = inner_rigor::detect_paragraph(&plain, &lang, &cats, &watched);
    let items: Vec<Value> = findings
        .iter()
        .map(|(signal, description)| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("signal".into(), Value::from_string(signal.as_code()));
            m.insert("label".into(), Value::from_string(signal.label()));
            m.insert("description".into(), Value::from_string(description));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}
