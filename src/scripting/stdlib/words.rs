//! 3.0.6 — `ink.words` Bund stdlib: introspect the `ink.*` surface itself. Lists
//! every registered `ink.*` word with its policy category, so a script (or a
//! writer at the Bund prompt) can discover what's available and which words are
//! gated — without reading the source. Pure VM introspection: touches no store,
//! filesystem, or network.
//!
//! - `ink.words` ( -- list )  every registered `ink.*` word as {word, category},
//!   sorted by word. `category` is the policy class (store_read, store_write,
//!   fs_read, …) or "pure" for the intentionally-uncategorised value transforms.

use std::collections::{HashMap, HashSet};

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::push;
use crate::scripting::policy::{PURE_UNCATEGORISED, WORD_CATEGORIES};

pub fn register(vm: &mut VM) -> Result<()> {
    vm.register_inline("ink.words".to_string(), w_words)
        .map_err(|e| anyhow!("register ink.words: {e}"))?;
    let _ = vm.register_alias("words".to_string(), "ink.words".to_string());
    Ok(())
}

/// ( -- list ) every registered `ink.*` word as {word, category}.
fn w_words(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    let cats: HashMap<&str, &str> = WORD_CATEGORIES.iter().copied().collect();
    let pure: HashSet<&str> = PURE_UNCATEGORISED.iter().copied().collect();

    // `register_inline` keys each handler as `<name>_inline`; aliases live in a
    // separate map, so this enumerates the canonical `ink.*` words only.
    let mut names: Vec<String> = vm
        .inline_fun
        .keys()
        .filter_map(|k| k.strip_suffix("_inline"))
        .filter(|n| n.starts_with("ink."))
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
        // Pure introspection — no project store needed.
        let out = scripting::eval("ink.words").expect("eval");
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
}
