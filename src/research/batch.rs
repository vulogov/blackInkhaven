//! RESRCH-2 (R2-F) — batch / headless research. `inkhaven research --batch
//! questions.txt` runs a question list non-interactively: for each question it
//! researches an answer (Facts-grounded), distils one candidate fact, scores the
//! model's confidence, and — **only** under `--auto-confirm` above `--confidence`
//! — inserts it (with `model` provenance). A Markdown report is written either
//! way. The interactive default still confirms every insertion; this relaxes the
//! rule *only* behind the explicit flag + threshold.

use std::path::Path;

use anyhow::{Result, anyhow};

use crate::ai::stream::collect_blocking;
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, SYSTEM_TAG_FACTS, Store};

use super::extract::{self, TargetBook};
use super::thread::RagMode;

/// One question's outcome, for the report. Reused by the RESRCH-6 agentic loop.
pub(crate) struct Outcome {
    pub question: String,
    pub title: String,
    pub fact: String,
    pub confidence: f64,
    pub action: String,
    /// The node id of the emitted Fact (when one was inserted) — the agentic
    /// contradiction gate scans these.
    pub inserted_id: Option<uuid::Uuid>,
}

/// Run a batch file. `auto_confirm` + `threshold` gate insertion; `out` is the
/// report path (stdout when `None`).
pub(crate) fn run(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    path: &str,
    auto_confirm: bool,
    threshold: f64,
    out: Option<&str>,
) -> Result<()> {
    let raw = std::fs::read_to_string(path).map_err(|e| anyhow!("read {path}: {e}"))?;
    let questions: Vec<String> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();
    if questions.is_empty() {
        return Err(anyhow!("no questions in {path} (one per line; # comments ignored)"));
    }

    let ai = crate::ai::AiClient::from_config(&cfg.llm).map_err(|e| anyhow!("no LLM provider: {e}"))?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, None).map_err(|e| anyhow!("provider: {e}"))?;
    let (lang, _note) = crate::prose::resolve_prose_language(None, &cfg.language);
    let language = extract::language_name(&lang);

    let hierarchy = Hierarchy::load(store)?;
    let facts_book = hierarchy
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_FACTS))
        .map(|n| n.id);

    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut inserted = 0usize;
    for q in &questions {
        eprintln!("· {q}");
        let outcome = process_one(
            layout, cfg, store, &hierarchy, facts_book, &ai, &model, language, q, auto_confirm,
            threshold,
        );
        if outcome.action.starts_with("inserted") {
            inserted += 1;
        }
        outcomes.push(outcome);
    }

    let report = render_report(&outcomes, auto_confirm, threshold, inserted);
    match out {
        Some(p) => {
            std::fs::write(p, &report).map_err(|e| anyhow!("write {p}: {e}"))?;
            eprintln!("report → {p}  ({inserted}/{} inserted)", questions.len());
        }
        None => print!("{report}"),
    }
    Ok(())
}

/// Research one question, distil a candidate fact, score it, and (under
/// `auto_confirm` + `threshold`) insert it into the Facts book with `model`
/// provenance. The single-question chain reused by both `--batch` and the
/// RESRCH-6 agentic loop. `provenance_thread` labels where the fact came from
/// (`"batch"` vs `"agentic"`), so the author can tell autonomous facts apart.
#[allow(clippy::too_many_arguments)]
pub(crate) fn process_one(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    hierarchy: &Hierarchy,
    facts_book: Option<uuid::Uuid>,
    ai: &crate::ai::AiClient,
    model: &str,
    language: &str,
    question: &str,
    auto_confirm: bool,
    threshold: f64,
) -> Outcome {
    process_one_tagged(
        layout, cfg, store, hierarchy, facts_book, ai, model, language, question, auto_confirm,
        threshold, "batch",
    )
}

/// [`process_one`] with an explicit provenance thread label.
#[allow(clippy::too_many_arguments)]
pub(crate) fn process_one_tagged(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    hierarchy: &Hierarchy,
    facts_book: Option<uuid::Uuid>,
    ai: &crate::ai::AiClient,
    model: &str,
    language: &str,
    question: &str,
    auto_confirm: bool,
    threshold: f64,
    provenance_thread: &str,
) -> Outcome {
    // 1. Research the answer, grounded on the Facts corpus (RAG).
    let (rag, _sources) =
        super::rag::build_context(store, cfg, hierarchy, facts_book, &[], RagMode::FactsPlusFull, question);
    let system = super::llm::system_prompt(RagMode::FactsPlusFull, rag.as_deref());
    let answer = match collect_blocking(ai.client.clone(), model.to_string(), Some(system), question.to_string()) {
        Ok(a) => a,
        Err(e) => return skipped(question, format!("research failed: {e}")),
    };

    // 2. Distil one candidate fact.
    let ex_system =
        extract::system_prompt(TargetBook::Facts, language, extract::default_instruction(TargetBook::Facts), &answer);
    let ex_raw = match collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(ex_system),
        "Produce the entry as specified.".to_string(),
    ) {
        Ok(r) => r,
        Err(e) => return skipped(question, format!("extraction failed: {e}")),
    };
    let fact = extract::parse(&ex_raw);
    if fact.text.trim().is_empty() {
        return Outcome {
            question: question.to_string(),
            title: fact.title.clone(),
            fact: String::new(),
            confidence: 0.0,
            action: "skipped (no fact extracted)".to_string(),
            inserted_id: None,
        };
    }

    // 3. Score the model's confidence in the candidate fact.
    let confidence = score_confidence(ai, model, language, &fact.text);

    // 4. Insert only under the explicit flag + threshold.
    let (action, inserted_id) = if !auto_confirm {
        ("candidate (run with --auto-confirm to insert)".to_string(), None)
    } else if confidence < threshold {
        (format!("skipped (confidence {confidence:.2} < {threshold:.2})"), None)
    } else if let Some(book_id) = facts_book {
        match super::insert::insert_paragraph(store, cfg, hierarchy, book_id, None, &fact.title, &fact.text) {
            Ok(new_id) => {
                let now = chrono::Utc::now().to_rfc3339();
                let prov_note = super::provenance::Provenance::record(
                    layout,
                    &new_id.to_string(),
                    super::provenance::SourceRecord::new("model", "", question, provenance_thread, now),
                )
                .err()
                .map(|e| format!(" (provenance not recorded: {e})"))
                .unwrap_or_default();
                let path =
                    Hierarchy::load(store).ok().and_then(|h| h.get(new_id).map(|n| h.slug_path(n))).unwrap_or_default();
                (format!("inserted → {path}{prov_note}"), Some(new_id))
            }
            Err(e) => (format!("skipped (insert failed: {e})"), None),
        }
    } else {
        ("skipped (no Facts book)".to_string(), None)
    };

    Outcome { question: question.to_string(), title: fact.title, fact: fact.text, confidence, action, inserted_id }
}

fn skipped(question: &str, reason: String) -> Outcome {
    Outcome { question: question.to_string(), title: String::new(), fact: String::new(), confidence: 0.0, action: format!("skipped ({reason})"), inserted_id: None }
}

/// Ask the model for a 0..1 confidence that the statement is accurate.
fn score_confidence(ai: &crate::ai::AiClient, model: &str, language: &str, fact: &str) -> f64 {
    let system = format!(
        "Rate your confidence that the following statement is factually accurate, as a single number \
         between 0.0 and 1.0. Reply with ONLY the number, no words. (Reasoning language: {language}.)"
    );
    match collect_blocking(ai.client.clone(), model.to_string(), Some(system), fact.to_string()) {
        Ok(r) => parse_confidence(&r),
        Err(_) => 0.0,
    }
}

/// Extract the confidence the model reported. The prompt asks for a single
/// 0.0–1.0 number; scan every numeric run and return the first that is genuinely a
/// probability — in `[0,1]` AND either a decimal (`0.85`) or exactly `0`/`1`. Bare
/// integers > 1 (a stray `90`, a year like `1918`, a `90%` that lost its point)
/// are NOT confidence: the previous version grabbed the first digit-run and
/// clamped it, so `"90% confident"` and `"In 1918…"` both scored 1.0 and sailed
/// through the auto-insert threshold. Reject those (fail closed → 0.0) instead.
fn parse_confidence(reply: &str) -> f64 {
    let mut token = String::new();
    for ch in reply.chars().chain(std::iter::once(' ')) {
        if ch.is_ascii_digit() || ch == '.' {
            token.push(ch);
        } else if !token.is_empty() {
            if let Ok(v) = token.parse::<f64>() {
                let is_probability = token.contains('.') || token == "0" || token == "1";
                if is_probability && (0.0..=1.0).contains(&v) {
                    return v;
                }
            }
            token.clear();
        }
    }
    0.0
}

fn render_report(outcomes: &[Outcome], auto_confirm: bool, threshold: f64, inserted: usize) -> String {
    let mut s = String::from("# Research batch report\n\n");
    s.push_str(&format!(
        "{} question(s) · auto-confirm {} · threshold {:.2} · {inserted} inserted\n\n",
        outcomes.len(),
        if auto_confirm { "on" } else { "off" },
        threshold,
    ));
    for (i, o) in outcomes.iter().enumerate() {
        s.push_str(&format!("## {}. {}\n\n", i + 1, o.question));
        if !o.title.is_empty() {
            s.push_str(&format!("**{}**\n\n", o.title));
        }
        if !o.fact.is_empty() {
            s.push_str(&format!("{}\n\n", o.fact));
            s.push_str(&format!("_confidence {:.2} · {}_\n\n", o.confidence, o.action));
        } else {
            s.push_str(&format!("_{}_\n\n", o.action));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::parse_confidence;

    #[test]
    fn parses_confidence_forms() {
        assert!((parse_confidence("0.82") - 0.82).abs() < 1e-9);
        assert!((parse_confidence("Confidence: 0.5 (medium)") - 0.5).abs() < 1e-9);
        assert!((parse_confidence("1") - 1.0).abs() < 1e-9);
        assert_eq!(parse_confidence("0"), 0.0);
        assert_eq!(parse_confidence("no number here"), 0.0);
    }

    #[test]
    fn rejects_non_probability_numbers_fail_closed() {
        // The gate must not read a stray integer or year as max confidence.
        assert_eq!(parse_confidence("I am 90% confident"), 0.0);
        assert_eq!(parse_confidence("In 1918 the flu killed millions"), 0.0);
        assert_eq!(parse_confidence("Я на 90% уверен"), 0.0);
        assert_eq!(parse_confidence("1.7"), 0.0); // out of range → not a probability
        // A real probability preceded by a stray integer is still found.
        assert!((parse_confidence("re 1918: confidence 0.3") - 0.3).abs() < 1e-9);
    }
}
