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
