//! LING-1 L-P3 — `inkhaven language universals <lang>`: judge a language's
//! grammar block against the typological baseline (head-directionality harmony +
//! the classic implicational universals) and survey its word order + morphotype.
//! Read-only; a violation is a flag, not an error. `--json` for machine use.

use std::path::Path;

use crate::conlang::universals::{Branch, TypologyReport, Verdict};
use crate::error::{Error, Result};

use super::*;

pub(crate) fn universals(project: &Path, language: &str, json: bool) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let report = crate::conlang::universals::survey(&spec);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing typology: {e}")))?
        );
        return Ok(());
    }

    print_report(language, &report);
    Ok(())
}

fn branch_label(b: Branch) -> &'static str {
    match b {
        Branch::HeadInitial => "head-initial",
        Branch::HeadFinal => "head-final",
        Branch::Mixed => "mixed / disharmonic",
        Branch::Unknown => "unknown (no word-order features set)",
    }
}

fn verdict_glyph(v: Verdict) -> &'static str {
    match v {
        Verdict::Satisfied => "✓",
        Verdict::Violated => "✗",
        Verdict::NotApplicable => "·",
    }
}

fn print_report(language: &str, r: &TypologyReport) {
    println!("typological universals · {language}");
    println!(
        "  order      · {}",
        r.word_order.as_deref().unwrap_or("(unspecified)")
    );
    if let Some(mt) = &r.morphological_type {
        println!("  morphology · {mt}");
    }
    if let Some(hm) = &r.head_marking {
        println!("  marking    · {hm}");
    }
    println!("  branching  · {}", branch_label(r.branch));
    let measured = r.harmonic + r.disharmonic.len();
    if measured > 0 {
        print!(
            "  harmony    · {}/{} correlates aligned ({:.0}%)",
            r.harmonic,
            measured,
            r.harmony_score * 100.0
        );
        if r.disharmonic.is_empty() {
            println!();
        } else {
            println!(" · disharmonic: {}", r.disharmonic.join(", "));
        }
    }
    println!("  universals ·");
    for c in &r.checks {
        println!("      {} {} — {}", verdict_glyph(c.verdict), c.statement, c.detail);
    }
    let violations = r.checks.iter().filter(|c| c.verdict == Verdict::Violated).count();
    if violations == 0 {
        println!("  (no universal violated — but a natural conlang may still break tendencies)");
    } else {
        println!("  ({violations} typologically marked combination(s) flagged)");
    }
}
