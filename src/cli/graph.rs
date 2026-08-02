//! SEMNET-P0 — `inkhaven graph` verbs. The user-facing window onto the
//! knowledge-graph edge layer. P0 ships `stats` (counts + per-kind breakdown)
//! and `rebuild` (drop + re-derive the rebuildable-cache edges); the traversal
//! verbs (`neighbors`, `contradicting`) land with the migrations that populate
//! the graph (P1+).

use std::path::Path;

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::graph::EndpointRef;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

fn parse_uuid(s: &str, what: &str) -> Result<Uuid> {
    Uuid::parse_str(s).map_err(|e| Error::Config(format!("invalid {what} uuid `{s}`: {e}")))
}

fn open(project: &Path) -> Result<Store> {
    Ok(open_with_cfg(project)?.0)
}

fn open_with_cfg(project: &Path) -> Result<(Store, Config)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    Ok((store, cfg))
}

/// `inkhaven graph stats` — node + edge counts and a per-kind breakdown.
pub fn stats(project: &Path) -> Result<()> {
    let store = open(project)?;
    let s = store.graph_stats()?;
    println!("nodes: {}", s.nodes);
    println!("edges: {}", s.edges);
    if s.by_kind.is_empty() {
        println!("(no edges yet — the graph is populated by the SEMNET migrations, P1+)");
    } else {
        println!("by kind:");
        for (kind, n) in &s.by_kind {
            println!("  {kind:<16} {n}");
        }
    }
    Ok(())
}

/// `inkhaven graph rebuild` — drop and re-derive the derivable edges
/// (`Structural`/`Derived`/`Imported`); the user's `Authorial`/`Promoted` edges
/// are untouched. P1 re-derives the structural edges (`LinksTo`,
/// `EventInvolves`) from the current node fields.
pub fn rebuild(project: &Path) -> Result<()> {
    let (store, cfg) = open_with_cfg(project)?;
    let r = store.graph_rebuild(&cfg)?;
    println!("graph rebuild: cleared {} derivable edge(s), re-derived {}", r.cleared, r.added);
    let s = store.graph_stats()?;
    println!("graph now holds {} edge(s) across {} node(s)", s.edges, s.nodes);
    if !s.by_kind.is_empty() {
        for (kind, n) in &s.by_kind {
            println!("  {kind:<16} {n}");
        }
    }
    Ok(())
}

/// `inkhaven graph contradicting <node>` — the recorded stance clashes touching
/// a node (Contradicts / InTension, either direction).
pub fn contradicting(project: &Path, node: &str) -> Result<()> {
    let store = open(project)?;
    let id = parse_uuid(node, "node")?;
    let edges = store.contradicting(id)?;
    if edges.is_empty() {
        println!("no contradictions recorded for {id}");
        return Ok(());
    }
    let here = EndpointRef::Node(id);
    for e in &edges {
        let (k, r) = e.other_endpoint(&here).as_columns();
        let reason = e.reason.as_deref().unwrap_or("");
        let sep = if reason.is_empty() { "" } else { " — " };
        println!("{}  [{}·{}]  {k}:{r}{sep}{reason}", e.id, e.kind.as_str(), e.origin.as_str());
    }
    Ok(())
}

/// `inkhaven graph promote <edge>` — accept a Judged stance edge (→ Promoted).
pub fn promote(project: &Path, edge: &str) -> Result<()> {
    let store = open(project)?;
    let id = parse_uuid(edge, "edge")?;
    if store.promote_edge(id)? {
        println!("promoted edge {id} (kept across rebuilds)");
    } else {
        println!("no edge with id {id}");
    }
    Ok(())
}

/// `inkhaven graph dismiss <edge>` — delete a stance edge.
pub fn dismiss(project: &Path, edge: &str) -> Result<()> {
    let store = open(project)?;
    let id = parse_uuid(edge, "edge")?;
    store.dismiss_edge(id)?;
    println!("dismissed edge {id}");
    Ok(())
}

/// `inkhaven graph neighbors <node>` — the node's one-hop neighbourhood rendered
/// as a terminal-native tree: what it links to, contradicts, is sourced from,
/// cites, and the senses it mentions.
pub fn neighbors(project: &Path, node: &str) -> Result<()> {
    let store = open(project)?;
    let id = parse_uuid(node, "node")?;
    let edges = store.subgraph(id, 1, &[])?;
    let h = Hierarchy::load(&store)?;
    let label = |ep: &EndpointRef| -> String {
        match ep {
            EndpointRef::Node(u) => h
                .get(*u)
                .map(|n| n.title.clone())
                .filter(|t| !t.trim().is_empty())
                .unwrap_or_else(|| format!("node {}", &u.to_string()[..8])),
            EndpointRef::Extern(_) => {
                let (k, r) = ep.as_columns();
                format!("{k} {r}")
            }
        }
    };
    print!("{}", crate::store::graph::render_neighbourhood(id, &edges, label));
    Ok(())
}

/// A human label for an endpoint: a node's title (via the hierarchy), or an
/// extern's `kind ref`.
fn endpoint_label(ep: &EndpointRef, h: &Hierarchy) -> String {
    match ep {
        EndpointRef::Node(u) => h
            .get(*u)
            .map(|n| n.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("node {}", &u.to_string()[..8])),
        EndpointRef::Extern(_) => {
            let (k, r) = ep.as_columns();
            format!("{k} {r}")
        }
    }
}

/// `inkhaven graph link <node>` — propose stance edges from a fact to its
/// nearest related facts (the confront judge over your own corpus): retrieve the
/// neighbours, grade each relation, and persist the non-Silent ones as advisory
/// `Judged` edges. Triage them with `graph pending`. Needs an LLM provider.
pub fn link(project: &Path, node: &str) -> Result<()> {
    let (store, cfg) = open_with_cfg(project)?;
    let id = parse_uuid(node, "node")?;
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("LLM provider: {e:#}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("LLM provider: {e:#}")))?;
    let h = Hierarchy::load(&store)?;
    let Some(facts_book) = h
        .iter()
        .find(|n| n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_FACTS))
        .map(|n| n.id)
    else {
        return Err(Error::Config("this project has no Facts book to link against".into()));
    };
    let body = store
        .raw()
        .get_content(id)
        .map_err(|e| Error::Store(e.to_string()))?
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .ok_or_else(|| Error::Config(format!("no content for node {id}")))?;
    // The /relate judge writes its reasons in the project language (a name, e.g.
    // "English"), matching the confront path.
    let iso = crate::ai::prompts::iso_from_long(&cfg.language);
    let language = crate::inner_editor::prompt::language_name(iso).to_string();

    let n = crate::research::graph_link::link_fact(
        &store, &h, &cfg, &ai, &model, facts_book, id, &body, &language,
    );
    if n == 0 {
        println!("no stance edges proposed (no related facts, or all silent)");
    } else {
        println!("proposed {n} advisory edge(s) — triage with `graph pending` (`graph promote`/`dismiss`)");
    }
    Ok(())
}

/// `inkhaven graph pending` — the advisory (Judged) stance edges awaiting triage:
/// the edge inbox. Promote the ones that stick with `graph promote <id>`, reject
/// the rest with `graph dismiss <id>`.
pub fn pending(project: &Path) -> Result<()> {
    let store = open(project)?;
    let edges = store.pending_edges()?;
    if edges.is_empty() {
        println!("no pending edges — the graph's advisory layer is clear");
        return Ok(());
    }
    let h = Hierarchy::load(&store)?;
    println!(
        "{} pending edge(s) — `graph promote <id>` to keep (across rebuilds), `graph dismiss <id>` to reject:",
        edges.len()
    );
    for e in &edges {
        let src = endpoint_label(&e.src, &h);
        let dst = endpoint_label(&e.dst, &h);
        let reason = e
            .reason
            .as_deref()
            .filter(|r| !r.is_empty())
            .map(|r| format!(" — {r}"))
            .unwrap_or_default();
        println!("  {}  [{}]  {src} ⇢ {dst}{reason}", e.id, e.kind.as_str());
    }
    Ok(())
}

/// `inkhaven graph loci <node>` — the primary-source loci a node cites.
pub fn loci(project: &Path, node: &str) -> Result<()> {
    use crate::store::graph::EdgeKind;
    let store = open(project)?;
    let id = parse_uuid(node, "node")?;
    let edges = store.edges_out(id, &[EdgeKind::CitesLocus])?;
    if edges.is_empty() {
        println!("{id} cites no primary-source loci");
        return Ok(());
    }
    for e in &edges {
        let (_k, r) = e.dst.as_columns();
        let key = e.attrs.get("key").and_then(|v| v.as_str()).unwrap_or("");
        println!("@{key}  {r}");
    }
    Ok(())
}

/// GRAPHMIND GM-P5 — the read-only [`GraphOracle`] over a real `Store`: the
/// graph-query surface the `graph ask` tool-loop walks. Every method degrades to
/// a readable "(none)" / error line rather than aborting — a single failed query
/// shouldn't kill the exploration.
struct StoreOracle<'a> {
    store: &'a Store,
    h: &'a Hierarchy,
}

impl StoreOracle<'_> {
    fn node_label(&self, id: Uuid) -> String {
        self.h
            .get(id)
            .map(|n| n.title.clone())
            .filter(|t| !t.trim().is_empty())
            .unwrap_or_else(|| format!("node {}", &id.to_string()[..8]))
    }
    fn ep_label(&self, ep: &EndpointRef) -> String {
        match ep {
            EndpointRef::Node(u) => self.node_label(*u),
            EndpointRef::Extern(_) => {
                let (k, r) = ep.as_columns();
                format!("{k} {r}")
            }
        }
    }
}

impl crate::graph_rag::ask::GraphOracle for StoreOracle<'_> {
    fn search(&self, query: &str, limit: usize) -> Vec<(Uuid, String)> {
        let pool = (limit * 3 + 8).max(8);
        let raw = self.store.search_text(query, pool).unwrap_or_default();
        let mut out = Vec::new();
        for v in &raw {
            let Some(id) = v
                .get("id")
                .and_then(|x| x.as_str())
                .and_then(|s| Uuid::parse_str(s).ok())
            else {
                continue;
            };
            if self.h.get(id).is_none() {
                continue;
            }
            out.push((id, self.node_label(id)));
            if out.len() >= limit {
                break;
            }
        }
        out
    }

    fn neighbors(&self, node: Uuid) -> String {
        let edges = self.store.subgraph(node, 1, &[]).unwrap_or_default();
        crate::store::graph::render_neighbourhood(node, &edges, |ep| self.ep_label(ep))
    }

    fn contradicting(&self, node: Uuid) -> String {
        let edges = match self.store.contradicting(node) {
            Ok(e) => e,
            Err(e) => return format!("(graph error: {e})"),
        };
        if edges.is_empty() {
            return "(no contradictions recorded)".to_string();
        }
        let here = EndpointRef::Node(node);
        edges
            .iter()
            .map(|e| {
                let reason = e.reason.as_deref().filter(|r| !r.is_empty());
                let sep = if reason.is_some() { " — " } else { "" };
                format!(
                    "  [{}] {}{sep}{}",
                    e.kind.as_str(),
                    self.ep_label(e.other_endpoint(&here)),
                    reason.unwrap_or("")
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn loci(&self, node: Uuid) -> String {
        use crate::store::graph::EdgeKind;
        let edges = match self.store.edges_out(node, &[EdgeKind::CitesLocus]) {
            Ok(e) => e,
            Err(e) => return format!("(graph error: {e})"),
        };
        if edges.is_empty() {
            return "(cites no primary-source loci)".to_string();
        }
        edges
            .iter()
            .map(|e| {
                let (_k, r) = e.dst.as_columns();
                let key = e.attrs.get("key").and_then(|v| v.as_str()).unwrap_or("");
                format!("  @{key}  {r}")
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn paths(&self, from: Uuid, to: Uuid) -> String {
        use crate::store::graph::EdgeKind;
        match self.store.paths(from, to, &[EdgeKind::Cites, EdgeKind::LinksTo], 8) {
            Ok(Some(path)) => {
                let hops = path.len().saturating_sub(1);
                let names: Vec<String> = path.iter().map(|id| self.node_label(*id)).collect();
                format!("  {hops} hop(s): {}", names.join(" → "))
            }
            Ok(None) => "(no path within 8 hops)".to_string(),
            Err(e) => format!("(graph error: {e})"),
        }
    }

    fn label(&self, node: Uuid) -> String {
        self.node_label(node)
    }
}

/// `inkhaven graph ask <question>` — GRAPHMIND GM-P5. Answer a question by
/// *walking* the knowledge graph: the model searches for seed nodes, then issues
/// read-only graph queries (neighbours / contradictions / loci / paths) turn by
/// turn until it can answer, grounding the answer in what it observed. Needs an
/// LLM provider. The exploration transcript goes to stderr; the answer to stdout.
pub fn ask(project: &Path, question: &str) -> Result<()> {
    let (store, cfg) = open_with_cfg(project)?;
    let question = question.trim();
    if question.is_empty() {
        return Err(Error::Config(
            "give a question, e.g. `graph ask \"what contradicts the storm scene?\"`".into(),
        ));
    }
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| Error::Config(format!("LLM provider: {e:#}")))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| Error::Config(format!("LLM provider: {e:#}")))?;
    let h = Hierarchy::load(&store)?;
    let iso = crate::ai::prompts::iso_from_long(&cfg.language);
    let system = crate::graph_rag::ask::system_prompt(iso).to_string();
    let oracle = StoreOracle { store: &store, h: &h };

    let max_steps = cfg.research.agentic.max_rounds.max(1) * 2 + 2;
    let search_limit = 6usize;
    let client = ai.client.clone();
    let modelname = model.to_string();

    eprintln!("» graph ask: {question}");
    let outcome = crate::graph_rag::ask::ask(
        &oracle,
        |prompt| {
            crate::ai::stream::collect_blocking(
                client.clone(),
                modelname.clone(),
                Some(system.clone()),
                prompt.to_string(),
            )
        },
        question,
        max_steps,
        search_limit,
    )
    .map_err(|e| Error::Config(format!("graph ask: {e}")))?;

    for s in &outcome.steps {
        eprintln!("{s}");
    }
    eprintln!(
        "» {} step(s), {} model turn(s){}",
        outcome.steps.len(),
        outcome.llm_calls,
        if outcome.forced { " · forced final answer (step budget spent)" } else { "" }
    );
    println!("{}", outcome.answer);
    Ok(())
}

/// `inkhaven graph lexical` — (re)build the WordNet lexical bridge for the
/// project language: link the manuscript's words to their senses + the local
/// semantic net (hypernym/hyponym/antonym) + cross-lingual ILI.
pub fn lexical(project: &Path) -> Result<()> {
    let (store, cfg) = open_with_cfg(project)?;
    let r = store.rebuild_lexical(&cfg)?;
    if !r.installed {
        let code = crate::ai::prompts::iso_from_long(&cfg.language);
        println!("no `{code}` wordnet installed — run `inkhaven wordnet fetch {code}` first");
        return Ok(());
    }
    println!("lexical bridge: cleared {} prior edge(s), imported {}", r.cleared, r.added);
    let s = store.graph_stats()?;
    for (kind, n) in &s.by_kind {
        println!("  {kind:<16} {n}");
    }
    Ok(())
}

/// `inkhaven graph paths <from> <to>` — a bounded citation/link path between two
/// nodes (over Cites + LinksTo, ≤ 8 hops).
pub fn paths(project: &Path, from: &str, to: &str) -> Result<()> {
    use crate::store::graph::EdgeKind;
    let store = open(project)?;
    let a = parse_uuid(from, "from")?;
    let b = parse_uuid(to, "to")?;
    match store.paths(a, b, &[EdgeKind::Cites, EdgeKind::LinksTo], 8)? {
        Some(path) => {
            let hops = path.len().saturating_sub(1);
            println!("path found ({hops} hop(s)):");
            for id in &path {
                println!("  {id}");
            }
        }
        None => println!("no path from {a} to {b} within 8 hops"),
    }
    Ok(())
}
