//! RESRCH-6 (R6-P1/P2) — the autonomous, deep-research loop.
//!
//! Given a topic, it **decomposes** it into a handful of specific sub-questions
//! (the planner), then for each one reuses the single-question research chain
//! ([`super::batch::process_one_tagged`]) to gather evidence, distil a candidate
//! fact, score it, and — above a confidence threshold — **emit it as a Facts
//! paragraph into the Facts system book**, with `model` provenance and an
//! `agentic` thread label. The output is the *growing Facts book*, never a
//! standalone article; the author reviews, promotes, or disputes each emitted
//! fact through the ordinary Facts machinery (fact-check, `/undisputed`).
//!
//! Gated by `research.agentic.enabled` (on by default; the author can turn it
//! off). Cost is bounded by `research.agentic.max_subquestions`.
//!
//! This is the first cut — a single planning + gather pass. The gap-driven
//! iterate step (R6-P3) and citation-following (snowball) land in later phases.

use anyhow::{Result, anyhow};

use crate::ai::stream::collect_blocking;
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{NodeKind, SYSTEM_TAG_FACTS, Store};

use super::batch::{Outcome, process_one_tagged};
use super::extract;

/// A candidate fact below this model-confidence is not emitted (skipped, logged).
const CONFIDENCE_THRESHOLD: f64 = 0.6;

/// Run an agentic research pass over `topic`, emitting Facts into the Facts book.
/// `out` is the report path (stdout when `None`).
pub(crate) fn run(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    topic: &str,
    out: Option<&str>,
) -> Result<()> {
    if !cfg.research.agentic.enabled {
        return Err(anyhow!(
            "agentic research is disabled — set `research.agentic.enabled: true` in inkhaven.hjson to enable it"
        ));
    }
    let topic = topic.trim();
    if topic.is_empty() {
        return Err(anyhow!("give a topic to research, e.g. `--agentic \"the causes of the 1918 flu\"`"));
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
    if facts_book.is_none() {
        return Err(anyhow!("this project has no Facts book — agentic research emits into it"));
    }

    // 1. Plan — decompose the topic into sub-questions.
    let max = cfg.research.agentic.max_subquestions.max(1);
    let subqs = plan_subquestions(&ai, &model, language, topic, max)?;
    if subqs.is_empty() {
        return Err(anyhow!("the planner returned no sub-questions for `{topic}`"));
    }
    eprintln!("⟳ agentic: {} sub-question(s) for \"{topic}\"", subqs.len());

    // 2. Gather — research each sub-question and emit a Fact (auto-insert above
    //    the confidence threshold; the facts land untrusted for review).
    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut emitted = 0usize;
    for q in &subqs {
        eprintln!("· {q}");
        let o = process_one_tagged(
            layout, cfg, store, &hierarchy, facts_book, &ai, &model, language, q,
            /* auto_confirm */ true, CONFIDENCE_THRESHOLD, "agentic",
        );
        if o.action.starts_with("inserted") {
            emitted += 1;
        }
        outcomes.push(o);
    }

    let report = render_report(topic, &subqs, &outcomes, emitted);
    match out {
        Some(p) => {
            std::fs::write(p, &report).map_err(|e| anyhow!("write {p}: {e}"))?;
            eprintln!("report → {p}  ({emitted} fact(s) emitted into the Facts book)");
        }
        None => print!("{report}"),
    }
    eprintln!(
        "✓ agentic pass complete — {emitted} Fact(s) emitted (model provenance, untrusted). \
         Review them in the Facts book: promote, dispute, or /factcheck."
    );
    Ok(())
}

/// The planner (R6-P1): ask the model to decompose a topic into specific,
/// individually-answerable sub-questions, one per line. Language-keyed.
fn plan_subquestions(
    ai: &crate::ai::AiClient,
    model: &str,
    language: &str,
    topic: &str,
    max: usize,
) -> Result<Vec<String>> {
    let system = format!(
        "You are a research planner. Decompose the user's topic into at most {max} SPECIFIC, \
         individually-answerable sub-questions that together cover it well. Each must be a single \
         factual question, not a task. Reply with ONLY the questions, one per line, no numbering, \
         no preamble, no commentary. Write the questions in {language}."
    );
    let raw = collect_blocking(ai.client.clone(), model.to_string(), Some(system), topic.to_string())
        .map_err(|e| anyhow!("planner failed: {e}"))?;
    Ok(parse_subquestions(&raw, max))
}

/// Parse the planner's reply into sub-questions: one per non-empty line, leading
/// list markers (`1.`, `-`, `*`, `•`) stripped, capped at `max`.
fn parse_subquestions(reply: &str, max: usize) -> Vec<String> {
    reply
        .lines()
        .map(|l| {
            l.trim()
                .trim_start_matches(|c: char| c.is_ascii_digit() || matches!(c, '.' | ')' | '-' | '*' | '•' | ' ' | '\t'))
                .trim()
                .to_string()
        })
        .filter(|l| l.len() > 3)
        .take(max)
        .collect()
}

fn render_report(topic: &str, subqs: &[String], outcomes: &[Outcome], emitted: usize) -> String {
    let mut s = String::from("# Agentic research report\n\n");
    s.push_str(&format!(
        "**Topic:** {topic}\n\n{} sub-question(s) · {emitted} Fact(s) emitted into the Facts book \
         (model provenance, untrusted — review to promote or dispute).\n\n",
        subqs.len()
    ));
    for (i, o) in outcomes.iter().enumerate() {
        s.push_str(&format!("## {}. {}\n\n", i + 1, o.question));
        if !o.title.is_empty() {
            s.push_str(&format!("**{}**\n\n", o.title));
        }
        if !o.fact.is_empty() {
            s.push_str(&format!("{}\n\n_confidence {:.2} · {}_\n\n", o.fact, o.confidence, o.action));
        } else {
            s.push_str(&format!("_{}_\n\n", o.action));
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::parse_subquestions;

    #[test]
    fn parses_planner_replies_in_many_shapes() {
        let reply = "1. What caused the 1918 flu?\n\
                     2) Where did it originate?\n\
                     - How many died?\n\
                     * Why was it called Spanish flu?\n\
                     \n\
                     ok\n\
                     • What ended it?";
        let qs = parse_subquestions(reply, 10);
        assert_eq!(qs.len(), 5, "numbered/bulleted lines parse, short junk drops");
        assert_eq!(qs[0], "What caused the 1918 flu?");
        assert_eq!(qs[1], "Where did it originate?");
        assert_eq!(qs[4], "What ended it?");
        // The `max` cap holds.
        assert_eq!(parse_subquestions(reply, 2).len(), 2);
    }
}
