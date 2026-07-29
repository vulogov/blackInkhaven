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
//! R6-P3 adds the **gap-driven iterate** step: after the initial plan+gather, a
//! critic reviews the topic and the Facts emitted so far and proposes follow-up
//! sub-questions for the gaps (and any contradictions it spots); the loop runs
//! further rounds until it **converges** (the critic finds nothing more), the
//! **round cap** (`max_rounds`) is hit, or the **budget** (`max_subquestions`,
//! the total across all rounds) is spent — whichever comes first. Dropped
//! questions are logged, never silently truncated.
//!
//! Citation-following (snowball) and the review-queue TUI land in later phases.

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

/// Why the iterate loop stopped — reported so the run is never a black box.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// The critic proposed no further sub-questions — the topic is covered.
    Converged,
    /// The total sub-question budget (`max_subquestions`) was spent.
    BudgetSpent,
    /// The round cap (`max_rounds`) was reached with budget still available.
    RoundCap,
}

impl Stop {
    fn describe(self) -> &'static str {
        match self {
            Stop::Converged => "converged (the critic found no further gaps)",
            Stop::BudgetSpent => "budget spent (max_subquestions reached)",
            Stop::RoundCap => "round cap reached (max_rounds)",
        }
    }
}

/// The termination decision — the heart of R6-P3, factored out so it is pure and
/// unit-testable. Returns `Some(reason)` to stop before the next round, `None`
/// to continue. Checked *after* a round completes.
fn decide_stop(
    next_is_empty: bool,
    budget_remaining: usize,
    rounds_done: usize,
    max_rounds: usize,
) -> Option<Stop> {
    if budget_remaining == 0 {
        Some(Stop::BudgetSpent)
    } else if rounds_done >= max_rounds {
        Some(Stop::RoundCap)
    } else if next_is_empty {
        Some(Stop::Converged)
    } else {
        None
    }
}

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

    let mut budget = cfg.research.agentic.max_subquestions.max(1);
    let max_rounds = cfg.research.agentic.max_rounds.max(1);

    // Round 1 — plan the initial decomposition.
    let mut pending = plan_subquestions(&ai, &model, language, topic, budget)?;
    if pending.is_empty() {
        return Err(anyhow!("the planner returned no sub-questions for `{topic}`"));
    }

    let mut outcomes: Vec<Outcome> = Vec::new();
    let mut emitted = 0usize;
    let mut dropped = 0usize;
    let mut rounds = 0usize;
    // Structured (fact-a, fact-b, reason) contradictions found among the run's own
    // emitted facts — the in-loop gate's output; deduped by the fact pair.
    let mut clashes: Vec<(String, String, String)> = Vec::new();
    let stop;
    loop {
        // Never exceed the total budget; anything over is logged, not silently cut.
        let take = pending.len().min(budget);
        dropped += pending.len() - take;
        rounds += 1;
        eprintln!("» agentic round {rounds}: {take} sub-question(s)");
        for q in pending.iter().take(take) {
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
        budget -= take;

        // In-loop contradiction gate: rigorously scan THIS run's emitted facts for
        // `⇄` clashes with the dedicated primitives (not the critic's eye), so the
        // critic can spawn resolution questions and the report flags them.
        let emitted_ids: Vec<uuid::Uuid> = outcomes.iter().filter_map(|o| o.inserted_id).collect();
        for c in detect_contradictions(layout, store, &ai, &model, language, &emitted_ids) {
            if !clashes.iter().any(|x| x.0 == c.0 && x.1 == c.1) {
                eprintln!("! contradiction: {} ⇄ {}", short(&c.0), short(&c.1));
                clashes.push(c);
            }
        }

        // Critique the gaps for a possible next round (only if worth asking). The
        // critic is told about the contradictions so it can propose questions that
        // resolve them.
        let next = if budget == 0 || rounds >= max_rounds {
            Vec::new()
        } else {
            match critique_gaps(&ai, &model, language, topic, &outcomes, &clashes, budget) {
                Ok(qs) => qs,
                Err(e) => {
                    // Surface the failure instead of silently reporting "converged"
                    // — an errored critic round is not the same as a covered topic.
                    eprintln!(
                        "! agentic round {rounds}: critic call failed ({e}) — no follow-ups proposed this round"
                    );
                    Vec::new()
                }
            }
        };
        if let Some(reason) = decide_stop(next.is_empty(), budget, rounds, max_rounds) {
            stop = reason;
            break;
        }
        pending = next;
    }

    let report = render_report(topic, &outcomes, emitted, dropped, rounds, stop, &clashes);
    match out {
        Some(p) => {
            std::fs::write(p, &report).map_err(|e| anyhow!("write {p}: {e}"))?;
            eprintln!("report → {p}  ({emitted} fact(s) emitted over {rounds} round(s))");
        }
        None => print!("{report}"),
    }
    let clash_note = if clashes.is_empty() {
        String::new()
    } else {
        format!(" · ! {} contradiction(s) among the emitted facts — resolve before trusting", clashes.len())
    };
    eprintln!(
        "✓ agentic run complete — {emitted} Fact(s) over {rounds} round(s), {}{clash_note} \
         (model provenance, untrusted). Triage in the Facts book: /review, then /factcheck.",
        stop.describe()
    );
    Ok(())
}

/// The in-loop contradiction gate: rigorously scan the run's *own* emitted facts
/// for `⇄` clashes using the dedicated contradiction primitives (the same ones
/// `/contradict` uses), returning `(fact-a, fact-b, reason)` tuples. Only the
/// run's facts are scanned (not the whole book), so it's bounded and focuses on
/// what the autonomous emission introduced.
fn detect_contradictions(
    layout: &ProjectLayout,
    store: &Store,
    ai: &crate::ai::AiClient,
    model: &str,
    language: &str,
    emitted_ids: &[uuid::Uuid],
) -> Vec<(String, String, String)> {
    if emitted_ids.len() < 2 {
        return Vec::new();
    }
    let Ok(h) = Hierarchy::load(store) else { return Vec::new() };
    let Some(facts_book) = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_FACTS))
        .map(|n| n.id)
    else {
        return Vec::new();
    };
    let ids: std::collections::HashSet<uuid::Uuid> = emitted_ids.iter().copied().collect();
    let facts: Vec<_> = super::factcheck::gather_facts(store, &h, facts_book)
        .into_iter()
        .filter(|f| ids.contains(&f.id))
        .collect();
    if facts.len() < 2 {
        return Vec::new();
    }
    let prov = super::provenance::Provenance::load(layout);
    let sourced = super::contradiction::join_provenance(&facts, &prov);
    let groups = super::factcheck::consistency_groups(&facts, super::factcheck::CONSIST_MAX);
    let system = super::factcheck::consistency_system(language);
    let mut out = Vec::new();
    for g in &groups {
        let gf: Vec<_> = g.idxs.iter().filter_map(|&i| sourced.get(i).cloned()).collect();
        if gf.len() < 2 {
            continue;
        }
        let user = super::contradiction::consistency_user(&gf);
        match collect_blocking(ai.client.clone(), model.to_string(), Some(system.clone()), user) {
            Ok(reply) => {
                for c in super::contradiction::parse_clashes(&reply, &gf) {
                    out.push((c.a.text.clone(), c.b.text.clone(), c.reason.clone()));
                }
            }
            Err(e) => {
                // Don't silently drop a group's clashes — say the gate was partial.
                eprintln!("! agentic contradiction gate: a group check failed ({e}) — its clashes may be missing");
            }
        }
    }
    out
}

/// Truncate a fact to a short display snippet for the progress line.
fn short(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() > 60 {
        format!("{}…", t.chars().take(59).collect::<String>())
    } else {
        t.to_string()
    }
}

/// The gap critic (R6-P3): given the topic and the facts emitted so far, propose
/// follow-up sub-questions for what is still unanswered or contradictory — or
/// nothing when the topic is adequately covered. Bounded by the remaining budget.
#[allow(clippy::too_many_arguments)]
fn critique_gaps(
    ai: &crate::ai::AiClient,
    model: &str,
    language: &str,
    topic: &str,
    outcomes: &[Outcome],
    contradictions: &[(String, String, String)],
    budget_remaining: usize,
) -> Result<Vec<String>> {
    // Feed the critic what's been established (titles + facts) so it reasons about
    // real coverage, not the plan.
    let mut established = String::new();
    for o in outcomes {
        if !o.fact.is_empty() {
            established.push_str(&format!("- {}: {}\n", o.title, o.fact));
        }
    }
    if established.is_empty() {
        established.push_str("(nothing established yet)\n");
    }
    // The contradiction gate found these — the critic should propose questions
    // that resolve them (which fact is right, and why).
    let mut conflicts = String::new();
    for (a, b, reason) in contradictions {
        conflicts.push_str(&format!("- \"{}\" ⇄ \"{}\" ({reason})\n", short(a), short(b)));
    }
    let system = format!(
        "You are a research critic. Given a TOPIC, the facts established so far, and any detected \
         CONTRADICTIONS, list up to {budget_remaining} follow-up sub-questions that are still \
         UNANSWERED, or that would RESOLVE a contradiction (establish which side is correct and \
         why), or that close a gap. If the topic is already covered adequately and nothing \
         contradicts, reply with nothing at all. Reply with ONLY the questions, one per line, no \
         numbering, no commentary. Write them in {language}."
    );
    let user = if conflicts.is_empty() {
        format!("TOPIC: {topic}\n\nESTABLISHED SO FAR:\n{established}")
    } else {
        format!("TOPIC: {topic}\n\nESTABLISHED SO FAR:\n{established}\nDETECTED CONTRADICTIONS:\n{conflicts}")
    };
    let raw = collect_blocking(ai.client.clone(), model.to_string(), Some(system), user)
        .map_err(|e| anyhow!("critic failed: {e}"))?;
    Ok(parse_subquestions(&raw, budget_remaining))
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

#[allow(clippy::too_many_arguments)]
fn render_report(
    topic: &str,
    outcomes: &[Outcome],
    emitted: usize,
    dropped: usize,
    rounds: usize,
    stop: Stop,
    clashes: &[(String, String, String)],
) -> String {
    let mut s = String::from("# Agentic research report\n\n");
    s.push_str(&format!(
        "**Topic:** {topic}\n\n{} sub-question(s) over {rounds} round(s) · {emitted} Fact(s) \
         emitted into the Facts book (model provenance, untrusted — review to promote or dispute) \
         · stopped: {}.\n\n",
        outcomes.len(),
        stop.describe()
    ));
    if dropped > 0 {
        s.push_str(&format!(
            "> {dropped} proposed sub-question(s) were dropped when the budget was reached — raise \
             `research.agentic.max_subquestions` to cover them.\n\n"
        ));
    }
    if !clashes.is_empty() {
        s.push_str(&format!(
            "## ! Contradictions among the emitted facts ({})\n\n\
             Resolve these before trusting the affected facts (`/contradict` re-scans them).\n\n",
            clashes.len()
        ));
        for (a, b, reason) in clashes {
            s.push_str(&format!("- **{}** ⇄ **{}**\n  — {reason}\n", short(a), short(b)));
        }
        s.push('\n');
    }
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
    use super::{Stop, decide_stop, parse_subquestions};

    #[test]
    fn termination_is_bounded_and_prioritised() {
        // Budget spent wins even if the critic wants more and rounds remain.
        assert_eq!(decide_stop(false, 0, 1, 3), Some(Stop::BudgetSpent));
        // Round cap stops when budget remains but rounds are exhausted.
        assert_eq!(decide_stop(false, 4, 3, 3), Some(Stop::RoundCap));
        // Convergence when the critic proposes nothing (budget + rounds left).
        assert_eq!(decide_stop(true, 4, 1, 3), Some(Stop::Converged));
        // Otherwise keep going.
        assert_eq!(decide_stop(false, 4, 1, 3), None);
        // A single-pass config (max_rounds = 1) always stops after round 1 —
        // never loops.
        assert!(decide_stop(false, 5, 1, 1).is_some());
    }

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
