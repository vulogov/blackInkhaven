//! 3.0.4 Phase-1 — `ink.wordnet.*` Bund stdlib: the multilingual thesaurus,
//! read-only. Phase 1 exposes the installed-sources listing (pure filesystem
//! `exists()` checks — no index load). Sense lookups (`ink.wordnet.lookup`) load
//! the WordNet index and land in a later phase; `fetch` (network) and `import`
//! (file write) are deliberately kept to the CLI.
//!
//! - `ink.wordnet.list` ( -- list )  every known source as {lang, name, installed}.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::push;
use crate::wordnet;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.wordnet.list", w_list)];
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
