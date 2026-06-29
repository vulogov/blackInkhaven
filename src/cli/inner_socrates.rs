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
use crate::inner_socrates::types::{Persona, SocraticFinding, Stance};

pub fn run(project: &Path, cmd: InnerSocratesCommand) -> Result<()> {
    match cmd {
        InnerSocratesCommand::Check { text, paragraph, path, slow, max_cost, force } => {
            check(project, text, paragraph, path, slow, max_cost, force)
        }
        InnerSocratesCommand::Timeline { max_cost, force } => timeline(project, max_cost, force),
        InnerSocratesCommand::Findings(cmd) => findings(project, cmd),
        InnerSocratesCommand::Ledger => ledger(project),
        InnerSocratesCommand::Persona(cmd) => persona(project, cmd),
        InnerSocratesCommand::Suggestions(cmd) => suggestions(project, cmd),
        InnerSocratesCommand::Bundle(cmd) => bundle(project, cmd),
    }
}

/// The `.isl` ledger-bundle surface — export / import.
fn bundle(project: &Path, cmd: crate::cli::BundleCommand) -> Result<()> {
    use crate::cli::BundleCommand;
    use crate::inner_socrates::bundle as isl;
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
    match cmd {
        BundleCommand::Export { scope_level, out } => {
            let scope = match scope_level.as_str() {
                "project" => isl::ExportScope::Project,
                "all" => isl::ExportScope::All,
                _ => isl::ExportScope::Series,
            };
            let entries = store.list_intents().map_err(|e| Error::Store(format!("{e}")))?;
            let body = isl::export(&entries, scope);
            let count = isl::parse(&body).map(|e| e.len()).unwrap_or(0);
            let path = out.map(std::path::PathBuf::from).unwrap_or_else(|| project.join("intent-ledger.isl"));
            crate::io_atomic::write(&path, body.as_bytes())
                .map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
            println!("exported {count} entry(ies) → {}", path.display());
            Ok(())
        }
        BundleCommand::Import { path, conflict } => {
            let body = std::fs::read_to_string(&path)
                .map_err(|e| Error::Config(format!("reading {path}: {e}")))?;
            let incoming = isl::parse(&body).map_err(|e| Error::Config(e.to_string()))?;
            let existing: std::collections::HashSet<String> = store
                .list_intents()
                .map_err(|e| Error::Store(format!("{e}")))?
                .into_iter()
                .map(|e| e.id)
                .collect();
            let (mut added, mut skipped) = (0usize, 0usize);
            for e in &incoming {
                if existing.contains(&e.id) && conflict == "skip" {
                    skipped += 1;
                    continue;
                }
                store.add_intent(e).map_err(|err| Error::Store(format!("{err}")))?;
                added += 1;
            }
            println!("imported {added} entry(ies), {skipped} skipped");
            Ok(())
        }
    }
}

/// The promotion-candidate surface — list / promote / dismiss.
fn suggestions(project: &Path, cmd: crate::cli::SuggestionsCommand) -> Result<()> {
    use crate::cli::SuggestionsCommand;
    use crate::inner_socrates::intent::{IntentEntry, IntentKind, IntentScope, ScopeLevel};
    use crate::inner_socrates::types::Category;
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
    let cat = |s: &str| Category::from_id(s).ok_or_else(|| Error::Config(format!("unknown category `{s}`")));
    match cmd {
        SuggestionsCommand::List { threshold } => {
            let cands = store.promotion_candidates(threshold).map_err(|e| Error::Store(format!("{e}")))?;
            if cands.is_empty() {
                println!("(no promotion candidates — patterns accumulate as you dismiss findings)");
                return Ok(());
            }
            for c in &cands {
                let where_ = if c.chapter_id.is_empty() { "project-wide".into() } else { format!("chapter {}", c.chapter_id) };
                println!(
                    "  {} ({}×, {}) → propose `{}`",
                    c.category.id(),
                    c.count,
                    where_,
                    IntentKind::proposed_for(c.category).id()
                );
            }
            println!("\npromote with: inkhaven inner-socrates suggestions promote <category> [--chapter <id>]");
            Ok(())
        }
        SuggestionsCommand::Promote { category, chapter, description } => {
            let category = cat(&category)?;
            let scope = match &chapter {
                Some(c) => IntentScope::Chapter(c.clone()),
                None => IntentScope::Project,
            };
            let entry = IntentEntry {
                id: uuid::Uuid::new_v4().to_string(),
                kind: IntentKind::proposed_for(category),
                description: description.unwrap_or_else(|| {
                    format!("{} is a deliberate choice here", category.label())
                }),
                scope,
                coverage: vec![category],
                scope_level: ScopeLevel::Project,
            };
            store.add_intent(&entry).map_err(|e| Error::Store(format!("{e}")))?;
            // Refuse the candidate so it doesn't re-suggest now that it's declared.
            let _ = store.refuse_promotion(category, chapter.as_deref().unwrap_or(""));
            println!("declared intent `{}` covering {} — the interrogator will respect it", entry.id, category.id());
            Ok(())
        }
        SuggestionsCommand::Dismiss { category, chapter } => {
            let category = cat(&category)?;
            store
                .refuse_promotion(category, chapter.as_deref().unwrap_or(""))
                .map_err(|e| Error::Store(format!("{e}")))?;
            println!("won't re-suggest promoting {}", category.id());
            Ok(())
        }
    }
}

/// The Reader Persona surface — list / show / activate.
fn persona(project: &Path, cmd: crate::cli::PersonaCommand) -> Result<()> {
    use crate::cli::PersonaCommand;
    use crate::inner_socrates::personas;
    use crate::inner_socrates::types::Category;
    let active = personas::active(project).id;
    match cmd {
        PersonaCommand::List => {
            for p in personas::load_all(project) {
                let mark = if p.id == active { "→" } else { " " };
                println!("{mark} {:<18} {}", p.id, p.voice_summary);
            }
            Ok(())
        }
        PersonaCommand::Show { id } => {
            let p = personas::by_id(project, &id);
            if p.id != id {
                return Err(Error::Config(format!("no persona `{id}`")));
            }
            println!("{} ({})", p.name, p.id);
            println!("  {}", p.voice_summary);
            if !p.voice_notes.is_empty() {
                println!("\n{}\n", p.voice_notes);
            }
            let leaned: Vec<String> = Category::FAST
                .into_iter()
                .chain(Category::SLOW)
                .filter(|c| (p.emphasis_for(*c) - 1.0).abs() > f32::EPSILON)
                .map(|c| format!("{}={:.1}", c.id(), p.emphasis_for(c)))
                .collect();
            if !leaned.is_empty() {
                println!("emphasis: {}", leaned.join(", "));
            }
            Ok(())
        }
        PersonaCommand::Activate { id } => {
            let p = personas::by_id(project, &id);
            if p.id != id {
                return Err(Error::Config(format!("no persona `{id}` (try `persona list`)")));
            }
            let store = InnerSocratesStore::open_for_project(project)
                .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
            store.set_active_persona(&id).map_err(|e| Error::Store(format!("{e}")))?;
            println!("active persona: {} ({})", p.name, p.id);
            Ok(())
        }
        PersonaCommand::New { id, name } => {
            let dir = personas::project_personas_dir(project);
            std::fs::create_dir_all(&dir)
                .map_err(|e| Error::Store(format!("creating {}: {e}", dir.display())))?;
            let path = dir.join(format!("{id}.hjson"));
            if path.exists() {
                return Err(Error::Config(format!("{} already exists", path.display())));
            }
            let name = name.unwrap_or_else(|| id.clone());
            let body = format!(
                "{{\n    id: \"{id}\"\n    name: \"{name}\"\n    \
                 voice_summary: \"A one-line tagline for this reader.\"\n    \
                 voice_notes: \"200-400 words of character — how this reader attends, what it \
                 notices, what it leaves alone. This feeds the Slow-track prompt.\"\n    \
                 emphasis: {{\n        // category id -> weight (1.0 = default, 0.0 mutes)\n        \
                 framing_interrogation: 1.2\n        assumption_surfacing: 1.1\n    }}\n}}\n"
            );
            crate::io_atomic::write(&path, body.as_bytes())
                .map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
            println!("scaffolded {} — edit it, then `persona activate {id}`", path.display());
            println!("(or use `Ctrl+B J → N` in the TUI for an AI-guided wizard)");
            Ok(())
        }
    }
}

/// Run the Fast track over prose and surface its questions. When the project has
/// an Inner Socrates store, the intent ledger is consulted, findings persist (a
/// re-check replaces a paragraph's prior ones), and they emit to Output.
/// Run the Socratic pass over `prose`: the Fast track (skipped for verdict
/// personas — they can neither praise nor charge deterministically), then the
/// LLM Slow track when `slow`, then persist + emit every finding to the Output
/// pane. Returns the findings + the active persona. Shared by the CLI `check`
/// command and the TUI background engage (`Ctrl+B J → E`), so both run one code
/// path. Quiet-safe: the only stderr notice is suppressed when an Output store
/// is installed (the TUI).
pub(crate) fn run_socratic_pass(
    project: &Path,
    prose: &str,
    paragraph_id: Option<uuid::Uuid>,
    slow: bool,
    max_cost: usize,
    force: bool,
) -> Result<(Vec<SocraticFinding>, Persona)> {
    let store = InnerSocratesStore::open_for_project(project).ok();
    let ledger = store
        .as_ref()
        .and_then(|s| s.load_ledger().ok())
        .unwrap_or_default();
    let persona = crate::inner_socrates::personas::active(project);
    let ctx = FindingContext {
        paragraph_id: paragraph_id.map(|p| p.to_string()),
        ..Default::default()
    };

    // The two verdict personas (Defender/Prosecutor) speak only via the LLM
    // verdict — the deterministic Fast track can neither praise nor charge.
    let mut findings = if persona.stance.is_verdict() {
        Vec::new()
    } else {
        fast::check_paragraph(prose, &persona, &ledger, &ctx)
    };

    // The Slow track (LLM) adds the deep questions patterns miss; the fast
    // findings are the seam (the prompt tells the model not to repeat them).
    if slow {
        match run_slow(project, prose, &persona, &ledger, &ctx, &findings, max_cost, force) {
            Ok(mut deep) => findings.append(&mut deep),
            // Quiet in the TUI (Output store installed) — a stray stderr write
            // would corrupt the frame; visible on the CLI.
            Err(e) => {
                if crate::pane::output::active().is_none() {
                    eprintln!("slow track skipped: {e}");
                } else {
                    tracing::warn!("socratic slow track skipped: {e}");
                }
            }
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
    Ok((findings, persona))
}

#[allow(clippy::too_many_arguments)]
fn check(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
    path: Option<String>,
    slow: bool,
    max_cost: usize,
    force: bool,
) -> Result<()> {
    let (prose, paragraph_id) = resolve_prose(project, text, paragraph, path)?;
    let (findings, persona) =
        run_socratic_pass(project, &prose, paragraph_id, slow, max_cost, force)?;

    // Verdict personas need --slow to say anything (Fast track is skipped).
    if persona.stance.is_verdict() && !slow {
        eprintln!(
            "{} delivers an LLM verdict — re-run with --slow",
            persona.name
        );
    }

    // Stance-aware nouns: the verdict personas state praise / concern, the rest ask.
    let noun = match persona.stance {
        Stance::Praise => "point(s) of praise",
        Stance::Concern => "concern(s)",
        Stance::Question => "question(s)",
    };
    if findings.is_empty() {
        let empty = match persona.stance {
            Stance::Praise => "nothing to defend".to_string(),
            Stance::Concern => "no charges to bring".to_string(),
            Stance::Question if slow => "no questions raised".to_string(),
            Stance::Question => "no questions raised (fast track)".to_string(),
        };
        println!("\u{2713} {empty}");
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
    println!("\n{} {noun} · persona: {}", findings.len(), persona.name);
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
        apply_persona_and_ledger, build_slow_prompt, build_verdict_prompt, intent_summary,
        parse_slow_findings, slow_system_for,
    };
    let lang = crate::world::fact_check_lang::detect(prose);
    // AUDIENCE-1: the Socratic framing is genre-aware. A nonfiction / technical
    // genre swaps the fiction assumption for a calibrated context line.
    let genre = crate::config::Config::load_layered(
        &crate::project::ProjectLayout::new(project).config_path(),
    )
    .ok()
    .and_then(|c| c.genre);
    // The two verdict personas (Defender/Prosecutor) get a verdict system prompt
    // + a seam-free user prompt; every other persona keeps the neutral question
    // path unchanged. Output stays bilingual in both modes.
    let system = slow_system_for(persona.stance, genre.as_deref());
    // WORLD-6 — the utopian-architect persona opens with coherence grounding
    // (utopia.duckdb findings → World book structure → Facts), prefixed to the
    // prose context. Only the opening context changes; the question bank does
    // not. Any other persona is untouched.
    let grounded;
    let prose: &str = if persona.id == crate::world::utopia::UTOPIAN_ARCHITECT {
        match crate::world::utopia::build_grounding(project) {
            Some(g) => {
                grounded = format!("{g}\n\n--- MANUSCRIPT ---\n{prose}");
                &grounded
            }
            None => prose,
        }
    } else {
        prose
    };
    let prompt = if persona.stance.is_verdict() {
        build_verdict_prompt(persona, prose, &intent_summary(ledger), lang)
    } else {
        build_slow_prompt(persona, prose, &intent_summary(ledger), fast_findings, lang)
    };
    let raw = socratic_llm_call(project, "slow track", &system, prompt, soft_cap, force)?;
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

    const SUB_BUDGET: &str = InnerSocratesStore::SLOW_SUB_BUDGET;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
    let used = store.llm_calls_today(&day, SUB_BUDGET).map_err(|e| Error::Store(format!("{e}")))?;

    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("no LLM provider for the {label}: {e}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("resolving provider: {e}")))?;

    let effective_soft = if force { 0 } else { soft_cap };
    let daily_cap = cfg.cost.inner_socrates_daily_call_cap;
    let (pf, verdict) = slow_preflight(system, &prompt, used, daily_cap, effective_soft);
    // Quiet on the TUI background path (an Output store is installed): stray
    // stderr writes from a worker thread would corrupt the ratatui frame. The
    // CLI keeps the informative preflight notices.
    let quiet = crate::pane::output::active().is_some();
    match verdict {
        // Informative, not a gate: warn past the daily budget and keep going.
        PreflightVerdict::DailyCapReached => {
            if !quiet {
                eprintln!(
                    "{label}: past today's slow-track budget ({}/{} calls) — continuing (the cap is informative, see `inkhaven cost`).",
                    pf.calls_used, daily_cap
                );
            }
        }
        PreflightVerdict::OverSoftCap { est_total_tokens, soft_cap } => {
            return Err(Error::Config(format!(
                "{label} skipped: estimated ~{est_total_tokens} tokens exceeds soft cap {soft_cap} — \
                 re-run with --force or raise --max-cost"
            )));
        }
        PreflightVerdict::Proceed => {}
    }
    if !quiet {
        eprintln!(
            "{label} · model: {model} · ~{} tokens · {}/{} calls today · reading…",
            pf.est_total_tokens, pf.calls_used, pf.daily_cap
        );
    }

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
                    if !quiet {
                        eprintln!("  transient error ({last_err}); retrying in {:.1}s…", d.as_secs_f32());
                    }
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
    let persona = crate::inner_socrates::personas::active(project);

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

/// Resolve `(prose, paragraph_id)` from `--text`, `--paragraph <uuid>`, or
/// `--path <slug-path>` (the path shown in `inkhaven list`).
fn resolve_prose(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
    path: Option<String>,
) -> Result<(String, Option<uuid::Uuid>)> {
    if let Some(t) = text {
        return Ok((t, None));
    }
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::Store;
    use crate::store::hierarchy::Hierarchy;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    // Resolve the target paragraph id from --paragraph (uuid) or --path (slug).
    let id = match (paragraph, path) {
        (Some(pid), _) => uuid::Uuid::parse_str(&pid)
            .map_err(|e| Error::Config(format!("bad paragraph id `{pid}`: {e}")))?,
        (None, Some(p)) => {
            let hierarchy =
                Hierarchy::load(&store).map_err(|e| Error::Store(format!("hierarchy: {e}")))?;
            crate::scripting::stdlib::helpers::resolve_path(
                &hierarchy,
                &p,
                "inner-socrates check --path",
            )
            .map_err(|e| Error::Config(format!("{e}")))?
            .ok_or_else(|| Error::Config(format!("no node at path `{p}`")))?
        }
        (None, None) => {
            return Err(Error::Config(
                "give --text \"…\", --paragraph <id>, or --path <book/chapter/paragraph>".into(),
            ));
        }
    };
    let bytes = store
        .get_content(id)
        .map_err(|e| Error::Store(format!("reading paragraph: {e}")))?
        .ok_or_else(|| {
            Error::Config(format!(
                "no readable paragraph at `{id}` (is the path a paragraph, not a book/chapter?)"
            ))
        })?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), Some(id)))
}

/// Inspect persisted findings — `list` (all) / `history` (one paragraph, over time).
fn findings(project: &Path, cmd: crate::cli::FindingsCommand) -> Result<()> {
    use crate::cli::FindingsCommand;
    let store = InnerSocratesStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-socrates store: {e}")))?;
    match cmd {
        FindingsCommand::List => {
            let all = store.list_findings().map_err(|e| Error::Store(format!("{e}")))?;
            if all.is_empty() {
                println!("(no findings recorded yet)");
                return Ok(());
            }
            for sf in &all {
                println!("  [{}] {}", sf.finding.category.label(), sf.finding.question);
            }
            println!("\n{} finding(s).", all.len());
            Ok(())
        }
        FindingsCommand::History { paragraph } => {
            let id = uuid::Uuid::parse_str(&paragraph)
                .map_err(|e| Error::Config(format!("bad paragraph id `{paragraph}`: {e}")))?;
            let hist = store.findings_history(id).map_err(|e| Error::Store(format!("{e}")))?;
            if hist.is_empty() {
                println!("(no findings recorded for that paragraph)");
                return Ok(());
            }
            for (at, f) in &hist {
                let when = chrono::DateTime::<chrono::Utc>::from_timestamp(*at, 0)
                    .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
                    .unwrap_or_default();
                println!("  {when}  [{}] {}", f.category.label(), f.question);
            }
            println!("\n{} finding(s) over time.", hist.len());
            Ok(())
        }
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
