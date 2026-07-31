//! SEMNET-P0/P1 — the `Store`-level graph API.
//!
//! Ergonomic, node-centric wrappers over [`crate::storage::edge_store`]: add an
//! edge, ask what a node points at / what points at it, rebuild the derived
//! edges, and summarise the graph. The persistence lives in the storage layer;
//! this module is the domain-facing surface the editor / CLI / (later) Inner
//! family call.
//!
//! P0 shipped the substrate + queries + rebuild/stats plumbing. **P1 — the
//! structural lift** — derives the first real edges from durable node fields:
//! `linked_paragraphs` → [`EdgeKind::LinksTo`] and `event.characters`/`.places`
//! → [`EdgeKind::EventInvolves`] (both `origin = Structural`). They are a
//! projection of the node fields (still the write path), so `graph_rebuild`
//! reproduces them exactly and idempotently. Later migrations (provenance,
//! stance, …) register their own re-derivations in `graph_rebuild`.
//!
//! The traversal methods (`add_edge`, `edges_out`, `neighbors`, …) are consumed
//! by P2+ and tests ahead of a CLI caller; the module-level `allow(dead_code)`
//! keeps the warning-free bar until the surfacing phase wires the verbs.
#![allow(dead_code)]

use std::collections::HashSet;

use uuid::Uuid;

use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::Store;

// Re-export the domain types so callers use `store::graph::Edge` etc. (P2+
// migrations reach for `ExternRef`/`Registry` via `crate::storage::edge_store`
// directly until they surface here.)
pub use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EndpointRef};

/// The outcome of a [`Store::graph_rebuild`] — how many rebuildable edges were
/// cleared and how many re-derived.
#[derive(Debug, Clone, Copy)]
pub struct GraphRebuild {
    /// Rebuildable edges dropped (Structural/Derived/Imported).
    pub cleared: usize,
    /// Edges re-derived from the current node state.
    pub added: usize,
}

/// A summary of the graph — for `inkhaven graph stats`.
#[derive(Debug, Clone)]
pub struct GraphStats {
    /// Total edges.
    pub edges: usize,
    /// Total nodes (the vertices the edges overlay).
    pub nodes: usize,
    /// Per-kind edge counts, ascending by kind name.
    pub by_kind: Vec<(String, usize)>,
}

impl Store {
    /// Add one edge to the graph.
    pub fn add_edge(&self, edge: &Edge) -> Result<()> {
        self.raw().add_edge(edge).map_err(map_edge_err)
    }

    /// Add many edges atomically (all land or none).
    pub fn add_edges(&self, edges: &[Edge]) -> Result<()> {
        self.raw().add_edges(edges).map_err(map_edge_err)
    }

    /// The edge with this id, if any.
    pub fn edge(&self, id: Uuid) -> Result<Option<Edge>> {
        self.raw().edge(id).map_err(map_edge_err)
    }

    /// Edges leaving `node`, filtered to `kinds` (empty slice = any kind).
    pub fn edges_out(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.raw().edges_out(node, kinds).map_err(map_edge_err)
    }

    /// Edges arriving at `node` — "what points at this?".
    pub fn edges_in(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.raw().edges_in(node, kinds).map_err(map_edge_err)
    }

    /// Every edge touching `node` on either side (deduped) — the one-hop
    /// neighbourhood.
    pub fn neighbors(&self, node: Uuid, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.raw().edges_around(node, kinds).map_err(map_edge_err)
    }

    /// Delete one edge by id.
    pub fn delete_edge(&self, id: Uuid) -> Result<()> {
        self.raw().delete_edge(id).map_err(map_edge_err)
    }

    /// Summarise the graph (node/edge counts + per-kind breakdown).
    pub fn graph_stats(&self) -> Result<GraphStats> {
        let edges = self.raw().edge_count().map_err(map_edge_err)?;
        let by_kind = self.raw().edges_by_kind().map_err(map_edge_err)?;
        let nodes = self.raw().row_count().map_err(map_edge_err)?;
        Ok(GraphStats { edges, nodes, by_kind })
    }

    /// Rebuild the derivable edges: drop every edge that a derivation below can
    /// recompute (`Structural`/`Derived`/`Imported`) and re-derive them from the
    /// current node state. User decisions (`Authorial`/`Promoted`) are preserved.
    ///
    /// Idempotent: running it twice on unchanged nodes yields the same edge set
    /// (edge ids differ — freshly minted — but the endpoints/kinds match).
    ///
    /// P1 re-derives the structural edges (`LinksTo`, `EventInvolves`). Later
    /// phases add their re-derivations here (Derived similarity, Imported
    /// bridges, provenance/verdict projections, …).
    pub fn graph_rebuild(&self) -> Result<GraphRebuild> {
        let mut cleared = 0;
        for origin in [EdgeOrigin::Structural, EdgeOrigin::Derived, EdgeOrigin::Imported] {
            cleared += self
                .raw()
                .delete_edges_by_origin(origin)
                .map_err(map_edge_err)?;
        }

        // P1 — structural lift from node fields.
        let hierarchy = Hierarchy::load(self)?;
        let nodes: Vec<Node> = hierarchy.flatten().into_iter().map(|(n, _)| n.clone()).collect();
        let structural = derive_structural_edges(&nodes);
        let added = structural.len();
        if !structural.is_empty() {
            self.raw().add_edges(&structural).map_err(map_edge_err)?;
        }

        Ok(GraphRebuild { cleared, added })
    }

    /// Edge-store integrity check (`"ok"` when healthy).
    pub fn graph_integrity_check(&self) -> Result<String> {
        self.raw().edges_integrity_check().map_err(map_edge_err)
    }
}

fn map_edge_err(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}

/// SEMNET-P1 — derive the structural edges from the node set: `LinksTo` from
/// each node's `linked_paragraphs`, `EventInvolves` from each event's
/// `characters`/`places` (tagged `attrs.role`). Targets that aren't real nodes
/// in `nodes` are skipped, so a rebuild never produces a dangling edge. All are
/// `origin = Structural` — a projection of durable node fields.
///
/// Pure (no I/O) so it's unit-testable without a live `Store`.
pub(crate) fn derive_structural_edges(nodes: &[Node]) -> Vec<Edge> {
    let ids: HashSet<Uuid> = nodes.iter().map(|n| n.id).collect();
    let mut edges = Vec::new();
    for n in nodes {
        for target in &n.linked_paragraphs {
            if ids.contains(target) {
                edges.push(Edge::new(
                    EndpointRef::Node(n.id),
                    EdgeKind::LinksTo,
                    EndpointRef::Node(*target),
                    EdgeOrigin::Structural,
                ));
            }
        }
        if let Some(ev) = &n.event {
            for c in &ev.characters {
                if ids.contains(c) {
                    edges.push(
                        Edge::new(
                            EndpointRef::Node(n.id),
                            EdgeKind::EventInvolves,
                            EndpointRef::Node(*c),
                            EdgeOrigin::Structural,
                        )
                        .with_attrs(serde_json::json!({ "role": "character" })),
                    );
                }
            }
            for p in &ev.places {
                if ids.contains(p) {
                    edges.push(
                        Edge::new(
                            EndpointRef::Node(n.id),
                            EdgeKind::EventInvolves,
                            EndpointRef::Node(*p),
                            EdgeOrigin::Structural,
                        )
                        .with_attrs(serde_json::json!({ "role": "place" })),
                    );
                }
            }
        }
    }
    edges
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::edge_store::EdgeStore;
    use tempfile::TempDir;

    fn para(id: Uuid, links: &[Uuid]) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": "paragraph", "title": "p", "slug": "p",
            "path": [], "parent_id": null, "order": 1, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
            "linked_paragraphs": links,
        }))
        .expect("test node deserialises")
    }

    fn event_node(id: Uuid, chars: &[Uuid], places: &[Uuid]) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": "paragraph", "title": "e", "slug": "e",
            "path": [], "parent_id": null, "order": 1, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
            "event": { "start_ticks": 0, "characters": chars, "places": places },
        }))
        .expect("test event node deserialises")
    }

    /// The endpoint+kind signature of an edge, ignoring its (freshly minted) id.
    fn sig(e: &Edge) -> (EndpointRef, EdgeKind, EndpointRef) {
        (e.src.clone(), e.kind, e.dst.clone())
    }

    #[test]
    fn links_and_events_become_edges() {
        let (a, b, c) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        // a links to b; an event involves a (character) and b (place).
        let ev = Uuid::now_v7();
        let nodes = vec![
            para(a, &[b]),
            para(b, &[]),
            para(c, &[]),
            event_node(ev, &[a], &[b]),
        ];
        let edges = derive_structural_edges(&nodes);
        assert_eq!(edges.len(), 3);

        let links: Vec<_> = edges.iter().filter(|e| e.kind == EdgeKind::LinksTo).collect();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].src, EndpointRef::Node(a));
        assert_eq!(links[0].dst, EndpointRef::Node(b));
        assert!(links[0].directed);
        assert_eq!(links[0].origin, EdgeOrigin::Structural);

        let involves: Vec<_> = edges.iter().filter(|e| e.kind == EdgeKind::EventInvolves).collect();
        assert_eq!(involves.len(), 2);
        let char_edge = involves.iter().find(|e| e.dst == EndpointRef::Node(a)).unwrap();
        assert_eq!(char_edge.attrs["role"], "character");
        let place_edge = involves.iter().find(|e| e.dst == EndpointRef::Node(b)).unwrap();
        assert_eq!(place_edge.attrs["role"], "place");
    }

    #[test]
    fn dangling_targets_are_skipped() {
        let a = Uuid::now_v7();
        let ghost = Uuid::now_v7(); // not in the node set
        let ev = Uuid::now_v7();
        let nodes = vec![para(a, &[ghost]), event_node(ev, &[ghost], &[ghost])];
        assert!(
            derive_structural_edges(&nodes).is_empty(),
            "edges to non-existent nodes must be skipped, no dangling edges"
        );
    }

    #[test]
    fn derivation_is_idempotent_by_endpoints() {
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let nodes = vec![para(a, &[b]), para(b, &[a])];
        let mut first: Vec<_> = derive_structural_edges(&nodes).iter().map(sig).collect();
        let mut second: Vec<_> = derive_structural_edges(&nodes).iter().map(sig).collect();
        first.sort_by_key(|(s, _, d)| (format!("{s:?}"), format!("{d:?}")));
        second.sort_by_key(|(s, _, d)| (format!("{s:?}"), format!("{d:?}")));
        assert_eq!(first, second, "rebuild must be idempotent by endpoint set");
    }

    #[test]
    fn derived_edges_answer_reverse_index_queries() {
        // The P1 output, stored through the P0 edge store, is queryable by the
        // reverse index: "what links to b?" finds a→b.
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let edges = derive_structural_edges(&[para(a, &[b]), para(b, &[])]);
        let dir = TempDir::new().unwrap();
        let store = EdgeStore::new(dir.path().join("edges.db"), 2).unwrap();
        store.insert_batch(&edges).unwrap();

        let into_b = store.incoming(&EndpointRef::Node(b), &[EdgeKind::LinksTo]).unwrap();
        assert_eq!(into_b.len(), 1);
        assert_eq!(into_b[0].src, EndpointRef::Node(a));
        assert!(store.incoming(&EndpointRef::Node(a), &[EdgeKind::LinksTo]).unwrap().is_empty());
    }
}
