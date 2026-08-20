//! 3.0.4 — `ink.wordnet.*` Bund stdlib: the multilingual thesaurus, read-only.
//! `list` reports the installed sources (pure filesystem `exists()` checks);
//! `lookup` / `suggest` load an installed `.wn` index (an FS read, no network)
//! and return the same senses / replacement picks the in-editor `Ctrl+V Shift+Y`
//! panel shows. `fetch` (network) and `import` (file write) stay CLI-only.
//!
//! - `ink.wordnet.list`    ( -- list )       every known source {lang, name, installed}.
//! - `ink.wordnet.lookup`  ( word lang -- dict )  the senses of `word` in `lang`:
//!   {word, senses:[{pos, definition, synonyms, antonyms, hypernyms, hyponyms}]}.
//! - `ink.wordnet.suggest` ( word lang -- list )  a flat replacement pick-list
//!   {kind, word} (the editor panel's list).

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_string};
use crate::wordnet::{self, SenseView, WordNet};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.wordnet.list", w_list),
        ("ink.wordnet.lookup", w_lookup),
        ("ink.wordnet.suggest", w_suggest),
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

fn installed(lang: &str) -> bool {
    wordnet::index_path(lang).map(|p| p.exists()).unwrap_or(false)
}

fn source_dict(lang: &str, name: &str) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("lang".into(), Value::from_string(lang));
    m.insert("name".into(), Value::from_string(name));
    m.insert("installed".into(), Value::from_bool(installed(lang)));
    Value::from_dict(m)
}

/// ( -- list ) every known WordNet source as {lang, name, installed}. Mirrors
/// `inkhaven wordnet list`: the fetchable en/fr/de/es sources plus the
/// import-only Russian entry.
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let mut items: Vec<Value> =
        wordnet::fetch::SOURCES.iter().map(|s| source_dict(s.lang, s.name)).collect();
    // Russian has no open distribution — the CLI lists it as import-only.
    items.push(source_dict("ru", "RuWordNet — import your own"));
    push(vm, Value::from_list(items));
    Ok(vm)
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

/// Load an installed `.wn` index for `lang`, erroring cleanly when the language
/// isn't installed (`ink.wordnet.list` reports which are). Pure FS read.
fn load_lang(lang: &str, tag: &str) -> Result<WordNet> {
    let path = wordnet::index_path(lang)
        .ok_or_else(|| anyhow!("{tag}: no data directory for wordnet indexes"))?;
    if !path.exists() {
        return Err(anyhow!(
            "{tag}: wordnet for `{lang}` is not installed \
             (see ink.wordnet.list; install with `inkhaven wordnet fetch {lang}`)"
        ));
    }
    WordNet::load(&path).map_err(|e| anyhow!("{tag}: load `{lang}`: {e}"))
}

fn sense_dict(s: &SenseView) -> Value {
    let strs = |xs: &[String]| Value::from_list(xs.iter().map(Value::from_string).collect());
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("pos".into(), Value::from_string(&s.pos));
    m.insert(
        "definition".into(),
        Value::from_string(s.definition.as_deref().unwrap_or("")),
    );
    m.insert("synonyms".into(), strs(&s.synonyms));
    m.insert("antonyms".into(), strs(&s.antonyms));
    m.insert("hypernyms".into(), strs(&s.hypernyms));
    m.insert("hyponyms".into(), strs(&s.hyponyms));
    Value::from_dict(m)
}

/// ( word lang -- dict ) the senses of `word` in `lang`.
fn w_lookup(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_lookup(vm).map_err(to_bund_err)
}
fn do_lookup(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.wordnet.lookup";
    require_depth(vm, 2, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let wn = load_lang(&lang, tag)?;
    let lookup = wn.lookup_with_pivot(&word, None);
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("word".into(), Value::from_string(&lookup.word));
    m.insert(
        "senses".into(),
        Value::from_list(lookup.senses.iter().map(sense_dict).collect()),
    );
    push(vm, Value::from_dict(m));
    Ok(vm)
}

/// ( word lang -- list ) a flat replacement pick-list {kind, word}.
fn w_suggest(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_suggest(vm).map_err(to_bund_err)
}
fn do_suggest(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.wordnet.suggest";
    require_depth(vm, 2, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let wn = load_lang(&lang, tag)?;
    let items: Vec<Value> = wn
        .lookup_with_pivot(&word, None)
        .suggestions()
        .iter()
        .map(|s| {
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("kind".into(), Value::from_string(s.kind));
            m.insert("word".into(), Value::from_string(&s.word));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}
