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

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::Path;

use serde::Deserialize;
use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::index_locorum::{self, LocusScheme};
use crate::sources::BibEntry;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};
use crate::store::Store;
use crate::wordnet::{self, SenseNode, WordNet};

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

/// The outcome of a [`Store::rebuild_lexical`].
#[derive(Debug, Clone, Copy)]
pub struct LexicalRebuild {
    /// Whether a WordNet was installed for the project language.
    pub installed: bool,
    /// Prior lexical edges cleared.
    pub cleared: usize,
    /// Lexical edges imported.
    pub added: usize,
}

/// The lexical-bridge edge kinds (cleared + rebuilt together by the WordNet
/// import; `origin = Imported`, so `graph_rebuild` leaves them alone).
const LEXICAL_KINDS: &[EdgeKind] = &[
    EdgeKind::Mentions,
    EdgeKind::Hypernym,
    EdgeKind::Hyponym,
    EdgeKind::Antonym,
    EdgeKind::Translates,
];

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

    /// Rebuild the derivable edges: drop the `Structural` projection and
    /// recompute it from the current node state + sidecars + manuscript
    /// citations.
    ///
    /// Only `Structural` is cleared — it is the set fully re-derived here.
    /// `Derived` isn't produced yet; `Imported` (WordNet / citation registries)
    /// is external reference data that isn't re-derivable offline, so it is
    /// preserved. User `Authorial` / `Promoted` / `Judged` edges are always kept.
    ///
    /// Idempotent: running it twice on unchanged inputs yields the same edge set
    /// (edge ids differ — freshly minted — but the endpoints/kinds match).
    pub fn graph_rebuild(&self, cfg: &Config) -> Result<GraphRebuild> {
        let cleared = self
            .raw()
            .delete_edges_by_origin(EdgeOrigin::Structural)
            .map_err(map_edge_err)?;

        let hierarchy = Hierarchy::load(self)?;
        let nodes: Vec<Node> = hierarchy.flatten().into_iter().map(|(n, _)| n.clone()).collect();
        let ids: HashSet<Uuid> = nodes.iter().map(|n| n.id).collect();

        // P1 — structural lift from node fields.
        let mut derived = derive_structural_edges(&nodes);
        // P2 — provenance + verdict sidecars → SourcedFrom / GradedAs.
        derived.extend(derive_sidecar_edges(self.project_root(), &ids));
        // P4 — @key[locus] citations → CitesLocus.
        derived.extend(self.gather_locus_edges(&hierarchy, cfg));

        let added = derived.len();
        if !derived.is_empty() {
            self.raw().add_edges(&derived).map_err(map_edge_err)?;
        }

        Ok(GraphRebuild { cleared, added })
    }

    /// Harvest `@key[locus]` citations from the manuscript's user-book paragraphs
    /// and turn them into `CitesLocus` edges, canonicalizing each locus under its
    /// source's reference scheme exactly as the Index Locorum does. I/O (reads
    /// paragraph files + the Sources book); the pure mapping is
    /// [`derive_locus_edges`].
    fn gather_locus_edges(&self, h: &Hierarchy, cfg: &Config) -> Vec<Edge> {
        let root = self.project_root();
        let mut cites: Vec<(Uuid, String, String)> = Vec::new();
        for book in h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        {
            for id in h.collect_subtree(book.id) {
                let Some(n) = h.get(id) else { continue };
                if n.kind != NodeKind::Paragraph {
                    continue;
                }
                let Some(rel) = n.file.as_ref() else { continue };
                let Ok(raw) = std::fs::read_to_string(root.join(rel)) else { continue };
                for (key, locus) in crate::sources::extract_cite_loci(&raw) {
                    if let Some(l) = locus.map(|s| s.trim().to_string()).filter(|s| !s.is_empty()) {
                        cites.push((n.id, key, l));
                    }
                }
            }
        }
        if cites.is_empty() {
            return Vec::new();
        }
        let declared = collect_declared_schemes(root, h);
        let mut keys: Vec<String> = cites.iter().map(|c| c.1.clone()).collect();
        keys.sort();
        keys.dedup();
        let (schemes, _errs) =
            index_locorum::resolve_schemes(&cfg.sources.ref_schemes, &declared, &keys);
        derive_locus_edges(&cites, &schemes)
    }

    /// SEMNET-P4 — a bounded path search between two nodes over the given edge
    /// kinds (e.g. citation chains over `Cites`). Returns the node sequence of
    /// the first path found within `max_hops` edges, or `None`. Hop- and
    /// visit-bounded so a pathological graph can't hang the caller.
    pub fn paths(
        &self,
        from: Uuid,
        to: Uuid,
        kinds: &[EdgeKind],
        max_hops: usize,
    ) -> Result<Option<Vec<Uuid>>> {
        bfs_path(from, to, max_hops, |node| {
            let here = EndpointRef::Node(node);
            let mut out = Vec::new();
            for e in self.raw().edges_around(node, kinds).map_err(map_edge_err)? {
                if let EndpointRef::Node(other) = e.other_endpoint(&here) {
                    out.push(*other);
                }
            }
            Ok(out)
        })
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

    // ── SEMNET-P6: surfacing ───────────────────────────────────────

    /// The edges in the neighbourhood of `seed`: its direct edges, expanded
    /// `radius` rings through node endpoints (extern endpoints are leaves).
    /// Deduped by edge id and hard-capped so a hub node can't blow up the view.
    pub fn subgraph(&self, seed: Uuid, radius: usize, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        collect_subgraph(seed, radius, |node| {
            self.raw().edges_around(node, kinds).map_err(map_edge_err)
        })
    }

    // ── SEMNET-P5: lexical bridge ──────────────────────────────────

    /// (Re)build the lexical bridge from the installed WordNet for the project
    /// language: clear the prior lexical edges, extract the manuscript's content
    /// lemmas, and import the senses they touch (plus one-hop taxonomy + ILI).
    /// A no-op (returns `installed: false`) when no WordNet is installed. The
    /// import is `Imported`, so a plain `graph_rebuild` leaves it intact.
    pub fn rebuild_lexical(&self, cfg: &Config) -> Result<LexicalRebuild> {
        // Resolve the project language to a wordnet code ("english" → "en"),
        // matching the editor's thesaurus chord.
        let lang = crate::ai::prompts::iso_from_long(&cfg.language).to_string();
        let Some(path) = wordnet::index_path(&lang).filter(|p| p.exists()) else {
            return Ok(LexicalRebuild { installed: false, cleared: 0, added: 0 });
        };
        let wn = WordNet::load(&path).map_err(Error::Store)?;

        let hierarchy = Hierarchy::load(self)?;
        let root = self.project_root();
        let mut occ: Vec<(Uuid, String, Vec<SenseNode>)> = Vec::new();
        for book in hierarchy
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        {
            for id in hierarchy.collect_subtree(book.id) {
                let Some(n) = hierarchy.get(id) else { continue };
                if n.kind != NodeKind::Paragraph {
                    continue;
                }
                let Some(rel) = n.file.as_ref() else { continue };
                let Ok(raw) = std::fs::read_to_string(root.join(rel)) else { continue };
                let mut seen: HashSet<String> = HashSet::new();
                for lemma in salient_lemmas(&raw) {
                    if !seen.insert(lemma.clone()) {
                        continue;
                    }
                    let senses = wn.sense_nodes(&lemma);
                    if !senses.is_empty() {
                        occ.push((n.id, lemma, senses));
                    }
                }
            }
        }

        let edges = derive_lexical_edges(&occ, &lang);
        let cleared = self.raw().delete_edges_by_kinds(LEXICAL_KINDS).map_err(map_edge_err)?;
        let added = edges.len();
        if !edges.is_empty() {
            self.raw().add_edges(&edges).map_err(map_edge_err)?;
        }
        Ok(LexicalRebuild { installed: true, cleared, added })
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

/// Bounded breadth-first path search over a node-neighbour lookup. Returns the
/// node sequence of the first path from `from` to `to` within `max_hops` edges,
/// or `None`. Hop- and visit-bounded (a hard visit cap) so a pathological or
/// cyclic graph can't hang. Pure over the `neighbors` closure — unit-testable.
fn bfs_path(
    from: Uuid,
    to: Uuid,
    max_hops: usize,
    mut neighbors: impl FnMut(Uuid) -> Result<Vec<Uuid>>,
) -> Result<Option<Vec<Uuid>>> {
    if from == to {
        return Ok(Some(vec![from]));
    }
    const MAX_VISITS: usize = 20_000;
    let mut visited: HashSet<Uuid> = HashSet::from([from]);
    let mut queue: VecDeque<Vec<Uuid>> = VecDeque::from([vec![from]]);
    let mut budget = MAX_VISITS;
    while let Some(path) = queue.pop_front() {
        // A path of `n` nodes carries `n - 1` edges; extend only while adding one
        // more edge stays within `max_hops`.
        if path.len() > max_hops {
            continue;
        }
        let tail = *path.last().expect("path is non-empty");
        for other in neighbors(tail)? {
            if budget == 0 {
                return Ok(None);
            }
            budget -= 1;
            if other == to {
                let mut p = path.clone();
                p.push(other);
                return Ok(Some(p));
            }
            if visited.insert(other) {
                let mut p = path.clone();
                p.push(other);
                queue.push_back(p);
            }
        }
    }
    Ok(None)
}

// ── SEMNET-P4: bibliographic (CitesLocus) ────────────────────────────

/// Sources-book cite-key → declared reference-scheme name (the scheme mapping
/// the locus canonicalizer needs; a minimal mirror of the Index Locorum CLI's
/// collector).
fn collect_declared_schemes(root: &Path, h: &Hierarchy) -> HashMap<String, String> {
    let mut declared = HashMap::new();
    let Some(sources) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SOURCES)
    }) else {
        return declared;
    };
    for id in h.collect_subtree(sources.id) {
        let Some(n) = h.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &n.file else { continue };
        let Ok(raw) = std::fs::read_to_string(root.join(rel)) else { continue };
        let body = crate::typst_prose::strip_leading_heading(&raw);
        if let Some(e) = BibEntry::from_hjson(&body) {
            if let Some(scheme) = e.scheme.as_ref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                declared.insert(e.key.clone(), scheme.to_string());
            }
        }
    }
    declared
}

/// SEMNET-P4 — `CitesLocus` edges: each `@key[locus]` citation → an edge from
/// the citing node to `Extern::Locus { scheme, canonical }`. The locus is
/// canonicalized under its source's reference scheme (so `Jn 3.16`,
/// `иоанна 3:16`, `John 3:16` collapse to one endpoint), matching the Index
/// Locorum; the cite key rides `attrs`. Pure.
fn derive_locus_edges(
    cites: &[(Uuid, String, String)],
    schemes: &HashMap<String, LocusScheme>,
) -> Vec<Edge> {
    let mut edges = Vec::new();
    for (node, key, raw) in cites {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        let (scheme, canonical) = match schemes.get(key) {
            Some(s) => (s.name().to_string(), s.canonicalize(raw)),
            None => (String::new(), raw.to_string()),
        };
        edges.push(
            Edge::new(
                EndpointRef::Node(*node),
                EdgeKind::CitesLocus,
                EndpointRef::Extern(ExternRef::Locus { scheme, canonical }),
                EdgeOrigin::Structural,
            )
            .with_attrs(serde_json::json!({ "key": key })),
        );
    }
    edges
}

// ── SEMNET-P5: lexical bridge (WordNet) ──────────────────────────────

/// Distinct content-word candidates from paragraph prose: alphabetic tokens of
/// ≥3 characters, lower-cased (Unicode-aware). A crude "salient lemma" harvest —
/// Typst-markup tokens that aren't real words simply miss in the WordNet lookup
/// and produce no edges (the lazy bound).
fn salient_lemmas(prose: &str) -> Vec<String> {
    prose
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 3)
        .map(|w| w.to_lowercase())
        .collect()
}

/// SEMNET-P5 — the lexical bridge. Per manuscript lemma occurrence (node + lemma
/// + its sense nodes), emit `Mentions` (node → `Sense`, lemma in attrs),
/// `Hypernym`/`Hyponym`/`Antonym` (`Sense` → target `Sense`, one hop), and
/// `Translates` (`Sense` → `Ili`). All `origin = Imported`. Lazy: only the given
/// lemmas' senses + their direct targets appear as endpoints. The sense→sense /
/// sense→ili edges dedup (a synset's relations are the same whichever occurrence
/// surfaced them). Pure — testable with synthetic [`SenseNode`]s.
fn derive_lexical_edges(occurrences: &[(Uuid, String, Vec<SenseNode>)], lang: &str) -> Vec<Edge> {
    let sense = |synset: &str| {
        EndpointRef::Extern(ExternRef::Sense { lang: lang.to_string(), synset: synset.to_string() })
    };
    let mut edges = Vec::new();
    let mut structural: HashSet<(String, EdgeKind, String)> = HashSet::new();
    let mut mentioned: HashSet<(Uuid, String)> = HashSet::new();
    for (node, lemma, senses) in occurrences {
        for sn in senses {
            if mentioned.insert((*node, sn.synset.clone())) {
                edges.push(
                    Edge::new(
                        EndpointRef::Node(*node),
                        EdgeKind::Mentions,
                        sense(&sn.synset),
                        EdgeOrigin::Imported,
                    )
                    .with_attrs(serde_json::json!({ "lemma": lemma })),
                );
            }
            for (kind, targets) in [
                (EdgeKind::Hypernym, &sn.hypernyms),
                (EdgeKind::Hyponym, &sn.hyponyms),
                (EdgeKind::Antonym, &sn.antonyms),
            ] {
                for t in targets {
                    if structural.insert((sn.synset.clone(), kind, t.clone())) {
                        edges.push(Edge::new(sense(&sn.synset), kind, sense(t), EdgeOrigin::Imported));
                    }
                }
            }
            if let Some(ili) = &sn.ili {
                if structural.insert((sn.synset.clone(), EdgeKind::Translates, ili.clone())) {
                    edges.push(Edge::new(
                        sense(&sn.synset),
                        EdgeKind::Translates,
                        EndpointRef::Extern(ExternRef::Ili { id: ili.clone() }),
                        EdgeOrigin::Imported,
                    ));
                }
            }
        }
    }
    edges
}

// ── SEMNET-P6: surfacing (subgraph + neighbourhood view) ─────────────

/// Bounded neighbourhood collection: the edges within `radius` rings of `seed`,
/// expanding only through node endpoints (externs are leaves). Deduped by edge
/// id, hard-capped at 500 edges. Pure over the `neighbors` closure.
fn collect_subgraph(
    seed: Uuid,
    radius: usize,
    mut neighbors: impl FnMut(Uuid) -> Result<Vec<Edge>>,
) -> Result<Vec<Edge>> {
    const MAX_EDGES: usize = 500;
    let mut collected: Vec<Edge> = Vec::new();
    let mut seen_edges: HashSet<Uuid> = HashSet::new();
    let mut visited: HashSet<Uuid> = HashSet::from([seed]);
    let mut frontier: Vec<Uuid> = vec![seed];
    for _ in 0..radius.max(1) {
        let mut next: Vec<Uuid> = Vec::new();
        for &node in &frontier {
            for e in neighbors(node)? {
                if seen_edges.insert(e.id) {
                    collected.push(e.clone());
                    if collected.len() >= MAX_EDGES {
                        return Ok(collected);
                    }
                }
                if let EndpointRef::Node(other) = e.other_endpoint(&EndpointRef::Node(node)) {
                    if visited.insert(*other) {
                        next.push(*other);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        frontier = next;
    }
    Ok(collected)
}

/// Render a node's neighbourhood as a terminal-native monospace tree, grouped by
/// edge kind with per-group direction arrows (`→` out, `←` in, `⇄` symmetric),
/// the far endpoint, and any reason. Large groups are truncated with a `… +N
/// more`. `label` resolves an endpoint to a human string (node title / extern
/// description). Pure.
pub fn render_neighbourhood(
    focus: Uuid,
    edges: &[Edge],
    label: impl Fn(&EndpointRef) -> String,
) -> String {
    const PER_GROUP: usize = 8;
    let here = EndpointRef::Node(focus);
    let mut out = format!("◆ {}\n", label(&here));
    if edges.is_empty() {
        out.push_str("  (no edges — run `graph rebuild` / `graph lexical` to populate)\n");
        return out;
    }
    let mut order: Vec<EdgeKind> = Vec::new();
    let mut groups: HashMap<EdgeKind, Vec<&Edge>> = HashMap::new();
    for e in edges {
        if !groups.contains_key(&e.kind) {
            order.push(e.kind);
        }
        groups.entry(e.kind).or_default().push(e);
    }
    for kind in &order {
        let g = &groups[kind];
        out.push_str(&format!("├─ {} ({})\n", kind.as_str(), g.len()));
        for e in g.iter().take(PER_GROUP) {
            let arrow = if !e.directed {
                "⇄"
            } else if e.src == here {
                "→"
            } else {
                "←"
            };
            let other = label(e.other_endpoint(&here));
            let reason = e
                .reason
                .as_deref()
                .filter(|r| !r.is_empty())
                .map(|r| format!(" — {r}"))
                .unwrap_or_default();
            out.push_str(&format!("│    {arrow} {other}{reason}\n"));
        }
        if g.len() > PER_GROUP {
            out.push_str(&format!("│    … +{} more\n", g.len() - PER_GROUP));
        }
    }
    out
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

    // ── P4: bibliographic ──────────────────────────────────────────

    fn bible_scheme() -> std::collections::HashMap<String, LocusScheme> {
        // Resolve the built-in `bible` scheme under the key "bible" (a cite of
        // `@bible[...]` with no declared scheme resolves the key as the name).
        let (schemes, _) = index_locorum::resolve_schemes(
            &crate::config::Config::default().sources.ref_schemes,
            &std::collections::HashMap::new(),
            &["bible".to_string()],
        );
        schemes
    }

    #[test]
    fn locus_edges_regroup_variant_scripture_spellings() {
        // The multilingual faithfulness test: `Jn 3.16`, `иоанна 3:16`, and
        // `John 3:16` all canonicalize to one endpoint, so three citing nodes
        // point at the SAME locus (as the Index Locorum groups them).
        let schemes = bible_scheme();
        let (n1, n2, n3) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let cites = vec![
            (n1, "bible".to_string(), "Joh 3.16".to_string()), // prefix + dot separator
            (n2, "bible".to_string(), "Иоанна 3:16".to_string()), // Russian Synodal
            (n3, "bible".to_string(), "John 3:16".to_string()), // English
        ];
        let edges = derive_locus_edges(&cites, &schemes);
        assert_eq!(edges.len(), 3);
        let dsts: std::collections::HashSet<_> = edges.iter().map(|e| e.dst.clone()).collect();
        assert_eq!(dsts.len(), 1, "all three variants collapse to one locus endpoint");
        let want = EndpointRef::Extern(ExternRef::Locus { scheme: "bible".into(), canonical: "John 3:16".into() });
        assert!(dsts.contains(&want), "canonical endpoint is {want:?}, got {dsts:?}");
        assert_eq!(edges[0].kind, EdgeKind::CitesLocus);
        assert_eq!(edges[0].origin, EdgeOrigin::Structural);
        assert_eq!(edges[0].attrs["key"], "bible");
    }

    #[test]
    fn locus_without_scheme_passes_through_verbatim() {
        let node = Uuid::now_v7();
        let cites = vec![(node, "smith".to_string(), "§4.2".to_string())];
        let edges = derive_locus_edges(&cites, &std::collections::HashMap::new());
        assert_eq!(edges.len(), 1);
        assert_eq!(
            edges[0].dst,
            EndpointRef::Extern(ExternRef::Locus { scheme: String::new(), canonical: "§4.2".into() })
        );
    }

    // ── P5: lexical bridge ─────────────────────────────────────────

    fn sn(synset: &str, ili: Option<&str>, hyper: &[&str], hypo: &[&str], anto: &[&str]) -> SenseNode {
        SenseNode {
            synset: synset.into(),
            ili: ili.map(|s| s.into()),
            hypernyms: hyper.iter().map(|s| s.to_string()).collect(),
            hyponyms: hypo.iter().map(|s| s.to_string()).collect(),
            antonyms: anto.iter().map(|s| s.to_string()).collect(),
        }
    }

    // ── P6: surfacing ──────────────────────────────────────────────

    #[test]
    fn subgraph_is_radius_bounded() {
        // Chain a → b → c → d (LinksTo).
        let ids: Vec<Uuid> = (0..4).map(|_| Uuid::now_v7()).collect();
        let mk = |a: Uuid, b: Uuid| {
            Edge::new(EndpointRef::Node(a), EdgeKind::LinksTo, EndpointRef::Node(b), EdgeOrigin::Structural)
        };
        let edges = vec![mk(ids[0], ids[1]), mk(ids[1], ids[2]), mk(ids[2], ids[3])];
        let nb = |n: Uuid| {
            Ok(edges
                .iter()
                .filter(|e| e.src == EndpointRef::Node(n) || e.dst == EndpointRef::Node(n))
                .cloned()
                .collect::<Vec<_>>())
        };
        // radius 1: only a's own edges (a→b).
        assert_eq!(collect_subgraph(ids[0], 1, nb).unwrap().len(), 1);
        // radius 2: a→b + b→c.
        assert_eq!(collect_subgraph(ids[0], 2, nb).unwrap().len(), 2);
        // radius 3: the whole chain.
        assert_eq!(collect_subgraph(ids[0], 3, nb).unwrap().len(), 3);
    }

    #[test]
    fn neighbourhood_render_groups_caps_and_shows_direction() {
        let focus = Uuid::now_v7();
        let here = EndpointRef::Node(focus);
        let mut edges = vec![
            // an incoming link, a symmetric contradiction with a reason
            Edge::new(EndpointRef::Node(Uuid::now_v7()), EdgeKind::LinksTo, here.clone(), EdgeOrigin::Structural),
            Edge::new(here.clone(), EdgeKind::Contradicts, EndpointRef::Extern(ExternRef::Evidence { label: "fact: X".into() }), EdgeOrigin::Judged)
                .with_reason("opposes §3"),
        ];
        // 10 mentions → the group truncates at 8.
        for i in 0..10 {
            edges.push(Edge::new(here.clone(), EdgeKind::Mentions, EndpointRef::Extern(ExternRef::Sense { lang: "en".into(), synset: format!("s{i}") }), EdgeOrigin::Imported));
        }
        let label = |ep: &EndpointRef| match ep {
            EndpointRef::Node(u) if *u == focus => "THIS".to_string(),
            EndpointRef::Node(_) => "other-para".to_string(),
            EndpointRef::Extern(_) => {
                let (k, r) = ep.as_columns();
                format!("{k} {r}")
            }
        };
        let s = render_neighbourhood(focus, &edges, label);
        assert!(s.starts_with("◆ THIS\n"));
        assert!(s.contains("├─ contradicts (1)"));
        assert!(s.contains("⇄ evidence fact: X — opposes §3"), "symmetric arrow + reason: {s}");
        assert!(s.contains("← other-para"), "incoming link arrow");
        assert!(s.contains("├─ mentions (10)"));
        assert!(s.contains("… +2 more"), "mentions group capped at 8: {s}");
    }

    #[test]
    fn wordnet_rel_becomes_edge_and_bridge() {
        let node = Uuid::now_v7();
        let occ = vec![(node, "dog".to_string(), vec![sn("s-dog", Some("i-dog"), &["s-animal"], &["s-puppy"], &[])])];
        let edges = derive_lexical_edges(&occ, "en");

        let sense = |s: &str| EndpointRef::Extern(ExternRef::Sense { lang: "en".into(), synset: s.into() });
        // Mentions bridge, with the lemma.
        let mentions = edges.iter().find(|e| e.kind == EdgeKind::Mentions).unwrap();
        assert_eq!(mentions.src, EndpointRef::Node(node));
        assert_eq!(mentions.dst, sense("s-dog"));
        assert_eq!(mentions.attrs["lemma"], "dog");
        assert_eq!(mentions.origin, EdgeOrigin::Imported);
        // Taxonomy: sense → target sense.
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Hypernym && e.src == sense("s-dog") && e.dst == sense("s-animal")));
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Hyponym && e.dst == sense("s-puppy")));
        // Translates: sense → ILI.
        assert!(edges.iter().any(|e| e.kind == EdgeKind::Translates
            && e.dst == EndpointRef::Extern(ExternRef::Ili { id: "i-dog".into() })));
    }

    #[test]
    fn lexical_import_is_bounded_to_touched_senses() {
        // Only the given lemma's sense + its direct targets appear — no
        // unreferenced synset leaks in.
        let node = Uuid::now_v7();
        let occ = vec![(node, "cat".to_string(), vec![sn("s-cat", None, &["s-feline"], &[], &[])])];
        let edges = derive_lexical_edges(&occ, "en");
        let mut synsets: std::collections::HashSet<String> = std::collections::HashSet::new();
        for e in &edges {
            for ep in [&e.src, &e.dst] {
                if let EndpointRef::Extern(ExternRef::Sense { synset, .. }) = ep {
                    synsets.insert(synset.clone());
                }
            }
        }
        assert_eq!(synsets, std::collections::HashSet::from(["s-cat".to_string(), "s-feline".to_string()]));
        // No Translates edge when the sense carries no ILI.
        assert!(!edges.iter().any(|e| e.kind == EdgeKind::Translates));
    }

    #[test]
    fn cross_lingual_senses_share_an_ili_bucket() {
        // A Russian and a German sense of the same concept both Translate to the
        // same ILI; the reverse index groups them → the cross-lingual pivot.
        let (n_ru, n_de) = (Uuid::now_v7(), Uuid::now_v7());
        let mut edges = derive_lexical_edges(&[(n_ru, "кошка".into(), vec![sn("ru-1", Some("i-cat"), &[], &[], &[])])], "ru");
        edges.extend(derive_lexical_edges(&[(n_de, "Katze".into(), vec![sn("de-1", Some("i-cat"), &[], &[], &[])])], "de"));

        let dir = TempDir::new().unwrap();
        let store = EdgeStore::new(dir.path().join("edges.db"), 2).unwrap();
        store.insert_batch(&edges).unwrap();

        let ili = EndpointRef::Extern(ExternRef::Ili { id: "i-cat".into() });
        let into_ili = store.incoming(&ili, &[EdgeKind::Translates]).unwrap();
        assert_eq!(into_ili.len(), 2, "ru + de senses both translate to the shared ILI");
        let srcs: std::collections::HashSet<_> = into_ili.iter().map(|e| e.src.clone()).collect();
        assert!(srcs.contains(&EndpointRef::Extern(ExternRef::Sense { lang: "ru".into(), synset: "ru-1".into() })));
        assert!(srcs.contains(&EndpointRef::Extern(ExternRef::Sense { lang: "de".into(), synset: "de-1".into() })));
    }

    #[test]
    fn salient_lemmas_are_lowercased_content_words() {
        let got = salient_lemmas("The Quick brown fox, a #emph[test]! И кошка.");
        assert!(got.contains(&"quick".to_string()));
        assert!(got.contains(&"brown".to_string()));
        assert!(got.contains(&"кошка".to_string()), "Unicode content words kept");
        assert!(!got.iter().any(|w| w == "a"), "short tokens dropped");
    }

    #[test]
    fn citation_path_is_hop_bounded() {
        // A chain a → b → c → d → e via an in-memory adjacency.
        let ids: Vec<Uuid> = (0..5).map(|_| Uuid::now_v7()).collect();
        let mut adj: BTreeMap<Uuid, Vec<Uuid>> = BTreeMap::new();
        for w in ids.windows(2) {
            adj.entry(w[0]).or_default().push(w[1]);
            adj.entry(w[1]).or_default().push(w[0]); // symmetric (Cites is directed, but test both)
        }
        let nb = |n: Uuid| Ok(adj.get(&n).cloned().unwrap_or_default());

        // 4 hops reaches e; 3 hops does not.
        let p = bfs_path(ids[0], ids[4], 4, nb).unwrap().unwrap();
        assert_eq!(p, ids, "full chain within 4 hops");
        assert_eq!(p.len() - 1, 4, "exactly 4 hops");
        assert!(bfs_path(ids[0], ids[4], 3, nb).unwrap().is_none(), "5 nodes need 4 hops; 3 is too few");
        assert_eq!(bfs_path(ids[0], ids[0], 0, nb).unwrap(), Some(vec![ids[0]]), "self is zero hops");
    }
}
