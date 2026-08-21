//! 3.0.4 — `ink.verborum.*` Bund stdlib: the Index Verborum (scholarly-term
//! index), read-only. Bund harvests the same Glossary lexicon terms that appear
//! in prose (whole-word + `term#super[N]` sense tags) that `inkhaven
//! index-verborum` does, and reports the built index / its rendering. Disk reads
//! only — no store write, no LLM.
//!
//! - `ink.verborum.build`  ( -- list )        [{term, original_forms, senses:[{label, gloss, chapters}], chapters}].
//! - `ink.verborum.render` ( fmt -- string )  the compiled index ("md" | "typst" | "json").

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, require_depth, value_to_string};
use crate::cli::index_verborum::{harvest_usages, to_lex_term};
use crate::index_verborum::{self, VerbumEntry};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.verborum.build", w_build), ("ink.verborum.render", w_render)];
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

/// Filter the Glossary to its scholarly-lexicon terms, harvest their prose
/// usages, and build the index entries. The whole-project sweep.
fn build_entries(tag: &str) -> Result<Vec<VerbumEntry>> {
    let store: &Store = active_store(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;

    let lexicon: Vec<crate::glossary::GlossaryEntry> =
        crate::glossary::glossary_entries_from_store(store, &h, None)
            .into_iter()
            .filter(|e| e.is_valid() && e.is_lexicon_term())
            .collect();
    let lex_terms: Vec<_> = lexicon.iter().map(to_lex_term).collect();
    let (usages, sense_usages) =
        harvest_usages(&layout, &h, None, &lexicon).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(index_verborum::build(&lex_terms, &usages, &sense_usages))
}

fn entry_dict(e: &VerbumEntry) -> Value {
    let senses: Vec<Value> = e
        .senses
        .iter()
        .map(|s| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("label".into(), Value::from_string(&s.label));
            m.insert("gloss".into(), Value::from_string(&s.gloss));
            m.insert(
                "chapters".into(),
                Value::from_list(s.chapters.iter().map(Value::from_string).collect()),
            );
            Value::from_dict(m)
        })
        .collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("term".into(), Value::from_string(&e.term));
    m.insert(
        "original_forms".into(),
        Value::from_list(e.original_forms.iter().map(Value::from_string).collect()),
    );
    m.insert("senses".into(), Value::from_list(senses));
    m.insert(
        "chapters".into(),
        Value::from_list(e.chapters.iter().map(Value::from_string).collect()),
    );
    Value::from_dict(m)
}

word!(w_build, do_build);
fn do_build(vm: &mut VM) -> Result<&mut VM> {
    let entries = build_entries("ink.verborum.build")?;
    push(vm, Value::from_list(entries.iter().map(entry_dict).collect()));
    Ok(vm)
}

word!(w_render, do_render);
fn do_render(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.verborum.render";
    require_depth(vm, 1, tag)?;
    let fmt = value_to_string(pull(vm, tag)?, "fmt", tag)?;
    let cfg = active_config(tag)?;
    let entries = build_entries(tag)?;
    let heading = index_verborum::heading_for_language(&cfg.language);
    let text = match fmt.to_lowercase().as_str() {
        "md" | "markdown" => index_verborum::render_md(&entries, heading),
        "typst" | "typ" => index_verborum::render_typst(&entries, heading),
        "json" => index_verborum::render_json(&entries),
        other => return Err(anyhow!("{tag}: unknown format `{other}` (use md | typst | json)")),
    };
    push(vm, Value::from_string(text));
    Ok(vm)
}
