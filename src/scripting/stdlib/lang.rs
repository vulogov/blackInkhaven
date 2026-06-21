//! 1.3.21 — `ink.lang.*` Bund stdlib: drive the ConLang Suite from a script.
//!
//! Goal: a whole constructed language — its phonology, grammar, morphology,
//! lexicon — can be **defined and generated from a single Bund script**, as an
//! alternative to hand-authoring HJSON blocks in the Language book. The two
//! representations are equivalent: `ink.lang.define` writes exactly the blocks
//! the book stores, so an author may use JSON/HJSON *or* Bund, freely mixed.
//!
//! Policy: the read-only inspectors (`list` / `generate_word` / `syllabify` /
//! `ipa` / `gloss` / `sentence`) are `store_read` (default-allowed). The
//! mutators (`init` / `define` / `add_word`) are `store_write` (default-denied)
//! — they create book nodes, so they sit behind the same gate as `ink.tree.*`.
//!
//! The engine itself lives in `crate::conlang::*`; the store-based loaders and
//! node-creators are reused from `crate::cli::language` so a Bund-built language
//! is byte-for-byte the same as a hand-authored one.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{
    active_config, active_store, pull, push, require_depth, value_to_i64, value_to_string,
};
use crate::cli::language as langapi;
use crate::conlang::types::TemplateRole;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        // read-only inspectors
        ("ink.lang.list", w_list),
        ("ink.lang.generate_word", w_generate_word),
        ("ink.lang.syllabify", w_syllabify),
        ("ink.lang.ipa", w_ipa),
        ("ink.lang.gloss", w_gloss),
        ("ink.lang.sentence", w_sentence),
        // mutators (store_write)
        ("ink.lang.init", w_init),
        ("ink.lang.define", w_define),
        ("ink.lang.add_word", w_add_word),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    Ok(())
}

fn to_bund_err(e: anyhow::Error) -> BundError {
    easy_error::err_msg(e.to_string())
}

// ── shared loading ───────────────────────────────────────────────────────

/// Resolve the active store + a fresh hierarchy + a named language sub-book.
fn ctx(tag: &str, name: &str) -> Result<(&'static Store, Hierarchy, Node)> {
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = langapi::find_language_book(&hierarchy, name).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok((store, hierarchy, book))
}

// ── read-only inspectors ─────────────────────────────────────────────────

// ( -- list-of-names )
fn w_list(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_list(vm).map_err(to_bund_err)
}
fn do_list(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.list";
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let root = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_LANGUAGES)
    });
    let mut names: Vec<Value> = Vec::new();
    if let Some(root) = root {
        for n in hierarchy.children_of(Some(root.id)) {
            if n.kind == NodeKind::Book {
                names.push(Value::from_string(n.title.clone()));
            }
        }
    }
    push(vm, Value::from_list(names));
    Ok(vm)
}

// ( lang role seed -- word )
fn w_generate_word(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_generate_word(vm).map_err(to_bund_err)
}
fn do_generate_word(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.generate_word";
    require_depth(vm, 3, tag)?;
    let seed = value_to_i64(pull(vm, tag)?, "seed", tag)?;
    let role_s = value_to_string(pull(vm, tag)?, "role", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let role = TemplateRole::parse(&role_s)
        .ok_or_else(|| anyhow!("{tag}: unknown role `{role_s}` (root/prefix/suffix/…)"))?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no phonology block"))?;
    let word = crate::conlang::generate::word::generate_word(&phon, role, seed as u64)
        .ok_or_else(|| anyhow!("{tag}: no `{role_s}` template to generate from"))?;
    push(vm, Value::from_string(word));
    Ok(vm)
}

// ( lang word -- list-of-syllables )
fn w_syllabify(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_syllabify(vm).map_err(to_bund_err)
}
fn do_syllabify(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.syllabify";
    require_depth(vm, 2, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let seq = phon.segment(&word);
    let sylls = crate::conlang::phonology::syllable::syllabify(&phon, &seq);
    let out: Vec<Value> = sylls
        .iter()
        .map(|s| {
            let mut seg = String::new();
            seg.push_str(&s.onset.join(""));
            seg.push_str(&s.nucleus.join(""));
            seg.push_str(&s.coda.join(""));
            Value::from_string(seg)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( lang word -- ipa-surface-string )
fn w_ipa(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ipa(vm).map_err(to_bund_err)
}
fn do_ipa(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.ipa";
    require_depth(vm, 2, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let underlying = phon.segment(&word);
    let surface = crate::conlang::phonology::allophony_eval::surface_form(&phon, &underlying);
    push(vm, Value::from_string(surface.join("")));
    Ok(vm)
}

// ( lang text -- interlinear-gloss-string )
fn w_gloss(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_gloss(vm).map_err(to_bund_err)
}
fn do_gloss(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.gloss";
    require_depth(vm, 2, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let index = crate::conlang::morphology::gloss::build_index(&phon, &morph, &entries);
    let items = index.gloss_text(&text);
    let glossed = items
        .iter()
        .map(|it| {
            format!(
                "{}={}",
                it.surface,
                it.gloss.clone().unwrap_or_else(|| "?".to_string())
            )
        })
        .collect::<Vec<_>>()
        .join(" ");
    push(vm, Value::from_string(glossed));
    Ok(vm)
}

// ( lang subject verb object -- dict{surface,gloss,literal} )
// Each of subject/verb/object is "root" or "root:gloss"; an empty object string
// makes the clause intransitive.
fn w_sentence(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_sentence(vm).map_err(to_bund_err)
}
fn do_sentence(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::syntax::{self, Clause, NounPhrase};
    let tag = "ink.lang.sentence";
    require_depth(vm, 4, tag)?;
    let object = value_to_string(pull(vm, tag)?, "object", tag)?;
    let verb = value_to_string(pull(vm, tag)?, "verb", tag)?;
    let subject = value_to_string(pull(vm, tag)?, "subject", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (grammar_spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;

    let np = |w: &str| {
        if w.trim().is_empty() {
            None
        } else {
            Some(NounPhrase {
                head: langapi::parse_word(w),
                number: "sg".into(),
                adjective: None,
            })
        }
    };
    let clause = Clause {
        subject: np(&subject),
        verb: if verb.trim().is_empty() {
            None
        } else {
            Some(langapi::parse_word(&verb))
        },
        verb_person: "3".into(),
        object: np(&object),
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
        ..Default::default()
    };
    let r = syntax::assemble(&phon, &morph, &grammar_spec.grammar, &clause);
    let gloss = r
        .words
        .iter()
        .map(|(w, g)| format!("{w}={g}"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("surface".into(), Value::from_string(r.surface));
    h.insert("gloss".into(), Value::from_string(gloss));
    h.insert("literal".into(), Value::from_string(r.literal));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ── mutators (store_write) ───────────────────────────────────────────────

// ( name -- )
fn w_init(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_init(vm).map_err(to_bund_err)
}
fn do_init(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.init";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "name", tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    langapi::init_language(store, cfg, &name).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( lang chapter block -- )
// `block` is either an HJSON/JSON string (used verbatim) or a Bund dict/list
// (serialized to JSON). It is written as a paragraph under `chapter`
// (Phonology / Grammar / Sample texts / Meta), exactly as the book stores it.
fn w_define(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_define(vm).map_err(to_bund_err)
}
fn do_define(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.define";
    require_depth(vm, 3, tag)?;
    let block_v = pull(vm, tag)?;
    let chapter = value_to_string(pull(vm, tag)?, "chapter", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;

    // Accept the block as a ready string, or convert a native Bund value.
    let (body, title) = match block_v.clone().cast_string() {
        // Bund stores `\"` / `\\` verbatim (its lexer doesn't unescape), so an
        // author writing a JSON block as a Bund string types `\"` for each
        // quote. Unescape here so the book stores clean HJSON.
        Ok(s) => (bund_unescape(&s), "block".to_string()),
        Err(_) => {
            let json = crate::scripting::value_to_json(&block_v);
            // Name the paragraph after the block's first key for a readable book.
            let title = json
                .as_object()
                .and_then(|m| m.keys().next().cloned())
                .unwrap_or_else(|| "block".to_string());
            let body = serde_json::to_string_pretty(&json)
                .map_err(|e| anyhow!("{tag}: serialize block: {e}"))?;
            (body, title)
        }
    };

    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = langapi::find_language_book(&hierarchy, &name).map_err(|e| anyhow!("{tag}: {e}"))?;
    langapi::create_chapter_paragraph(store, cfg, &book, &chapter, &title, &body)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

/// Undo the literal backslash-escapes a Bund string carries (`\"` → `"`,
/// `\\` → `\`, `\n` → newline), so a JSON/HJSON block typed into a Bund string
/// lands as clean text. Unknown escapes keep both characters.
fn bund_unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

// ( lang word pos translation -- )
fn w_add_word(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_add_word(vm).map_err(to_bund_err)
}
fn do_add_word(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.add_word";
    require_depth(vm, 4, tag)?;
    let translation = value_to_string(pull(vm, tag)?, "translation", tag)?;
    let pos = value_to_string(pull(vm, tag)?, "pos", tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let store = active_store(tag)?;
    let cfg = active_config(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let book = langapi::find_language_book(&hierarchy, &name).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entry = langapi::ImportEntry {
        word,
        pos,
        translation,
        ..Default::default()
    };
    langapi::add_imported_dictionary_entry(store, cfg, &book, &entry)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::bund_unescape;

    #[test]
    fn unescape_recovers_json_quotes() {
        // What a Bund string `"{ipa:\"k\"}"` arrives as → clean HJSON.
        assert_eq!(bund_unescape("{ipa:\\\"k\\\"}"), "{ipa:\"k\"}");
        assert_eq!(bund_unescape("a\\\\b"), "a\\b"); // \\ -> \
        assert_eq!(bund_unescape("x\\ny"), "x\ny"); // \n -> newline
        // unknown escape keeps both chars; plain text untouched
        assert_eq!(bund_unescape("a\\zb"), "a\\zb");
        assert_eq!(bund_unescape("plain"), "plain");
    }
}
