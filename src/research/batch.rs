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

/// One question's outcome, for the report.
struct Outcome {
    question: String,
    title: String,
    fact: String,
    confidence: f64,
    action: String,
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

#[allow(clippy::too_many_arguments)]
fn process_one(
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
        };
    }

    // 3. Score the model's confidence in the candidate fact.
    let confidence = score_confidence(ai, model, language, &fact.text);

    // 4. Insert only under the explicit flag + threshold.
    let action = if !auto_confirm {
        "candidate (run with --auto-confirm to insert)".to_string()
    } else if confidence < threshold {
        format!("skipped (confidence {confidence:.2} < {threshold:.2})")
    } else if let Some(book_id) = facts_book {
        match super::insert::insert_paragraph(store, cfg, hierarchy, book_id, None, &fact.title, &fact.text) {
            Ok(new_id) => {
                let now = chrono::Utc::now().to_rfc3339();
                let prov_note = super::provenance::Provenance::record(
                    layout,
                    &new_id.to_string(),
                    super::provenance::SourceRecord::new("model", "", question, "batch", now),
                )
                .err()
                .map(|e| format!(" (provenance not recorded: {e})"))
                .unwrap_or_default();
                let path =
                    Hierarchy::load(store).ok().and_then(|h| h.get(new_id).map(|n| h.slug_path(n))).unwrap_or_default();
                format!("inserted → {path}{prov_note}")
            }
            Err(e) => format!("skipped (insert failed: {e})"),
        }
    } else {
        "skipped (no Facts book)".to_string()
    };

    Outcome { question: question.to_string(), title: fact.title, fact: fact.text, confidence, action }
}

fn skipped(question: &str, reason: String) -> Outcome {
    Outcome { question: question.to_string(), title: String::new(), fact: String::new(), confidence: 0.0, action: format!("skipped ({reason})") }
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

/// Extract the first 0..1 float from the model's reply.
fn parse_confidence(reply: &str) -> f64 {
    let mut num = String::new();
    for ch in reply.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            num.push(ch);
        } else if !num.is_empty() {
            break;
        }
    }
    num.parse::<f64>().unwrap_or(0.0).clamp(0.0, 1.0)
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
        assert_eq!(parse_confidence("no number here"), 0.0);
        assert_eq!(parse_confidence("1.7"), 1.0); // clamped
    }
}
