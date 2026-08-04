//! REDLINE-1 (RD-P8) — `ink.revise.*` Bund stdlib: the unified revision worklist,
//! read-only. Bund reads the same deterministic queue the Editorial Pass and
//! `inkhaven revise` show — every reader's finding, each tagged with *how* it can
//! be acted on (rewrite / decision / brief). The AI editorial letter and any
//! prose rewrite are NOT exposed here (they cost and/or mutate the manuscript);
//! Bund only reads the diagnosis.
//!
//! - `ink.revise.findings` ( -- list )  the ranked findings as dicts
//!   {category, severity, response, location, message, source}.
//! - `ink.revise.check`    ( -- dict )  summary counts (by severity, by response,
//!   by category) — a pass/fail gate for a revision-readiness script.

use std::collections::HashMap;

use anyhow::{anyhow, Result};
use easy_error::Error as BundError;
use rust_dynamic::value::Value;
use rust_multistackvm::multistackvm::VM;

use super::helpers::{active_store, push};
use crate::editorial::{EditorialFinding, Severity};

pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> std::result::Result<&mut VM, BundError>)] =
        &[("ink.revise.findings", w_findings), ("ink.revise.check", w_check)];
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

/// The severity word shared with `inkhaven revise --json` (high / med / low).
fn severity_word(s: Severity) -> &'static str {
    match s {
        Severity::Error => "high",
        Severity::Warn => "med",
        Severity::Info => "low",
    }
}

fn finding_to_dict(f: &EditorialFinding) -> Value {
    let mut m: HashMap<String, Value> = HashMap::new();
    m.insert("category".into(), Value::from_string(&f.category));
    m.insert("severity".into(), Value::from_string(severity_word(f.severity)));
    m.insert("response".into(), Value::from_string(f.response().label()));
    m.insert("location".into(), Value::from_string(&f.location.label()));
    m.insert("message".into(), Value::from_string(&f.message));
    m.insert("source".into(), Value::from_string(f.source));
    Value::from_dict(m)
}

/// The unified worklist — the same `collect` the Editorial Pass and `revise` use.
/// Deferred findings stay hidden (as in the default `edit` view). `collect` opens
/// its own read handle — the same concurrent-open the TUI Editorial Pass already
/// does while the app store is live.
fn worklist(tag: &str) -> Result<Vec<EditorialFinding>> {
    let store = active_store(tag)?;
    let root = store.project_root().to_path_buf();
    let report = crate::cli::editorial::collect(&root, None, None, false)
        .map_err(|e| anyhow!("{tag}: {e}"))?;
    Ok(report.findings)
}

word!(w_findings, do_findings);
fn do_findings(vm: &mut VM) -> Result<&mut VM> {
    let findings = worklist("ink.revise.findings")?;
    push(vm, Value::from_list(findings.iter().map(finding_to_dict).collect()));
    Ok(vm)
}

word!(w_check, do_check);
fn do_check(vm: &mut VM) -> Result<&mut VM> {
    let findings = worklist("ink.revise.check")?;

    let (mut high, mut med, mut low) = (0i64, 0i64, 0i64);
    let mut by_response: HashMap<String, i64> = HashMap::new();
    let mut by_category: HashMap<String, i64> = HashMap::new();
    for f in &findings {
        match f.severity {
            Severity::Error => high += 1,
            Severity::Warn => med += 1,
            Severity::Info => low += 1,
        }
        *by_response.entry(f.response().label().to_string()).or_insert(0) += 1;
        *by_category.entry(f.category.clone()).or_insert(0) += 1;
    }
    let to_dict = |m: HashMap<String, i64>| -> Value {
        Value::from_dict(m.into_iter().map(|(k, v)| (k, Value::from_int(v))).collect())
    };

    let mut out: HashMap<String, Value> = HashMap::new();
    out.insert("findings".into(), Value::from_int(findings.len() as i64));
    out.insert("high".into(), Value::from_int(high));
    out.insert("med".into(), Value::from_int(med));
    out.insert("low".into(), Value::from_int(low));
    // `clean` = a simple pass/fail gate: no high-severity findings.
    out.insert("clean".into(), Value::from_bool(high == 0));
    out.insert("by_response".into(), to_dict(by_response));
    out.insert("by_category".into(), to_dict(by_category));
    push(vm, Value::from_dict(out));
    Ok(vm)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editorial::Location;

    #[test]
    fn severity_word_matches_the_cli_json_vocabulary() {
        // Bund and `inkhaven revise --json` must speak the same words.
        assert_eq!(severity_word(Severity::Error), "high");
        assert_eq!(severity_word(Severity::Warn), "med");
        assert_eq!(severity_word(Severity::Info), "low");
    }

    #[test]
    fn finding_dict_carries_the_response_kind_and_source() {
        // A co_location break is a Decision from SENTINEL.
        let f = EditorialFinding {
            category: "co_location".into(),
            severity: Severity::Error,
            location: Location { chapter: Some("ch. 3".into()), ..Default::default() },
            message: "Mara is in two places".into(),
            hint: None,
            source: "continuity",
            autofixable: false,
        };
        let d = finding_to_dict(&f);
        // Round-trips through the dict as the documented shape.
        let m = d.cast_dict().expect("a dict");
        assert_eq!(m.get("severity").and_then(|v| v.cast_string().ok()).as_deref(), Some("high"));
        assert_eq!(m.get("response").and_then(|v| v.cast_string().ok()).as_deref(), Some("decision"));
        assert_eq!(m.get("source").and_then(|v| v.cast_string().ok()).as_deref(), Some("continuity"));
        assert_eq!(m.get("category").and_then(|v| v.cast_string().ok()).as_deref(), Some("co_location"));
    }
}
