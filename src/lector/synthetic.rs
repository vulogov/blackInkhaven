//! LECTOR-1 LR-P4 — the synthetic first-read (the one LLM pass).
//!
//! Reads the book **forward, one chapter at a time**, as a first-time reader who
//! does not know the ending, and reports the reader-experience problems the
//! deterministic walk can't judge — real confusion, illegible stakes, flagging
//! engagement, a put-down point. Explicit, cost-capped, opt-in (never automatic).
//!
//! Forward-only is enforced by *construction*: each chapter's call is given only a
//! compact recap of what the reader has met *so far* (from the LR-P3 walk) plus the
//! current chapter's prose — never a later chapter. The recap is the grounding, and
//! it is spoiler-free by definition, which is why LECTOR grounds on the walk state
//! rather than the declared arcs/world (those would tell the "first reader" the
//! ending).
//!
//! Reuses the world fact-checker's cost-capped `slow_llm_call` (per-chapter
//! preflight against the daily cap) and its JSON-array parser; no call/cost logic
//! is re-implemented. Findings arrive `source:"reader"`.

// Consumed by LR-P5 (`readthrough --deep`) / LR-P6 (the ledger's `k`); scaffolding
// until then.
#![allow(dead_code)]

use std::path::Path;

use super::{ReaderFinding, Severity};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

/// The system prompt: a sparing first reader, forward-only, structured output.
const FIRST_READER_SYSTEM: &str = "You are a thoughtful first-time reader of a novel, reading it one \
chapter at a time IN ORDER. You do NOT know what happens in later chapters — react only to what you \
have read so far. You are given a short recap of what you've read up to now, then the current \
chapter. Report ONLY genuine first-reader problems with THIS chapter: confusion (you can't tell who \
someone is or what is happening), unclear stakes (you don't see why it matters), flagging engagement \
(your attention drifts), or a point where you would put the book down. Do NOT critique prose style, \
do NOT guess the plot, do NOT praise, do NOT invent problems. Be sparing — most chapters are fine. \
Respond ONLY with a JSON array; each item is {\"category\": one of \
confusion|stakes|engagement|pacing|put_down|other, \"severity\": notice|concern, \"explanation\": a \
one-sentence reaction in the reader's voice (\"I lost track of who Joren is\")}. Return [] if the \
chapter reads clean.";

/// How many recap entries to carry (bounds the prompt size).
const RECAP_CAP: usize = 24;

/// Build the running recap a reader carries into a chapter. Forward-only: only
/// entities met and threads still open *before* this chapter.
fn build_recap(met: &[String], open: &[String]) -> String {
    if met.is_empty() && open.is_empty() {
        return "This is the opening — you know nothing yet.".to_string();
    }
    let tail = |v: &[String]| -> String {
        let start = v.len().saturating_sub(RECAP_CAP);
        v[start..].join(", ")
    };
    let mut s = String::new();
    if !met.is_empty() {
        s.push_str(&format!("People/places you've met: {}.", tail(met)));
    }
    if !open.is_empty() {
        if !s.is_empty() {
            s.push(' ');
        }
        s.push_str(&format!("Open questions still unanswered: {}.", tail(open)));
    }
    s
}

/// Compose the per-chapter first-read prompt.
fn build_prompt(recap: &str, title: &str, prose: &str) -> String {
    format!(
        "WHAT YOU'VE READ SO FAR:\n{recap}\n\nCURRENT CHAPTER (\u{201c}{title}\u{201d}):\n{prose}\n\n\
         React as a first reader to THIS chapter only — you do not know what happens next. \
         Return the JSON array of first-reader problems (or [])."
    )
}

/// Map a parsed reader category to a finding kind + severity. Severity is driven
/// by the category (the underlying parser collapses the model's severity string).
fn map_category(category: &str) -> (&'static str, Severity) {
    match category.trim().to_ascii_lowercase().as_str() {
        "put_down" => ("put_down_risk", Severity::Concern),
        "confusion" => ("confusion", Severity::Notice),
        "stakes" => ("stakes_gap", Severity::Notice),
        "pacing" => ("attention_dip", Severity::Notice),
        "engagement" => ("engagement", Severity::Info),
        _ => ("reader", Severity::Info),
    }
}

/// Run the synthetic first-read over the whole manuscript, forward. Self-contained
/// (opens its own store) so it is safe from a background worker; cost-capped
/// per chapter. Returns the reader findings (`source:"reader"`).
pub(crate) fn run(project: &Path, max_cost: usize, force: bool) -> Result<Vec<ReaderFinding>, String> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(|e| e.to_string())?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| e.to_string())?;
    let h = Hierarchy::load(&store).map_err(|e| e.to_string())?;

    // The deterministic walk gives each chapter's forward state (met entities /
    // opened+resolved threads); its ordering matches user_book_chapters.
    let read = super::walk::read_forward(&store, &cfg, &layout, &h);
    let chapters = h.user_book_chapters();

    let mut out: Vec<ReaderFinding> = Vec::new();
    let mut met: Vec<String> = Vec::new();
    let mut open: Vec<String> = Vec::new();
    let mut counter = 0usize;

    for (idx, cr) in read.chapters.iter().enumerate() {
        let Some((chapter_id, _)) = chapters.get(idx) else { break };
        let raw = crate::cli::book_walk::chapter_raw_prose(&layout, &h, *chapter_id);
        let prose = crate::audiobook::typst_to_plain(&raw);
        if prose.trim().is_empty() {
            continue;
        }

        let recap = build_recap(&met, &open);
        let prompt = build_prompt(&recap, &cr.title, &prose);
        let findings = crate::cli::realworld::slow_llm_call(
            project,
            "lector-read",
            FIRST_READER_SYSTEM,
            prompt,
            max_cost,
            force,
        )
        .map_err(|e| e.to_string())?;

        for f in findings {
            let (kind, severity) = map_category(&f.category);
            counter += 1;
            out.push(ReaderFinding {
                kind,
                severity,
                chapter: cr.chapter,
                anchor: None,
                entities: Vec::new(),
                message: f.body,
                source: "reader",
                dedup_key: format!("reader|{kind}|{}|{counter}", cr.chapter),
            });
        }

        // Advance the reader's state past this chapter (forward-only).
        met.extend(cr.new_entities.iter().cloned());
        for t in &cr.opened_threads {
            open.push(t.clone());
        }
        open.retain(|t| !cr.resolved_threads.contains(t));
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recap_opens_blank_then_accumulates() {
        assert!(build_recap(&[], &[]).contains("opening"));
        let r = build_recap(&["Mara".into(), "Velmaril".into()], &["the sealed letter".into()]);
        assert!(r.contains("Mara") && r.contains("Velmaril"));
        assert!(r.contains("the sealed letter"));
    }

    #[test]
    fn prompt_carries_recap_prose_and_forward_only_framing() {
        let p = build_prompt("You've met: Mara.", "The Crossing", "Aldous rowed on.");
        assert!(p.contains("You've met: Mara."));
        assert!(p.contains("The Crossing"));
        assert!(p.contains("Aldous rowed on."));
        assert!(p.contains("you do not know what happens next"));
    }

    #[test]
    fn categories_map_to_kind_and_severity() {
        assert_eq!(map_category("put_down"), ("put_down_risk", Severity::Concern));
        assert_eq!(map_category("Confusion").0, "confusion");
        assert_eq!(map_category("stakes"), ("stakes_gap", Severity::Notice));
        assert_eq!(map_category("engagement").1, Severity::Info);
        assert_eq!(map_category("whatever"), ("reader", Severity::Info));
    }

    #[test]
    fn system_prompt_is_forward_only_and_structured() {
        assert!(FIRST_READER_SYSTEM.contains("do NOT know what happens in later chapters"));
        assert!(FIRST_READER_SYSTEM.contains("JSON array"));
        assert!(FIRST_READER_SYSTEM.contains("Return [] if"));
    }
}
