//! INNER_SOCRATES-1 — the `inkhaven inner-socrates` CLI surface. P2 ships `check`
//! (run the deterministic Fast track over prose) and `ledger` (list declared
//! intentions). The Slow track, personas, and conversation land in later phases.

use std::path::Path;

use crate::cli::InnerSocratesCommand;
use crate::error::{Error, Result};
use crate::inner_socrates::fast;
use crate::inner_socrates::intent::{FindingContext, IntentLedger};
use crate::inner_socrates::output::emit_finding;
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::inner_socrates::types::Persona;

pub fn run(project: &Path, cmd: InnerSocratesCommand) -> Result<()> {
    match cmd {
        InnerSocratesCommand::Check { text, paragraph, slow, max_cost, force } => {
            check(project, text, paragraph, slow, max_cost, force)
        }
        InnerSocratesCommand::Timeline { max_cost, force } => timeline(project, max_cost, force),
        InnerSocratesCommand::Ledger => ledger(project),
    }
}

/// Run the Fast track over prose and surface its questions. When the project has
/// an Inner Socrates store, the intent ledger is consulted, findings persist (a
/// re-check replaces a paragraph's prior ones), and they emit to Output.
fn check(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
    slow: bool,
    max_cost: usize,
    force: bool,
) -> Result<()> {
    let store = InnerSocratesStore::open_for_project(project).ok();
    let ledger = store
        .as_ref()
        .and_then(|s| s.load_ledger().ok())
        .unwrap_or_default();

    let (prose, paragraph_id) = resolve_prose(project, text, paragraph)?;
    let persona = Persona::default_inner_socrates();
    let ctx = FindingContext { paragraph_id: paragraph_id.map(|p| p.to_string()), ..Default::default() };

    let mut findings = fast::check_paragraph(&prose, &persona, &ledger, &ctx);

    // The Slow track (LLM) adds the deep questions patterns miss; the fast
    // findings are the seam (the prompt tells the model not to repeat them).
    if slow {
        match run_slow(project, &prose, &persona, &ledger, &ctx, &findings, max_cost, force) {
            Ok(mut deep) => findings.append(&mut deep),
            Err(e) => eprintln!("slow track skipped: {e}"),
        }
    }

    // Persist + emit when a paragraph is identified and a store is present.
    if let (Some(s), Some(pid)) = (store.as_ref(), paragraph_id) {
        let _ = s.clear_findings_for_paragraph(pid);
        for f in &findings {
            let _ = s.insert_finding(f, Some(pid), None);
            emit_finding(f, Some(pid));
        }
    }

    if findings.is_empty() {
        println!("\u{2713} no questions raised (fast track)");
        return Ok(());
    }
    for f in &findings {
        let icon = match f.severity {
            crate::inner_socrates::types::Severity::Probe => "\u{25c6}",   // ◆
            crate::inner_socrates::types::Severity::Inquiry => "\u{25c7}", // ◇
            crate::inner_socrates::types::Severity::Notice => "\u{00b7}",  // ·
        };
        println!(
            "{icon} {} [{}] {}",
            f.severity.label(),
            f.category.label(),
            f.question
        );
    }
    println!("\n{} question(s) · persona: {}", findings.len(), persona.name);
    Ok(())
}

/// Run the Slow (LLM) track over one paragraph: build the persona/intent prompt,
/// call the provider with a cost preflight + retry (reusing WORLD-4's helpers),
/// record usage against the Inner Socrates `slow_track` sub-budget, and return the
/// findings (persona-muted + ledger-suppressed). Cost-capped; errors cleanly with
/// no provider.
#[allow(clippy::too_many_arguments)]
fn run_slow(
    project: &Path,
    prose: &str,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
    fast_findings: &[crate::inner_socrates::types::SocraticFinding],
    soft_cap: usize,
    force: bool,
) -> Result<Vec<crate::inner_socrates::types::SocraticFinding>> {
    use crate::inner_socrates::slow::{
        apply_persona_and_ledger, build_slow_prompt, intent_summary, parse_slow_findings, SLOW_SYSTEM,
    };
    let lang = crate::world::fact_check_lang::detect(prose);
    let prompt = build_slow_prompt(persona, prose, &intent_summary(ledger), fast_findings, lang);
    let raw = socratic_llm_call(project, "slow track", SLOW_SYSTEM, prompt, soft_cap, force)?;
    let parsed = parse_slow_findings(&raw, &persona.id);
    Ok(apply_persona_and_ledger(parsed, persona, ledger, ctx))
}

/// The shared Socratic LLM call: daily-cap check, provider resolution, cost
/// preflight (soft cap overridable with `force`), retry-on-transient with
/// backoff, usage record (`slow_track` sub-budget), returning the raw response.
/// Reused by the prose slow track and the timeline pass.
fn socratic_llm_call(
    project: &Path,
    label: &str,
    system: &str,
    prompt: String,
    soft_cap: usize,
    force: bool,
) -> Result<String> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::world::fact_check_slow::{backoff_delay, is_transient, slow_preflight, PreflightVerdict};

    const DAILY_CAP: i64 = 150;
    const SUB_BUDGET: &str = "slow_track";
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
    let used = store.llm_calls_today(&day, SUB_BUDGET).map_err(|e| Error::Store(format!("{e}")))?;

    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("no LLM provider for the {label}: {e}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("resolving provider: {e}")))?;

    let effective_soft = if force { 0 } else { soft_cap };
    let (pf, verdict) = slow_preflight(system, &prompt, used, DAILY_CAP, effective_soft);
    match verdict {
        PreflightVerdict::DailyCapReached => {
            return Err(Error::Config(format!("daily slow-track cap reached ({DAILY_CAP} calls)")));
        }
        PreflightVerdict::OverSoftCap { est_total_tokens, soft_cap } => {
            return Err(Error::Config(format!(
                "{label} skipped: estimated ~{est_total_tokens} tokens exceeds soft cap {soft_cap} — \
                 re-run with --force or raise --max-cost"
            )));
        }
        PreflightVerdict::Proceed => {}
    }
    eprintln!(
        "{label} · model: {model} · ~{} tokens · {}/{} calls today · reading…",
        pf.est_total_tokens, pf.calls_used, pf.daily_cap
    );

    const MAX_ATTEMPTS: u32 = 3;
    let mut last_err = String::new();
    for attempt in 0..MAX_ATTEMPTS {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            prompt.clone(),
        ) {
            Ok(raw) => {
                let _ = store.record_llm_call(&day, SUB_BUDGET);
                return Ok(raw);
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt + 1 < MAX_ATTEMPTS && is_transient(&last_err) {
                    let d = backoff_delay(attempt);
                    eprintln!("  transient error ({last_err}); retrying in {:.1}s…", d.as_secs_f32());
                    std::thread::sleep(d);
                    continue;
                }
                break;
            }
        }
    }
    Err(Error::Store(format!("LLM error: {last_err}")))
}

/// `inner-socrates timeline` — the timeline pass: compare the project's timeline
/// of events against the prose and ask whether what is declared is dramatized.
/// Silently does nothing when the project has no events.
fn timeline(project: &Path, soft_cap: usize, force: bool) -> Result<()> {
    use crate::config::Config;
    use crate::inner_socrates::slow::{
        build_timeline_prompt, intent_summary, parse_timeline_findings, TIMELINE_SYSTEM,
    };
    use crate::inner_socrates::timeline as tl;
    use crate::inner_socrates::types::Persona;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::Store;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let events = tl::gather_events(&hierarchy);
    if events.is_empty() {
        println!("(no timeline events — nothing to examine)");
        return Ok(());
    }

    let is_store = InnerSocratesStore::open_for_project(project).ok();
    let ledger = is_store.as_ref().and_then(|s| s.load_ledger().ok()).unwrap_or_default();
    let persona = Persona::default_inner_socrates();

    let summary = tl::timeline_summary(&events);
    let densest = tl::densest_window(&events, 365); // a year, in day-ticks
    let prompt = build_timeline_prompt(&persona, &summary, densest, &intent_summary(&ledger));

    let raw = socratic_llm_call(project, "timeline pass", TIMELINE_SYSTEM, prompt, soft_cap, force)?;
    let findings: Vec<_> = parse_timeline_findings(&raw, &persona.id)
        .into_iter()
        .filter(|f| !persona.mutes(f.category))
        .collect();

    let gaps = tl::dramatization_gaps(&events).len();
    println!("timeline · {} event(s), {} undepicted", events.len(), gaps);
    if findings.is_empty() {
        println!("\u{2713} the prose and timeline sit well together");
        return Ok(());
    }
    for f in &findings {
        emit_finding(f, None);
        println!("\u{25c7} {} [{}] {}", f.severity.label(), f.category.label(), f.question);
    }
    println!("\n{} question(s) · persona: {}", findings.len(), persona.name);
    Ok(())
}

/// Resolve `(prose, paragraph_id)` from `--text` or `--paragraph <id>`.
fn resolve_prose(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
) -> Result<(String, Option<uuid::Uuid>)> {
    match (text, paragraph) {
        (Some(t), _) => Ok((t, None)),
        (None, Some(pid)) => {
            use crate::config::Config;
            use crate::project::ProjectLayout;
            use crate::store::Store;
            let id = uuid::Uuid::parse_str(&pid)
                .map_err(|e| Error::Config(format!("bad paragraph id `{pid}`: {e}")))?;
            let layout = ProjectLayout::new(project);
            layout.require_initialized()?;
            let cfg = Config::load_layered(&layout.config_path())?;
            let store = Store::open(layout, &cfg)?;
            let bytes = store
                .get_content(id)
                .map_err(|e| Error::Store(format!("reading paragraph: {e}")))?
                .ok_or_else(|| Error::Config(format!("paragraph `{pid}` not found")))?;
            Ok((String::from_utf8_lossy(&bytes).into_owned(), Some(id)))
        }
        (None, None) => Err(Error::Config("give --text \"…\" or --paragraph <id>".into())),
    }
}

/// List the intent ledger.
fn ledger(project: &Path) -> Result<()> {
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening inner-socrates store: {e}")))?;
    let entries = store.list_intents().map_err(|e| Error::Store(format!("listing: {e}")))?;
    if entries.is_empty() {
        println!("(no intent ledger entries yet)");
        return Ok(());
    }
    for e in &entries {
        let cats: Vec<&str> = e.coverage.iter().map(|c| c.id()).collect();
        println!("  {} [{}] · covers [{}]", e.id, e.kind.id(), cats.join(", "));
        if !e.description.is_empty() {
            println!("      {}", e.description);
        }
    }
    println!("\n{} intent entry(ies).", entries.len());
    Ok(())
}
