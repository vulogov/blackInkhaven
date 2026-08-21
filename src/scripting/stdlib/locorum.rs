//! 3.0.4 — `ink.locorum.*` Bund stdlib: the Index Locorum (cited-loci index),
//! read-only. Bund harvests the same `@key[locus]` citations `inkhaven
//! index-locorum` does, resolves each to its Sources-book title + reference
//! scheme, and reports the built index / its rendering / the malformed loci.
//! Disk reads only (the files assembly compiles) — no store write, no LLM.
//!
//! - `ink.locorum.build`     ( -- list )        the index: [{key, title, loci:[{locus, chapters, valid}]}].
//! - `ink.locorum.malformed` ( -- list )        loci that fail their scheme: [{key, title, locus, expected}].
//! - `ink.locorum.render`    ( fmt -- string )  the compiled index ("md" | "typst" | "json").

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_config, active_store, pull, push, require_depth, value_to_string};
use crate::cli::index_locorum::{collect_titles_and_schemes, gather_citations};
use crate::index_locorum;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.locorum.build", w_build),
        ("ink.locorum.malformed", w_malformed),
        ("ink.locorum.render", w_render),
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

/// Harvest → resolve schemes → build the index entries (+ the resolved schemes,
/// which `malformed` also needs). The whole-project sweep (all user books).
fn build_entries(
    tag: &str,
) -> Result<(Vec<index_locorum::LocorumEntry>, HashMap<String, index_locorum::LocusScheme>)> {
    let store: &Store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let layout = ProjectLayout::new(store.project_root());
    let h = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;

    let cites = gather_citations(&layout, &h, None).map_err(|e| anyhow!("{tag}: {e}"))?;
    let (titles, declared) = collect_titles_and_schemes(&layout, &h);
    let mut keys: Vec<String> = cites.iter().map(|c| c.key.clone()).collect();
    keys.sort();
    keys.dedup();
    let (schemes, _unknown) =
        index_locorum::resolve_schemes(&cfg.sources.ref_schemes, &declared, &keys);
    let entries = index_locorum::build(&cites, &titles, &schemes);
    Ok((entries, schemes))
}

fn entry_dict(e: &index_locorum::LocorumEntry) -> Value {
    let loci: Vec<Value> = e
        .loci
        .iter()
        .map(|r| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("locus".into(), Value::from_string(&r.locus));
            m.insert(
                "chapters".into(),
                Value::from_list(r.chapters.iter().map(Value::from_string).collect()),
            );
            m.insert("valid".into(), Value::from_bool(r.valid));
            Value::from_dict(m)
        })
        .collect();
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("key".into(), Value::from_string(&e.key));
    m.insert("title".into(), Value::from_string(&e.title));
    m.insert("loci".into(), Value::from_list(loci));
    Value::from_dict(m)
}

word!(w_build, do_build);
fn do_build(vm: &mut VM) -> Result<&mut VM> {
    let (entries, _schemes) = build_entries("ink.locorum.build")?;
    push(vm, Value::from_list(entries.iter().map(entry_dict).collect()));
    Ok(vm)
}

word!(w_malformed, do_malformed);
fn do_malformed(vm: &mut VM) -> Result<&mut VM> {
    let (entries, schemes) = build_entries("ink.locorum.malformed")?;
    let items: Vec<Value> = index_locorum::malformed(&entries, &schemes)
        .iter()
        .map(|m| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("key".into(), Value::from_string(&m.key));
            d.insert("title".into(), Value::from_string(&m.title));
            d.insert("locus".into(), Value::from_string(&m.locus));
            d.insert("expected".into(), Value::from_string(&m.expected));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

word!(w_render, do_render);
fn do_render(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.locorum.render";
    require_depth(vm, 1, tag)?;
    let fmt = value_to_string(pull(vm, tag)?, "fmt", tag)?;
    let cfg = active_config(tag)?;
    let (entries, _schemes) = build_entries(tag)?;
    let heading = index_locorum::heading_for_language(&cfg.language);
    let text = match fmt.to_lowercase().as_str() {
        "md" | "markdown" => index_locorum::render_md(&entries, heading),
        "typst" | "typ" => index_locorum::render_typst(&entries, heading),
        "json" => index_locorum::render_json(&entries),
        other => return Err(anyhow!("{tag}: unknown format `{other}` (use md | typst | json)")),
    };
    push(vm, Value::from_string(text));
    Ok(vm)
}
