//! CL-P3 — the `inkhaven canon` CLI: run the ledger's read-side queries.
//!
//!   inkhaven canon list                 — the recorded canon decisions
//!   inkhaven canon impact <id>          — what breaks if a decision is cut
//!   inkhaven canon why <id>             — the grounds a decision rests on
//!
//! `<id>` is a canonical or short id prefix as printed by `list` (resolved to a
//! unique decision). The ledger is populated deterministically from authored
//! tags on save (CL-P2); LLM harvest of prose is a later, opt-in phase.

use std::path::Path;

use smysl::Commitment;

use crate::ai::stream::collect_blocking;
use crate::ai::AiClient;
use crate::canon::{
    language_name, parse_proposals, system_prompt, CanonView, CommitmentForkView, CommitmentWarning,
    DecisionHistory, LogEntry, MergeSummary, PackedContext, Proposal, StagedCanon,
};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;

fn open(project: &Path) -> Result<Store> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    Store::open(layout, &cfg)
}

fn store_err(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

/// The manuscript breadcrumb for a node (`book/ch1/scene1`).
fn breadcrumb_of(node: &Node) -> String {
    let mut b = node.path.join("/");
    if !b.is_empty() {
        b.push('/');
    }
    b.push_str(&node.slug);
    b
}

/// Collect the paragraph nodes at or under `node`.
fn collect_paragraphs<'a>(h: &'a Hierarchy, node: &'a Node, out: &mut Vec<&'a Node>) {
    if node.kind == NodeKind::Paragraph {
        out.push(node);
    }
    for child in h.children_of(Some(node.id)) {
        collect_paragraphs(h, child, out);
    }
}

/// `inkhaven canon harvest [<scope>]` — opt-in LLM harvest of prose into
/// *staged* proposals (nothing enters the ledger until `canon accept`).
pub fn harvest(project: &Path, scope: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, None)?;
    let model = model.to_string();
    let (lang, _note) = crate::prose::resolve_prose_language(None, &cfg.language);
    let system = system_prompt(language_name(&lang));

    let h = Hierarchy::load(&store)?;
    let mut paragraphs: Vec<&Node> = Vec::new();
    if scope.is_empty() || scope == "." || scope == "all" {
        for root in h.children_of(None) {
            collect_paragraphs(&h, root, &mut paragraphs);
        }
    } else {
        let node = h
            .find_by_path(scope)
            .ok_or_else(|| Error::Store(format!("no node at path {scope:?}")))?;
        collect_paragraphs(&h, node, &mut paragraphs);
    }
    if paragraphs.is_empty() {
        eprintln!("No paragraphs under {scope:?} to harvest.");
        return Ok(());
    }

    eprintln!(
        "harvesting canon from {} paragraph(s) via {model} — one LLM call each…",
        paragraphs.len()
    );
    let mut proposals: Vec<Proposal> = Vec::new();
    for (i, node) in paragraphs.iter().enumerate() {
        let bytes = store.get_content(node.id)?.unwrap_or_default();
        let text = String::from_utf8_lossy(&bytes);
        if text.trim().is_empty() {
            continue;
        }
        eprint!("  [{}/{}] {} … ", i + 1, paragraphs.len(), node.slug);
        let raw = collect_blocking(ai.client.clone(), model.clone(), Some(system.clone()), text.into_owned())
            .map_err(|e| Error::Store(format!("llm harvest failed: {e}")))?;
        let found = parse_proposals(&raw, node.id, &breadcrumb_of(node));
        eprintln!("{} proposal(s)", found.len());
        proposals.extend(found);
    }

    let staged = StagedCanon { proposals };
    let n = staged.proposals.len();
    staged.save(&layout).map_err(store_err)?;
    eprintln!("\nstaged {n} proposal(s) → .inkhaven/canon-staged.json (nothing entered the ledger).");
    eprintln!("review with `inkhaven canon staged`, then `inkhaven canon accept`.");
    Ok(())
}

/// A `wall_ms` epoch timestamp as `YYYY-MM-DD HH:MM`, or `—` when unset (0).
fn fmt_ts(wall_ms: u64) -> String {
    if wall_ms == 0 {
        return "—".to_string();
    }
    chrono::DateTime::from_timestamp_millis(wall_ms as i64)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_else(|| "—".to_string())
}

/// `inkhaven canon history <id>` — a decision's development history: its grounds
/// and its commitment trajectory over time (CG-P4).
pub fn history(project: &Path, id: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    let h: DecisionHistory = match canon.history(uid).map_err(store_err)? {
        Some(h) => h,
        None => {
            eprintln!("no canon decision matches id {id:?}");
            return Ok(());
        }
    };
    eprint!("Decision: ");
    print_view(&h.decision);
    if h.grounds.is_empty() {
        eprintln!("  rests on nothing recorded — a base decision.");
    } else {
        eprintln!("  rests on:");
        for g in &h.grounds {
            print_view(g);
        }
    }
    if h.trajectory.is_empty() {
        eprintln!("  no commitments recorded yet — set canonicity with `canon commit`.");
    } else {
        eprintln!("  commitment history:");
        for e in &h.trajectory {
            println!("    {}  {} → {}", fmt_ts(e.wall_ms), e.agent, e.level);
        }
    }
    Ok(())
}

/// `inkhaven canon log` — the ledger's commitment log, the story's canon settling
/// over time (oldest first, CG-P4).
pub fn log(project: &Path) -> Result<()> {
    let store = open(project)?;
    let entries: Vec<LogEntry> = store.raw().canon().log().map_err(store_err)?;
    if entries.is_empty() {
        eprintln!("No commitments recorded yet. Set canonicity with `canon commit <id> --level <l>`.");
        return Ok(());
    }
    eprintln!("canon log — {} commitment event(s), oldest first:", entries.len());
    for e in &entries {
        println!("  {}  {}  {}  [{}]  {}", fmt_ts(e.wall_ms), e.uid.short(), e.agent, e.level, e.gist);
    }
    Ok(())
}

/// `inkhaven canon ground <id> --on <ground>` — ground one decision on another by
/// hand (the edge the harvest missed), via the supersede primitive.
pub fn ground(project: &Path, id: &str, on: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let src = canon.resolve(id).map_err(store_err)?;
    let dst = canon.resolve(on).map_err(store_err)?;
    if src == dst {
        return Err(Error::Store("a decision cannot ground on itself".into()));
    }
    let new = canon.reground(src, &[dst]).map_err(store_err)?;
    canon.sync().map_err(store_err)?;
    if new == src {
        eprintln!("{} already rests on {} — no change.", src.short(), dst.short());
    } else {
        eprintln!("{} now rests on {} (was {}).", new.short(), dst.short(), src.short());
    }
    Ok(())
}

/// `inkhaven canon unground <id> --from <ground>` — remove a ground (a wrong or
/// unwanted edge), superseding back.
pub fn unground(project: &Path, id: &str, from: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let src = canon.resolve(id).map_err(store_err)?;
    let dst = canon.resolve(from).map_err(store_err)?;
    let new = canon.unground(src, &[dst]).map_err(store_err)?;
    canon.sync().map_err(store_err)?;
    if new == src {
        eprintln!("{} did not rest on {} — no change.", src.short(), dst.short());
    } else {
        eprintln!("{} no longer rests on {} (was {}).", new.short(), dst.short(), src.short());
    }
    Ok(())
}

/// `inkhaven canon staged` — list the model's proposals awaiting confirmation.
pub fn staged(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let staged = StagedCanon::load(&layout).map_err(store_err)?;
    if staged.proposals.is_empty() {
        eprintln!("No staged canon proposals. Run `inkhaven canon harvest <scope>`.");
        return Ok(());
    }
    eprintln!("{} staged proposal(s) (not yet in the ledger):", staged.proposals.len());
    for p in &staged.proposals {
        let kind = p.kind.schema_str().trim_start_matches("x.narrative/");
        println!("  [{kind}]  {}  ({})", p.gist, p.breadcrumb);
        for g in &p.grounds {
            println!("      ↳ rests on: {g}");
        }
    }
    Ok(())
}

/// `inkhaven canon accept` — the author's confirmation: record the staged
/// proposals into the ledger (uncommitted), then clear staging.
pub fn accept(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let staged = StagedCanon::load(&layout).map_err(store_err)?;
    if staged.proposals.is_empty() {
        eprintln!("Nothing staged to accept.");
        return Ok(());
    }
    let canon = store.raw().canon();
    // CG-P1 — decisions are born with their deterministically-inferred grounds,
    // in the project language (so `canon impact`/`why` have a graph to walk).
    let (language, _) = crate::prose::resolve_prose_language(None, &cfg.language);
    let items: Vec<crate::canon::NewDecision> = staged
        .proposals
        .iter()
        .map(|p| crate::canon::NewDecision {
            kind: p.kind,
            gist: p.gist.clone(),
            node: p.node,
            breadcrumb: p.breadcrumb.clone(),
            proposed_grounds: p.grounds.clone(),
        })
        .collect();
    let n = canon.record_grounded_batch(&items, &language).map_err(store_err)?.len();
    canon.sync().map_err(store_err)?;
    StagedCanon::clear(&layout).map_err(store_err)?;
    eprintln!("accepted {n} proposal(s) into the canon ledger (uncommitted — set canonicity with `canon commit`).");
    Ok(())
}

/// `inkhaven canon list`
pub fn list(project: &Path) -> Result<()> {
    let store = open(project)?;
    let decisions = store.raw().canon().all_decisions().map_err(store_err)?;
    if decisions.is_empty() {
        eprintln!(
            "No canon decisions recorded yet. They are harvested from authored tags \
             (e.g. `rel:<kind>:<A>:<B>`) when a paragraph is saved."
        );
        return Ok(());
    }
    eprintln!("{} canon decision(s):", decisions.len());
    for v in &decisions {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon impact <id>`
pub fn impact(project: &Path, id: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    let target = canon.view(uid).map_err(store_err)?;
    if let Some(t) = &target {
        eprint!("If cut: ");
        print_view(t);
    }
    let hits = canon.impact(uid).map_err(store_err)?;
    if hits.is_empty() {
        eprintln!("  nothing else rests on it — safe to cut.");
        return Ok(());
    }
    eprintln!("  {} decision(s) would dangle:", hits.len());
    for v in &hits {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon why <id>`
pub fn why(project: &Path, id: &str) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    if let Some(t) = canon.view(uid).map_err(store_err)? {
        eprint!("Decision: ");
        print_view(&t);
    }
    let grounds = canon.why(uid).map_err(store_err)?;
    if grounds.is_empty() {
        eprintln!("  rests on nothing recorded — a base decision.");
        return Ok(());
    }
    eprintln!("  rests on {} decision(s):", grounds.len());
    for v in &grounds {
        print_view(v);
    }
    Ok(())
}

/// `inkhaven canon commit <id> --level <level> [--as <agent>]`
pub fn commit(project: &Path, id: &str, level: &str, agent: Option<String>) -> Result<()> {
    let store = open(project)?;
    let canon = store.raw().canon();
    let uid = canon.resolve(id).map_err(store_err)?;
    let level = Commitment::parse(level).ok_or_else(|| {
        let valid: Vec<String> = Commitment::ALL.iter().map(|c| c.to_string()).collect();
        Error::Store(format!("unknown commitment level {level:?} — valid: {}", valid.join(", ")))
    })?;
    let agent = agent.unwrap_or_else(|| "author".to_string());
    canon.commit(uid, level, &agent).map_err(store_err)?;
    eprintln!("committed {} → {level}", uid.short());
    Ok(())
}

/// `inkhaven canon check` — the SMY-W057 advisory.
pub fn check(project: &Path) -> Result<()> {
    let store = open(project)?;
    let warnings: Vec<CommitmentWarning> =
        store.raw().canon().commitment_warnings().map_err(store_err)?;
    if warnings.is_empty() {
        eprintln!("No commitment issues — nothing is committed above what it rests on (SMY-W057).");
        return Ok(());
    }
    eprintln!(
        "{} decision(s) committed above their foundation (SMY-W057 — \"built on sand\"):",
        warnings.len()
    );
    for w in &warnings {
        println!(
            "  {} [{}]  rests on  {} [{}]",
            w.unit.uid.short(),
            w.level,
            w.weakest_ground.uid.short(),
            w.ground_level
        );
        println!("      {}", w.unit.gist);
        println!("        ⟵ needs: {}", w.weakest_ground.gist);
    }
    Ok(())
}

/// `inkhaven canon merge <path>`
pub fn merge(project: &Path, other: &str) -> Result<()> {
    let store = open(project)?;
    let summary: MergeSummary = store.raw().canon().merge_from(other).map_err(store_err)?;
    eprintln!(
        "merged {other}: {} added, {} duplicate(s), {} commitment fork(s)",
        summary.added, summary.duplicates, summary.forks
    );
    if summary.forks > 0 {
        eprintln!("  run `inkhaven canon forks` to review them.");
    }
    Ok(())
}

/// `inkhaven canon forks`
pub fn forks(project: &Path) -> Result<()> {
    let store = open(project)?;
    let forks: Vec<CommitmentForkView> = store.raw().canon().commitment_forks().map_err(store_err)?;
    if forks.is_empty() {
        eprintln!("No commitment forks — no decision has agents disagreeing on how settled it is.");
        return Ok(());
    }
    eprintln!(
        "{} commitment fork(s) (SMY-W058 — agents disagree on canonicity):",
        forks.len()
    );
    for f in &forks {
        let status = if f.resolved { "  (resolved)" } else { "" };
        println!("  {}  {}{status}", f.unit.uid.short(), f.unit.gist);
        for (agent, level) in &f.positions {
            println!("      {agent} → {level}");
        }
    }
    Ok(())
}

/// `inkhaven canon context <query> [--budget N] [--reserve N] [--limit N]`
pub fn context(
    project: &Path,
    query: &str,
    limit: usize,
    budget: Option<usize>,
    reserve: Option<usize>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    // `--budget` / `--reserve` win; else the `canon:` config defaults.
    let budget = budget.unwrap_or(cfg.canon.context_budget);
    let reserve = reserve.unwrap_or(cfg.canon.context_reserve);
    let store = Store::open(layout, &cfg)?;
    let ctx: PackedContext = store
        .raw()
        .canon_context_for_query(query, limit, budget, reserve)
        .map_err(store_err)?;
    if ctx.views.is_empty() {
        eprintln!("No canon context for {query:?} (empty ledger, or nothing relevant).");
        return Ok(());
    }
    eprintln!(
        "canon context for {query:?} — {} decision(s), {}/{} tokens ({} reserved, {} dropped):",
        ctx.views.len(),
        ctx.used,
        ctx.budget,
        ctx.reserved,
        ctx.dropped
    );
    print!("{}", ctx.to_prompt());
    Ok(())
}

fn print_view(v: &CanonView) {
    let kind = v
        .kind
        .map(|k| k.schema_str().trim_start_matches("x.narrative/").to_string())
        .unwrap_or_else(|| "?".to_string());
    let commitment = v
        .commitment
        .map(|c| format!(" «{c}»"))
        .unwrap_or_default();
    let loc = v.locator.as_deref().unwrap_or("");
    let loc = if loc.is_empty() {
        String::new()
    } else {
        format!("  ({loc})")
    };
    println!("  {}  [{kind}]{commitment}  {}{loc}", v.uid.short(), v.gist);
}
