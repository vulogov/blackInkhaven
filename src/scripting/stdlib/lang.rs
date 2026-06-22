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
        // ── read-only inspectors (store_read) ──
        ("ink.lang.list", w_list),
        ("ink.lang.generate_word", w_generate_word),
        ("ink.lang.syllabify", w_syllabify),
        ("ink.lang.ipa", w_ipa),
        ("ink.lang.stress", w_stress),
        ("ink.lang.tone", w_tone),
        ("ink.lang.transliterate", w_transliterate),
        ("ink.lang.gloss", w_gloss),
        ("ink.lang.paradigm", w_paradigm),
        ("ink.lang.derive", w_derive),
        ("ink.lang.agree", w_agree),
        ("ink.lang.sentence", w_sentence),
        ("ink.lang.translate", w_translate),
        ("ink.lang.reverse", w_reverse),
        ("ink.lang.cross", w_cross),
        ("ink.lang.memory", w_memory),
        ("ink.lang.corpus", w_corpus),
        ("ink.lang.eval", w_eval),
        ("ink.lang.relative", w_relative),
        ("ink.lang.complement", w_complement),
        ("ink.lang.coordinate", w_coordinate),
        ("ink.lang.stats", w_stats),
        ("ink.lang.audit", w_audit),
        ("ink.lang.query", w_query),
        ("ink.lang.gaps", w_gaps),
        ("ink.lang.sound_change", w_sound_change),
        ("ink.lang.cognates", w_cognates),
        ("ink.lang.family_tree", w_family_tree),
        ("ink.lang.names", w_names),
        ("ink.lang.prose", w_prose),
        ("ink.lang.poem", w_poem),
        ("ink.lang.varieties", w_varieties),
        ("ink.lang.lect", w_lect),
        ("ink.lang.borrow", w_borrow),
        ("ink.lang.areal", w_areal),
        ("ink.lang.idiolect", w_idiolect),
        ("ink.lang.ecology", w_ecology),
        // ── pure data constructor (uncategorised, allowed) ──
        ("ink.lang.dict", w_dict),
        // ── mutators (store_write) ──
        ("ink.lang.init", w_init),
        ("ink.lang.define", w_define),
        ("ink.lang.add_word", w_add_word),
        ("ink.lang.remember", w_remember),
        ("ink.lang.remove_word", w_remove_word),
        ("ink.lang.derive_add", w_derive_add),
        ("ink.lang.grammar_set", w_grammar_set),
        ("ink.lang.idiom_add", w_idiom_add),
        ("ink.lang.metaphor_add", w_metaphor_add),
        // ── AI-backed (ai_write; advisory) ──
        ("ink.lang.compose", w_compose),
        ("ink.lang.reconstruct", w_reconstruct),
        ("ink.lang.realism_check", w_realism_check),
        ("ink.lang.generate_lexicon", w_generate_lexicon),
        // ── file / document output (fs_write; glyph_draft also ai_write) ──
        ("ink.lang.glyph_lint", w_glyph_lint),
        ("ink.lang.dictionary", w_dictionary),
        ("ink.lang.grammar_book", w_grammar_book),
        ("ink.lang.font_build", w_font_build),
        ("ink.lang.glyph_draft", w_glyph_draft),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f)
            .map_err(|e| anyhow!("register {name}: {e}"))?;
    }

    // Ergonomic aliases. Every `ink.lang.X` also answers to the shorter
    // `lang.X`; the data constructor `ink.lang.dict` additionally answers to
    // self-documenting names (`word`, `rule`, `phoneme`, `block`) so a script
    // reads like the artefact it builds. Aliases resolve to the target word, so
    // they inherit its policy gate.
    for (name, _) in words {
        if let Some(short) = name.strip_prefix("ink.") {
            let _ = vm.register_alias(short.to_string(), name.to_string());
        }
    }
    for sugar in ["word", "rule", "phoneme", "block"] {
        let _ = vm.register_alias(sugar.to_string(), "ink.lang.dict".to_string());
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

// ( lang text -- dict{surface,gloss,literal,confidence,unresolved} )
// LANG-3 Tier 1 (RBMT): translate English into the conlang, deterministically.
fn w_translate(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_translate(vm).map_err(to_bund_err)
}
fn do_translate(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate;
    let tag = "ink.lang.translate";
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
    let (grammar_spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    // Tier 2 (retrieval): layer the translation memory over the rule-based result.
    let mem = translate::memory::TranslationMemory::load(store.project_root(), &name)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let query_embedding: Option<Vec<f32>> = if mem.is_empty() {
        None
    } else {
        store.embed_batch(&[text.as_str()]).ok().and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
    };
    let t = translate::apply_memory(
        translate::translate(&phon, &morph, &grammar_spec.grammar, &entries, &text),
        &mem,
        query_embedding.as_deref(),
    );
    let gloss =
        t.words.iter().map(|(w, g)| format!("{w}={g}")).collect::<Vec<_>>().join(" ");
    let alts: Vec<Value> = t
        .alternatives
        .iter()
        .map(|a| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("text".into(), Value::from_string(a.text.clone()));
            d.insert("rationale".into(), Value::from_string(a.rationale.clone()));
            Value::from_dict(d)
        })
        .collect();
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("surface".into(), Value::from_string(t.target));
    h.insert("gloss".into(), Value::from_string(gloss));
    h.insert("literal".into(), Value::from_string(t.literal));
    h.insert("confidence".into(), Value::from_float(t.confidence as f64));
    h.insert(
        "unresolved".into(),
        Value::from_list(t.unresolved.into_iter().map(Value::from_string).collect()),
    );
    h.insert("alternatives".into(), Value::from_list(alts));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( lang surface -- dict{english,gloss,confidence,unresolved} )
// LANG-3 Tier 1 (RBMT): reverse a conlang sentence back into English.
fn w_reverse(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_reverse(vm).map_err(to_bund_err)
}
fn do_reverse(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::reverse;
    let tag = "ink.lang.reverse";
    require_depth(vm, 2, tag)?;
    let surface = value_to_string(pull(vm, tag)?, "surface", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let r = reverse::reverse(&phon, &morph, &spec.grammar, &entries, &surface);
    let gloss = r.words.iter().map(|(w, g)| format!("{w}={g}")).collect::<Vec<_>>().join(" ");
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("english".into(), Value::from_string(r.english));
    h.insert("gloss".into(), Value::from_string(gloss));
    h.insert("confidence".into(), Value::from_float(r.confidence as f64));
    h.insert(
        "unresolved".into(),
        Value::from_list(r.unresolved.into_iter().map(Value::from_string).collect()),
    );
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( from to surface -- dict{english,target,gloss,confidence,unresolved} )
// LANG-3 Tier 1 (RBMT): cross-translate conlang `from` → `to` via English.
fn w_cross(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_cross(vm).map_err(to_bund_err)
}
fn do_cross(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::reverse::{self, LangCtx};
    let tag = "ink.lang.cross";
    require_depth(vm, 3, tag)?;
    let surface = value_to_string(pull(vm, tag)?, "surface", tag)?;
    let to = value_to_string(pull(vm, tag)?, "to", tag)?;
    let from = value_to_string(pull(vm, tag)?, "from", tag)?;
    let (store, hierarchy, from_book) = ctx(tag, &from)?;
    let to_book =
        langapi::find_language_book(&hierarchy, &to).map_err(|e| anyhow!("{tag}: {e}"))?;

    let load = |book: &Node| -> Result<_> {
        let phon = langapi::load_phonology(store, &hierarchy, book)
            .map_err(|e| anyhow!("{tag}: {e}"))?
            .unwrap_or_default();
        let morph = langapi::load_morphology(store, &hierarchy, book)
            .map_err(|e| anyhow!("{tag}: {e}"))?
            .unwrap_or_default();
        let (spec, _) =
            langapi::load_grammar_spec(store, &hierarchy, book).map_err(|e| anyhow!("{tag}: {e}"))?;
        let entries =
            langapi::load_dictionary(store, &hierarchy, book).map_err(|e| anyhow!("{tag}: {e}"))?;
        Ok((phon, morph, spec, entries))
    };
    let (f_phon, f_morph, f_spec, f_entries) = load(&from_book)?;
    let (t_phon, t_morph, t_spec, t_entries) = load(&to_book)?;
    let fromctx =
        LangCtx { phon: &f_phon, morph: &f_morph, typology: &f_spec.grammar, entries: &f_entries };
    let toctx =
        LangCtx { phon: &t_phon, morph: &t_morph, typology: &t_spec.grammar, entries: &t_entries };
    let c = reverse::cross(&fromctx, &toctx, &surface);
    let gloss = c.words.iter().map(|(w, g)| format!("{w}={g}")).collect::<Vec<_>>().join(" ");
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("english".into(), Value::from_string(c.english));
    h.insert("target".into(), Value::from_string(c.target));
    h.insert("gloss".into(), Value::from_string(gloss));
    h.insert("confidence".into(), Value::from_float(c.confidence as f64));
    h.insert(
        "unresolved".into(),
        Value::from_list(c.unresolved.into_iter().map(Value::from_string).collect()),
    );
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( lang -- list-of-{english,conlang} )  the translation memory's pairs
fn w_memory(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_memory(vm).map_err(to_bund_err)
}
fn do_memory(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::memory::TranslationMemory;
    let tag = "ink.lang.memory";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let store = active_store(tag)?;
    let mem = TranslationMemory::load(store.project_root(), &name)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let out: Vec<Value> = mem
        .entries()
        .map(|(en, con)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("english".into(), Value::from_string(en.to_string()));
            d.insert("conlang".into(), Value::from_string(con.to_string()));
            Value::from_dict(d)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( lang -- {scanned,accepted,acceptance_rate,top_missing,pairs} )  synthetic corpus
// over the bundled English pool (advisory — seed via add to memory with lang.remember).
fn w_corpus(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_corpus(vm).map_err(to_bund_err)
}
fn do_corpus(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::corpus;
    let tag = "ink.lang.corpus";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let pool = corpus::parse_pool(corpus::BUNDLED_POOL);
    let report = corpus::generate(&phon, &morph, &spec.grammar, &entries, &pool);
    let pairs: Vec<Value> = report
        .accepted
        .iter()
        .map(|(en, con)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("english".into(), Value::from_string(en.clone()));
            d.insert("conlang".into(), Value::from_string(con.clone()));
            Value::from_dict(d)
        })
        .collect();
    let missing: Vec<Value> = report
        .top_missing(10)
        .into_iter()
        .map(|(w, n)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("word".into(), Value::from_string(w));
            d.insert("count".into(), Value::from_int(n as i64));
            Value::from_dict(d)
        })
        .collect();
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("scanned".into(), Value::from_int(report.scanned as i64));
    h.insert("accepted".into(), Value::from_int(report.accepted.len() as i64));
    h.insert("acceptance_rate".into(), Value::from_float(report.acceptance_rate() as f64));
    h.insert("top_missing".into(), Value::from_list(missing));
    h.insert("pairs".into(), Value::from_list(pairs));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( lang -- {sentences,covered,coverage,round_trip_similarity} )  evaluate quality
// (round-trip similarity + coverage) over the bundled English pool.
fn w_eval(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_eval(vm).map_err(to_bund_err)
}
fn do_eval(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::{corpus, eval};
    let tag = "ink.lang.eval";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let sentences = corpus::parse_pool(corpus::BUNDLED_POOL);
    let items = eval::round_trip_all(&phon, &morph, &spec.grammar, &entries, &sentences);
    let coverage = eval::coverage(&items);
    let covered: Vec<&eval::RoundTrip> = items.iter().filter(|i| i.covered).collect();
    let round_trip = if covered.is_empty() {
        Value::nodata()
    } else {
        let mut texts: Vec<&str> = Vec::with_capacity(covered.len() * 2);
        for i in &covered {
            texts.push(i.english.as_str());
            texts.push(i.recovered.as_str());
        }
        match store.embed_batch(&texts) {
            Ok(emb) => {
                let sum: f32 =
                    (0..covered.len()).map(|k| cosine_sim(&emb[2 * k], &emb[2 * k + 1])).sum();
                Value::from_float((sum / covered.len() as f32) as f64)
            }
            Err(_) => Value::nodata(),
        }
    };
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("sentences".into(), Value::from_int(items.len() as i64));
    h.insert("covered".into(), Value::from_int(covered.len() as i64));
    h.insert("coverage".into(), Value::from_float(coverage as f64));
    h.insert("round_trip_similarity".into(), round_trip);
    push(vm, Value::from_dict(h));
    Ok(vm)
}

/// Cosine similarity of two embedding vectors.
fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

// ── mutators (store_write) ───────────────────────────────────────────────

// ( lang english conlang -- )  remember a confirmed translation (correction loop)
fn w_remember(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_remember(vm).map_err(to_bund_err)
}
fn do_remember(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::translate::memory::TranslationMemory;
    let tag = "ink.lang.remember";
    require_depth(vm, 3, tag)?;
    let conlang = value_to_string(pull(vm, tag)?, "conlang", tag)?;
    let english = value_to_string(pull(vm, tag)?, "english", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let store = active_store(tag)?;
    let root = store.project_root();
    let mut mem = TranslationMemory::load(root, &name).map_err(|e| anyhow!("{tag}: {e}"))?;
    mem.add(&english, &conlang);
    // Cache the embedding so a later semantic lookup only embeds the query.
    let need = mem.needs_embeddings();
    if !need.is_empty() {
        let refs: Vec<&str> = need.iter().map(String::as_str).collect();
        if let Ok(vecs) = store.embed_batch(&refs) {
            for (en, v) in need.iter().zip(vecs) {
                mem.set_embedding(en, v);
            }
        }
    }
    mem.save(root, &name).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

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

// ── native artefact construction (the user-suggested aliasing path) ──────
//
// Bund's `{ … }` builds a lambda/ctx, not a dict, so `ink.lang.dict` turns a
// flat Bund list `[ key val key val … ]` into a real dict Value. Nest it (and
// the `[ … ]` list literal) to build any artefact natively — then hand it to
// `ink.lang.define` with no string escaping. Authors typically alias it, e.g.
// `'ink.lang.dict alias d` or define `word` / `rule` constructors on top.
//
// ( [k v k v …] -- dict )
fn w_dict(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_dict(vm).map_err(to_bund_err)
}
fn do_dict(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.dict";
    require_depth(vm, 1, tag)?;
    let list = pull(vm, tag)?
        .cast_list()
        .map_err(|e| anyhow!("{tag}: expected a list of key/value pairs: {e}"))?;
    if list.len() % 2 != 0 {
        return Err(anyhow!(
            "{tag}: list has an odd length ({}) — it must be alternating key value pairs",
            list.len()
        ));
    }
    let mut h: HashMap<String, Value> = HashMap::new();
    let mut it = list.into_iter();
    while let (Some(k), Some(v)) = (it.next(), it.next()) {
        let key = k
            .cast_string()
            .map_err(|e| anyhow!("{tag}: dict key must be a string: {e}"))?;
        h.insert(key, v);
    }
    push(vm, Value::from_dict(h));
    Ok(vm)
}

/// `serde_json::Value` → Bund `Value`, the inverse of `value_to_json` — lets any
/// `Serialize` engine output return as native Bund data (dicts / lists).
fn json_to_value(j: &serde_json::Value) -> Value {
    match j {
        serde_json::Value::Null => Value::nodata(),
        serde_json::Value::Bool(b) => Value::from_bool(*b),
        serde_json::Value::Number(n) => n
            .as_i64()
            .map(Value::from_int)
            .unwrap_or_else(|| Value::from_float(n.as_f64().unwrap_or(0.0))),
        serde_json::Value::String(s) => Value::from_string(s.clone()),
        serde_json::Value::Array(a) => Value::from_list(a.iter().map(json_to_value).collect()),
        serde_json::Value::Object(m) => {
            let mut h: HashMap<String, Value> = HashMap::new();
            for (k, v) in m {
                h.insert(k.clone(), json_to_value(v));
            }
            Value::from_dict(h)
        }
    }
}

/// Serialize anything `Serialize` straight to a Bund value.
fn serial_to_value<T: serde::Serialize>(tag: &str, v: &T) -> Result<Value> {
    let j = serde_json::to_value(v).map_err(|e| anyhow!("{tag}: serialize: {e}"))?;
    Ok(json_to_value(&j))
}

/// Parse a `key=val,key=val` feature string into a map.
fn parse_features(s: &str) -> std::collections::BTreeMap<String, String> {
    let mut m = std::collections::BTreeMap::new();
    for pair in s.split(',') {
        if let Some((k, v)) = pair.split_once('=') {
            let (k, v) = (k.trim(), v.trim());
            if !k.is_empty() {
                m.insert(k.to_string(), v.to_string());
            }
        }
    }
    m
}

fn rendered_to_dict(r: &crate::conlang::syntax::RenderedClause) -> Value {
    let gloss = r
        .words
        .iter()
        .map(|(w, g)| format!("{w}={g}"))
        .collect::<Vec<_>>()
        .join(" ");
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("surface".into(), Value::from_string(r.surface.clone()));
    h.insert("gloss".into(), Value::from_string(gloss));
    h.insert("literal".into(), Value::from_string(r.literal.clone()));
    Value::from_dict(h)
}

// ── phonology inspectors ─────────────────────────────────────────────────

// ( lang word -- stress-marked-string )
fn w_stress(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_stress(vm).map_err(to_bund_err)
}
fn do_stress(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::phonology::{stress_eval, syllable};
    let tag = "ink.lang.stress";
    require_depth(vm, 2, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let rule = phon
        .stress
        .clone()
        .ok_or_else(|| anyhow!("{tag}: language `{name}` declares no `stress` rule"))?;
    let seq = phon.segment(&word);
    let sylls = syllable::syllabify(&phon, &seq);
    let stressed = stress_eval::primary_stress(&rule, &sylls);
    let marked = sylls
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let body = format!("{}{}{}", s.onset.join(""), s.nucleus.join(""), s.coda.join(""));
            if Some(i) == stressed {
                format!("ˈ{body}")
            } else {
                body
            }
        })
        .collect::<Vec<_>>()
        .join(".");
    push(vm, Value::from_string(marked));
    Ok(vm)
}

// ( lang tones -- result )  tones e.g. "3 3 3" → "2 2 3"
fn w_tone(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_tone(vm).map_err(to_bund_err)
}
fn do_tone(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.tone";
    require_depth(vm, 2, tag)?;
    let tones = value_to_string(pull(vm, tag)?, "tones", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let system = phon
        .tone
        .clone()
        .ok_or_else(|| anyhow!("{tag}: language `{name}` declares no `tone` system"))?;
    let seq: Vec<String> = tones.split_whitespace().map(String::from).collect();
    let out = crate::conlang::phonology::tone_eval::apply_sandhi(&system, &seq);
    push(vm, Value::from_string(out.join(" ")));
    Ok(vm)
}

// ( lang text -- script )
fn w_transliterate(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_transliterate(vm).map_err(to_bund_err)
}
fn do_transliterate(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.transliterate";
    require_depth(vm, 2, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let font = langapi::load_font_config(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no font config to transliterate with"))?;
    let out = crate::conlang::writing::input::to_script(&font, &text);
    push(vm, Value::from_string(out.script));
    Ok(vm)
}

// ── variety inspectors (LANG-2 P1) ───────────────────────────────────────

// ( lang -- list-of-variety-dicts )
fn w_varieties(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_varieties(vm).map_err(to_bund_err)
}
fn do_varieties(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.varieties";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let vs = langapi::load_varieties(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let out: Vec<Value> = vs
        .varieties
        .iter()
        .map(|v| {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert("id".into(), Value::from_string(v.id.clone()));
            h.insert("kind".into(), Value::from_string(v.kind.clone()));
            h.insert("axis".into(), Value::from_string(v.axis.clone()));
            h.insert(
                "prestige".into(),
                v.prestige.clone().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            h.insert("sound_changes".into(), Value::from_int(v.sound_changes.len() as i64));
            h.insert("overrides".into(), Value::from_int(v.lexicon.len() as i64));
            h.insert(
                "note".into(),
                v.note.clone().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( lang variety word -- rendered )  render a base form in a variety
fn w_lect(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_lect(vm).map_err(to_bund_err)
}
fn do_lect(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.lect";
    require_depth(vm, 3, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let variety = value_to_string(pull(vm, tag)?, "variety", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let vs = langapi::load_varieties(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let v = vs
        .get(&variety)
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no variety `{variety}`"))?;
    push(vm, Value::from_string(crate::conlang::variety::render_form(&phon, v, &word)));
    Ok(vm)
}

// ( recipient donor-form -- {donor,adapted,ipa,steps} )  nativise a loanword (advisory)
fn w_borrow(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_borrow(vm).map_err(to_bund_err)
}
fn do_borrow(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.borrow";
    require_depth(vm, 2, tag)?;
    let donor = value_to_string(pull(vm, tag)?, "donor-form", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no phoneme block"))?;
    let loan =
        langapi::load_loan_phonology(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let a = crate::conlang::contact::adapt(&phon, &loan, &donor);
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("donor".into(), Value::from_string(a.donor));
    h.insert("adapted".into(), Value::from_string(a.adapted));
    h.insert(
        "ipa".into(),
        Value::from_list(a.ipa.into_iter().map(Value::from_string).collect()),
    );
    h.insert(
        "steps".into(),
        Value::from_list(a.steps.into_iter().map(Value::from_string).collect()),
    );
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( lang -- {region,with,convergence} )  areal convergence overlay (advisory)
fn w_areal(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_areal(vm).map_err(to_bund_err)
}
fn do_areal(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.areal";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let contact =
        langapi::load_contact(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let mut h: HashMap<String, Value> = HashMap::new();
    match contact {
        None => {
            h.insert("region".into(), Value::nodata());
            h.insert("with".into(), Value::from_list(Vec::new()));
            h.insert("convergence".into(), Value::from_list(Vec::new()));
        }
        Some(c) => {
            let (spec, _) = langapi::load_grammar_spec(store, &hierarchy, &book)
                .map_err(|e| anyhow!("{tag}: {e}"))?;
            h.insert("region".into(), Value::from_string(c.region.clone()));
            h.insert(
                "with".into(),
                Value::from_list(c.with.iter().map(|s| Value::from_string(s.clone())).collect()),
            );
            let conv: Vec<Value> =
                crate::conlang::contact::converge(&spec.grammar, &c.areal_features)
                    .into_iter()
                    .map(|c| {
                        let mut d: HashMap<String, Value> = HashMap::new();
                        d.insert("feature".into(), Value::from_string(c.feature));
                        d.insert("areal_value".into(), Value::from_string(c.areal_value));
                        d.insert(
                            "current".into(),
                            c.current.map(Value::from_string).unwrap_or_else(Value::nodata),
                        );
                        d.insert("status".into(), Value::from_string(c.status.as_str().to_string()));
                        Value::from_dict(d)
                    })
                    .collect();
            h.insert("convergence".into(), Value::from_list(conv));
        }
    }
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( character word -- rendered )  render a form in a character's idiolect
fn w_idiolect(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_idiolect(vm).map_err(to_bund_err)
}
fn do_idiolect(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::links::ConlangLinks;
    let tag = "ink.lang.idiolect";
    require_depth(vm, 2, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let character = value_to_string(pull(vm, tag)?, "character", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let links = ConlangLinks::load(store.project_root())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let link = links
        .characters
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&character))
        .map(|(_, v)| v)
        .ok_or_else(|| anyhow!("{tag}: no links for character `{character}`"))?;
    let lang = link
        .languages
        .first()
        .map(|p| p.language.clone())
        .ok_or_else(|| anyhow!("{tag}: `{character}` commands no language"))?;
    let var_id = link
        .native_variety
        .clone()
        .ok_or_else(|| anyhow!("{tag}: `{character}` has no native variety"))?;
    let book = langapi::find_language_book(&hierarchy, &lang).map_err(|e| anyhow!("{tag}: {e}"))?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let vs = langapi::load_varieties(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let v = vs
        .get(&var_id)
        .ok_or_else(|| anyhow!("{tag}: language `{lang}` has no variety `{var_id}`"))?;
    push(vm, Value::from_string(crate::conlang::variety::render_form(&phon, v, &word)));
    Ok(vm)
}

// ( -- {places,characters} )  the speech-community ecology
fn w_ecology(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_ecology(vm).map_err(to_bund_err)
}
fn do_ecology(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::links::ConlangLinks;
    let tag = "ink.lang.ecology";
    let store = active_store(tag)?;
    let links = ConlangLinks::load(store.project_root())
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let places: Vec<Value> = links
        .places
        .iter()
        .map(|(name, l)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("place".into(), Value::from_string(name.clone()));
            d.insert(
                "language".into(),
                l.primary.clone().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            d.insert(
                "variety".into(),
                l.variety.clone().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            Value::from_dict(d)
        })
        .collect();
    let characters: Vec<Value> = links
        .characters
        .iter()
        .map(|(name, c)| {
            let mut d: HashMap<String, Value> = HashMap::new();
            d.insert("character".into(), Value::from_string(name.clone()));
            d.insert(
                "languages".into(),
                Value::from_list(c.languages.iter().map(|p| Value::from_string(p.language.clone())).collect()),
            );
            d.insert(
                "native_variety".into(),
                c.native_variety.clone().map(Value::from_string).unwrap_or_else(Value::nodata),
            );
            Value::from_dict(d)
        })
        .collect();
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("places".into(), Value::from_list(places));
    h.insert("characters".into(), Value::from_list(characters));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ── morphology inspectors ────────────────────────────────────────────────

// ( lang root template gloss -- list-of-dicts )
fn w_paradigm(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_paradigm(vm).map_err(to_bund_err)
}
fn do_paradigm(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.paradigm";
    require_depth(vm, 4, tag)?;
    let gloss = value_to_string(pull(vm, tag)?, "gloss", tag)?;
    let template = value_to_string(pull(vm, tag)?, "template", tag)?;
    let root = value_to_string(pull(vm, tag)?, "root", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let tmpl = morph
        .paradigm(&template)
        .ok_or_else(|| anyhow!("{tag}: no paradigm named `{template}`"))?;
    let rows = crate::conlang::morphology::paradigm::generate(&phon, &morph, tmpl, &root, &gloss);
    let out: Vec<Value> = rows
        .iter()
        .map(|r| {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert("form".into(), Value::from_string(r.form.clone()));
            h.insert("gloss".into(), Value::from_string(r.gloss.clone()));
            let feats: HashMap<String, Value> = r
                .features
                .iter()
                .map(|(k, v)| (k.clone(), Value::from_string(v.clone())))
                .collect();
            h.insert("features".into(), Value::from_dict(feats));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( lang root gloss pos -- list-of-dicts )
fn w_derive(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_derive(vm, false).map_err(to_bund_err)
}
// ( lang root gloss pos -- added-count )  — derive AND commit to the lexicon.
fn w_derive_add(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_derive(vm, true).map_err(to_bund_err)
}
fn do_derive(vm: &mut VM, commit: bool) -> Result<&mut VM> {
    let tag = if commit { "ink.lang.derive_add" } else { "ink.lang.derive" };
    require_depth(vm, 4, tag)?;
    let pos = value_to_string(pull(vm, tag)?, "pos", tag)?;
    let gloss = value_to_string(pull(vm, tag)?, "gloss", tag)?;
    let root = value_to_string(pull(vm, tag)?, "root", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let derived = crate::conlang::morphology::derive::generate(&phon, &morph, &root, &gloss, &pos);
    if commit {
        let cfg = active_config(tag)?;
        let mut added = 0i64;
        for d in &derived {
            let entry = langapi::ImportEntry {
                word: d.form.clone(),
                pos: d.pos.clone(),
                translation: d.gloss.clone(),
                etymology: format!("derived from {root} via {}", d.rule),
                ..Default::default()
            };
            if langapi::add_imported_dictionary_entry(store, cfg, &book, &entry).is_ok() {
                added += 1;
            }
        }
        push(vm, Value::from_int(added));
    } else {
        let out: Vec<Value> = derived
            .iter()
            .map(|d| {
                let mut h: HashMap<String, Value> = HashMap::new();
                h.insert("form".into(), Value::from_string(d.form.clone()));
                h.insert("gloss".into(), Value::from_string(d.gloss.clone()));
                h.insert("pos".into(), Value::from_string(d.pos.clone()));
                h.insert("rule".into(), Value::from_string(d.rule.clone()));
                Value::from_dict(h)
            })
            .collect();
        push(vm, Value::from_list(out));
    }
    Ok(vm)
}

// ( lang word pos features -- dict )  features e.g. "number=pl,case=dat"
fn w_agree(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_agree(vm).map_err(to_bund_err)
}
fn do_agree(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.agree";
    require_depth(vm, 4, tag)?;
    let features = value_to_string(pull(vm, tag)?, "features", tag)?;
    let pos = value_to_string(pull(vm, tag)?, "pos", tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let rule = morph
        .agreement_for(&pos)
        .ok_or_else(|| anyhow!("{tag}: no agreement rule for pos `{pos}`"))?;
    let head = parse_features(&features);
    let a = crate::conlang::morphology::agreement::agree(&phon, &morph, rule, &word, &word, &head)
        .ok_or_else(|| anyhow!("{tag}: no matching paradigm cell for those features"))?;
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("form".into(), Value::from_string(a.form));
    h.insert("gloss".into(), Value::from_string(a.gloss));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ── syntax inspectors ────────────────────────────────────────────────────

// ( lang head role verb with relativizer -- dict )
fn w_relative(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_relative(vm).map_err(to_bund_err)
}
fn do_relative(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::syntax::{self, RelativeClause};
    let tag = "ink.lang.relative";
    require_depth(vm, 6, tag)?;
    let relativizer = value_to_string(pull(vm, tag)?, "relativizer", tag)?;
    let with = value_to_string(pull(vm, tag)?, "with", tag)?;
    let verb = value_to_string(pull(vm, tag)?, "verb", tag)?;
    let role = value_to_string(pull(vm, tag)?, "role", tag)?;
    let head = value_to_string(pull(vm, tag)?, "head", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let head_is_subject = !role.eq_ignore_ascii_case("object");
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (gs, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let rc = RelativeClause {
        head_is_subject,
        verb: langapi::parse_word(&verb),
        other: if with.trim().is_empty() {
            None
        } else {
            Some(langapi::parse_word(&with))
        },
        relativizer: if relativizer.trim().is_empty() {
            None
        } else {
            Some(langapi::parse_word(&relativizer))
        },
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
    };
    let r = syntax::relative_np(&phon, &morph, &gs.grammar, &langapi::parse_word(&head), &rc);
    push(vm, rendered_to_dict(&r));
    Ok(vm)
}

// ( lang subject verb complementizer comp-subject comp-verb comp-object -- dict )
fn w_complement(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_complement(vm).map_err(to_bund_err)
}
fn do_complement(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::syntax::{self, Clause, ComplementSentence, NounPhrase};
    let tag = "ink.lang.complement";
    require_depth(vm, 7, tag)?;
    let comp_object = value_to_string(pull(vm, tag)?, "comp_object", tag)?;
    let comp_verb = value_to_string(pull(vm, tag)?, "comp_verb", tag)?;
    let comp_subject = value_to_string(pull(vm, tag)?, "comp_subject", tag)?;
    let complementizer = value_to_string(pull(vm, tag)?, "complementizer", tag)?;
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
    let (gs, _) =
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
    let embedded = Clause {
        subject: np(&comp_subject),
        verb: Some(langapi::parse_word(&comp_verb)),
        verb_person: "3".into(),
        object: np(&comp_object),
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
        ..Default::default()
    };
    let cs = ComplementSentence {
        matrix_subject: if subject.trim().is_empty() {
            None
        } else {
            Some(langapi::parse_word(&subject))
        },
        matrix_verb: langapi::parse_word(&verb),
        matrix_person: "1".into(),
        matrix_number: "sg".into(),
        complementizer: if complementizer.trim().is_empty() {
            None
        } else {
            Some(langapi::parse_word(&complementizer))
        },
        complement: embedded,
        noun_paradigm: "noun".into(),
        verb_paradigm: "verb".into(),
    };
    let r = syntax::complement_sentence(&phon, &morph, &gs.grammar, &cs);
    push(vm, rendered_to_dict(&r));
    Ok(vm)
}

// ( lang clause-list conjunction -- dict )  clause-list = list of "subj verb [obj]"
fn w_coordinate(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_coordinate(vm).map_err(to_bund_err)
}
fn do_coordinate(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::syntax::{self, Clause, NounPhrase};
    let tag = "ink.lang.coordinate";
    require_depth(vm, 3, tag)?;
    let conj = value_to_string(pull(vm, tag)?, "conjunction", tag)?;
    let clauses_v = pull(vm, tag)?
        .cast_list()
        .map_err(|e| anyhow!("{tag}: expected a list of clause strings: {e}"))?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (gs, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let np = |w: &str| NounPhrase {
        head: langapi::parse_word(w),
        number: "sg".into(),
        adjective: None,
    };
    let mut parts = Vec::new();
    for cv in clauses_v {
        let spec = cv.cast_string().map_err(|e| anyhow!("{tag}: clause must be a string: {e}"))?;
        let toks: Vec<&str> = spec.split_whitespace().collect();
        if toks.len() < 2 {
            return Err(anyhow!("{tag}: clause `{spec}` needs a subject and a verb"));
        }
        let c = Clause {
            subject: Some(np(toks[0])),
            verb: Some(langapi::parse_word(toks[1])),
            verb_person: "3".into(),
            object: toks.get(2).map(|t| np(t)),
            noun_paradigm: "noun".into(),
            verb_paradigm: "verb".into(),
            ..Default::default()
        };
        parts.push(syntax::assemble(&phon, &morph, &gs.grammar, &c));
    }
    let conj_word = if conj.trim().is_empty() {
        None
    } else {
        Some(langapi::parse_word(&conj))
    };
    let r = syntax::coordinate(&parts, conj_word.as_ref());
    push(vm, rendered_to_dict(&r));
    Ok(vm)
}

// ── analysis / lexicon inspectors ────────────────────────────────────────

// ( lang -- profile-dict )
fn w_stats(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_stats(vm).map_err(to_bund_err)
}
fn do_stats(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.stats";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let profile = crate::conlang::analysis::profile(&phon, &entries);
    push(vm, serial_to_value(tag, &profile)?);
    Ok(vm)
}

// ( lang -- report-dict )
fn w_audit(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_audit(vm).map_err(to_bund_err)
}
fn do_audit(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.audit";
    require_depth(vm, 1, tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let report = crate::conlang::lexicon::analyze(&phon, &entries);
    push(vm, serial_to_value(tag, &report)?);
    Ok(vm)
}

// ( lang text -- list-of-entry-dicts )  text filters the gloss/headword
fn w_query(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_query(vm).map_err(to_bund_err)
}
fn do_query(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.query";
    require_depth(vm, 2, tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let text_opt = if text.trim().is_empty() { None } else { Some(text.as_str()) };
    let filter = crate::conlang::lexicon::Filter {
        register: None,
        domain: None,
        era: None,
        pos: None,
        text: text_opt,
    };
    let matches = crate::conlang::lexicon::filter(&entries, &filter);
    let out: Vec<Value> = matches
        .iter()
        .map(|e| {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert("word".into(), Value::from_string(e.word.clone()));
            h.insert("pos".into(), Value::from_string(e.pos.clone()));
            h.insert("translation".into(), Value::from_string(e.translation.clone()));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( lang scope -- gap-report-dict )  scope = "swadesh_100" or a file path
fn w_gaps(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_gaps(vm).map_err(to_bund_err)
}
fn do_gaps(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::gaps as gapmod;
    let tag = "ink.lang.gaps";
    require_depth(vm, 2, tag)?;
    let scope = value_to_string(pull(vm, tag)?, "scope", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let glosses: Vec<String> = entries
        .iter()
        .map(|e| e.translation.clone())
        .filter(|g| !g.trim().is_empty())
        .collect();
    let (scope_name, concepts) = if gapmod::is_builtin(&scope) {
        (
            format!("Swadesh-100 ({})", cfg.language),
            gapmod::swadesh_100(&cfg.language),
        )
    } else {
        let body = std::fs::read_to_string(&scope)
            .map_err(|e| anyhow!("{tag}: scope `{scope}` is not a builtin nor a readable file: {e}"))?;
        let parsed = gapmod::ScopeFile::from_hjson(&body).map_err(|e| anyhow!("{tag}: {e}"))?;
        let nm = parsed.name.clone().unwrap_or_else(|| scope.clone());
        (nm, parsed.into_concepts())
    };
    let report = gapmod::find_gaps(&scope_name, &concepts, &glosses);
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("scope".into(), Value::from_string(report.scope_name.clone()));
    h.insert("total".into(), Value::from_int(report.total as i64));
    h.insert(
        "coverage_pct".into(),
        Value::from_float(report.coverage_pct() as f64),
    );
    h.insert(
        "covered".into(),
        Value::from_list(report.covered.iter().map(|s| Value::from_string(s.clone())).collect()),
    );
    h.insert(
        "missing".into(),
        Value::from_list(report.missing.iter().map(|s| Value::from_string(s.clone())).collect()),
    );
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ── diachronics inspectors ───────────────────────────────────────────────

// ( lang form -- evolved-form )  evolve a proto-form through the daughter's chain
fn w_sound_change(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_sound_change(vm).map_err(to_bund_err)
}
fn do_sound_change(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.sound_change";
    require_depth(vm, 2, tag)?;
    let form = value_to_string(pull(vm, tag)?, "form", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let dia = langapi::load_diachronics(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` declares no `diachronics` block"))?;
    // Sound changes operate on the proto's inventory (segmentation + classes).
    let proto_phon = match dia.proto.as_deref() {
        Some(p) => {
            let pbook = langapi::find_language_book(&hierarchy, p)
                .map_err(|e| anyhow!("{tag}: proto `{p}`: {e}"))?;
            langapi::load_phonology(store, &hierarchy, &pbook)
                .map_err(|e| anyhow!("{tag}: {e}"))?
                .unwrap_or_default()
        }
        None => langapi::load_phonology(store, &hierarchy, &book)
            .map_err(|e| anyhow!("{tag}: {e}"))?
            .unwrap_or_default(),
    };
    let out = crate::conlang::diachronic::apply::derive_form(&proto_phon, &dia.rules, &form);
    push(vm, Value::from_string(out));
    Ok(vm)
}

// ( proto form -- list-of-{language,reflex} )
fn w_cognates(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_cognates(vm).map_err(to_bund_err)
}
fn do_cognates(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.cognates";
    require_depth(vm, 2, tag)?;
    let form = value_to_string(pull(vm, tag)?, "form", tag)?;
    let proto = value_to_string(pull(vm, tag)?, "proto", tag)?;
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let proto_book =
        langapi::find_language_book(&hierarchy, &proto).map_err(|e| anyhow!("{tag}: {e}"))?;
    let proto_phon = langapi::load_phonology(store, &hierarchy, &proto_book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    // Daughters: language books whose diachronics name this proto.
    let root = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_LANGUAGES)
    });
    let mut out: Vec<Value> = Vec::new();
    if let Some(root) = root {
        for n in hierarchy.children_of(Some(root.id)) {
            if n.kind != NodeKind::Book || n.id == proto_book.id {
                continue;
            }
            if let Ok(Some(dia)) = langapi::load_diachronics(store, &hierarchy, n) {
                if dia.proto.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(&proto)) {
                    let reflex = crate::conlang::diachronic::apply::derive_form(
                        &proto_phon,
                        &dia.rules,
                        &form,
                    );
                    let mut h: HashMap<String, Value> = HashMap::new();
                    h.insert("language".into(), Value::from_string(n.title.clone()));
                    h.insert("reflex".into(), Value::from_string(reflex));
                    out.push(Value::from_dict(h));
                }
            }
        }
    }
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ( -- tree-string )
fn w_family_tree(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_family_tree(vm).map_err(to_bund_err)
}
fn do_family_tree(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.family_tree";
    let store = active_store(tag)?;
    let hierarchy = Hierarchy::load(store).map_err(|e| anyhow!("{tag}: {e}"))?;
    let root = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book
            && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_LANGUAGES)
    });
    let mut pairs: Vec<(String, Option<String>)> = Vec::new();
    if let Some(root) = root {
        for n in hierarchy.children_of(Some(root.id)) {
            if n.kind != NodeKind::Book {
                continue;
            }
            let proto = langapi::load_diachronics(store, &hierarchy, n)
                .ok()
                .flatten()
                .and_then(|d| d.proto);
            pairs.push((n.title.clone(), proto));
        }
    }
    let tree = crate::conlang::diachronic::family::render_tree(&pairs);
    push(vm, Value::from_string(tree));
    Ok(vm)
}

// ── creative inspectors ──────────────────────────────────────────────────

// ( lang count seed -- list )
fn w_names(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_names(vm).map_err(to_bund_err)
}
fn do_names(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.names";
    require_depth(vm, 3, tag)?;
    let seed = value_to_i64(pull(vm, tag)?, "seed", tag)?;
    let count = value_to_i64(pull(vm, tag)?, "count", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let names = crate::conlang::creative::names(&phon, count.max(0) as usize, seed as u64);
    push(vm, Value::from_list(names.into_iter().map(Value::from_string).collect()));
    Ok(vm)
}

// ( lang count seed -- list-of-sentence-dicts )
fn w_prose(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_prose(vm).map_err(to_bund_err)
}
fn do_prose(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.prose";
    require_depth(vm, 3, tag)?;
    let seed = value_to_i64(pull(vm, tag)?, "seed", tag)?;
    let count = value_to_i64(pull(vm, tag)?, "count", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let morph = langapi::load_morphology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let (gs, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let lines = crate::conlang::creative::prose(
        &phon,
        &morph,
        &gs.grammar,
        &entries,
        count.max(0) as usize,
        seed as u64,
    );
    push(vm, Value::from_list(lines.iter().map(rendered_to_dict).collect()));
    Ok(vm)
}

// ( lang meter seed -- list-of-lines )  meter e.g. "5,7,5"
fn w_poem(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_poem(vm).map_err(to_bund_err)
}
fn do_poem(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.poem";
    require_depth(vm, 3, tag)?;
    let seed = value_to_i64(pull(vm, tag)?, "seed", tag)?;
    let meter_s = value_to_string(pull(vm, tag)?, "meter", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let meter: Vec<usize> = meter_s
        .split(',')
        .filter_map(|s| s.trim().parse::<usize>().ok())
        .filter(|n| *n > 0)
        .collect();
    if meter.is_empty() {
        return Err(anyhow!("{tag}: invalid meter `{meter_s}` (e.g. 5,7,5)"));
    }
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let lines = crate::conlang::creative::poem(&phon, &entries, &meter, seed as u64);
    push(vm, Value::from_list(lines.iter().map(|l| Value::from_string(l.text.clone())).collect()));
    Ok(vm)
}

// ── mutators (store_write) ───────────────────────────────────────────────

// ( lang word -- )
fn w_remove_word(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_remove_word(vm).map_err(to_bund_err)
}
fn do_remove_word(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.remove_word";
    require_depth(vm, 2, tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let _ = hierarchy;
    langapi::remove_dictionary_entry(store, &book, &word).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( lang feature value -- )
fn w_grammar_set(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_grammar_set(vm).map_err(to_bund_err)
}
fn do_grammar_set(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.grammar_set";
    require_depth(vm, 3, tag)?;
    let value = value_to_string(pull(vm, tag)?, "value", tag)?;
    let feature = value_to_string(pull(vm, tag)?, "feature", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let _ = hierarchy;
    langapi::set_grammar_feature(store, cfg, &book, &feature, &value)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( lang form literal meaning -- )
fn w_idiom_add(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_idiom_add(vm).map_err(to_bund_err)
}
fn do_idiom_add(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.idiom_add";
    require_depth(vm, 4, tag)?;
    let meaning = value_to_string(pull(vm, tag)?, "meaning", tag)?;
    let literal = value_to_string(pull(vm, tag)?, "literal", tag)?;
    let form = value_to_string(pull(vm, tag)?, "form", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let _ = hierarchy;
    langapi::add_idiom(store, cfg, &book, &form, &literal, &meaning)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ( lang source target -- )
fn w_metaphor_add(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_metaphor_add(vm).map_err(to_bund_err)
}
fn do_metaphor_add(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.metaphor_add";
    require_depth(vm, 3, tag)?;
    let target = value_to_string(pull(vm, tag)?, "target", tag)?;
    let source = value_to_string(pull(vm, tag)?, "source", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let _ = hierarchy;
    langapi::add_metaphor(store, cfg, &book, &source, &target).map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(vm)
}

// ── AI-backed words (ai_write; advisory — return data, never write the book) ─
//
// Each calls the configured LLM (an optional trailing `provider` string picks a
// non-default provider; empty = the configured default) and returns the model's
// output as data. Nothing is committed: a script that wants to keep generated
// words calls `lang.add_word` itself, so the AI stays advisory (the same
// principle as the CLI's `--yes` gate).

/// Run one blocking chat completion against the configured provider.
fn ai_call(
    tag: &str,
    cfg: &crate::config::Config,
    provider: &str,
    system: String,
    user: String,
) -> Result<String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm).map_err(|e| anyhow!("{tag}: {e}"))?;
    let prov = if provider.trim().is_empty() { None } else { Some(provider) };
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, prov)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    crate::ai::stream::collect_blocking(ai.client.clone(), model.to_string(), Some(system), user)
        .map_err(|e| anyhow!("{tag}: inference error: {e}"))
}

// ( lang kind provider -- text )  themed text: blessing | curse | incantation
fn w_compose(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_compose(vm).map_err(to_bund_err)
}
fn do_compose(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.compose";
    require_depth(vm, 3, tag)?;
    let provider = value_to_string(pull(vm, tag)?, "provider", tag)?;
    let kind = value_to_string(pull(vm, tag)?, "kind", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let (gs, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    if entries.is_empty() {
        return Err(anyhow!("{tag}: lexicon is empty — add words before composing themed text"));
    }
    let summary = langapi::summarize_typology(&gs.grammar);
    let (system, user) =
        crate::conlang::creative::themed_prompt(&book.title, &kind, &cfg.language, &summary, &entries);
    let raw = ai_call(tag, cfg, &provider, system, user)?;
    push(vm, Value::from_string(raw.trim().to_string()));
    Ok(vm)
}

// ( forms gloss provider -- text )  reconstruct a proto-form from cognate forms
fn w_reconstruct(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_reconstruct(vm).map_err(to_bund_err)
}
fn do_reconstruct(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.reconstruct";
    require_depth(vm, 3, tag)?;
    let provider = value_to_string(pull(vm, tag)?, "provider", tag)?;
    let gloss = value_to_string(pull(vm, tag)?, "gloss", tag)?;
    let forms = value_to_string(pull(vm, tag)?, "forms", tag)?;
    let cfg = active_config(tag)?;
    let gloss_clause = if gloss.trim().is_empty() {
        String::new()
    } else {
        format!(" meaning '{}'", gloss.trim())
    };
    let user = format!("Cognate daughter forms{gloss_clause}: {forms}.\n\nReconstruct the proto-form.");
    let raw = ai_call(tag, cfg, &provider, langapi::RECONSTRUCT_SYSTEM.to_string(), user)?;
    push(vm, Value::from_string(raw.trim().to_string()));
    Ok(vm)
}

// ( lang provider -- text )  assess the plausibility of the sound-change chain
fn w_realism_check(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_realism_check(vm).map_err(to_bund_err)
}
fn do_realism_check(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.realism_check";
    require_depth(vm, 2, tag)?;
    let provider = value_to_string(pull(vm, tag)?, "provider", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let dia = langapi::load_diachronics(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no diachronics chain to check"))?;
    let rules_text = dia
        .rules
        .iter()
        .enumerate()
        .map(|(i, r)| format!("{}. {}", i + 1, r.source))
        .collect::<Vec<_>>()
        .join("\n");
    let proto = dia.proto.as_deref().unwrap_or("the proto-language");
    let user = format!(
        "Sound-change chain deriving {name} from {proto} (applied in order):\n{rules_text}\n\n\
         Assess the plausibility, rule by rule, then give an overall verdict."
    );
    let raw = ai_call(tag, cfg, &provider, langapi::REALISM_SYSTEM.to_string(), user)?;
    push(vm, Value::from_string(raw.trim().to_string()));
    Ok(vm)
}

// ( lang topic count provider -- list-of-{word,gloss,pos} )
// Generates themed words: forms come from the deterministic generator (so they
// obey the phonotactics), the AI assigns meaning, and the dedup gate drops
// homophones / duplicate senses. Returns the survivors — advisory, uncommitted.
fn w_generate_lexicon(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_generate_lexicon(vm).map_err(to_bund_err)
}
fn do_generate_lexicon(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.generate_lexicon";
    require_depth(vm, 4, tag)?;
    let provider = value_to_string(pull(vm, tag)?, "provider", tag)?;
    let count = value_to_i64(pull(vm, tag)?, "count", tag)?.max(1) as usize;
    let topic = value_to_string(pull(vm, tag)?, "topic", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no phonology block"))?;
    let existing =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let pool = crate::conlang::generate::lexicon::build_pool(&phon, &existing, count);
    if pool.is_empty() {
        return Err(anyhow!("{tag}: could not generate candidate forms — loosen the phonotactics"));
    }
    let work_lang = if cfg.language.trim().is_empty() { "english" } else { cfg.language.trim() };
    let topic_opt = if topic.trim().is_empty() { None } else { Some(topic.as_str()) };
    let user = langapi::build_lexgen_prompt(&book.title, topic_opt, count, None, None, work_lang, &pool);
    let raw = ai_call(tag, cfg, &provider, langapi::LEXGEN_SYSTEM.to_string(), user)?;
    let proposals = crate::conlang::generate::lexicon::parse_proposals(&raw)
        .map_err(|e| anyhow!("{tag}: could not parse model reply: {e}"))?;
    let (kept, _rejected) =
        crate::conlang::generate::lexicon::dedup(&phon, &existing, proposals);
    let out: Vec<Value> = kept
        .iter()
        .map(|p| {
            let mut h: HashMap<String, Value> = HashMap::new();
            h.insert("word".into(), Value::from_string(p.form.clone()));
            h.insert("gloss".into(), Value::from_string(p.gloss.clone()));
            h.insert("pos".into(), Value::from_string(p.pos.clone()));
            Value::from_dict(h)
        })
        .collect();
    push(vm, Value::from_list(out));
    Ok(vm)
}

// ── file / document output (fs_write; some also ai_write) ────────────────
//
// These produce a file (a font, a typeset book, a glyph SVG). The table gates
// them on the capability that's *always* exercised; a word that ALSO calls the
// LLM checks `ai_write` manually via `require_category`, so it needs both. Paths
// go through the same project-root sandbox as `ink.fs.write`.

/// Error if `category` is denied by the active policy — for words needing a
/// second capability beyond their table gate.
fn require_category(tag: &str, category: &str) -> Result<()> {
    if let Some(p) = crate::scripting::active_policy() {
        if p.denies(category) {
            return Err(anyhow!(
                "{tag}: blocked — enable the `{category}` scripting category in inkhaven.hjson"
            ));
        }
    }
    Ok(())
}

fn parse_doc_format(tag: &str, fmt: &str) -> Result<bool> {
    match fmt.to_ascii_lowercase().as_str() {
        "md" | "markdown" => Ok(false),
        "typ" | "typst" => Ok(true),
        other => Err(anyhow!("{tag}: unknown format `{other}` (expected md or typ)")),
    }
}

// ( svg-path -- report-dict )  lint an SVG file for font suitability
fn w_glyph_lint(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_glyph_lint(vm).map_err(to_bund_err)
}
fn do_glyph_lint(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.lang.glyph_lint";
    require_depth(vm, 1, tag)?;
    let path = value_to_string(pull(vm, tag)?, "svg-path", tag)?;
    let resolved = super::helpers::resolve_fs_path(tag, &path)?;
    let body = std::fs::read_to_string(&resolved)
        .map_err(|e| anyhow!("{tag}: reading {}: {e}", resolved.display()))?;
    let report = crate::conlang::writing::preflight::lint_svg(&body);
    let mut h: HashMap<String, Value> = HashMap::new();
    h.insert("usable".into(), Value::from_bool(report.is_usable()));
    let strs = |v: &[String]| Value::from_list(v.iter().map(|s| Value::from_string(s.clone())).collect());
    h.insert("info".into(), strs(&report.info));
    h.insert("warnings".into(), strs(&report.warnings));
    h.insert("errors".into(), strs(&report.errors));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( lang format out font -- out-path )
fn w_dictionary(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_dictionary(vm).map_err(to_bund_err)
}
fn do_dictionary(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::analysis;
    use crate::conlang::output::{self, DictMeta, RenderEntry};
    use crate::conlang::writing::input;
    let tag = "ink.lang.dictionary";
    require_depth(vm, 4, tag)?;
    let font = value_to_string(pull(vm, tag)?, "font", tag)?;
    let out = value_to_string(pull(vm, tag)?, "out", tag)?;
    let fmt = value_to_string(pull(vm, tag)?, "format", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let typst = parse_doc_format(tag, &fmt)?;
    let resolved = super::helpers::resolve_fs_path(tag, &out)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let font_cfg =
        langapi::load_font_config(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let profile = analysis::profile(&phon, &entries);
    let family = if font.trim().is_empty() {
        font_cfg.as_ref().and_then(|c| c.family.clone())
    } else {
        Some(font.clone())
    };
    let can_translit = font_cfg.as_ref().is_some_and(|c| !c.glyphs.is_empty());
    let rendered: Vec<RenderEntry> = entries
        .iter()
        .map(|e| RenderEntry {
            headword: e.word.clone(),
            conscript: match (&font_cfg, can_translit) {
                (Some(c), true) => {
                    let o = input::to_script(c, &e.word);
                    (o.mapped > 0).then_some(o.script)
                }
                _ => None,
            },
            pronunciation: langapi::pronounce(&phon, &e.word),
            pos: e.pos.clone(),
            gloss: e.translation.clone(),
            registers: e.registers.clone(),
            domain: e.domain.clone(),
            era: e.era.clone(),
            etymology: e.etymology.clone(),
            example: (!e.example.trim().is_empty()).then(|| e.example.clone()),
        })
        .collect();
    let meta = DictMeta {
        language: &book.title,
        font_family: if typst { family.as_deref() } else { None },
        profile: Some(&profile),
    };
    let doc = if typst {
        output::dictionary_typst(&meta, &rendered)
    } else {
        output::dictionary_markdown(&meta, &rendered)
    };
    crate::io_atomic::write(&resolved, doc.as_bytes())
        .map_err(|e| anyhow!("{tag}: write {}: {e}", resolved.display()))?;
    push(vm, Value::from_string(resolved.display().to_string()));
    Ok(vm)
}

// ( lang format out font -- out-path )
fn w_grammar_book(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_grammar_book(vm).map_err(to_bund_err)
}
fn do_grammar_book(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::analysis;
    use crate::conlang::output::{self, GrammarBook};
    let tag = "ink.lang.grammar_book";
    require_depth(vm, 4, tag)?;
    let font = value_to_string(pull(vm, tag)?, "font", tag)?;
    let out = value_to_string(pull(vm, tag)?, "out", tag)?;
    let fmt = value_to_string(pull(vm, tag)?, "format", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let typst = parse_doc_format(tag, &fmt)?;
    let resolved = super::helpers::resolve_fs_path(tag, &out)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let phon = langapi::load_phonology(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .unwrap_or_default();
    let entries =
        langapi::load_dictionary(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let morphology =
        langapi::load_morphology(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let (grammar_spec, _) =
        langapi::load_grammar_spec(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let (expressions, _) =
        langapi::load_expressions(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let samples =
        langapi::load_samples(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let font_cfg =
        langapi::load_font_config(store, &hierarchy, &book).map_err(|e| anyhow!("{tag}: {e}"))?;
    let profile = analysis::profile(&phon, &entries);
    let family = if font.trim().is_empty() {
        font_cfg.as_ref().and_then(|c| c.family.clone())
    } else {
        Some(font.clone())
    };
    let example_sentence = morphology
        .as_ref()
        .and_then(|m| langapi::build_example_sentence(&phon, m, &grammar_spec.grammar, &entries));
    let has_expr = !expressions.idioms.is_empty() || !expressions.metaphors.is_empty();
    let variation = langapi::build_variation(store, &hierarchy, &book, &phon, &entries, 12)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    let bk = GrammarBook {
        language: &book.title,
        font_family: if typst { family.as_deref() } else { None },
        profile: &profile,
        phonology: &phon,
        morphology: morphology.as_ref(),
        typology: &grammar_spec.grammar,
        expressions: has_expr.then_some(&expressions),
        samples: &samples,
        study: None,
        example_sentence,
        variation,
    };
    let doc = if typst {
        output::grammar_typst(&bk)
    } else {
        output::grammar_markdown(&bk)
    };
    crate::io_atomic::write(&resolved, doc.as_bytes())
        .map_err(|e| anyhow!("{tag}: write {}: {e}", resolved.display()))?;
    push(vm, Value::from_string(resolved.display().to_string()));
    Ok(vm)
}

// ( lang format out -- out-stem )  compile the font to UFO / TTF
fn w_font_build(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_font_build(vm).map_err(to_bund_err)
}
fn do_font_build(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::writing::font::GlyphSource;
    use crate::conlang::writing::preflight;
    let tag = "ink.lang.font_build";
    require_depth(vm, 3, tag)?;
    let out = value_to_string(pull(vm, tag)?, "out", tag)?;
    let fmt = value_to_string(pull(vm, tag)?, "format", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let (want_ufo, want_ttf) = match fmt.to_ascii_lowercase().as_str() {
        "ufo" => (true, false),
        "ttf" => (false, true),
        "both" => (true, true),
        other => return Err(anyhow!("{tag}: unknown format `{other}` (ufo/ttf/both)")),
    };
    let resolved = super::helpers::resolve_fs_path(tag, &out)?;
    let (store, hierarchy, book) = ctx(tag, &name)?;
    let cfg = langapi::load_font_config(store, &hierarchy, &book)
        .map_err(|e| anyhow!("{tag}: {e}"))?
        .ok_or_else(|| anyhow!("{tag}: language `{name}` has no `font` block"))?;
    if cfg.glyphs.is_empty() {
        return Err(anyhow!("{tag}: language `{name}` declares no glyphs"));
    }
    let family = cfg.family.clone().unwrap_or_else(|| book.title.clone());
    let dir = langapi::glyph_store_dir(store.project_root(), &name);
    let mut sources: Vec<GlyphSource> = Vec::new();
    let mut skipped = 0usize;
    for g in &cfg.glyphs {
        let path = dir.join(format!("{}.svg", g.name));
        let svg = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !preflight::lint_svg(&svg).is_usable() {
            skipped += 1;
            continue;
        }
        sources.push(GlyphSource {
            name: g.name.clone(),
            codepoint: g.codepoint,
            svg,
        });
    }
    langapi::emit_font(
        &family,
        cfg.upm,
        &sources,
        skipped,
        Some(&resolved),
        want_ufo,
        want_ttf,
    )
    .map_err(|e| anyhow!("{tag}: {e}"))?;
    push(vm, Value::from_string(resolved.display().to_string()));
    Ok(vm)
}

// ( lang describe phoneme out provider -- out-path )  AI-draft a glyph SVG (advisory)
fn w_glyph_draft(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_glyph_draft(vm).map_err(to_bund_err)
}
fn do_glyph_draft(vm: &mut VM) -> Result<&mut VM> {
    use crate::conlang::writing::{draft, preflight};
    let tag = "ink.lang.glyph_draft";
    require_depth(vm, 5, tag)?;
    require_category(tag, "ai_write")?; // table gates fs_write; this needs both
    let provider = value_to_string(pull(vm, tag)?, "provider", tag)?;
    let out = value_to_string(pull(vm, tag)?, "out", tag)?;
    let phoneme = value_to_string(pull(vm, tag)?, "phoneme", tag)?;
    let describe = value_to_string(pull(vm, tag)?, "describe", tag)?;
    let name = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let cfg = active_config(tag)?;
    let resolved = super::helpers::resolve_fs_path(tag, &out)?;
    let phon_clause = if phoneme.trim().is_empty() {
        String::new()
    } else {
        format!(" It renders the phoneme /{}/.", phoneme.trim())
    };
    let user = format!(
        "Draft a glyph for the constructed writing system of the language '{name}'.{phon_clause}\n\n\
         Description: {describe}"
    );
    let raw = ai_call(tag, cfg, &provider, langapi::GLYPH_DRAFT_SYSTEM.to_string(), user)?;
    let svg = draft::extract_svg(&raw)
        .ok_or_else(|| anyhow!("{tag}: the model did not return an SVG glyph"))?;
    let report = preflight::lint_svg(&svg);
    if !report.is_usable() {
        return Err(anyhow!(
            "{tag}: drafted glyph is not usable — {}",
            report.errors.join("; ")
        ));
    }
    crate::io_atomic::write(&resolved, svg.as_bytes())
        .map_err(|e| anyhow!("{tag}: write {}: {e}", resolved.display()))?;
    push(vm, Value::from_string(resolved.display().to_string()));
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
