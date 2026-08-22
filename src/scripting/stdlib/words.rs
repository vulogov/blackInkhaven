//! 3.0.6 — `ink.words` Bund stdlib: introspect the `ink.*` surface itself. Lists
//! every registered `ink.*` word with its policy category, so a script (or a
//! writer at the Bund prompt) can discover what's available and which words are
//! gated — without reading the source. Pure VM introspection: touches no store,
//! filesystem, or network.
//!
//! - `ink.words` ( prefix -- list )  the registered `ink.*` words as
//!   {word, category}, sorted by word, keeping only those starting with `prefix`
//!   (an empty string lists everything). `category` is the policy class
//!   (store_read, store_write, fs_read, …) or "pure" for the intentionally-
//!   uncategorised value transforms.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_string};
use crate::scripting::policy::{PURE_UNCATEGORISED, WORD_CATEGORIES};

pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.words".to_string(), w_words)
        .map_err(|e| anyhow!("register ink.words: {e}"))?;
    let _ = vm.register_alias("words".to_string(), "ink.words".to_string());
    Ok(())
}

/// ( prefix -- list ) the registered `ink.*` words as {word, category}, filtered
/// to those starting with `prefix` ("" = all).
fn w_words(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_words(vm).map_err(|e| easy_error::err_msg(e.to_string()))
}
fn do_words(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.words";
    require_depth(vm, 1, tag)?;
    let prefix = value_to_string(pull(vm, tag)?, "prefix", tag)?;

    let cats: HashMap<&str, &str> = WORD_CATEGORIES.iter().copied().collect();
    let pure: HashSet<&str> = PURE_UNCATEGORISED.iter().copied().collect();

    // `register_inline` keys each handler as `<name>_inline`; aliases live in a
    // separate map, so this enumerates the canonical `ink.*` words only.
    let mut names: Vec<String> = vm
        .inline_fun
        .keys()
        .filter_map(|k| k.strip_suffix("_inline"))
        .filter(|n| n.starts_with("ink."))
        .filter(|n| prefix.is_empty() || n.starts_with(prefix.as_str()))
        .map(String::from)
        .collect();
    names.sort();
    names.dedup();

    let items: Vec<Value> = names
        .iter()
        .map(|n| {
            let category = cats
                .get(n.as_str())
                .copied()
                .unwrap_or(if pure.contains(n.as_str()) { "pure" } else { "uncategorised" });
            let mut m: HashMap<String, Value> = HashMap::new();
            m.insert("word".into(), Value::from_string(n));
            m.insert("category".into(), Value::from_string(category));
            Value::from_dict(m)
        })
        .collect();
    push(vm, Value::from_list(items));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use crate::scripting;

    #[test]
    fn words_lists_the_ink_surface_with_categories() {
        // Pure introspection — no project store needed. "" = every word.
        let out = scripting::eval("\"\" ink.words").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert!(list.len() > 100, "the ink.* surface is large (got {})", list.len());

        // Every row carries a word + a category, and a known word resolves to its
        // policy class (proving the table join works, not just the enumeration).
        let mut found_rigor = false;
        for row in &list {
            let d = row.cast_dict().expect("a dict");
            assert!(d.contains_key("word") && d.contains_key("category"));
            if d.get("word").and_then(|v| v.cast_string().ok()).as_deref() == Some("ink.rigor.scan")
            {
                found_rigor = true;
                assert_eq!(
                    d.get("category").and_then(|v| v.cast_string().ok()).as_deref(),
                    Some("store_read"),
                );
            }
        }
        assert!(found_rigor, "ink.rigor.scan should appear in ink.words");
    }

    #[test]
    fn words_prefix_filters() {
        let out = scripting::eval("\"ink.rigor\" ink.words").expect("eval");
        let list = out.top.expect("a result").cast_list().expect("a list");
        assert_eq!(list.len(), 3, "three ink.rigor.* words");
        for row in &list {
            let d = row.cast_dict().expect("a dict");
            let w = d.get("word").and_then(|v| v.cast_string().ok()).unwrap_or_default();
            assert!(w.starts_with("ink.rigor"), "prefix filter leaked `{w}`");
        }
    }
}
