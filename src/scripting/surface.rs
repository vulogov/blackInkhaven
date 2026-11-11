//! The Bund scripting surface as the VM sees it (3.21): the registered
//! `ink.*` / `calc.*` words and their short aliases, behind two docs-drift
//! tests. `policy::tests::word_reference_doc_matches_the_policy_table` already
//! pins `Documentation/Bund/WORD_REFERENCE.md` to the policy table; these
//! tests cover the rest of the documentation — every `ink.*` token any guide
//! or tutorial names must be a registered word, an alias, or a namespace — and
//! the reference's promise that every word answers to its `ink.`-less alias.
#![cfg(test)]

use std::collections::{BTreeMap, BTreeSet};

use rust_multistackvm::multistackvm::VM;

/// The words inkhaven adds to a bare VM (canonical names, `_inline` stripped)
/// and the alias → canonical map it registers alongside them.
fn registered() -> (BTreeSet<String>, BTreeMap<String, String>) {
    let (words, aliases, _) = registered_with_base();
    (words, aliases)
}

/// [`registered`] plus the words bundcore itself provides (canonical names).
fn registered_with_base() -> (BTreeSet<String>, BTreeMap<String, String>, BTreeSet<String>) {
    let mut vm = VM::new();
    let before: BTreeSet<String> = vm.inline_fun.keys().cloned().collect();
    crate::scripting::stdlib::register_ink_stdlib(&mut vm).expect("register the ink stdlib");
    let words: BTreeSet<String> = vm
        .inline_fun
        .keys()
        .filter(|k| !before.contains(*k))
        .filter_map(|k| k.strip_suffix("_inline").map(String::from))
        .collect();
    let aliases: BTreeMap<String, String> = vm
        .name_mapping
        .iter()
        .filter(|(_, canonical)| words.contains(*canonical))
        .map(|(a, c)| (a.clone(), c.clone()))
        .collect();
    let base: BTreeSet<String> =
        before.iter().filter_map(|k| k.strip_suffix("_inline").map(String::from)).collect();
    (words, aliases, base)
}

/// Every `ink.…` / `calc.…` token in `text` with its 1-based line: a run of
/// identifier characters and dots that starts at a word boundary. A trailing
/// `.` or `.*` (prose punctuation, a namespace glob) is trimmed.
fn word_tokens(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let bytes = line.as_bytes();
        for prefix in ["ink.", "calc."] {
            let mut from = 0;
            while let Some(pos) = line[from..].find(prefix) {
                let start = from + pos;
                let boundary = start == 0 || {
                    let c = bytes[start - 1] as char;
                    !(c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '-' || c == '/')
                };
                let mut end = start;
                while end < line.len()
                    && line[end..]
                        .chars()
                        .next()
                        .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '*')
                {
                    end += 1;
                }
                if boundary {
                    let tok = line[start..end].trim_end_matches(['.', '*']).to_string();
                    if tok.len() > prefix.len() {
                        out.push((i + 1, tok));
                    }
                }
                from = end.max(start + 1);
            }
        }
    }
    out
}

/// Every `ink.*` / `calc.*` name the documentation shows must be a registered
/// word, a registered alias, or the namespace of registered words
/// (`ink.node` → `ink.node.list`…). Release notes and proposals are history
/// and are not checked (same set as the CLI drift test).
#[test]
fn docs_name_only_registered_bund_words() {
    let (words, aliases) = registered();
    let mut unknown: Vec<String> = Vec::new();
    for (file, text) in crate::cli::surface::docs() {
        for (line, tok) in word_tokens(&text) {
            let ok = words.contains(&tok)
                || aliases.contains_key(&tok)
                || words.iter().any(|w| w.starts_with(&format!("{tok}.")));
            if !ok {
                unknown.push(format!("{file}:{line}: `{tok}`"));
            }
        }
    }
    assert!(
        unknown.is_empty(),
        "{} documented Bund word(s) are not registered (fix the doc, or register the word):\n{}",
        unknown.len(),
        unknown.join("\n")
    );
}

/// `WORD_REFERENCE.md`: "Every word also answers to its short alias — the same
/// name without the `ink.` prefix". Hold the stdlib to it; a short name that
/// bundcore already owns, or that another word claimed first, is the allowed
/// exception (reported, not failed, so the reference can list it).
#[test]
fn every_ink_word_answers_to_its_short_alias() {
    let (words, aliases, base) = registered_with_base();
    let mut missing: Vec<String> = Vec::new();
    for w in words.iter().filter(|w| w.starts_with("ink.")) {
        let short = &w[4..];
        if base.contains(short) {
            println!("{w}: short name `{short}` is bundcore's — no alias");
            continue;
        }
        if let Some(other) = aliases.get(short).filter(|c| *c != w) {
            println!("{w}: short name `{short}` is already `{other}`'s alias");
            continue;
        }
        let ok = aliases.get(short).is_some_and(|c| c == w) || words.contains(short);
        if !ok {
            missing.push(w.clone());
        }
    }
    assert!(
        missing.is_empty(),
        "{} `ink.*` word(s) have no short alias:\n{}",
        missing.len(),
        missing.join("\n")
    );
}

#[test]
fn token_scanner_reads_prose_tables_and_globs() {
    let text = "Use `ink.node.list` and `ink.io.message.*` here.\n\
                | `calc.world.get` | store_read |\n\
                a sentence ending with ink.tag.list.\n\
                the thinking.ink.not a word, nor inkhaven.ink.x\n";
    let got: Vec<String> = word_tokens(text).into_iter().map(|(l, t)| format!("{l}:{t}")).collect();
    assert_eq!(
        got,
        vec!["1:ink.node.list", "1:ink.io.message", "2:calc.world.get", "3:ink.tag.list"]
    );
}

/// The inventory, for a maintainer: `cargo test --bin inkhaven -- --ignored
/// --nocapture bund_surface_inventory`.
#[test]
#[ignore]
fn bund_surface_inventory() {
    let (words, aliases) = registered();
    println!("{} words, {} aliases", words.len(), aliases.len());
    for w in &words {
        let a: Vec<&str> = aliases.iter().filter(|(_, c)| *c == w).map(|(a, _)| a.as_str()).collect();
        println!("{w}  ←  {}", a.join(", "));
    }
}
