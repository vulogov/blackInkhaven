//! WORLD-4 — the `inkhaven realworld` CLI surface (RFC §10.1).
//!
//! P0 ships the astronomy slice: scaffold a `world.hjson`, validate it, and
//! compile the astronomy layer to a human summary or JSON. Storage,
//! materialization into the World book, and the remaining layers land in later
//! increments; the command surface grows with them.

use std::path::Path;

use crate::cli::{ProposalsCommand, RealworldCommand};
use crate::error::{Error, Result};
use crate::world::compile::compile_astronomy;
use crate::world::types::WorldDefinition;

/// The default world-definition filename at the project root.
const WORLD_FILE: &str = "world.hjson";

pub fn run(project: &Path, cmd: RealworldCommand) -> Result<()> {
    match cmd {
        RealworldCommand::New { name, force } => new(project, &name, force),
        RealworldCommand::Validate => validate(project),
        RealworldCommand::Variants { count } => variants(project, count),
        RealworldCommand::Show { json } => show(project, json),
        RealworldCommand::Compile { layer, json, materialize } => {
            compile(project, layer.as_deref(), json, materialize)
        }
        RealworldCommand::Propose => propose(project),
        RealworldCommand::ProposeMyth => propose_myth(project),
        RealworldCommand::ProposeRulers => propose_rulers(project),
        RealworldCommand::ProposeLanguage => propose_language(project),
        RealworldCommand::Proposals { cmd } => proposals(project, cmd),
        RealworldCommand::Places => places(project),
        RealworldCommand::SetCoords { name, x, y, lat, lon } => set_coords(project, &name, x, y, lat, lon),
        RealworldCommand::Calendar => calendar(project),
        RealworldCommand::Chronicle { json } => chronicle(project, json),
        RealworldCommand::Name { json } => name(project, json),
        RealworldCommand::Trade { json } => trade(project, json),
        RealworldCommand::Gazetteer { output } => gazetteer(project, output.as_deref()),
        RealworldCommand::History { json, materialize } => history(project, json, materialize),
        RealworldCommand::Weather { day, lat } => weather(project, day, lat),
        RealworldCommand::Ecology => ecology(project),
        RealworldCommand::Polities => polities(project),
        RealworldCommand::Culture => culture(project),
        RealworldCommand::Travel { from, to, from_x, from_y, to_x, to_y, days, mode } => {
            travel(project, from, to, from_x, from_y, to_x, to_y, days, &mode)
        }
        RealworldCommand::Scene { place, day, lat } => scene(project, place, day, lat),
        RealworldCommand::Magic { materialize } => magic(project, materialize),
        RealworldCommand::Map { spec_only, no_ingest } => map(project, spec_only, no_ingest),
        RealworldCommand::CoLocation => co_location(project),
        RealworldCommand::Coherence { node, max_cost, force } => {
            coherence(project, &node, max_cost, force)
        }
        RealworldCommand::Critique { max_cost, force, write_notes, lints_only } => {
            critique(project, max_cost, force, write_notes, lints_only)
        }
    }
}

/// Fact-check prose against the world (fast track). `--text` checks a literal
/// string; `--paragraph` reads a paragraph's content from the store.
#[allow(clippy::too_many_arguments)]
pub fn fact_check(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
    slow: bool,
    max_cost: usize,
    force: bool,
    timeline_aware: &str,
    timeline_only: bool,
) -> Result<()> {
    use crate::world::fact_check::check_paragraph;
    // The magic ledger (if any) is consulted; a missing world.hjson is fine.
    let def = load(project).ok();
    let ledger = def.as_ref().and_then(|d| d.magic.clone()).unwrap_or_default();

    let (prose, paragraph_id) = match (text, paragraph) {
        (Some(t), _) => (t, None),
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
            (String::from_utf8_lossy(&bytes).into_owned(), Some(id))
        }
        (None, None) => {
            return Err(Error::Config("give --text \"…\" or --paragraph <id>".into()));
        }
    };

    // Build the world context: the gazetteer (world-linked Places) lets the
    // climate + demographics checks resolve place names; the moon names feed the
    // astronomy check.
    let mut places = crate::world::storage::WorldStore::open_for_project(project)
        .ok()
        .and_then(|ws| ws.list_place_links().ok())
        .unwrap_or_default();
    let moons: Vec<String> = def
        .as_ref()
        .map(|d| {
            crate::world::compile::compile_astronomy(&d.astronomy)
                .moons
                .iter()
                .map(|m| m.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut minerals: Vec<String> = def
        .as_ref()
        .and_then(|d| geology_for(project, d).ok())
        .map(|g| g.minerals.iter().map(|m| m.mineral.clone()).collect())
        .unwrap_or_default();
    // Merge author-declared geography landmarks + economy/geology resources.
    if let Some(d) = def.as_ref() {
        places.extend(crate::world::fact_check::declared_places(d));
        minerals.extend(d.declared_minerals());
    }
    let world_ctx = if !places.is_empty() || !moons.is_empty() || !minerals.is_empty() {
        Some(crate::world::fact_check::WorldContext::new(
            crate::world::fact_check::Gazetteer::new(places.clone()),
            moons.clone(),
            minerals.clone(),
        ))
    } else {
        None
    };

    // WORLD-5 — `--timeline-only` skips the world checks; `--timeline-aware off`
    // skips the timeline ones. Default (`auto`) runs the timeline checks when the
    // paragraph is identified and the project has events.
    let mut findings = if timeline_only {
        Vec::new()
    } else {
        check_paragraph(&prose, &ledger, &[], world_ctx.as_ref())
    };
    if timeline_aware != "off" {
        if let Some(pid) = paragraph_id {
            findings.extend(timeline_findings(project, pid, &prose, &ledger));
        } else if timeline_aware == "on" || timeline_only {
            eprintln!("timeline checks need --paragraph <id> (a linked paragraph), not --text");
        }
    }
    if slow {
        match run_slow_track(project, &prose, def.as_ref(), &ledger, &places, &moons, &minerals, &findings, max_cost, force) {
            Ok(mut slow_findings) => findings.append(&mut slow_findings),
            Err(e) => eprintln!("slow track skipped: {e}"),
        }
    }
    if findings.is_empty() {
        println!("✓ no issues found");
        eprintln!("({})", crate::world::fact_check_lang::backend_note());
        return Ok(());
    }
    for f in &findings {
        let icon = match f.severity.as_str() {
            "contradiction" => "⊗",
            "warning" => "⚠",
            _ => "●",
        };
        let note = f.suppressed_by.as_deref().map(|r| format!(" (ok — magic rule `{r}`)")).unwrap_or_default();
        println!("{icon} [{}] {}{note}", f.category, f.body);
    }
    println!("\n{} finding(s).", findings.len());
    eprintln!("({})", crate::world::fact_check_lang::backend_note());
    Ok(())
}

/// Build the fact-checker's magic ledger + world context for a project (the world
/// half of `inkhaven check`). Returns a default ledger + `None` context when there's
/// no `world.hjson`; `check_paragraph` still runs the prose-only checks (travel
/// time) against an absent context.
pub(crate) fn build_world_context(
    project: &Path,
) -> (
    crate::world::types::MagicLedger,
    Option<crate::world::fact_check::WorldContext>,
) {
    let def = load(project).ok();
    let ledger = def.as_ref().and_then(|d| d.magic.clone()).unwrap_or_default();
    let mut places = crate::world::storage::WorldStore::open_for_project(project)
        .ok()
        .and_then(|ws| ws.list_place_links().ok())
        .unwrap_or_default();
    let moons: Vec<String> = def
        .as_ref()
        .map(|d| {
            crate::world::compile::compile_astronomy(&d.astronomy)
                .moons
                .iter()
                .map(|m| m.name.clone())
                .collect()
        })
        .unwrap_or_default();
    let mut minerals: Vec<String> = def
        .as_ref()
        .and_then(|d| geology_for(project, d).ok())
        .map(|g| g.minerals.iter().map(|m| m.mineral.clone()).collect())
        .unwrap_or_default();
    if let Some(d) = def.as_ref() {
        places.extend(crate::world::fact_check::declared_places(d));
        minerals.extend(d.declared_minerals());
    }
    let ctx = if !places.is_empty() || !moons.is_empty() || !minerals.is_empty() {
        Some(crate::world::fact_check::WorldContext::new(
            crate::world::fact_check::Gazetteer::new(places),
            moons,
            minerals,
        ))
    } else {
        None
    };
    (ledger, ctx)
}

/// The slow track: an LLM pass for subtle contradictions the patterns miss.
/// Cost-capped (daily call ceiling). Returns its findings; the fast findings are
/// passed in as the seam (the prompt tells the model not to repeat them).
#[allow(clippy::too_many_arguments)]
fn run_slow_track(
    project: &Path,
    prose: &str,
    def: Option<&WorldDefinition>,
    ledger: &crate::world::types::MagicLedger,
    places: &[crate::world::proposals::PlaceLink],
    moons: &[String],
    minerals: &[String],
    fast: &[crate::world::fact_check::Finding],
    soft_cap: usize,
    force: bool,
) -> Result<Vec<crate::world::fact_check::Finding>> {
    use crate::world::fact_check_slow::{build_slow_prompt, magic_summary, world_summary, SLOW_SYSTEM};

    let def = def.ok_or_else(|| Error::Config("slow track needs a world.hjson".into()))?;
    let summary = world_summary(def, places, moons, minerals);
    let magic = magic_summary(ledger);
    let prompt = build_slow_prompt(prose, &summary, &magic, fast);
    slow_llm_call(project, "slow track", SLOW_SYSTEM, prompt, soft_cap, force)
}

/// TUI entry point for the slow track: build the world context from the project,
/// run the fast check as the seam, then the cost-capped slow call. Returns the
/// findings or a single error string (no stderr noise). Safe to call from a
/// background worker thread (it opens its own world store). The daily cap and the
/// per-call soft cap are enforced inside.
pub(crate) fn slow_track_for_tui(
    project: &Path,
    prose: &str,
) -> std::result::Result<Vec<crate::world::fact_check::Finding>, String> {
    use crate::world::compile::compile_astronomy;
    use crate::world::fact_check::{check_paragraph, Gazetteer, WorldContext};
    use crate::world::storage::WorldStore;

    let def = load(project).map_err(|e| e.to_string())?;
    let ledger = def.magic.clone().unwrap_or_default();
    let mut places = WorldStore::open_for_project(project)
        .ok()
        .and_then(|ws| ws.list_place_links().ok())
        .unwrap_or_default();
    places.extend(crate::world::fact_check::declared_places(&def));
    let moons: Vec<String> =
        compile_astronomy(&def.astronomy).moons.iter().map(|m| m.name.clone()).collect();
    let mut minerals: Vec<String> = geology_for(project, &def)
        .map(|g| g.minerals.iter().map(|m| m.mineral.clone()).collect())
        .unwrap_or_default();
    minerals.extend(def.declared_minerals());

    let ctx = WorldContext::new(Gazetteer::new(places.clone()), moons.clone(), minerals.clone());
    let fast = check_paragraph(prose, &ledger, &[], Some(&ctx));
    run_slow_track(project, prose, Some(&def), &ledger, &places, &moons, &minerals, &fast, 6000, false)
        .map_err(|e| e.to_string())
}

/// The shared slow-track LLM call: daily-cap check, provider resolution, cost
/// preflight (soft cap overridable with `force`), retry-on-transient with
/// backoff, usage record, and response parse. Used by both the per-paragraph slow
/// track and the cross-paragraph coherence pass.
fn slow_llm_call(
    project: &Path,
    label: &str,
    system: &str,
    prompt: String,
    soft_cap: usize,
    force: bool,
) -> Result<Vec<crate::world::fact_check::Finding>> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::world::fact_check_slow::{
        backoff_delay, is_transient, parse_slow_findings, slow_preflight, PreflightVerdict,
    };
    use crate::world::storage::WorldStore;

    // Load config first so the daily-cap day-key honors goals.day_boundary.
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("world store: {e}")))?;
    let used = store.llm_calls_today(&day).map_err(|e| Error::Store(format!("{e}")))?;

    // The LLM provider (errors cleanly when none is configured).
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("no LLM provider for the {label}: {e}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("resolving provider: {e}")))?;

    // Preflight: estimate the cost and gate on the daily hard cap + per-call soft
    // cap (the soft cap is overridable with --force; 0 disables it).
    let effective_soft = if force { 0 } else { soft_cap };
    let (pf, verdict) =
        slow_preflight(system, &prompt, used, cfg.cost.world_daily_call_cap, effective_soft);
    match verdict {
        // Cost control is informative, not a gate: past the daily budget we warn
        // and proceed — the author decides whether to keep going.
        PreflightVerdict::DailyCapReached => {
            eprintln!(
                "{label}: past today's slow-track budget ({}/{} calls) — continuing (the cap is informative, see `inkhaven cost`).",
                pf.calls_used, cfg.cost.world_daily_call_cap
            );
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
        "{label} · model: {model} · ~{} tokens · {}/{} calls today · checking…",
        pf.est_total_tokens, pf.calls_used, pf.daily_cap
    );

    // Call with retry-on-transient (rate limit / timeout / upstream 5xx).
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
                let _ = store.record_llm_call(&day);
                return Ok(parse_slow_findings(&raw));
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

/// WORLD-4 slow-track **coherence pass**: gather every paragraph under a node
/// (book / chapter) in document order and ask the LLM for contradictions *between*
/// them — a character in two places, a fact reversed, a timeline that doesn't add
/// up. One cost-capped call; findings cite the `¶` numbers.
fn coherence(project: &Path, node_id: &str, max_cost: usize, force: bool) -> Result<()> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::node::NodeKind;
    use crate::store::Store;
    use crate::world::fact_check_slow::{
        build_coherence_prompt, magic_summary, world_summary, COHERENCE_SYSTEM,
    };
    use crate::world::storage::WorldStore;

    let id = uuid::Uuid::parse_str(node_id)
        .map_err(|e| Error::Config(format!("bad node id `{node_id}`: {e}")))?;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    // The container's paragraphs, document order.
    let para_ids: Vec<uuid::Uuid> = hierarchy
        .collect_subtree(id)
        .into_iter()
        .filter(|pid| hierarchy.get(*pid).map(|n| n.kind == NodeKind::Paragraph).unwrap_or(false))
        .collect();
    if para_ids.is_empty() {
        return Err(Error::Config("no paragraphs under that node".into()));
    }
    let labeled: Vec<(String, String)> = para_ids
        .iter()
        .map(|pid| {
            let label = hierarchy.get(*pid).map(|n| hierarchy.slug_path(n)).unwrap_or_else(|| pid.to_string());
            let text = store
                .get_content(*pid)
                .ok()
                .flatten()
                .map(|b| String::from_utf8_lossy(&b).into_owned())
                .unwrap_or_default();
            (label, text)
        })
        .collect();

    // World context (coherence needs a world.hjson, like the slow track).
    let def = load(project)?;
    let ledger = def.magic.clone().unwrap_or_default();
    let mut places = WorldStore::open_for_project(project)
        .ok()
        .and_then(|ws| ws.list_place_links().ok())
        .unwrap_or_default();
    places.extend(crate::world::fact_check::declared_places(&def));
    let moons: Vec<String> =
        crate::world::compile::compile_astronomy(&def.astronomy).moons.iter().map(|m| m.name.clone()).collect();
    let mut minerals: Vec<String> = geology_for(project, &def)
        .map(|g| g.minerals.iter().map(|m| m.mineral.clone()).collect())
        .unwrap_or_default();
    minerals.extend(def.declared_minerals());

    let summary = world_summary(&def, &places, &moons, &minerals);
    let magic = magic_summary(&ledger);
    let (prompt, kept) = build_coherence_prompt(&labeled, &summary, &magic);
    if kept.is_empty() {
        println!("✓ no non-empty paragraphs to check");
        return Ok(());
    }
    println!("coherence · {} paragraph(s) under `{}`", kept.len(), node_id);

    let findings = slow_llm_call(project, "coherence", COHERENCE_SYSTEM, prompt, max_cost, force)?;
    if findings.is_empty() {
        println!("✓ paragraphs are consistent");
        return Ok(());
    }
    for f in &findings {
        let icon = match f.severity.as_str() {
            "contradiction" => "⊗",
            "warning" => "⚠",
            _ => "●",
        };
        println!("{icon} [{}] {}", f.category, f.body);
    }
    println!("\n{} cross-paragraph finding(s).", findings.len());
    Ok(())
}

/// WORLD-5 — the `co_location` check: a character placed in two different places
/// at overlapping times, per the timeline. Pure (no LLM); respects the magic
/// ledger. Resolves character / place names from the hierarchy for the message.
fn co_location(project: &Path) -> Result<()> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::Store;
    use crate::world::fact_check::emit_finding;
    use crate::world::timeline_context as tc;

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let events = tc::gather_events(&hierarchy);
    if events.is_empty() {
        println!("(no timeline events — nothing to check)");
        return Ok(());
    }
    let ledger = load(project).ok().and_then(|d| d.magic).unwrap_or_default();
    let name = |id: uuid::Uuid| hierarchy.get(id).map(|n| n.title.clone()).unwrap_or_else(|| "?".into());

    let conflicts = tc::co_location_conflicts(&events);
    if conflicts.is_empty() {
        println!("\u{2713} no co-location conflicts in {} event(s)", events.len());
        return Ok(());
    }
    for c in &conflicts {
        let ctx = crate::world::types::magic::CheckContext { category: "co_location", ..Default::default() };
        let suppressed_by = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
        let severity = if suppressed_by.is_some() { "info" } else { "contradiction" };
        let body = format!(
            "{} is in {} (\u{201c}{}\u{201d}) and {} (\u{201c}{}\u{201d}) at overlapping times.",
            name(c.character), name(c.place_a), c.title_a, name(c.place_b), c.title_b
        );
        let finding = crate::world::fact_check::Finding {
            category: "co_location".into(),
            severity: severity.into(),
            body: body.clone(),
            body_en: body.clone(),
            suppressed_by: suppressed_by.clone(),
        };
        emit_finding(&finding, None);
        let icon = if suppressed_by.is_some() { "\u{25cf}" } else { "\u{2297}" };
        let note = suppressed_by.map(|r| format!(" (ok — magic rule `{r}`)")).unwrap_or_default();
        println!("{icon} [co_location] {body}{note}");
    }
    println!("\n{} co-location conflict(s).", conflicts.len());
    Ok(())
}

/// WORLD-5 — build a paragraph's timeline context (events + calendar) and run the
/// timeline-aware checks. Empty when the project has no events / no calendar.
fn timeline_findings(
    project: &Path,
    paragraph_id: uuid::Uuid,
    prose: &str,
    ledger: &crate::world::types::MagicLedger,
) -> Vec<crate::world::fact_check::Finding> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::Store;
    use crate::timeline::calendar::Calendar;
    use crate::world::timeline_context as tc;

    let layout = ProjectLayout::new(project);
    if layout.require_initialized().is_err() {
        return Vec::new();
    }
    let Ok(cfg) = Config::load_layered(&layout.config_path()) else {
        return Vec::new();
    };
    let Ok(store) = Store::open(layout, &cfg) else {
        return Vec::new();
    };
    let Ok(hierarchy) = Hierarchy::load(&store) else {
        return Vec::new();
    };
    let events = tc::gather_events(&hierarchy);
    if events.is_empty() {
        return Vec::new();
    }
    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());
    let day = calendar.ticks_per("day").unwrap_or(1);
    let ctx = tc::build_context(paragraph_id, &events, &calendar, 90 * day);
    let mut out = crate::world::fact_check::check_timeline(prose, &ctx, ledger);
    out.extend(crate::world::fact_check::check_date_coherence(prose, &ctx, ledger));
    out.extend(crate::world::fact_check::check_travel_timeline(prose, &ctx, &events, day, ledger));
    out
}

/// Show (and optionally materialize) the magic ledger.
fn magic(project: &Path, materialize: bool) -> Result<()> {
    let def = load(project)?;
    let ledger = def.magic.clone().unwrap_or_default();
    if ledger.rules.is_empty() {
        println!("(no magic rules — add a `magic:` block to world.hjson)");
    } else {
        println!("magic ledger · {} ({})", def.name, if ledger.enabled { "enabled" } else { "DISABLED" });
        for r in &ledger.rules {
            println!("  {} · covers [{}]", r.kind, r.covers.join(", "));
            if !r.description.is_empty() {
                println!("      {}", r.description);
            }
            if let Some(roles) = &r.applicable_to.roles {
                println!("      roles: {}", roles.join(", "));
            }
        }
    }
    // W7-P4 — validate the ledger for dead / malformed / redundant rules.
    let issues = ledger.lint();
    if issues.is_empty() {
        if !ledger.rules.is_empty() {
            println!("  ✓ ledger is consistent");
        }
    } else {
        println!("  {} issue(s):", issues.len());
        for w in &issues {
            println!("    ⚠ {w}");
        }
    }
    if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        let r = crate::world::materialize::materialize_magic(&store, &cfg, &ledger)?;
        println!("  → World/{}: {} created, {} updated", r.chapter, r.created.len(), r.updated.len());
    }
    Ok(())
}

/// List the Place ↔ World cross-references.
fn places(project: &Path) -> Result<()> {
    use crate::world::storage::WorldStore;
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let links = store.list_place_links().map_err(|e| Error::Store(format!("listing: {e}")))?;
    if links.is_empty() {
        println!("(no world-linked places yet — accept some proposals)");
        return Ok(());
    }
    for l in &links {
        println!(
            "  {:<16} ({:>3},{:<3}) · {:<18} · {:<14} · pop {}",
            l.name, l.x, l.y, l.climate_zone, l.hydrology_basis, l.population
        );
    }
    println!("\n{} world-linked place(s).", links.len());
    Ok(())
}

/// WORLD-12 — position a Place on the world grid so `realworld map` can draw it.
/// Resolves the Place by name in the Places book, converts geographic degrees to
/// grid cells when given, fills the biome from the compiled climate under the
/// cell, and writes (or moves) the Place ↔ World coordinate link. Works for a
/// hand-authored Place that never came from a compiler proposal.
fn set_coords(
    project: &Path,
    name: &str,
    x: Option<usize>,
    y: Option<usize>,
    lat: Option<f64>,
    lon: Option<f64>,
) -> Result<()> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::{NodeKind, Store, SYSTEM_TAG_PLACES};
    use crate::world::compile::{compile_astronomy, compile_climate};
    use crate::world::proposals::PlaceLink;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let (w, h) = (climate.width, climate.height);

    // Resolve the target cell from grid args or geographic degrees.
    let (cx, cy) = match (x, y, lat, lon) {
        (Some(gx), Some(gy), _, _) => (gx.min(w.saturating_sub(1)), gy.min(h.saturating_sub(1))),
        (_, _, Some(la), Some(lo)) => (lon_to_col(lo, w), lat_to_row(la, h)),
        _ => {
            return Err(Error::Config(
                "pass a location: --x <col> --y <row>, or --lat <deg> --lon <deg>".into(),
            ))
        }
    };

    // Resolve the Place node (by title) in the Places book to get its id.
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let h_tree = Hierarchy::load(&store)?;
    let places_book = h_tree
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_PLACES))
        .cloned()
        .ok_or_else(|| Error::Store("Places system book missing".into()))?;
    let node = h_tree
        .collect_subtree(places_book.id)
        .into_iter()
        .filter_map(|id| h_tree.get(id))
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| Error::Config(format!("no Place named `{name}` in the Places book")))?;

    let ws = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let existing = ws
        .list_place_links()
        .map_err(|e| Error::Store(format!("listing places: {e}")))?
        .into_iter()
        .find(|l| l.place_id == node.id);

    let biome = climate.biome.get(cy * w + cx).map(|b| b.as_str().to_string()).unwrap_or_default();

    // INSERT OR REPLACE keys on place_id, so a fresh link and a move both go
    // through insert_place_link; the biome is refreshed under the (new) cell.
    let link = PlaceLink {
        place_id: node.id,
        name: node.title.clone(),
        biome: biome.clone(),
        climate_zone: biome.clone(),
        hydrology_basis: existing.as_ref().map(|l| l.hydrology_basis.clone()).unwrap_or_default(),
        population: existing.as_ref().map(|l| l.population).unwrap_or(0),
        x: cx,
        y: cy,
    };
    ws.insert_place_link(&link).map_err(|e| Error::Store(format!("writing link: {e}")))?;

    let latd = row_to_latitude(cy, h);
    let lond = col_to_lon(cx, w);
    println!(
        "{} · cell ({cx},{cy}) · {:.1}°{} {:.1}°{}{}",
        node.title,
        latd.abs(),
        if latd >= 0.0 { "N" } else { "S" },
        lond.abs(),
        if lond >= 0.0 { "E" } else { "W" },
        if biome.is_empty() { String::new() } else { format!(" · {biome}") },
    );
    println!("  {} — render it with `inkhaven realworld map`", if existing.is_some() { "moved" } else { "placed" });
    Ok(())
}

/// WORLD-7 (W7-P3) — derive a story-Timeline calendar (`timeline.calendar`) from
/// the world's astronomy: the day→month→year unit stack (carrying any author
/// month names) and the four season markers. Pure + testable.
fn build_timeline_calendar(
    def: &WorldDefinition,
    astro: &crate::world::types::AstronomyOutput,
) -> crate::timeline::calendar::CalendarConfig {
    use crate::timeline::calendar::{CalendarConfig, SeasonDef, UnitDef};
    let cal = &def.astronomy.calendar;
    let months = cal.months.max(1);
    let month_len = cal.month_length_days.max(1);

    let units = vec![
        UnitDef { name: "day".into(), per_parent: month_len, names: Vec::new() },
        UnitDef { name: "month".into(), per_parent: months, names: cal.month_names.clone() },
        UnitDef { name: "year".into(), per_parent: 0, names: Vec::new() },
    ];

    // Each astronomy marker starts a season and spans to the next; marker names
    // like "spring_equinox" → "spring".
    let mut markers = astro.seasons.clone();
    markers.sort_by(|a, b| {
        a.year_fraction.partial_cmp(&b.year_fraction).unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seasons = Vec::new();
    for (i, m) in markers.iter().enumerate() {
        let start_month = ((m.year_fraction * months as f64).floor() as u32).min(months - 1) + 1;
        let next = markers.get(i + 1).map(|n| n.year_fraction).unwrap_or(markers[0].year_fraction + 1.0);
        let span = (((next - m.year_fraction) * months as f64).round() as i64).max(1) as u32;
        let name = m.name.split(['_', ' ']).next().unwrap_or(&m.name).to_lowercase();
        seasons.push(SeasonDef { name, start_month, span_months: span });
    }

    CalendarConfig {
        preset: "custom".into(),
        base_unit: "day".into(),
        units,
        seasons,
        epoch_label: String::new(),
        epoch_before_label: String::new(),
        display_format: String::new(),
        parse_aliases: Vec::new(),
    }
}

/// `realworld calendar` — print the story-Timeline calendar derived from the
/// world's astronomy, ready to adopt under `timeline.calendar`. The simulation
/// proposes; the author adopts (authority discipline).
fn calendar(project: &Path) -> Result<()> {
    use crate::world::compile::compile_astronomy;
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let cal = &def.astronomy.calendar;
    let tl = build_timeline_calendar(&def, &astro);

    let year_days = cal.months.saturating_mul(cal.month_length_days);
    println!(
        "calendar · {} — {} months × {} days = {}-day year",
        def.name, cal.months, cal.month_length_days, year_days
    );
    let names = if cal.month_names.is_empty() {
        format!("(unnamed — months 1..{})", cal.months)
    } else {
        cal.month_names.join(", ")
    };
    println!("  months:  {names}");
    println!(
        "  seasons: {}",
        tl.seasons
            .iter()
            .map(|s| format!("{} (from month {}, {} mo)", s.name, s.start_month, s.span_months))
            .collect::<Vec<_>>()
            .join(" · ")
    );

    let body = serde_json::to_string_pretty(&tl)
        .map_err(|e| Error::Store(format!("serializing calendar: {e}")))?;
    println!(
        "\nAdopt it as your story's calendar — set `timeline.enabled: true` and paste this\nas `timeline.calendar` in inkhaven.hjson:\n\n{body}"
    );
    Ok(())
}

/// Resolve an accepted Place / world-link name to its full record.
fn resolve_place_link(
    project: &Path,
    name: &str,
) -> Result<crate::world::proposals::PlaceLink> {
    use crate::world::storage::WorldStore;
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let links = store
        .list_place_links()
        .map_err(|e| Error::Store(format!("listing places: {e}")))?;
    links
        .into_iter()
        .find(|l| l.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            Error::Config(format!(
                "place `{name}` not found among accepted world places — accept it via `realworld proposals`, or pass coordinates"
            ))
        })
}

/// Resolve a place name to its map coordinates.
fn resolve_place(project: &Path, name: &str) -> Result<(f64, f64)> {
    let l = resolve_place_link(project, name)?;
    Ok((l.x as f64, l.y as f64))
}

/// Grid row → latitude in degrees, at the row's *cell centre* — the same
/// convention the climate layer uses (`90 − (y+0.5)/h·180`, climate_layer.rs).
/// BUG-16: the old `y/(h−1)` edge convention disagreed with the climate a
/// settlement actually experiences by a half-cell (~0.75° on the 120-row grid).
fn row_to_latitude(y: usize, height: usize) -> f64 {
    if height == 0 {
        return 0.0;
    }
    90.0 - (y as f64 + 0.5) / height as f64 * 180.0
}

/// Latitude (−90..90) → grid row (inverse of [`row_to_latitude`]), clamped.
fn lat_to_row(lat: f64, height: usize) -> usize {
    if height == 0 {
        return 0;
    }
    let y = (90.0 - lat.clamp(-90.0, 90.0)) / 180.0 * height as f64 - 0.5;
    y.round().clamp(0.0, (height - 1) as f64) as usize
}

/// Grid column → longitude in degrees, at the column's cell centre (col 0's
/// centre is just east of −180°, spanning the full 360°).
fn col_to_lon(x: usize, width: usize) -> f64 {
    if width == 0 {
        return 0.0;
    }
    (x as f64 + 0.5) / width as f64 * 360.0 - 180.0
}

/// Longitude (−180..180) → grid column (inverse of [`col_to_lon`]), clamped.
fn lon_to_col(lon: f64, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let x = (lon.clamp(-180.0, 180.0) + 180.0) / 360.0 * width as f64 - 0.5;
    x.round().clamp(0.0, (width - 1) as f64) as usize
}

/// WORLD-10 — `realworld scene --place <name> --day <N>`: a scene brief for the
/// writer — the local season + weather at the place's latitude on that day, the
/// place's biome / climate, and the culture whose realm it sits nearest. The
/// composition the in-editor world-context pane will show (auto-detecting the
/// scene's place + date is the remaining wiring).
fn scene(project: &Path, place: Option<String>, day: f64, lat: Option<f64>) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let seed = def.seed_u64();

    let link = place.as_deref().map(|n| resolve_place_link(project, n)).transpose()?;
    let latitude = lat.or_else(|| link.as_ref().map(|l| row_to_latitude(l.y, climate.height)));

    // Peoples for the nearest-realm culture, then the shared composition.
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cul = compile_culture(&pol, &capital_biomes, &def.cultures, seed);
    let brief = crate::world::scene::scene_brief(&astro, &pol, &cul, link.as_ref(), Some(day), latitude);

    println!("scene · {}", def.name);
    if let (Some(name), Some(biome)) = (&brief.place, &brief.biome) {
        let cz = brief.climate_zone.as_deref().unwrap_or("");
        println!("  place:    {name} · {biome} {cz}");
    }
    println!(
        "  when:     day {:.0} of {:.0}{}",
        day,
        astro.year_length_planet_days,
        latitude.map(|l| format!(" · lat {l:.0}°")).unwrap_or_default()
    );
    if let (Some(s), Some(c)) = (&brief.season, &brief.conditions) {
        println!("  season:   {s} · {c}");
    }
    if let Some(realm) = &brief.realm {
        println!("  people:   {realm} — {}", brief.ethos.as_deref().unwrap_or(""));
        if let (Some(b), Some(t)) = (&brief.belief, &brief.tongue) {
            println!("            belief: {b} · tongue: {t}");
        }
    }
    // WORLD-13 — nearest neighbouring Place, for spatial context while writing.
    // Any coordinate-bearing Place (compiler-born or hand-positioned via
    // `set-coords`) can be the anchor or the neighbour.
    if let Some(here) = &link {
        if let Some((name, kind, km, dir)) = nearest_feature(project, &def, geo.width, geo.height, here) {
            println!("  nearby:   {name} ({kind}) — {km:.0} km {dir}");
        }
    }
    Ok(())
}

/// The nearest other coordinate-bearing Place to `here`, as (name, km, bearing).
/// `None` when the world store is absent or no other Place has coordinates.
/// The nearest named feature to `here` — a coordinate-bearing Place, a declared
/// landmark (`geography.landmarks` with a position), or a declared named water
/// (`hydrology.rivers/lakes/seas` with a `from`/`to` cell) — as
/// (name, kind, km, bearing). `None` when nothing else has coordinates.
fn nearest_feature(
    project: &Path,
    def: &WorldDefinition,
    w: usize,
    h: usize,
    here: &crate::world::proposals::PlaceLink,
) -> Option<(String, &'static str, f64, &'static str)> {
    // Candidate coordinate-bearing named features (excluding the anchor's cell).
    let mut cands: Vec<(String, &'static str, usize, usize)> = Vec::new();
    if let Ok(ws) = crate::world::storage::WorldStore::open_for_project(project) {
        if let Ok(links) = ws.list_place_links() {
            for l in links {
                if l.place_id != here.place_id {
                    cands.push((l.name, "place", l.x, l.y));
                }
            }
        }
    }
    if let Some(g) = def.geography.as_ref() {
        for lm in &g.landmarks {
            if let Some((x, y)) = lm.grid(w, h) {
                if (x, y) != (here.x, here.y) {
                    cands.push((lm.name.clone(), "landmark", x, y));
                }
            }
        }
    }
    if let Some(hy) = def.hydrology.as_ref() {
        for wtr in hy.rivers.iter().chain(hy.lakes.iter()).chain(hy.seas.iter()) {
            if let Some([x, y]) = wtr.from.or(wtr.to) {
                if (x, y) != (here.x, here.y) {
                    cands.push((wtr.name.clone(), "water", x, y));
                }
            }
        }
    }
    let radius = def.astronomy.planet.radius_earth;
    cands
        .into_iter()
        .map(|(name, kind, x, y)| {
            let dx = x as f64 - here.x as f64;
            let dy = y as f64 - here.y as f64;
            (name, kind, crate::world::travel::distance_km(radius, w, h, dx, dy), bearing(here.x, here.y, x, y))
        })
        .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal))
}

/// A rough 8-point compass bearing from cell `(x0,y0)` to `(x1,y1)`. Grid row 0
/// is north, so a larger `y` lies further south.
fn bearing(x0: usize, y0: usize, x1: usize, y1: usize) -> &'static str {
    let dx = x1 as f64 - x0 as f64; // east positive
    let dy = y0 as f64 - y1 as f64; // north positive (row 0 = north pole)
    if dx == 0.0 && dy == 0.0 {
        return "adjacent";
    }
    let a = (dy.atan2(dx).to_degrees() + 360.0) % 360.0; // 0° = east, 90° = north
    ["E", "NE", "N", "NW", "W", "SW", "S", "SE"][(((a + 22.5) / 45.0) as usize) % 8]
}

/// WORLD-10 — `realworld travel`: is a journey between two map cells plausible
/// in the claimed time by the given mode? Uses the planet size + grid for the
/// real distance and consults the magic ledger's `travel_time` rules.
#[allow(clippy::too_many_arguments)]
fn travel(
    project: &Path,
    from: Option<String>,
    to: Option<String>,
    from_x: f64,
    from_y: f64,
    to_x: f64,
    to_y: f64,
    days: f64,
    mode: &str,
) -> Result<()> {
    let def = load(project)?;
    let geo = geology_for(project, &def)?;
    if !crate::world::travel::mode_recognized(mode) {
        println!("  ⚠ unrecognized mode `{mode}` — assessing at foot pace (30 km/day)");
    }
    // Named places (accepted Places / world links) resolve to coordinates;
    // otherwise the explicit --from-x/--to-x coordinates are used.
    let (from_x, from_y) = match &from {
        Some(n) => resolve_place(project, n)?,
        None => (from_x, from_y),
    };
    let (to_x, to_y) = match &to {
        Some(n) => resolve_place(project, n)?,
        None => (to_x, to_y),
    };
    let cells = ((to_x - from_x).powi(2) + (to_y - from_y).powi(2)).sqrt();
    let dist = crate::world::travel::distance_km(
        def.astronomy.planet.radius_earth,
        geo.width,
        geo.height,
        to_x - from_x,
        to_y - from_y,
    );
    let a = crate::world::travel::assess(dist, days, mode);

    if from.is_some() || to.is_some() {
        println!("travel · {} → {}", from.as_deref().unwrap_or("start"), to.as_deref().unwrap_or("end"));
    }
    println!(
        "travel · {} · {:.0} km ({:.1} cells) by {}",
        def.name, a.distance_km, cells, a.mode
    );
    println!(
        "  claimed {:.1} day(s) · needs ~{:.1} at {:.0} km/day",
        a.claimed_days,
        a.needed_days,
        crate::world::travel::speed_km_per_day(mode)
    );
    if a.plausible {
        println!("  ✓ plausible");
    } else {
        println!("  ⚠ too fast — the straight-line journey needs ~{:.1} day(s)", a.needed_days);
        if let Some(m) = &def.magic {
            let ctx = crate::world::types::CheckContext { category: "travel_time", ..Default::default() };
            if m.find_suppressor(&ctx).is_some() {
                println!("    (a magic rule covers travel_time — this may be sanctioned)");
            }
        }
    }
    Ok(())
}

/// WORLD-9 (Culture) — `realworld culture`: one culture per polity (ethos,
/// belief, a conlang typology profile to realise, a naming sample).
fn culture(project: &Path) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, def.seed_u64());
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|p| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == p.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cul = compile_culture(&pol, &capital_biomes, &def.cultures, def.seed_u64());

    use crate::world::compile::culture_layer::elaborate_role;
    println!("culture · {} — {} culture(s)", def.name, cul.cultures.len());
    for (i, c) in cul.cultures.iter().enumerate() {
        println!("\n  {} — {}", c.polity, c.ethos);
        println!("    belief:   {}", c.belief);
        println!("    language: {}  (realise with `inkhaven language`)", c.language_profile);
        println!("    naming:   e.g. {}", c.naming_sample);
        // WORLD-15 — the world's common social roles, in this realm's own terms.
        let biome = capital_biomes.get(i).map(String::as_str).unwrap_or("");
        let roles: Vec<String> =
            demo.role_archetypes.iter().map(|r| elaborate_role(r, c, biome)).collect();
        if !roles.is_empty() {
            println!("    roles:    {}", roles.join(", "));
        }
    }
    Ok(())
}

/// WORLD-9 (Polities) — `realworld polities`: the nations formed by clustering
/// settlements around their largest capitals, with populations and relations.
fn polities(project: &Path) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_hydrology, compile_polities,
    };
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, def.seed_u64());

    let warnings = crate::world::compile::polities_layer::lint_polities(&def.nations, &demo);
    if !warnings.is_empty() {
        println!("  {} declared-nation warning(s):", warnings.len());
        for w in &warnings {
            println!("    ⚠ {w}");
        }
    }
    let declared_n = def.nations.len();
    println!(
        "polities · {} — {} realm(s){}",
        def.name,
        pol.polities.len(),
        if declared_n > 0 { format!(" ({declared_n} declared)") } else { String::new() }
    );
    for (i, p) in pol.polities.iter().enumerate() {
        println!(
            "\n  [{i}] {} · capital {} at ({}, {})",
            p.name, p.capital, p.capital_pos.0, p.capital_pos.1
        );
        println!("      {} settlement(s) · population {}", p.member_count, fmt_pop(p.population));
    }
    let notable: Vec<&crate::world::compile::polities_layer::Relation> =
        pol.relations.iter().filter(|r| r.stance != "neutral").collect();
    if !notable.is_empty() {
        println!("\n  relations:");
        for r in notable {
            println!("    {} {} {}", pol.polities[r.a].name, r.stance, pol.polities[r.b].name);
        }
    }
    Ok(())
}

/// WORLD (Ecology) — `realworld ecology`: the flora / fauna archetypes + a
/// keystone animal for each land biome, derived from the compiled climate.
fn ecology(project: &Path) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_ecology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let eco = compile_ecology(&climate, def.ecology.as_ref().map(|e| e.regions.as_slice()).unwrap_or(&[]), def.seed_u64());
    println!("ecology · {} — {} land biome(s)", def.name, eco.biomes.len());
    for b in &eco.biomes {
        println!("\n  {} ({:.0}% of land)  · keystone: {}", b.biome, b.area_pct, b.keystone);
        println!("    flora: {}", b.flora.join(", "));
        println!("    fauna: {}", b.fauna.join(", "));
    }
    Ok(())
}

/// WORLD-10 — `realworld weather --day <N> --lat <deg>`: the local season +
/// relative insolation for a day-of-year at a latitude, from the compiled
/// astronomy. So a scene's weather stays consistent with the planet.
fn weather(project: &Path, day: f64, lat: f64) -> Result<()> {
    use crate::world::compile::compile_astronomy;
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let w = crate::world::weather::weather_at(&astro, day, lat);
    println!(
        "weather · {} · day {:.0} of {:.0} · lat {:.0}°",
        def.name, day, astro.year_length_planet_days, lat
    );
    println!("  season:     {}", w.season);
    println!("  conditions: {}", w.descriptor);
    println!(
        "  insolation: {:.2} (relative, at the {:.0}° band)",
        w.insolation, w.lat_band_deg
    );
    Ok(())
}

/// WORLD-8 (W8-P1) — `realworld history [--json]`: derive the world's founding
/// chronology + epochs from the compiled demographics and print it, plus an
/// adoptable Timeline block (the sim proposes; the author enters the events they
/// want via `inkhaven event`). Materialisation into the World book + direct
/// Timeline writes are W8-P2.
fn history(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_hydrology, compile_history,
    };
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let declared = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    let hist = compile_history(&demo, declared, def.seed_u64());

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_history(&store, &cfg, &hist)?)
    } else {
        None
    };

    if json {
        let v = serde_json::json!({
            "span_years": hist.span_years,
            "epochs": hist.epochs.iter().map(|e| serde_json::json!({
                "name": e.name, "start_year": e.start_year, "end_year": e.end_year, "note": e.note,
            })).collect::<Vec<_>>(),
            "foundings": hist.foundings.iter().map(|f| serde_json::json!({
                "year": f.year, "label": f.label, "class": f.class, "population": f.population,
            })).collect::<Vec<_>>(),
            "events": hist.events.iter().map(|e| serde_json::json!({
                "year": e.year, "kind": e.kind, "description": e.description,
            })).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(());
    }

    println!("history · {} — {} years of recorded past", def.name, hist.span_years);
    for e in &hist.epochs {
        println!("\n  {} ({}…{})", e.name, e.start_year, e.end_year);
        println!("    {}", e.note);
        for ev in hist.events.iter().filter(|v| v.year >= e.start_year && v.year < e.end_year) {
            println!("    · year {:>5}  {}", ev.year, ev.description);
        }
        for f in hist.foundings.iter().filter(|f| f.year >= e.start_year && f.year < e.end_year) {
            println!("    · year {:>5}  {} founded  (pop {})", f.year, f.label, fmt_pop(f.population));
        }
    }
    if let Some(r) = &mat_report {
        println!(
            "\n  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    // W11-P1 — verify the author's declared events (advisory).
    let warnings = crate::world::compile::history_layer::lint_history(declared, &hist);
    if !warnings.is_empty() {
        println!("\n  {} declared-event warning(s):", warnings.len());
        for w in &warnings {
            println!("    ⚠ {w}");
        }
    }

    println!("\nAdopt into the story Timeline (rename as you like):");
    // Declared events first — those are the ones you meant to keep.
    for d in declared {
        let place = d
            .places
            .as_ref()
            .and_then(|p| p.first())
            .map(|p| format!(" (at {p})"))
            .unwrap_or_default();
        println!("  inkhaven event add --start \"{}\" \"{}{place}\"", d.year, d.title);
    }
    for f in &hist.foundings {
        println!("  inkhaven event add --start \"{}\" \"{} founded\"", f.year, f.label);
    }
    Ok(())
}

/// WORLD-13 — `realworld chronicle`: the world's compiled past as a *state
/// trajectory*. Where `realworld history` lists the events, the chronicle reports
/// how far the world had grown by the close of each epoch — settlements (by
/// class), settled population, and realms standing — alongside that epoch's
/// events. Pure presentation of the history layer; no simulation, no new data.
fn chronicle(project: &Path, json: bool) -> Result<()> {
    use crate::world::compile::history_layer::state_at;
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_history, compile_hydrology,
    };
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let declared = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    let hist = compile_history(&demo, declared, def.seed_u64());

    if json {
        let v = serde_json::json!({
            "span_years": hist.span_years,
            "epochs": hist.epochs.iter().map(|e| {
                // State by the last year still inside the epoch.
                let st = state_at(&hist, e.end_year - 1);
                serde_json::json!({
                    "name": e.name, "start_year": e.start_year, "end_year": e.end_year, "note": e.note,
                    "by_close": {
                        "settlements": st.settlements, "cities": st.cities, "towns": st.towns,
                        "villages": st.villages, "settled_population": st.settled_population,
                        "realms_active": st.realms_active(), "realms_risen": st.realms_risen,
                        "realms_fallen": st.realms_fallen,
                    },
                    "events": hist.events.iter()
                        .filter(|v| v.year >= e.start_year && v.year < e.end_year)
                        .map(|v| serde_json::json!({ "year": v.year, "kind": v.kind, "description": v.description }))
                        .collect::<Vec<_>>(),
                })
            }).collect::<Vec<_>>(),
        });
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(());
    }

    println!(
        "chronicle · {} — {} epoch(s) across {} years of recorded past",
        def.name,
        hist.epochs.len(),
        hist.span_years
    );
    for e in &hist.epochs {
        let st = state_at(&hist, e.end_year - 1);
        println!("\n  {} ({}…{})", e.name, e.start_year, e.end_year);
        println!("    {}", e.note);
        let mut classes: Vec<String> = Vec::new();
        if st.cities > 0 {
            classes.push(format!("{} cities", st.cities));
        }
        if st.towns > 0 {
            classes.push(format!("{} towns", st.towns));
        }
        if st.villages > 0 {
            classes.push(format!("{} villages", st.villages));
        }
        let breakdown = if classes.is_empty() { "no settlements yet".into() } else { classes.join(", ") };
        println!(
            "    by its close: {} settlement(s) ({}) · ~{} settled · {} realm(s) standing",
            st.settlements,
            breakdown,
            fmt_pop(st.settled_population),
            st.realms_active(),
        );
        let events: Vec<&crate::world::compile::history_layer::HistEvent> =
            hist.events.iter().filter(|v| v.year >= e.start_year && v.year < e.end_year).collect();
        if events.is_empty() {
            println!("    (a quiet age — no recorded upheavals)");
        } else {
            for ev in events {
                println!("    · year {:>5}  [{}] {}", ev.year, ev.kind, ev.description);
            }
        }
    }
    println!("\nThe events are adoptable onto the story Timeline with `realworld history`.");
    Ok(())
}

/// WORLD-14 — `realworld name`: propose a name for each settlement in its realm's
/// phonic style, so a realm's towns share a family sound rather than the generic
/// placeholder names. Deterministic; a naming aid the author adopts on accept.
fn name(project: &Path, json: bool) -> Result<()> {
    use crate::world::compile::culture_layer::culture_style_name;
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    use crate::world::proposals::settlement_name;

    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cul = compile_culture(&pol, &capital_biomes, &def.cultures, seed);

    // Each settlement → its nearest realm (anisotropic grid weighting), indexed
    // within that realm so the culture-style names don't collide.
    let mut per_realm: Vec<usize> = vec![0; pol.polities.len()];
    let mut rows: Vec<(String, String, String)> = Vec::new(); // (generic, styled, realm)
    for s in &demo.settlements {
        let realm = pol.polities.iter().enumerate().min_by_key(|(_, q)| {
            let dx = q.capital_pos.0 as i64 - s.x as i64;
            let dy = q.capital_pos.1 as i64 - s.y as i64;
            9 * dx * dx + 4 * dy * dy
        });
        let generic = settlement_name(seed, s.x, s.y);
        match realm {
            Some((i, p)) => {
                let idx = per_realm[i];
                per_realm[i] += 1;
                let styled = cul
                    .cultures
                    .get(i)
                    .map(|c| culture_style_name(c, seed, idx))
                    .unwrap_or_else(|| generic.clone());
                rows.push((generic, styled, p.name.clone()));
            }
            None => rows.push((generic.clone(), generic, "—".into())),
        }
    }

    if json {
        let v = serde_json::json!(rows
            .iter()
            .map(|(g, s, r)| serde_json::json!({ "placeholder": g, "name": s, "realm": r }))
            .collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(());
    }

    println!("names · {} — {} settlement(s) in their realm's style", def.name, rows.len());
    let cap = 60;
    for (g, styled, realm) in rows.iter().take(cap) {
        println!("  {g:<15} → {styled:<15} ({realm})");
    }
    if rows.len() > cap {
        println!("  … and {} more", rows.len() - cap);
    }
    println!("\nThese are proposals in the world's style — adopt one when you accept its Place,");
    println!("or realise a realm's tongue in full with `realworld propose-language` + `inkhaven language`.");
    Ok(())
}

/// WORLD-15 — `realworld trade`: the trade network between realms. Each realm
/// links to its nearest non-rival neighbours; the route is a land road or a sea
/// lane by the two capitals' coasts. Connectivity, not simulated economics.
fn trade(project: &Path, json: bool) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_hydrology, compile_polities,
        compile_trade,
    };
    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let t = compile_trade(&pol, &geo, def.astronomy.planet.radius_earth);
    let realm = |i: usize| pol.polities.get(i).map(|p| p.name.as_str()).unwrap_or("?");

    if json {
        let v = serde_json::json!(t
            .routes
            .iter()
            .map(|r| serde_json::json!({
                "from": realm(r.from), "to": realm(r.to), "mode": r.mode,
                "stance": r.stance, "distance_km": r.distance_km.round() as i64,
            }))
            .collect::<Vec<_>>());
        println!("{}", serde_json::to_string_pretty(&v).unwrap_or_default());
        return Ok(());
    }

    println!("trade · {} — {} route(s) between {} realm(s)", def.name, t.routes.len(), pol.polities.len());
    if t.routes.is_empty() {
        println!("  (no routes — a lone realm, or every pair is a rival)");
        return Ok(());
    }
    for r in &t.routes {
        let via = if r.mode == "sea" { "by sea" } else { "overland" };
        println!(
            "  {:<14} ⇄ {:<14} · {via} · {:.0} km ({})",
            realm(r.from),
            realm(r.to),
            r.distance_km,
            r.stance
        );
    }
    println!("\nRivals never trade; each realm links to its nearest few non-rivals. Drawn on the map by `realworld map`.");
    Ok(())
}

/// WORLD-7 (W7-P4) — `realworld gazetteer [--output PATH]`: a consolidated,
/// Markdown world reference (calendar, sky, regions, landmarks, waters,
/// settlements, economy, magic) from the definition + compiled layers. Print to
/// stdout, or write it beside the manuscript as an appendix source.
fn gazetteer(project: &Path, output: Option<&str>) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    use std::fmt::Write;
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    let mut md = String::new();
    let _ = writeln!(md, "# {} — World Gazetteer\n", def.name);
    let _ = writeln!(md, "_Seed {:#x} · primary language {}_", def.seed_u64(), def.primary_language);

    let cal = &def.astronomy.calendar;
    let _ = writeln!(md, "\n## Calendar\n");
    let _ = writeln!(
        md,
        "{} months × {} days = {}-day year.",
        cal.months,
        cal.month_length_days,
        cal.months.saturating_mul(cal.month_length_days)
    );
    if !cal.month_names.is_empty() {
        let _ = writeln!(md, "\nMonths: {}.", cal.month_names.join(", "));
    }
    let mut seasons = astro.seasons.clone();
    seasons.sort_by(|a, b| a.year_fraction.partial_cmp(&b.year_fraction).unwrap_or(std::cmp::Ordering::Equal));
    if !seasons.is_empty() {
        let s = seasons.iter().map(|m| m.name.replace('_', " ")).collect::<Vec<_>>().join(" · ");
        let _ = writeln!(md, "\nSeasons: {s}.");
    }

    let _ = writeln!(md, "\n## Sky\n");
    let _ = writeln!(
        md,
        "- Star {} (luminosity {} L☉, {:.2} M☉)",
        def.astronomy.star.class, def.astronomy.star.luminosity_solar, astro.stellar_mass_solar
    );
    let _ = writeln!(
        md,
        "- Year {:.1} planet-days · axial tilt {:.1}°",
        astro.year_length_planet_days, astro.axial_tilt_deg
    );
    for m in &astro.moons {
        let _ = writeln!(md, "- Moon {} · synodic {:.1} planet-days", m.name, m.synodic_period_planet_days);
    }

    if let Some(g) = &def.geography {
        if !g.regions.is_empty() {
            let _ = writeln!(md, "\n## Regions\n");
            for r in &g.regions {
                let _ = write!(md, "- **{}**", r.name);
                let facets: Vec<&str> = [r.biome.as_str(), r.climate.as_str()]
                    .into_iter()
                    .filter(|s| !s.is_empty())
                    .collect();
                if !facets.is_empty() {
                    let _ = write!(md, " · {}", facets.join(", "));
                }
                if !r.description.is_empty() {
                    let _ = write!(md, " — {}", r.description);
                }
                let _ = writeln!(md);
            }
        }
        if !g.landmarks.is_empty() {
            let _ = writeln!(md, "\n## Landmarks\n");
            for l in &g.landmarks {
                let _ = write!(md, "- **{}**", l.name);
                if !l.kind.is_empty() {
                    let _ = write!(md, " ({})", l.kind);
                }
                if l.population > 0 {
                    let _ = write!(md, " · pop {}", fmt_pop(l.population));
                }
                if !l.description.is_empty() {
                    let _ = write!(md, " — {}", l.description);
                }
                let _ = writeln!(md);
            }
        }
    }

    let _ = writeln!(md, "\n## Waters\n");
    let _ = writeln!(
        md,
        "Procedural: {} river(s), {} lake(s), {} watershed(s).",
        hydro.river_count, hydro.lake_count, hydro.watershed_count
    );
    if let Some(h) = &def.hydrology {
        if !h.rainfall.is_empty() {
            let _ = writeln!(md, "\nRainfall: {}.", h.rainfall);
        }
        for (label, list) in [("Rivers", &h.rivers), ("Lakes", &h.lakes), ("Seas", &h.seas)] {
            if !list.is_empty() {
                let _ = writeln!(md, "\n**{label}**\n");
                for w in list {
                    let _ = write!(md, "- {}", w.name);
                    if !w.description.is_empty() {
                        let _ = write!(md, " — {}", w.description);
                    }
                    let _ = writeln!(md);
                }
            }
        }
    }

    if !demo.settlements.is_empty() {
        let _ = writeln!(md, "\n## Settlements\n");
        let _ = writeln!(
            md,
            "Population {} across {} settlement(s).\n",
            fmt_pop(demo.total_population),
            demo.settlements.len()
        );
        for s in demo.settlements.iter().take(12) {
            let _ = writeln!(
                md,
                "- {} at ({}, {}) · pop {} · {}",
                s.class, s.x, s.y, fmt_pop(s.population), s.biome
            );
        }
    }

    if let Some(e) = &def.economy {
        let _ = writeln!(md, "\n## Economy\n");
        if !e.tech_level.is_empty() {
            let _ = writeln!(md, "- Tech level: {}", e.tech_level);
        }
        if !e.currency.is_empty() {
            let _ = writeln!(md, "- Currency: {}", e.currency);
        }
        if !e.trade_goods.is_empty() {
            let _ = writeln!(md, "- Trade goods: {}", e.trade_goods.join(", "));
        }
        if !e.resources.is_empty() {
            let _ = writeln!(md, "- Resources: {}", e.resources.join(", "));
        }
    }

    if let Some(m) = &def.magic {
        if !m.rules.is_empty() {
            let _ = writeln!(md, "\n## Magic\n");
            let _ = writeln!(md, "_{}_\n", if m.enabled { "enabled" } else { "disabled" });
            for r in &m.rules {
                let _ = write!(md, "- **{}**", r.kind);
                if !r.covers.is_empty() {
                    let _ = write!(md, " (covers {})", r.covers.join(", "));
                }
                if !r.description.is_empty() {
                    let _ = write!(md, " — {}", r.description);
                }
                let _ = writeln!(md);
            }
        }
    }

    match output {
        Some(path) => {
            crate::io_atomic::write(std::path::Path::new(path), md.as_bytes())
                .map_err(|e| Error::Store(format!("writing {path}: {e}")))?;
            println!("gazetteer · {} → {} ({} lines)", def.name, path, md.lines().count());
        }
        None => print!("{md}"),
    }
    Ok(())
}

/// WORLD-14 — coordinate-bearing declared landmarks (`geography.landmarks` given
/// a `lat`/`lon` or `x`/`y`), projected to grid cells for the plakat map.
fn declared_map_landmarks(
    def: &WorldDefinition,
    w: usize,
    h: usize,
) -> Vec<crate::world::plakat::DeclaredLandmark> {
    def.geography
        .as_ref()
        .map(|g| {
            g.landmarks
                .iter()
                .filter_map(|lm| {
                    lm.grid(w, h).map(|(x, y)| crate::world::plakat::DeclaredLandmark {
                        name: lm.name.clone(),
                        kind: lm.kind.clone(),
                        x,
                        y,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Render the world map with plakat. Compiles every layer, emits a MapSpec from
/// the geology/climate/hydrology/demographics outputs, hands it to `plakat map`,
/// and reads the resolved landmark positions back to refine Place coordinates.
fn map(project: &Path, spec_only: bool, no_ingest: bool) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_hydrology, compile_polities,
        compile_trade,
    };
    use crate::world::plakat;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    // Accepted Places give landmarks stable ids so we can ingest resolved
    // positions back onto the right cross-reference. An absent store is fine.
    let store = WorldStore::open_for_project(project).ok();
    let links = store.as_ref().and_then(|s| s.list_place_links().ok()).unwrap_or_default();
    let declared = declared_map_landmarks(&def, geo.width, geo.height);
    let pol = compile_polities(&demo, &def.nations, def.seed_u64());
    let trade = compile_trade(&pol, &geo, def.astronomy.planet.radius_earth);

    let spec = plakat::build_map_spec(&def.name, &geo, &climate, &hydro, &demo, &links, &declared, &pol, &trade);
    let (gw, gh) = (geo.width, geo.height);

    if spec_only {
        let dir = plakat::maps_dir(project);
        std::fs::create_dir_all(&dir).map_err(|e| Error::Store(format!("creating {}: {e}", dir.display())))?;
        let path = dir.join("world.mapspec.json");
        let body = serde_json::to_string_pretty(&spec)
            .map_err(|e| Error::Store(format!("serializing spec: {e}")))?;
        crate::io_atomic::write(&path, body.as_bytes())
            .map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
        println!("map · {} ({}×{} grid)", def.name, gw, gh);
        println!(
            "  spec: {} ranges, {} rivers, {} regions, {} landmarks",
            spec["terrain"]["mountain_ranges"].as_array().map(|a| a.len()).unwrap_or(0),
            spec["water"]["rivers"].as_array().map(|a| a.len()).unwrap_or(0),
            spec["regions"].as_array().map(|a| a.len()).unwrap_or(0),
            spec["landmarks"].as_array().map(|a| a.len()).unwrap_or(0),
        );
        println!("  → {} (--spec-only; not rendered)", path.display());
        return Ok(());
    }

    if let Some(v) = plakat::detect() {
        println!("map · {} ({}×{} grid) · {v}", def.name, gw, gh);
    }
    let art = plakat::render(project, &spec, def.seed_u64(), gw, gh)
        .map_err(|e| Error::Config(format!("rendering map: {e}")))?;

    println!("  spec:     {}", art.spec_path.display());
    println!("  features: {}", art.png_path.display());
    println!("  geojson:  {}", art.geojson_path.display());

    // Ingest: refine each accepted Place's coordinates from the landmark plakat
    // resolved for it.
    let mut updated = 0usize;
    if !no_ingest {
        if let Some(s) = store.as_ref() {
            for lm in &art.landmarks {
                if let Some(pid) = lm.place_id() {
                    if s.update_place_link_coords(pid, lm.x, lm.y).is_ok() {
                        updated += 1;
                    }
                }
            }
        }
    }
    println!(
        "  {} landmark(s) resolved{}",
        art.landmarks.len(),
        if no_ingest { String::new() } else { format!(", {updated} Place coordinate(s) refined") }
    );
    Ok(())
}

/// Run the compiler through demographics and seed the proposal queue.
fn propose(project: &Path) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    use crate::world::proposals::place_proposals;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let resolved = store
        .resolved_signatures()
        .map_err(|e| Error::Store(format!("reading proposals: {e}")))?;
    store.clear_pending_kinds("place").map_err(|e| Error::Store(format!("clearing proposals: {e}")))?;

    let proposals = place_proposals(&demo, def.seed_u64());
    let (mut added, mut skipped) = (0usize, 0usize);
    for p in &proposals {
        if resolved.contains(&p.signature) {
            skipped += 1; // already accepted or rejected — don't re-propose
            continue;
        }
        store.insert(p).map_err(|e| Error::Store(format!("inserting proposal: {e}")))?;
        added += 1;
    }
    println!(
        "proposed {added} Place(s) into the queue ({skipped} already resolved, skipped)"
    );
    println!("review with `inkhaven realworld proposals list`");
    Ok(())
}

/// WORLD-12 — compile the culture layer and propose one Mythology entry per
/// distinct belief into the same proposal queue. Accepting commits a
/// `para:myth-*` paragraph into the Mythology book.
fn propose_myth(project: &Path) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    use crate::world::myth_proposals::myth_proposals;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cul = compile_culture(&pol, &capital_biomes, &def.cultures, seed);

    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let resolved = store
        .resolved_signatures()
        .map_err(|e| Error::Store(format!("reading proposals: {e}")))?;
    store
        .clear_pending_kinds("myth-%")
        .map_err(|e| Error::Store(format!("clearing proposals: {e}")))?;

    let proposals = myth_proposals(&cul, seed);
    let (mut added, mut skipped) = (0usize, 0usize);
    for p in &proposals {
        if resolved.contains(&p.signature) {
            skipped += 1; // already accepted or rejected — don't re-propose
            continue;
        }
        store.insert(p).map_err(|e| Error::Store(format!("inserting proposal: {e}")))?;
        added += 1;
    }
    if proposals.is_empty() {
        println!("no cultures with beliefs — compile a peopled world first (see `realworld compile`)");
        return Ok(());
    }
    println!(
        "proposed {added} Mythology entr{} into the queue ({skipped} already resolved, skipped)",
        if added == 1 { "y" } else { "ies" }
    );
    println!("review with `inkhaven realworld proposals list`, then accept into the Mythology book");
    Ok(())
}

/// WORLD-12 — compile the polities + cultures and propose one ruler Character per
/// realm into the same proposal queue. Accepting commits a Character stub.
fn propose_rulers(project: &Path) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    use crate::world::ruler_proposals::ruler_proposals;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cultures = compile_culture(&pol, &capital_biomes, &def.cultures, seed);

    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let resolved = store
        .resolved_signatures()
        .map_err(|e| Error::Store(format!("reading proposals: {e}")))?;
    store
        .clear_pending_kinds("character")
        .map_err(|e| Error::Store(format!("clearing proposals: {e}")))?;

    let proposals = ruler_proposals(&pol, &cultures, seed);
    let (mut added, mut skipped) = (0usize, 0usize);
    for p in &proposals {
        if resolved.contains(&p.signature) {
            skipped += 1;
            continue;
        }
        store.insert(p).map_err(|e| Error::Store(format!("inserting proposal: {e}")))?;
        added += 1;
    }
    if proposals.is_empty() {
        println!("no realms yet — compile a peopled world first (see `realworld polities`)");
        return Ok(());
    }
    println!(
        "proposed {added} ruler(s) into the queue ({skipped} already resolved, skipped)"
    );
    println!("review with `inkhaven realworld proposals list`, then accept into the Characters book");
    Ok(())
}

/// WORLD-13 — compile the polities + cultures and propose one language per realm
/// (from the culture's language profile) into the same proposal queue. Accepting
/// scaffolds a language book in the ConLang suite seeded with the world's brief.
fn propose_language(project: &Path) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };
    use crate::world::language_proposals::language_proposals;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cultures = compile_culture(&pol, &capital_biomes, &def.cultures, seed);

    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let resolved = store
        .resolved_signatures()
        .map_err(|e| Error::Store(format!("reading proposals: {e}")))?;
    store
        .clear_pending_kinds("language")
        .map_err(|e| Error::Store(format!("clearing proposals: {e}")))?;

    let proposals = language_proposals(&pol, &cultures, seed);
    let (mut added, mut skipped) = (0usize, 0usize);
    for p in &proposals {
        if resolved.contains(&p.signature) {
            skipped += 1;
            continue;
        }
        store.insert(p).map_err(|e| Error::Store(format!("inserting proposal: {e}")))?;
        added += 1;
    }
    if proposals.is_empty() {
        println!("no cultures with a language profile — compile a peopled world first (see `realworld culture`)");
        return Ok(());
    }
    println!(
        "proposed {added} language(s) into the queue ({skipped} already resolved, skipped)"
    );
    println!("review with `inkhaven realworld proposals list`, then accept to scaffold in the ConLang suite");
    Ok(())
}

fn proposals(project: &Path, cmd: ProposalsCommand) -> Result<()> {
    use crate::world::storage::WorldStore;
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    match cmd {
        ProposalsCommand::List { status } => {
            let list = store
                .list(status.as_deref())
                .map_err(|e| Error::Store(format!("listing proposals: {e}")))?;
            if list.is_empty() {
                println!("(no proposals{})", status.map(|s| format!(" with status {s}")).unwrap_or_default());
                return Ok(());
            }
            for p in &list {
                println!("{} [{}] {} — {}", &p.id.to_string()[..8], p.status, p.name, p.rationale);
            }
            println!("\n{} proposal(s). Accept with `realworld proposals accept <id>`.", list.len());
            Ok(())
        }
        ProposalsCommand::Accept { id } => {
            let uuid = parse_id(&store, &id)?;
            accept_one(project, &store, uuid)?;
            Ok(())
        }
        ProposalsCommand::Reject { id } => {
            let uuid = parse_id(&store, &id)?;
            store.set_status(uuid, "rejected").map_err(|e| Error::Store(format!("reject: {e}")))?;
            println!("rejected {id}");
            Ok(())
        }
        ProposalsCommand::AcceptAll => {
            let pending = store
                .list(Some("pending"))
                .map_err(|e| Error::Store(format!("listing: {e}")))?;
            let mut n = 0;
            for p in &pending {
                accept_one(project, &store, p.id)?;
                n += 1;
            }
            println!("accepted {n} proposal(s)");
            Ok(())
        }
        ProposalsCommand::Clear => {
            store.clear_pending().map_err(|e| Error::Store(format!("clear: {e}")))?;
            println!("cleared pending proposals");
            Ok(())
        }
    }
}

/// Resolve a possibly-abbreviated proposal id (first 8 chars) to a full UUID.
fn parse_id(store: &crate::world::storage::WorldStore, id: &str) -> Result<uuid::Uuid> {
    if let Ok(u) = uuid::Uuid::parse_str(id) {
        return Ok(u);
    }
    let list = store.list(None).map_err(|e| Error::Store(format!("listing: {e}")))?;
    list.iter()
        .find(|p| p.id.to_string().starts_with(id))
        .map(|p| p.id)
        .ok_or_else(|| Error::Config(format!("no proposal matching id `{id}`")))
}

/// Accept a proposal: commit its record (Place or Mythology entry) and mark it
/// accepted. The `kind` selects the target system book.
fn accept_one(
    project: &Path,
    _store: &crate::world::storage::WorldStore,
    id: uuid::Uuid,
) -> Result<()> {
    // The shared committer (used by the TUI too) dispatches on the proposal's
    // kind — Place, Mythology, Character, or language — and flips its status.
    let (label, name) = crate::world::commit::accept_by_id(project, id)?;
    println!("accepted {name} → {label}");
    Ok(())
}

/// Load + parse the project's `world.hjson`.
fn load(project: &Path) -> Result<WorldDefinition> {
    let path = project.join(WORLD_FILE);
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        Error::Config(format!(
            "reading {}: {e} — run `inkhaven realworld new <name>` to scaffold one",
            path.display()
        ))
    })?;
    WorldDefinition::from_hjson(&raw)
        .map_err(|e| Error::Config(format!("{}: {e}", path.display())))
}

/// Compile the geology layer, using the external DEM if the definition declares
/// one (its `path` is resolved relative to the project root) — else generating
/// it from the seed.
fn geology_for(
    project: &Path,
    def: &WorldDefinition,
) -> Result<crate::world::types::GeologyOutput> {
    use crate::world::compile::{compile_geology, compile_geology_dem};
    if let Some(dem) = def.geology.as_ref().and_then(|g| g.dem.as_ref()) {
        let path = project.join(&dem.path);
        compile_geology_dem(def, &path).map_err(Error::Config)
    } else {
        Ok(compile_geology(def))
    }
}

fn new(project: &Path, name: &str, force: bool) -> Result<()> {
    let path = project.join(WORLD_FILE);
    if path.exists() && !force {
        return Err(Error::Config(format!(
            "{} already exists — pass --force to overwrite",
            path.display()
        )));
    }
    let body = starter_template(name);
    crate::io_atomic::write(&path, body.as_bytes())
        .map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
    println!("scaffolded {} for world `{name}`", path.display());
    println!("edit it, then `inkhaven realworld compile`");
    Ok(())
}

/// WORLD (1.6.1) — propose N candidate worlds from consecutive seeds. Each row is
/// a one-line summary + the seed that grows it, so the author can pick one.
fn variants(project: &Path, count: usize) -> Result<()> {
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_demographics, compile_hydrology,
        compile_polities,
    };
    let def = load(project)?;
    let base = def.seed_u64();
    let n = count.clamp(1, 24);
    let dem = def.geology.as_ref().and_then(|g| g.dem.as_ref()).is_some();

    println!("variants · {n} candidate world(s) from seed {base}");
    if dem {
        println!("  (geology is DEM-sourced: the terrain is fixed; settlements and realms vary by seed)");
    }
    for i in 0..n as u64 {
        let seed = base.wrapping_add(i);
        let mut d = def.clone();
        d.seed = crate::world::types::SeedValue::Int(seed as i64);
        let astro = compile_astronomy(&d.astronomy);
        let geo = geology_for(project, &d)?;
        let climate = compile_climate(&d, &astro, &geo);
        let hydro = compile_hydrology(&geo, &climate);
        let demo = compile_demographics(&climate, &hydro);
        let pol = compile_polities(&demo, &d.nations, seed);
        let top_biome = climate
            .zones
            .iter()
            .filter(|z| z.biome != "ocean")
            .max_by(|a, b| a.area_pct.partial_cmp(&b.area_pct).unwrap_or(std::cmp::Ordering::Equal))
            .map(|z| z.biome.as_str())
            .unwrap_or("—");
        let top_realm = pol
            .polities
            .iter()
            .max_by_key(|p| p.population)
            .map(|p| format!(" · top realm {} (pop {})", p.name, fmt_pop(p.population)))
            .unwrap_or_default();
        println!("\n  seed {seed}{}", if i == 0 { "  (current)" } else { "" });
        println!(
            "    {} continent(s) · {:.0}% sea · {} settlement(s) · pop {}",
            geo.continents,
            geo.sea_coverage_pct,
            demo.settlements.len(),
            fmt_pop(demo.total_population)
        );
        println!("    dominant biome: {top_biome}{top_realm}");
    }
    println!("\n  To adopt one, set `seed: <value>` in world.hjson and run `realworld compile`.");
    Ok(())
}

/// WORLD-12 — the AI world-critique pass. Compile the world, print the free
/// deterministic lints, then (unless `--lints-only`) ask an LLM to critique the
/// world's consistency and realism, printing each finding and optionally filing
/// it as a Notes-book recommendation. Advisory: it never edits `world.hjson`.
fn critique(
    project: &Path,
    max_cost: Option<usize>,
    force: bool,
    write_notes: bool,
    lints_only: bool,
) -> Result<()> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::world::compile::{
        compile_astronomy, compile_climate, compile_culture, compile_demographics, compile_hydrology,
        compile_polities,
    };

    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let def = load(project)?;
    let seed = def.seed_u64();
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);
    let pol = compile_polities(&demo, &def.nations, seed);
    let capital_biomes: Vec<String> = pol
        .polities
        .iter()
        .map(|q| {
            demo.settlements
                .iter()
                .find(|s| (s.x, s.y) == q.capital_pos)
                .map(|s| s.biome.clone())
                .unwrap_or_default()
        })
        .collect();
    let cultures = compile_culture(&pol, &capital_biomes, &def.cultures, seed);

    println!("critique · {} (seed {:#x})", def.name, seed);

    // (1) The free deterministic lints — the baseline, always shown.
    let lints = collect_world_lints(&def, &geo, &climate, &demo, seed);
    if lints.is_empty() {
        println!("  lints:    ok · no deterministic issues");
    } else {
        println!("  lints:    {} advisory warning(s):", lints.len());
        for l in &lints {
            println!("    ⚠ {l}");
        }
    }

    if lints_only || !cfg.world.critique_enabled {
        if !cfg.world.critique_enabled && !lints_only {
            println!("  (AI critique disabled via `world.critique_enabled = false`)");
        }
        return Ok(());
    }

    // (2) The AI pass — a compact summary of declared + compiled, one capped call.
    let summary = compiled_summary(&def, &astro, &geo, &climate, &hydro, &demo, &pol, &cultures);
    let hjson = std::fs::read_to_string(project.join(WORLD_FILE)).unwrap_or_default();
    let lang = {
        use crate::prose::{resolve_prose_language, ProseLanguage};
        let (l, _) = resolve_prose_language(None, &cfg.language);
        match l {
            ProseLanguage::En | ProseLanguage::Other(_) => "English",
            ProseLanguage::Ru => "Russian",
            ProseLanguage::De => "German",
            ProseLanguage::Fr => "French",
            ProseLanguage::Es => "Spanish",
        }
        .to_string()
    };
    let system = crate::world::critique::critique_system(&lang);
    let prompt = crate::world::critique::build_critique_prompt(&hjson, &summary);
    let soft_cap = max_cost.unwrap_or(cfg.world.critique_max_tokens);

    let raw = world_llm_text(project, "world critique", &system, prompt, soft_cap, force)?;
    let items = crate::world::critique::parse_critique(&raw);

    if items.is_empty() {
        println!("  critique: the world reads as sound ✓");
        return Ok(());
    }
    println!("\n  critique: {} recommendation(s)\n", items.len());
    for it in &items {
        let sev = match it.severity_rank() {
            0 => "high",
            2 => "low",
            _ => "med",
        };
        println!("  • [{sev}] {} — {}", it.aspect.trim(), it.issue.trim());
        println!("      → {}", it.recommendation.trim());
    }

    if write_notes {
        let n = write_critique_notes(project, &def.name, &items)?;
        println!("\n  wrote {n} recommendation(s) into the Notes book");
    } else {
        println!("\n  (re-run with `--write-notes` to file these into the Notes book)");
    }
    Ok(())
}

/// Gather every advisory deterministic lint into aspect-tagged lines (the same
/// checks `realworld validate` prints, collected for the critique baseline).
fn collect_world_lints(
    def: &WorldDefinition,
    geo: &crate::world::types::GeologyOutput,
    climate: &crate::world::types::ClimateOutput,
    demo: &crate::world::types::DemographicsOutput,
    seed: u64,
) -> Vec<String> {
    use crate::world::compile::compile_polities;
    let mut out = Vec::new();
    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = crate::world::compile::compile_history(demo, declared_hist, seed);
        for w in crate::world::compile::history_layer::lint_history(declared_hist, &hist) {
            out.push(format!("history: {w}"));
        }
    }
    if !def.nations.is_empty() {
        for w in crate::world::compile::polities_layer::lint_polities(&def.nations, demo) {
            out.push(format!("nations: {w}"));
        }
    }
    if let Some(hy) = def.hydrology.as_ref() {
        if hy.rivers.iter().any(|r| r.from.is_some() && r.to.is_some()) {
            for w in crate::world::compile::hydrology_layer::lint_rivers(hy, geo) {
                out.push(format!("rivers: {w}"));
            }
        }
    }
    if !def.cultures.is_empty() {
        let pol = compile_polities(demo, &def.nations, seed);
        let capital_biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                demo.settlements
                    .iter()
                    .find(|s| (s.x, s.y) == q.capital_pos)
                    .map(|s| s.biome.clone())
                    .unwrap_or_default()
            })
            .collect();
        for w in crate::world::compile::culture_layer::lint_culture(&def.cultures, &pol, &capital_biomes) {
            out.push(format!("culture: {w}"));
        }
    }
    if let Some(eco) = def.ecology.as_ref().filter(|e| !e.regions.is_empty()) {
        for w in crate::world::compile::ecology_layer::lint_ecology(&eco.regions, climate) {
            out.push(format!("ecology: {w}"));
        }
    }
    if let Some(m) = def.magic.as_ref() {
        for w in m.lint() {
            out.push(format!("magic: {w}"));
        }
    }
    out
}

/// A compact declared+compiled summary for the critique prompt.
#[allow(clippy::too_many_arguments)]
fn compiled_summary(
    def: &WorldDefinition,
    astro: &crate::world::types::AstronomyOutput,
    geo: &crate::world::types::GeologyOutput,
    climate: &crate::world::types::ClimateOutput,
    hydro: &crate::world::types::HydrologyOutput,
    demo: &crate::world::types::DemographicsOutput,
    pol: &crate::world::compile::polities_layer::PolitiesOutput,
    cultures: &crate::world::compile::culture_layer::CultureOutput,
) -> String {
    use std::fmt::Write;
    let mut s = String::new();
    let st = &def.astronomy.star;
    let pl = &def.astronomy.planet;
    let _ = writeln!(
        s,
        "Sky: star class {} ({} L☉, {:.2} M☉); planet {:.2} M⊕, {:.2} R⊕, axial tilt {:.1}°, day {:.1} h; year {:.0} planet-days; {} moon(s).",
        st.class, st.luminosity_solar, astro.stellar_mass_solar,
        pl.mass_earth, pl.radius_earth, astro.axial_tilt_deg, pl.day_length_hours,
        astro.year_length_planet_days, astro.moons.len(),
    );
    let _ = writeln!(
        s,
        "Land: {} continent(s), sea covers {:.0}% of the surface.",
        geo.continents, geo.sea_coverage_pct
    );
    let mut biomes: Vec<&crate::world::types::ClimateZone> =
        climate.zones.iter().filter(|z| z.biome != "ocean").collect();
    biomes.sort_by(|a, b| b.area_pct.partial_cmp(&a.area_pct).unwrap_or(std::cmp::Ordering::Equal));
    let top: Vec<String> = biomes
        .iter()
        .take(4)
        .map(|z| format!("{} {:.0}%", z.biome.replace('_', " "), z.area_pct))
        .collect();
    let _ = writeln!(
        s,
        "Climate: mean land temp {:.1}°C, precip {:.0} mm/yr; dominant land biomes: {}.",
        climate.mean_land_temp_c, climate.mean_land_precip_mm, top.join(", ")
    );
    let _ = writeln!(s, "Water: {} river(s), {} lake(s).", hydro.river_count, hydro.lake_count);
    let _ = writeln!(
        s,
        "People: total ~{}, {} settlement(s), {} nation(s); common roles: {}.",
        fmt_pop(demo.total_population),
        demo.settlements.len(),
        pol.polities.len(),
        demo.role_archetypes.join(", ")
    );
    for (i, c) in cultures.cultures.iter().take(6).enumerate() {
        let realm = pol.polities.get(i).map(|p| p.name.as_str()).unwrap_or("?");
        let _ = writeln!(s, "  - {realm}: {} · believes in {}.", c.ethos, c.belief);
    }
    match def.magic.as_ref() {
        Some(m) if m.enabled => {
            let _ = writeln!(s, "Magic: enabled, {} declared rule(s).", m.rules.len());
        }
        _ => {
            let _ = writeln!(s, "Magic: none declared (a mundane world).");
        }
    }
    s
}

/// File each critique recommendation as a paragraph in the Notes system book —
/// the `create_place` write path pointed at Notes. Returns the count written.
fn write_critique_notes(
    project: &Path,
    world_name: &str,
    items: &[crate::world::critique::CritiqueItem],
) -> Result<usize> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_NOTES};

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let notes = Hierarchy::load(&store)?
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_NOTES))
        .cloned()
        .ok_or_else(|| Error::Store("Notes system book missing".into()))?;

    let mut written = 0usize;
    for it in items {
        let sev = match it.severity_rank() {
            0 => "high",
            2 => "low",
            _ => "medium",
        };
        let aspect = if it.aspect.trim().is_empty() { "world" } else { it.aspect.trim() };
        let title = format!("World critique — {aspect} ({sev})");
        let body = format!(
            "Recommendation for the world `{world_name}` ({aspect}, {sev} severity).\n\n\
             Issue: {}\n\nRecommendation: {}\n\n\
             // from `inkhaven realworld critique` — advisory; edit world.hjson as you see fit\n",
            it.issue.trim(),
            it.recommendation.trim(),
        );
        let h = Hierarchy::load(&store)?;
        let mut node = store
            .create_node(&cfg, &h, NodeKind::Paragraph, &title, Some(&notes), None, InsertPosition::End)
            .map_err(|e| Error::Store(format!("creating Note: {e}")))?;
        if let Some(rel) = &node.file {
            std::fs::write(store.project_root().join(rel), body.as_bytes())
                .map_err(|e| Error::Store(format!("writing Note: {e}")))?;
        }
        store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| Error::Store(format!("saving Note: {e}")))?;
        written += 1;
    }
    Ok(written)
}

/// A cost-capped one-shot LLM call returning the raw reply text — the fact-check
/// slow track's discipline (daily hard cap informs, per-call soft cap gates
/// unless `--force`, retry-on-transient), reused for the world critique.
fn world_llm_text(
    project: &Path,
    label: &str,
    system: &str,
    prompt: String,
    soft_cap: usize,
    force: bool,
) -> Result<String> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::world::fact_check_slow::{
        backoff_delay, is_transient, slow_preflight, PreflightVerdict,
    };
    use crate::world::storage::WorldStore;

    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("world store: {e}")))?;
    let used = store.llm_calls_today(&day).map_err(|e| Error::Store(format!("{e}")))?;

    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("no LLM provider for the {label}: {e}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("resolving provider: {e}")))?;

    let effective_soft = if force { 0 } else { soft_cap };
    let (pf, verdict) =
        slow_preflight(system, &prompt, used, cfg.cost.world_daily_call_cap, effective_soft);
    match verdict {
        PreflightVerdict::DailyCapReached => {
            eprintln!(
                "{label}: past today's slow-track budget ({}/{} calls) — continuing (the cap informs; see `inkhaven cost`).",
                pf.calls_used, cfg.cost.world_daily_call_cap
            );
        }
        PreflightVerdict::OverSoftCap { est_total_tokens, soft_cap } => {
            return Err(Error::Config(format!(
                "{label} skipped: estimated ~{est_total_tokens} tokens exceeds soft cap {soft_cap} — re-run with --force or raise --max-cost"
            )));
        }
        PreflightVerdict::Proceed => {}
    }
    eprintln!(
        "{label} · model: {model} · ~{} tokens · {}/{} calls today · analyzing…",
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
                let _ = store.record_llm_call(&day);
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

fn validate(project: &Path) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    let def = load(project)?;
    println!(
        "ok — world `{}`, seed {:#x}, primary language `{}`",
        def.name,
        def.seed_u64(),
        def.primary_language
    );
    // WORLD-7 (W7-P4) — validate every layer actually compiles, not just that
    // the definition parses. A broken DEM path or an inconsistent block surfaces
    // here as a compile error rather than at materialize time.
    let astro = compile_astronomy(&def.astronomy);
    println!(
        "  astronomy:    ok · {} moon(s), {}-month calendar",
        def.astronomy.moons.len(),
        def.astronomy.calendar.months
    );
    let geo = geology_for(project, &def)?;
    println!("  geology:      ok · {} plate(s), {} continent(s)", geo.plates.len(), geo.continents);
    if let Some(declared) = def.geology.as_ref().and_then(|g| g.generated.as_ref()) {
        if declared.plates > crate::world::compile::geology_layer::MAX_PLATES {
            println!(
                "                  ⚠ geology.generated.plates {} exceeds the cap {} — clamped (a typo for a small number?)",
                declared.plates,
                crate::world::compile::geology_layer::MAX_PLATES
            );
        }
    }
    let climate = compile_climate(&def, &astro, &geo);
    println!("  climate:      ok · {} biome(s)", climate.zones.len());
    let hydro = compile_hydrology(&geo, &climate);
    println!("  hydrology:    ok · {} river(s), {} lake(s)", hydro.river_count, hydro.lake_count);
    let demo = compile_demographics(&climate, &hydro);
    println!("  demographics: ok · {} settlement(s)", demo.settlements.len());
    // W11-P1 — verify declared history events (advisory).
    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = crate::world::compile::compile_history(&demo, declared_hist, def.seed_u64());
        let w = crate::world::compile::history_layer::lint_history(declared_hist, &hist);
        if w.is_empty() {
            println!("  history:      ok · {} declared event(s)", declared_hist.len());
        } else {
            println!("  history:      {} declared event(s), {} warning(s):", declared_hist.len(), w.len());
            for x in &w {
                println!("                  ⚠ {x}");
            }
        }
    }
    // W11-P2 — verify declared nations (advisory).
    if !def.nations.is_empty() {
        let w = crate::world::compile::polities_layer::lint_polities(&def.nations, &demo);
        if w.is_empty() {
            println!("  nations:      ok · {} declared", def.nations.len());
        } else {
            println!("  nations:      {} declared, {} warning(s):", def.nations.len(), w.len());
            for x in &w {
                println!("                  ⚠ {x}");
            }
        }
    }
    // W11-P3 — verify declared river courses (downhill; reaches water). Advisory.
    if let Some(hy) = def.hydrology.as_ref() {
        let courses = hy.rivers.iter().filter(|r| r.from.is_some() && r.to.is_some()).count();
        if courses > 0 {
            let w = crate::world::compile::hydrology_layer::lint_rivers(hy, &geo);
            if w.is_empty() {
                println!("  rivers:       ok · {courses} declared course(s) run downhill to water");
            } else {
                println!("  rivers:       {courses} declared course(s), {} warning(s):", w.len());
                for x in &w {
                    println!("                  ⚠ {x}");
                }
            }
        }
    }
    // W11-P4 — verify pinned cultures + ecology (advisory).
    if !def.cultures.is_empty() {
        let pol = crate::world::compile::compile_polities(&demo, &def.nations, def.seed_u64());
        let capital_biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                demo.settlements
                    .iter()
                    .find(|s| (s.x, s.y) == q.capital_pos)
                    .map(|s| s.biome.clone())
                    .unwrap_or_default()
            })
            .collect();
        let w = crate::world::compile::culture_layer::lint_culture(&def.cultures, &pol, &capital_biomes);
        if w.is_empty() {
            println!("  cultures:     ok · {} pinned", def.cultures.len());
        } else {
            println!("  cultures:     {} pinned, {} warning(s):", def.cultures.len(), w.len());
            for x in &w {
                println!("                  ⚠ {x}");
            }
        }
    }
    if let Some(eco) = def.ecology.as_ref().filter(|e| !e.regions.is_empty()) {
        let w = crate::world::compile::ecology_layer::lint_ecology(&eco.regions, &climate);
        if w.is_empty() {
            println!("  ecology:      ok · {} pinned biome(s)", eco.regions.len());
        } else {
            println!("  ecology:      {} pinned, {} warning(s):", eco.regions.len(), w.len());
            for x in &w {
                println!("                  ⚠ {x}");
            }
        }
    }
    if let Some(m) = def.magic.as_ref() {
        let issues = m.lint();
        if issues.is_empty() {
            println!("  magic:        ok · {}", if m.enabled { "ledger enabled" } else { "off" });
        } else {
            println!("  magic:        {} issue(s):", issues.len());
            for w in &issues {
                println!("                  ⚠ {w}");
            }
        }
    }
    println!("all layers compile.");
    Ok(())
}

fn show(project: &Path, json: bool) -> Result<()> {
    let def = load(project)?;
    if json {
        let v = serde_json::to_string_pretty(&def)
            .map_err(|e| Error::Store(format!("serializing definition: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("world: {}", def.name);
    println!("  seed:             {:#x}", def.seed_u64());
    println!("  primary_language: {}", def.primary_language);
    println!("  star:             {} (L={} L☉)", def.astronomy.star.class, def.astronomy.star.luminosity_solar);
    println!(
        "  planet:           {:.2} M⊕, tilt {:.1}°, day {:.1}h",
        def.astronomy.planet.mass_earth, def.astronomy.planet.axial_tilt_deg, def.astronomy.planet.day_length_hours
    );
    println!("  moons:            {}", def.astronomy.moons.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", "));
    Ok(())
}

/// WORLD-7 (W7-P1) — compile the WHOLE world in one command. Runs the five
/// physical layers in dependency order and, with `--materialize`, writes every
/// chapter — Astronomy → Geology → Climate → Hydrology → Demographics — plus the
/// author-declared Setting into the World book. Pure orchestration of the
/// existing `compile_*` + `materialize_*` building blocks.
fn compile_all_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    // Materialize first (a side effect that runs in both JSON and human modes),
    // in dependency order, so a whole world lands in one command.
    let mut reports: Vec<crate::world::materialize::MaterializeReport> = Vec::new();
    if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        use crate::world::materialize as m;
        reports.push(m::materialize_astronomy(&store, &cfg, &astro)?);
        reports.push(m::materialize_geology(&store, &cfg, &geo)?);
        reports.push(m::materialize_climate(&store, &cfg, &climate)?);
        reports.push(m::materialize_hydrology(&store, &cfg, &hydro)?);
        reports.push(m::materialize_demographics(&store, &cfg, &demo)?);
        // WORLD-8 — the world's past lands with the whole-world compile.
        let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
        let hist = crate::world::compile::compile_history(&demo, declared_hist, def.seed_u64());
        reports.push(m::materialize_history(&store, &cfg, &hist)?);
        // WORLD-14 — the human half of the world (nations, cultures, ecology).
        let seed = def.seed_u64();
        let pol = crate::world::compile::compile_polities(&demo, &def.nations, seed);
        let capital_biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                demo.settlements
                    .iter()
                    .find(|s| (s.x, s.y) == q.capital_pos)
                    .map(|s| s.biome.clone())
                    .unwrap_or_default()
            })
            .collect();
        let cul = crate::world::compile::compile_culture(&pol, &capital_biomes, &def.cultures, seed);
        let eco_declared = def.ecology.as_ref().map(|e| e.regions.as_slice()).unwrap_or(&[]);
        let eco = crate::world::compile::compile_ecology(&climate, eco_declared, seed);
        let trade = crate::world::compile::compile_trade(&pol, &geo, def.astronomy.planet.radius_earth);
        reports.push(m::materialize_polities(&store, &cfg, &pol)?);
        reports.push(m::materialize_culture(&store, &cfg, &cul, &demo.role_archetypes, &capital_biomes)?);
        reports.push(m::materialize_ecology(&store, &cfg, &eco)?);
        reports.push(m::materialize_trade(&store, &cfg, &pol, &trade)?);
        reports.push(m::materialize_setting(&store, &cfg, &def)?);
    }

    if json {
        let v = serde_json::json!({
            "astronomy": astro, "geology": geo, "climate": climate,
            "hydrology": hydro, "demographics": demo,
        });
        let s = serde_json::to_string_pretty(&v)
            .map_err(|e| Error::Store(format!("serializing world: {e}")))?;
        println!("{s}");
        return Ok(());
    }

    println!("world · {}", def.name);
    println!("  compiled 5 layers: astronomy · geology · climate · hydrology · demographics");
    println!(
        "  {} settlement(s), population {}",
        demo.settlements.len(),
        fmt_pop(demo.total_population)
    );
    if reports.is_empty() {
        println!("  (run with --materialize to write the World book)");
    } else {
        println!("  materialized {} chapter(s):", reports.len());
        for r in &reports {
            println!("    → World/{}: {} created, {} updated", r.chapter, r.created.len(), r.updated.len());
        }
    }
    Ok(())
}

fn compile(project: &Path, layer: Option<&str>, json: bool, materialize: bool) -> Result<()> {
    // WORLD-7 — a bare `realworld compile` (or `--layer all`) now compiles the
    // whole world; `--layer <name>` still compiles a single layer.
    let l = layer.unwrap_or("all");
    if l == "all" {
        return compile_all_cli(project, json, materialize);
    }
    let known = ["astronomy", "geology", "climate", "hydrology", "demographics"];
    if !known.contains(&l) {
        return Err(Error::Config(format!(
            "unknown layer `{l}` (one of: all, {})",
            known.join(", ")
        )));
    }
    match l {
        "geology" => return compile_geology_cli(project, json, materialize),
        "climate" => return compile_climate_cli(project, json, materialize),
        "hydrology" => return compile_hydrology_cli(project, json, materialize),
        "demographics" => return compile_demographics_cli(project, json, materialize),
        _ => {} // astronomy — falls through to the body below.
    }

    let def = load(project)?;
    let out = compile_astronomy(&def.astronomy);

    // Materialize first (a side effect that runs in both JSON and human modes),
    // so `--json --materialize` both writes the book and prints the output.
    let mat_report = if materialize {
        Some(materialize_to_store(project, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing astronomy: {e}")))?;
        println!("{v}");
        return Ok(());
    }

    println!("astronomy · {}", def.name);
    println!(
        "  year:     {:.1} planet-days  ({:.1} Earth-days, {:.3} M☉ star)",
        out.year_length_planet_days, out.orbital_period_days_earth, out.stellar_mass_solar
    );
    if let (Some(d), Some(div)) = (out.declared_year_length_days, out.year_length_divergence_pct) {
        let flag = if div.abs() > 1.0 { "  ⚠" } else { "" };
        println!("  declared: {d:.0} planet-days  ({div:+.1}% vs computed){flag}");
    }
    println!("  tilt:     {:.1}°", out.axial_tilt_deg);
    print!("  seasons:  ");
    let mut s = out.seasons.clone();
    s.sort_by(|a, b| a.year_fraction.partial_cmp(&b.year_fraction).unwrap_or(std::cmp::Ordering::Equal));
    println!(
        "{}",
        s.iter()
            .map(|m| format!("{} d{:.0}", m.name.replace('_', " "), m.planet_day_of_year))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    for m in &out.moons {
        println!(
            "  moon {}:  synodic {:.1} planet-days, {:.1} lunations/yr",
            m.name, m.synodic_period_planet_days, m.lunar_months_per_year
        );
    }
    if let Some(dom) = &out.tide.dominant_moon {
        println!(
            "  tides:    {} dominant; sun {:.2}× the dominant moon",
            dom, out.tide.solar_relative_to_dominant
        );
    }
    let c = &out.calendar_check;
    println!(
        "  calendar: {:.0} declared vs {:.1} computed days  ({})",
        c.declared_days,
        c.computed_days,
        if c.consistent { "consistent" } else { "off by >1 day ⚠" }
    );
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the generated geology layer. (Materialization into the World
/// book + heightmap PNG export lands in the next WORLD-4 increment.)
fn compile_geology_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    let def = load(project)?;
    let out = geology_for(project, &def)?;

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_geology(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing geology: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("geology · {} ({} source, {}×{} grid)", def.name, out.source, out.width, out.height);
    println!(
        "  plates:     {} ({} continental) · boundaries {}▲ {}▽ {}↔",
        out.plates.len(),
        out.plates.iter().filter(|p| p.continental).count(),
        out.boundaries.convergent,
        out.boundaries.divergent,
        out.boundaries.transform
    );
    println!(
        "  land:       {} continent(s) · {:.0}% ocean · land fraction {:.2}",
        out.continents, out.sea_coverage_pct, out.elevation.land_fraction
    );
    println!(
        "  elevation:  min {:.2} · mean {:.2} · max {:.2}",
        out.elevation.min, out.elevation.mean, out.elevation.max
    );
    println!("  mountains:  {} range(s)", out.mountain_ranges.len());
    for r in out.mountain_ranges.iter().take(4) {
        println!("    plates {}–{} · peak {:.2} · {} cells", r.plate_a, r.plate_b, r.peak_elevation, r.cell_count);
    }
    println!(
        "  minerals:   {}",
        out.minerals.iter().map(|m| m.mineral.as_str()).collect::<Vec<_>>().join(", ")
    );
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated; heightmap → assets/world/heightmap.png",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the climate layer (astronomy + geology → zonal climate).
fn compile_climate_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let out = compile_climate(&def, &astro, &geo);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_climate(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing climate: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("climate · {} ({}×{} grid)", def.name, out.width, out.height);
    println!(
        "  land mean: {:.1}°C · {:.0} mm/yr precipitation",
        out.mean_land_temp_c, out.mean_land_precip_mm
    );
    println!("  winds:     {}", out.winds.iter().map(|w| format!("{} ({})", w.name, w.direction)).collect::<Vec<_>>().join(" · "));
    println!("  biomes ({}):", out.zones.len());
    for z in out.zones.iter().take(8) {
        println!(
            "    {:<20} {:>4.0}% · {:>5.0}…{:<4.0}°C · {:>5.0}…{:.0} mm",
            z.biome, z.area_pct, z.temp_min_c, z.temp_max_c, z.precip_min_mm, z.precip_max_mm
        );
    }
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the hydrology layer (geology + climate → D8 flow).
fn compile_hydrology_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_hydrology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let out = compile_hydrology(&geo, &climate);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_hydrology(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing hydrology: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("hydrology · {} ({}×{} grid)", def.name, out.width, out.height);
    println!(
        "  rivers:     {} ({} major) · {} lake(s) · {} watershed(s)",
        out.river_count, out.major_rivers.len(), out.lake_count, out.watershed_count
    );
    for r in out.major_rivers.iter().take(4) {
        println!("    mouth ({:>3},{:>3}) · order {} · flow {:.0}", r.mouth_x, r.mouth_y, r.order, r.flow);
    }
    println!("  settlement priors ({}):", out.settlement_priors.len());
    for p in out.settlement_priors.iter().take(6) {
        println!("    {:<14} ({:>3},{:>3}) · score {:.0}", p.kind, p.x, p.y, p.score);
    }
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the demographics layer (climate + hydrology → settlements).
fn compile_demographics_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let out = compile_demographics(&climate, &hydro);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        let r = crate::world::materialize::materialize_demographics(&store, &cfg, &out)?;
        // Demographics is the terminal layer of a full compile — also flush the
        // author-declared Setting (geography / hydrology / economy) here.
        let _ = crate::world::materialize::materialize_setting(&store, &cfg, &def)?;
        Some(r)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing demographics: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("demographics · {} ({}×{} grid)", def.name, climate.width, climate.height);
    println!(
        "  population: {} · {:.0}% of land habitable",
        fmt_pop(out.total_population),
        out.habitable_fraction * 100.0
    );
    println!(
        "  settlements: {} ({} cities, {} towns, {} villages)",
        out.settlements.len(),
        out.size_classes.cities,
        out.size_classes.towns,
        out.size_classes.villages
    );
    for s in out.settlements.iter().take(6) {
        println!(
            "    {:<8} ({:>3},{:>3}) · pop {:>7} · {} · {}",
            s.class, s.x, s.y, fmt_pop(s.population), s.basis, s.biome
        );
    }
    println!("  roles: {}", out.role_archetypes.join(", "));
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Human-readable population (12,450 / 1.2M).
fn fmt_pop(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Open the project store and materialize the astronomy output into the World
/// system book. Requires an initialized project (the World book is seeded on
/// open).
fn materialize_to_store(
    project: &Path,
    out: &crate::world::types::AstronomyOutput,
) -> Result<crate::world::materialize::MaterializeReport> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::Store;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    crate::world::materialize::materialize_astronomy(&store, &cfg, out)
}

/// A minimal, valid starter `world.hjson` (Earth-like, one moon) — enough to
/// `compile` immediately; the author edits from here.
fn starter_template(name: &str) -> String {
    format!(
        r#"// A world definition for `inkhaven realworld`.
// Edit freely, then `inkhaven realworld compile --materialize` to compile and
// write the whole world (astronomy · geology · climate · hydrology · demographics)
// into the World book, or `compile --layer <name>` for one layer. Geology /
// climate / hydrology / demographics are generated from `seed` below; add an
// optional block for any of them to override the defaults, and `magic: {{ … }}`
// to declare an author rules ledger (`realworld magic`).
{{
    name: "{name}"
    seed: 0x1A2B3C
    primary_language: "en"

    astronomy: {{
        star: {{ class: "G2V", age_gyr: 4.6, luminosity_solar: 1.0 }}
        planet: {{
            mass_earth: 1.0
            radius_earth: 1.0
            axial_tilt_deg: 23.4
            day_length_hours: 24.0
            rotation_direction: "prograde"
        }}
        orbit: {{ semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }}
        moons: [
            {{ name: "Moon", mass_lunar: 1.0, period_days: 27.32 }}
        ]
        calendar: {{
            months: 12
            month_length_days: 30
            weekdays: 7
            new_year_aligns_to: "winter_solstice"
        }}
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn row_to_latitude_maps_poles_and_equator() {
        // BUG-16: cell-centre convention (matches the climate layer). The top
        // row's *centre* is half a cell south of the pole, not exactly 90°.
        let half = 180.0 / 181.0; // one cell, in degrees
        assert!((super::row_to_latitude(0, 181) - (90.0 - half / 2.0)).abs() < 1e-9);
        assert!((super::row_to_latitude(180, 181) + (90.0 - half / 2.0)).abs() < 1e-9);
        assert!((super::row_to_latitude(90, 181)).abs() < 1e-9); // equator (odd height)
        assert!(super::row_to_latitude(0, 181) < 90.0 && super::row_to_latitude(0, 181) > 89.0);
        assert_eq!(super::row_to_latitude(0, 1), 0.0); // degenerate grid → equator
    }

    #[test]
    fn bearing_reads_the_compass_with_north_at_row_zero() {
        // Row 0 = north pole, so a smaller y is north of a larger y.
        assert_eq!(bearing(10, 10, 20, 10), "E"); // +x = east
        assert_eq!(bearing(10, 10, 0, 10), "W");
        assert_eq!(bearing(10, 10, 10, 0), "N"); // smaller y = north
        assert_eq!(bearing(10, 10, 10, 20), "S");
        assert_eq!(bearing(10, 10, 20, 0), "NE");
        assert_eq!(bearing(10, 10, 0, 20), "SW");
        assert_eq!(bearing(5, 5, 5, 5), "adjacent");
    }

    #[test]
    fn geographic_degrees_round_trip_through_grid_cells() {
        // WORLD-12 — lat/lon → cell → back stays close (rounding to the nearest cell).
        let (w, h) = (160usize, 120usize);
        // Latitude: row inverse of row_to_latitude, clamped.
        assert_eq!(lat_to_row(90.0, h), 0); // north pole → row 0
        assert_eq!(lat_to_row(-90.0, h), h - 1); // south pole → last row
        assert_eq!(lat_to_row(0.0, h), (h - 1) / 2 + ((h - 1) % 2)); // ~equator, rounded
        // Longitude: col 0's centre is just east of −180°, last col near +180°.
        assert_eq!(lon_to_col(-180.0, w), 0);
        assert_eq!(lon_to_col(180.0, w), w - 1);
        assert!(col_to_lon(0, w) > -180.0 && col_to_lon(0, w) < -180.0 + 360.0 / w as f64);
        // A mid latitude round-trips to within one cell.
        let row = lat_to_row(45.0, h);
        assert!((row_to_latitude(row, h) - 45.0).abs() <= 180.0 / h as f64 + 1e-9);
        // Out-of-range degrees clamp rather than panic.
        assert_eq!(lat_to_row(200.0, h), 0);
        assert_eq!(lon_to_col(999.0, w), w - 1);
    }

    #[test]
    fn calendar_bridge_maps_astronomy() {
        // W7-P3 — the astronomy calendar → story-Timeline CalendarConfig mapping.
        let def = crate::world::types::WorldDefinition::from_hjson(&starter_template("Test"))
            .expect("starter template parses");
        let astro = crate::world::compile::compile_astronomy(&def.astronomy);
        let tl = build_timeline_calendar(&def, &astro);

        assert_eq!(tl.preset, "custom");
        assert_eq!(tl.base_unit, "day");
        // day → month → year stack, from the world calendar (30-day months, 12/yr).
        assert_eq!(tl.units.len(), 3);
        assert_eq!(tl.units[0].per_parent, 30);
        assert_eq!(tl.units[1].per_parent, 12);
        assert_eq!(tl.units[2].per_parent, 0); // year is unbounded
        // Four season markers, each a valid in-range span.
        assert_eq!(tl.seasons.len(), 4);
        for s in &tl.seasons {
            assert!((1..=12).contains(&s.start_month), "start_month {} in range", s.start_month);
            assert!(s.span_months >= 1);
        }
    }
}
