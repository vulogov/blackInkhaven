//! SEMNET-P0 — the `Store`-level graph API.
//!
//! Ergonomic, node-centric wrappers over [`crate::storage::edge_store`]: add an
//! edge, ask what a node points at / what points at it, rebuild the derived
//! cache, and summarise the graph. The persistence lives in the storage layer;
//! this module is the domain-facing surface the editor / CLI / (later) Inner
//! family call.
//!
//! P0 ships the substrate + queries + rebuild/stats plumbing. The migrations
//! that fill the table (structural lift, provenance, stance, …) are P1+.
//!
//! The traversal methods (`add_edge`, `edges_out`, `neighbors`, …) are consumed
//! by P1+ and tests ahead of a P0 CLI caller; the module-level `allow(dead_code)`
//! keeps the warning-free bar until then.
#![allow(dead_code)]

use uuid::Uuid;

use crate::error::{Error, Result};
use crate::store::Store;

// Re-export the domain types so callers use `store::graph::Edge` etc. (P1+
// migrations reach for `ExternRef`/`Registry` via `crate::storage::edge_store`
// directly until they surface here.)
pub use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EndpointRef};

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

    /// Rebuild the derived-cache edges. Drops every `Derived`/`Imported` edge
    /// (source-of-truth `Authorial`/`Structural`/`Promoted` edges are kept) and
    /// re-derives them from the current node state.
    ///
    /// P0: nothing is derived yet, so this only clears the cache and returns the
    /// number of edges dropped. P1+ each register a re-derivation step here.
    pub fn graph_rebuild(&self) -> Result<usize> {
        let mut dropped = 0;
        dropped += self
            .raw()
            .delete_edges_by_origin(EdgeOrigin::Derived)
            .map_err(map_edge_err)?;
        dropped += self
            .raw()
            .delete_edges_by_origin(EdgeOrigin::Imported)
            .map_err(map_edge_err)?;
        // P1+ : re-derive Structural edges from node fields, Imported from
        // WordNet/citation registries, etc. — registered here as they land.
        Ok(dropped)
    }

    /// Edge-store integrity check (`"ok"` when healthy).
    pub fn graph_integrity_check(&self) -> Result<String> {
        self.raw().edges_integrity_check().map_err(map_edge_err)
    }
}

fn map_edge_err(e: anyhow::Error) -> Error {
    Error::Store(e.to_string())
}
