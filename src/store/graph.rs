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

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use uuid::Uuid;

use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::Store;

// Re-export the domain types so callers use `store::graph::Edge` etc.
pub use crate::storage::edge_store::{Edge, EdgeKind, EdgeOrigin, EndpointRef};
use crate::storage::edge_store::{ExternRef, Registry};

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

        let hierarchy = Hierarchy::load(self)?;
        let nodes: Vec<Node> = hierarchy.flatten().into_iter().map(|(n, _)| n.clone()).collect();
        let ids: HashSet<Uuid> = nodes.iter().map(|n| n.id).collect();

        // P1 — structural lift from node fields.
        let mut derived = derive_structural_edges(&nodes);
        // P2 — provenance + verdict sidecars → SourcedFrom / GradedAs.
        derived.extend(derive_sidecar_edges(self.project_root(), &ids));

        let added = derived.len();
        if !derived.is_empty() {
            self.raw().add_edges(&derived).map_err(map_edge_err)?;
        }

        Ok(GraphRebuild { cleared, added })
    }

    /// Edge-store integrity check (`"ok"` when healthy).
    pub fn graph_integrity_check(&self) -> Result<String> {
        self.raw().edges_integrity_check().map_err(map_edge_err)
    }

    // ── SEMNET-P3: stance persistence ──────────────────────────────

    /// Replace a paragraph's `Judged` confront edges: drop the existing Judged
    /// stance edges leaving `node` (a re-confront supersedes the prior pass),
    /// then add the fresh ones. `Promoted` edges the user accepted are kept.
    pub fn replace_confront_edges(&self, node: Uuid, edges: &[Edge]) -> Result<()> {
        let existing = self.raw().edges_out(node, STANCE_KINDS).map_err(map_edge_err)?;
        for e in existing {
            if e.origin == EdgeOrigin::Judged {
                self.raw().delete_edge(e.id).map_err(map_edge_err)?;
            }
        }
        if !edges.is_empty() {
            self.raw().add_edges(edges).map_err(map_edge_err)?;
        }
        Ok(())
    }

    /// Promote a `Judged` stance edge to `Promoted` — a user-accepted judgement,
    /// kept across rebuilds. Returns whether an edge changed.
    pub fn promote_edge(&self, id: Uuid) -> Result<bool> {
        Ok(self.raw().set_edge_origin(id, EdgeOrigin::Promoted).map_err(map_edge_err)? > 0)
    }

    /// Dismiss (delete) a stance edge by id.
    pub fn dismiss_edge(&self, id: Uuid) -> Result<()> {
        self.raw().delete_edge(id).map_err(map_edge_err)
    }

    /// The stance edges against a node — everything `Contradicts` / `InTension`
    /// touching it, in either direction.
    pub fn contradicting(&self, node: Uuid) -> Result<Vec<Edge>> {
        self.raw()
            .edges_around(node, &[EdgeKind::Contradicts, EdgeKind::InTension])
            .map_err(map_edge_err)
    }
}

/// The confront/relate stance edge kinds (a re-confront replaces the Judged
/// ones of these kinds leaving a paragraph).
const STANCE_KINDS: &[EdgeKind] = &[
    EdgeKind::Contradicts,
    EdgeKind::InTension,
    EdgeKind::Qualifies,
    EdgeKind::Agrees,
];

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

// ── SEMNET-P2: provenance + verdict sidecars ─────────────────────────
//
// The research module owns the `.inkhaven/fact-sources.json` +
// `fact-verdicts.json` sidecars (its types are `pub(super)`), so we read the
// stable on-disk JSON contract directly rather than couple to those types.
// This phase derives the edges from the sidecars (a projection, like the P1
// structural lift) — the sidecar stays the write path; a later cutover makes it
// a read-through cache of the graph.

/// One `fact-sources.json` source row (mirror of `research::provenance::SourceRecord`).
#[derive(Debug, Clone, Deserialize)]
struct SidecarSource {
    #[serde(default)]
    origin: String,
    #[serde(default)]
    detail: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    thread: String,
    #[serde(default)]
    created_at: String,
}

#[derive(Debug, Default, Deserialize)]
struct SidecarSourceFile {
    #[serde(default)]
    facts: BTreeMap<String, SidecarSource>,
}

/// One `fact-verdicts.json` verdict row (mirror of `research::verdicts::Verdict`;
/// `level` serialises as the variant name, e.g. `"Inaccurate"`).
#[derive(Debug, Clone, Deserialize)]
struct SidecarVerdict {
    level: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    checked_at: String,
}

#[derive(Debug, Default, Deserialize)]
struct SidecarVerdictFile {
    #[serde(default)]
    facts: BTreeMap<String, SidecarVerdict>,
}

fn read_json_map<T: Default + serde::de::DeserializeOwned>(root: &Path, file: &str) -> T {
    let p = root.join(".inkhaven").join(file);
    std::fs::read_to_string(p)
        .ok()
        .and_then(|s| serde_json::from_str::<T>(&s).ok())
        .unwrap_or_default()
}

/// Read both sidecars and derive their edges (empty when the files are absent).
fn derive_sidecar_edges(root: &Path, node_ids: &HashSet<Uuid>) -> Vec<Edge> {
    let sources: SidecarSourceFile = read_json_map(root, "fact-sources.json");
    let verdicts: SidecarVerdictFile = read_json_map(root, "fact-verdicts.json");
    let mut edges = derive_provenance_edges(node_ids, &sources.facts);
    edges.extend(derive_verdict_edges(node_ids, &verdicts.facts));
    edges
}

/// Map a provenance `origin` string to a citation registry (unknown → `Other`).
fn registry_for_origin(origin: &str) -> Registry {
    match origin {
        "openalex" => Registry::OpenAlex,
        "arxiv" => Registry::Arxiv,
        "wikidata" => Registry::Wikidata,
        "geonames" => Registry::Geonames,
        _ => Registry::Other,
    }
}

/// SEMNET-P2 — `SourcedFrom` edges: fact-node → its source. The source endpoint
/// is `Extern::Work { registry(from origin), id }` (id = detail, or the origin
/// name when detail is empty, so distinct origins don't collapse). The full
/// record rides `attrs` for a lossless round-trip. Sidecar rows whose fact is
/// no longer a node are skipped. Pure (no I/O).
fn derive_provenance_edges(
    node_ids: &HashSet<Uuid>,
    sources: &BTreeMap<String, SidecarSource>,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (fact_id, rec) in sources {
        let Ok(fact) = Uuid::parse_str(fact_id) else { continue };
        if !node_ids.contains(&fact) {
            continue;
        }
        let id = if rec.detail.is_empty() { rec.origin.clone() } else { rec.detail.clone() };
        let dst = EndpointRef::Extern(ExternRef::Work {
            registry: registry_for_origin(&rec.origin),
            id,
        });
        edges.push(
            Edge::new(EndpointRef::Node(fact), EdgeKind::SourcedFrom, dst, EdgeOrigin::Structural)
                .with_attrs(serde_json::json!({
                    "origin": rec.origin,
                    "detail": rec.detail,
                    "query": rec.query,
                    "thread": rec.thread,
                    "created_at": rec.created_at,
                })),
        );
    }
    edges
}

/// SEMNET-P2 — `GradedAs` edges: fact-node → a verdict-grade bucket
/// (`Extern::Grade { level }`, lowercased), with `reason`/`checked_at` in
/// `attrs`. Lets facts be grouped by grade via the reverse index. Pure.
fn derive_verdict_edges(
    node_ids: &HashSet<Uuid>,
    verdicts: &BTreeMap<String, SidecarVerdict>,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (fact_id, v) in verdicts {
        let Ok(fact) = Uuid::parse_str(fact_id) else { continue };
        if !node_ids.contains(&fact) {
            continue;
        }
        let dst = EndpointRef::Extern(ExternRef::Grade { level: v.level.to_lowercase() });
        edges.push(
            Edge::new(EndpointRef::Node(fact), EdgeKind::GradedAs, dst, EdgeOrigin::Structural)
                .with_attrs(serde_json::json!({
                    "level": v.level,
                    "reason": v.reason,
                    "checked_at": v.checked_at,
                })),
        );
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

    // ── P2: provenance + verdicts ──────────────────────────────────

    fn src_rec(origin: &str, detail: &str) -> SidecarSource {
        SidecarSource {
            origin: origin.into(),
            detail: detail.into(),
            query: "q".into(),
            thread: "t".into(),
            created_at: "2026-07-01T10:00:00Z".into(),
        }
    }

    #[test]
    fn provenance_becomes_sourced_from_with_registry() {
        let fact = Uuid::now_v7();
        let ids = HashSet::from([fact]);
        let mut sources = BTreeMap::new();
        sources.insert(fact.to_string(), src_rec("arxiv", "2401.00001"));
        let edges = derive_provenance_edges(&ids, &sources);
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert_eq!(e.kind, EdgeKind::SourcedFrom);
        assert_eq!(e.src, EndpointRef::Node(fact));
        assert_eq!(
            e.dst,
            EndpointRef::Extern(ExternRef::Work { registry: Registry::Arxiv, id: "2401.00001".into() })
        );
        assert_eq!(e.attrs["origin"], "arxiv");
        assert_eq!(e.attrs["query"], "q");
        assert_eq!(e.origin, EdgeOrigin::Structural);
    }

    #[test]
    fn provenance_roundtrips_losslessly_across_the_origin_vocab() {
        // Every origin the sidecar can carry, plus a Cyrillic detail (multilingual).
        let origins = [
            ("model", ""),
            ("manual", ""),
            ("promoted", "notes/idea"),
            ("web", "https://example.org"),
            ("document", "sources/paper.pdf"),
            ("archive", "ia:xyz"),
            ("wikisource", "Page"),
            ("wikidata", "Q42"),
            ("geonames", "524901"),
            ("openalex", "W123"),
            ("arxiv", "2401.00001"),
            ("computed", "sum"),
            ("simulation", "Мир·тик-7"), // Cyrillic detail
        ];
        let mut ids = HashSet::new();
        let mut sources = BTreeMap::new();
        let mut want: BTreeMap<String, SidecarSource> = BTreeMap::new();
        for (origin, detail) in origins {
            let fact = Uuid::now_v7();
            ids.insert(fact);
            let rec = src_rec(origin, detail);
            sources.insert(fact.to_string(), rec.clone());
            want.insert(fact.to_string(), rec);
        }
        let edges = derive_provenance_edges(&ids, &sources);
        assert_eq!(edges.len(), origins.len());
        // Reconstruct the sidecar from the edges (src fact-id + attrs) and compare.
        let mut got: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for e in &edges {
            let EndpointRef::Node(fact) = &e.src else { panic!("src must be a node") };
            got.insert(fact.to_string(), e.attrs.clone());
        }
        for (fid, rec) in &want {
            let a = &got[fid];
            assert_eq!(a["origin"], rec.origin);
            assert_eq!(a["detail"], rec.detail, "detail (incl. Cyrillic) must survive");
            assert_eq!(a["query"], rec.query);
            assert_eq!(a["thread"], rec.thread);
            assert_eq!(a["created_at"], rec.created_at);
        }
    }

    #[test]
    fn verdict_becomes_graded_as_with_level_and_reason() {
        let fact = Uuid::now_v7();
        let ids = HashSet::from([fact]);
        let mut verdicts = BTreeMap::new();
        verdicts.insert(
            fact.to_string(),
            SidecarVerdict { level: "Inaccurate".into(), reason: "противоречит §3".into(), checked_at: "2026-07-02T09:00:00Z".into() },
        );
        let edges = derive_verdict_edges(&ids, &verdicts);
        assert_eq!(edges.len(), 1);
        let e = &edges[0];
        assert_eq!(e.kind, EdgeKind::GradedAs);
        assert_eq!(e.src, EndpointRef::Node(fact));
        assert_eq!(e.dst, EndpointRef::Extern(ExternRef::Grade { level: "inaccurate".into() }));
        assert_eq!(e.attrs["level"], "Inaccurate");
        assert_eq!(e.attrs["reason"], "противоречит §3");
    }

    #[test]
    fn sidecar_rows_for_missing_nodes_are_skipped() {
        // A source/verdict whose fact node no longer exists produces no edge.
        let live = Uuid::now_v7();
        let ghost = Uuid::now_v7();
        let ids = HashSet::from([live]);
        let mut sources = BTreeMap::new();
        sources.insert(ghost.to_string(), src_rec("model", ""));
        sources.insert("not-a-uuid".to_string(), src_rec("model", ""));
        assert!(derive_provenance_edges(&ids, &sources).is_empty());

        let mut verdicts = BTreeMap::new();
        verdicts.insert(
            ghost.to_string(),
            SidecarVerdict { level: "Dubious".into(), reason: String::new(), checked_at: String::new() },
        );
        assert!(derive_verdict_edges(&ids, &verdicts).is_empty());
    }

    #[test]
    fn graded_facts_group_by_grade_via_reverse_index() {
        // "All inaccurate facts" = incoming edges on the Grade endpoint.
        let (f1, f2, f3) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let ids = HashSet::from([f1, f2, f3]);
        let mut verdicts = BTreeMap::new();
        let mk = |lvl: &str| SidecarVerdict { level: lvl.into(), reason: String::new(), checked_at: String::new() };
        verdicts.insert(f1.to_string(), mk("Inaccurate"));
        verdicts.insert(f2.to_string(), mk("Inaccurate"));
        verdicts.insert(f3.to_string(), mk("Accurate"));
        let edges = derive_verdict_edges(&ids, &verdicts);

        let dir = TempDir::new().unwrap();
        let store = EdgeStore::new(dir.path().join("edges.db"), 2).unwrap();
        store.insert_batch(&edges).unwrap();

        let inaccurate = store
            .incoming(&EndpointRef::Extern(ExternRef::Grade { level: "inaccurate".into() }), &[EdgeKind::GradedAs])
            .unwrap();
        assert_eq!(inaccurate.len(), 2, "two facts graded inaccurate group under the bucket");
    }
}
