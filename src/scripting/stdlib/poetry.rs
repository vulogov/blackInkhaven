//! Bund `ink.poem.*` (POEM PO-P9) — the poetry engines exposed to scripts.
//!
//! These are the pure, read-only words (`store_read`, default-allowed): syllable
//! count, a line scan, rhyme classification, and form-completion status — each a
//! thin wrapper over the POEM engines, needing no store. The store-dependent
//! words the RFC also lists (`findings` / `suppress` / `set_form`, and the
//! open-paragraph `form` lookup) ride with the deferred `inner_poet.db` store.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{pull, push, require_depth, value_to_string};
use crate::prose::ProseLanguage;

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] = &[
        ("ink.poem.syllable_count", w_syllable_count),
        ("ink.poem.scan_line", w_scan_line),
        ("ink.poem.rhyme", w_rhyme),
        ("ink.poem.status", w_status),
    ];
    for (name, f) in words {
        vm.register_inline(name.to_string(), *f).map_err(|e| anyhow!("register {name}: {e}"))?;
    }
    // Every `ink.poem.X` also answers to the shorter `poem.X`; aliases inherit
    // the target's policy gate.
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

// ( word lang -- n )
fn w_syllable_count(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_syllable_count(vm).map_err(to_bund_err)
}
fn do_syllable_count(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.poem.syllable_count";
    require_depth(vm, 2, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let word = value_to_string(pull(vm, tag)?, "word", tag)?;
    let n = crate::poetry::syllabify::syllable_count(&word, ProseLanguage::from_label(&lang));
    push(vm, Value::from_int(n as i64));
    Ok(vm)
}

// ( line lang -- dict{pattern syllables metre} )
fn w_scan_line(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_scan_line(vm).map_err(to_bund_err)
}
fn do_scan_line(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.poem.scan_line";
    require_depth(vm, 2, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let line = value_to_string(pull(vm, tag)?, "line", tag)?;
    let beats = crate::poetry::metre::line_to_beats(&line, ProseLanguage::from_label(&lang));
    let pattern: String = beats.iter().map(|b| b.glyph()).collect();
    let metre = crate::poetry::metre::detect(&beats)
        .map(|m| m.name)
        .unwrap_or_else(|| "irregular".to_string());
    let mut h = HashMap::new();
    h.insert("pattern".to_string(), Value::from_string(pattern));
    h.insert("syllables".to_string(), Value::from_int(beats.len() as i64));
    h.insert("metre".to_string(), Value::from_string(metre));
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( word1 word2 lang -- dict{quality type shared note?} )
fn w_rhyme(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_rhyme(vm).map_err(to_bund_err)
}
fn do_rhyme(vm: &mut VM) -> Result<&mut VM> {
    use crate::poetry::rhyme::{RhymeQuality, RhymeType, analyse_rhyme};
    let tag = "ink.poem.rhyme";
    require_depth(vm, 3, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let w2 = value_to_string(pull(vm, tag)?, "word2", tag)?;
    let w1 = value_to_string(pull(vm, tag)?, "word1", tag)?;
    let r = analyse_rhyme(&w1, &w2, ProseLanguage::from_label(&lang));
    let quality = match r.quality {
        RhymeQuality::Perfect => "perfect",
        RhymeQuality::Near => "near",
        RhymeQuality::Eye => "eye",
        RhymeQuality::None => "none",
    };
    let rtype = match r.rhyme_type {
        RhymeType::Masculine => "masculine",
        RhymeType::Feminine => "feminine",
        RhymeType::Dactylic => "dactylic",
    };
    let mut h = HashMap::new();
    h.insert("quality".to_string(), Value::from_string(quality.to_string()));
    h.insert("type".to_string(), Value::from_string(rtype.to_string()));
    h.insert("shared".to_string(), Value::from_string(r.shared));
    if let Some(note) = r.note {
        h.insert("note".to_string(), Value::from_string(note));
    }
    push(vm, Value::from_dict(h));
    Ok(vm)
}

// ( text form lang -- dict{lines expected complete issues} )
fn w_status(vm: &mut VM) -> std::result::Result<&mut VM, BundError> {
    do_status(vm).map_err(to_bund_err)
}
fn do_status(vm: &mut VM) -> Result<&mut VM> {
    let tag = "ink.poem.status";
    require_depth(vm, 3, tag)?;
    let lang = value_to_string(pull(vm, tag)?, "lang", tag)?;
    let form_name = value_to_string(pull(vm, tag)?, "form", tag)?;
    let text = value_to_string(pull(vm, tag)?, "text", tag)?;
    let form = crate::poetry::form::FormsLibrary::builtin()
        .localized(&form_name, &lang)
        .ok_or_else(|| anyhow!("{tag}: unknown form `{form_name}`"))?;
    let st = crate::poetry::form_check::check_form(&text, &form);
    let mut h = HashMap::new();
    h.insert("lines".to_string(), Value::from_int(st.lines_written as i64));
    if let Some(e) = st.expected_lines {
        h.insert("expected".to_string(), Value::from_int(e as i64));
    }
    h.insert("complete".to_string(), Value::from_bool(st.complete));
    let issues: Vec<Value> = st.issues.into_iter().map(Value::from_string).collect();
    h.insert("issues".to_string(), Value::from_list(issues));
    push(vm, Value::from_dict(h));
    Ok(vm)
}
