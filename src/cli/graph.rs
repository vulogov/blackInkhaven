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
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    Store::open(layout, &cfg)
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
    let store = open(project)?;
    let r = store.graph_rebuild()?;
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
