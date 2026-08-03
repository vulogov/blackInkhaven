//! SEMNET-P0 — the EdgeStore substrate.
//!
//! A typed-edge layer over the UUID nodes the project already stores. One
//! DuckDB `edges` table (its own `edges.db`, beside `metadata.db`/`blobs.db`/
//! `vectors/`), indexed on **both** endpoints so "what points at this?" is a
//! query, not a full scan.
//!
//! An `Edge` is `(src, kind, dst, …)` where each endpoint is an
//! [`EndpointRef`] — either a manuscript [`EndpointRef::Node`] (a UUID) or an
//! addressable non-node [`ExternRef`] (a source, external work, locus, or
//! WordNet sense). The relational shape (typed columns, not a JSON blob) is
//! deliberate: the reverse index is the whole point, and it needs real indexed
//! columns.
//!
//! Durability follows the 1.2.15 bar and the `VectorEngine` doctrine: edges
//! whose [`EdgeOrigin`] is durable ([`EdgeOrigin::is_durable`]) are
//! source-of-truth (atomic DuckDB commits, survive `kill -9`); `Derived` /
//! `Imported` edges are a rebuildable cache. This is P0 — the substrate, CRUD,
//! reverse-index queries, and rebuild/stats plumbing; the migrations that fill
//! the table land in P1+.
//!
//! NOTE: the full CRUD/query surface is exercised by this module's test suite
//! and consumed by the P1+ migrations; only `stats`/`rebuild`/`GC` have a P0
//! CLI caller. The module-level `allow(dead_code)` holds the warning-free bar
//! until P1 wires the traversal verbs — remove it then.
#![allow(dead_code)]

use anyhow::{anyhow, Result};
use duckdb::types::Value as DuckValue;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;
use uuid::Uuid;

use crate::storage::engine::StorageEngine;

/// On-disk schema version for `edges.db`. Bump + migrate when the column shape
/// changes incompatibly (same mechanism as `metadata.db`).
const EDGE_SCHEMA_VERSION: i64 = 1;

/// Unit-separator: joins the sub-fields of an extern endpoint into one stable,
/// index-comparable `*_ref` string. Never appears in UUIDs, registry names,
/// language codes, or the identifiers we address, so a `split_once` round-trips.
const US: char = '\u{1f}';

const EDGE_INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS edges (
        id         TEXT    NOT NULL PRIMARY KEY,
        src_kind   TEXT    NOT NULL,
        src_ref    TEXT    NOT NULL,
        dst_kind   TEXT    NOT NULL,
        dst_ref    TEXT    NOT NULL,
        kind       TEXT    NOT NULL,
        directed   BOOLEAN NOT NULL,
        weight     DOUBLE  NOT NULL,
        reason     TEXT,
        origin     TEXT    NOT NULL,
        attrs      JSON    NOT NULL,
        created_at BIGINT  NOT NULL
    );
    -- The reverse index is the feature: look edges up by either endpoint.
    CREATE INDEX IF NOT EXISTS idx_edges_src  ON edges (src_kind, src_ref);
    CREATE INDEX IF NOT EXISTS idx_edges_dst  ON edges (dst_kind, dst_ref);
    CREATE INDEX IF NOT EXISTS idx_edges_kind ON edges (kind);
    CREATE TABLE IF NOT EXISTS _inkhaven_schema (
        singleton INTEGER NOT NULL PRIMARY KEY,
        version   BIGINT  NOT NULL
    );
";

const EDGE_INSERT_SQL: &str = "INSERT INTO edges \
     (id, src_kind, src_ref, dst_kind, dst_ref, kind, directed, weight, reason, origin, attrs, created_at) \
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CAST(? AS JSON), ?)";

// ── domain types ─────────────────────────────────────────────────────

/// The kind of relation an edge encodes. Language-neutral; grouped by the
/// implicit encoding each will (P1+) replace. Symmetric kinds
/// ([`EdgeKind::is_symmetric`]) are stored `directed = false` and matched from
/// either endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    /// Authorial paragraph→paragraph link (`Node.linked_paragraphs`).
    LinksTo,
    /// Timeline event → character/place (`EventData.characters`/`.places`).
    EventInvolves,
    /// Fact → its source/work (provenance).
    SourcedFrom,
    /// Fact → a verdict value (trust ladder).
    GradedAs,
    /// Symmetric: two claims contradict.
    Contradicts,
    /// Symmetric: two claims are in tension.
    InTension,
    /// Directed: A qualifies B.
    Qualifies,
    /// Symmetric: two claims agree.
    Agrees,
    /// Work → work citation.
    Cites,
    /// Node → primary-source locus.
    CitesLocus,
    /// Lexical: sense → broader sense.
    Hypernym,
    /// Lexical: sense → narrower sense.
    Hyponym,
    /// Symmetric lexical opposite.
    Antonym,
    /// Symmetric lexical synonym.
    Synonym,
    /// Cross-lingual sense equivalence (via ILI).
    Translates,
    /// A node uses a word whose sense is this — the manuscript↔lexicon bridge.
    Mentions,
    /// A book declares a world entity (character / symbol / motif / tension).
    Declares,
    /// Symmetric embedding-similarity (derived cache).
    SimilarTo,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::LinksTo => "links_to",
            EdgeKind::EventInvolves => "event_involves",
            EdgeKind::SourcedFrom => "sourced_from",
            EdgeKind::GradedAs => "graded_as",
            EdgeKind::Contradicts => "contradicts",
            EdgeKind::InTension => "in_tension",
            EdgeKind::Qualifies => "qualifies",
            EdgeKind::Agrees => "agrees",
            EdgeKind::Cites => "cites",
            EdgeKind::CitesLocus => "cites_locus",
            EdgeKind::Hypernym => "hypernym",
            EdgeKind::Hyponym => "hyponym",
            EdgeKind::Antonym => "antonym",
            EdgeKind::Synonym => "synonym",
            EdgeKind::Translates => "translates",
            EdgeKind::Mentions => "mentions",
            EdgeKind::Declares => "declares",
            EdgeKind::SimilarTo => "similar_to",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "links_to" => EdgeKind::LinksTo,
            "event_involves" => EdgeKind::EventInvolves,
            "sourced_from" => EdgeKind::SourcedFrom,
            "graded_as" => EdgeKind::GradedAs,
            "contradicts" => EdgeKind::Contradicts,
            "in_tension" => EdgeKind::InTension,
            "qualifies" => EdgeKind::Qualifies,
            "agrees" => EdgeKind::Agrees,
            "cites" => EdgeKind::Cites,
            "cites_locus" => EdgeKind::CitesLocus,
            "hypernym" => EdgeKind::Hypernym,
            "hyponym" => EdgeKind::Hyponym,
            "antonym" => EdgeKind::Antonym,
            "synonym" => EdgeKind::Synonym,
            "translates" => EdgeKind::Translates,
            "mentions" => EdgeKind::Mentions,
            "declares" => EdgeKind::Declares,
            "similar_to" => EdgeKind::SimilarTo,
            _ => return None,
        })
    }

    /// Symmetric kinds are stored `directed = false` and matched from either
    /// endpoint.
    pub fn is_symmetric(&self) -> bool {
        matches!(
            self,
            EdgeKind::Contradicts
                | EdgeKind::InTension
                | EdgeKind::Agrees
                | EdgeKind::Antonym
                | EdgeKind::Synonym
                | EdgeKind::SimilarTo
        )
    }

    /// Stance "against" (mirrors `research::contradiction::Stance::is_against`).
    pub fn is_against(&self) -> bool {
        matches!(self, EdgeKind::Contradicts | EdgeKind::InTension)
    }

    /// Stance "support".
    pub fn is_support(&self) -> bool {
        matches!(self, EdgeKind::Agrees | EdgeKind::Qualifies)
    }
}

/// How an edge came to exist — which also encodes its trust and durability.
/// Durable origins are source-of-truth; the rest are a rebuildable cache.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeOrigin {
    /// The user asserted it — highest trust, never GC'd.
    Authorial,
    /// Derived from durable node fields on migration (still source-of-truth).
    Structural,
    /// A judged relation the user accepted.
    Promoted,
    /// An LLM-judged relation, advisory until promoted.
    Judged,
    /// Recomputable (e.g. `SimilarTo`) — a rebuildable cache.
    Derived,
    /// Reference data bridged in (WordNet / citation registries).
    Imported,
}

impl EdgeOrigin {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeOrigin::Authorial => "authorial",
            EdgeOrigin::Structural => "structural",
            EdgeOrigin::Promoted => "promoted",
            EdgeOrigin::Judged => "judged",
            EdgeOrigin::Derived => "derived",
            EdgeOrigin::Imported => "imported",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "authorial" => EdgeOrigin::Authorial,
            "structural" => EdgeOrigin::Structural,
            "promoted" => EdgeOrigin::Promoted,
            "judged" => EdgeOrigin::Judged,
            "derived" => EdgeOrigin::Derived,
            "imported" => EdgeOrigin::Imported,
            _ => return None,
        })
    }

    /// Source-of-truth edges survive `kill -9`; the others are rebuildable and
    /// may be dropped by `graph rebuild`.
    pub fn is_durable(&self) -> bool {
        matches!(
            self,
            EdgeOrigin::Authorial | EdgeOrigin::Structural | EdgeOrigin::Promoted
        )
    }
}

/// The registry an external [`ExternRef::Work`] id belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Registry {
    OpenAlex,
    Arxiv,
    Wikidata,
    Geonames,
    Other,
}

impl Registry {
    pub fn as_str(&self) -> &'static str {
        match self {
            Registry::OpenAlex => "openalex",
            Registry::Arxiv => "arxiv",
            Registry::Wikidata => "wikidata",
            Registry::Geonames => "geonames",
            Registry::Other => "other",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        Some(match s {
            "openalex" => Registry::OpenAlex,
            "arxiv" => Registry::Arxiv,
            "wikidata" => Registry::Wikidata,
            "geonames" => Registry::Geonames,
            "other" => Registry::Other,
            _ => return None,
        })
    }
}

/// A non-node entity an edge can point at without forcing a full `Node` into
/// existence (a bibliography of 10 000 works must not become 10 000 nodes).
/// Value-addressed; two externs are equal iff their `(kind, ref)` columns are.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ExternRef {
    /// A Sources-book entry / `@cite` key (carries the book node for later
    /// reconciliation into a real node).
    Source { book_node: Uuid, key: String },
    /// An external work id in some registry.
    Work { registry: Registry, id: String },
    /// A canonical primary-source locus.
    Locus { scheme: String, canonical: String },
    /// A WordNet synset/sense in a given language.
    Sense { lang: String, synset: String },
    /// An interlingual index id (cross-lingual pivot).
    Ili { id: String },
    /// A verdict/assessment bucket (e.g. a fact-check grade) — lets facts be
    /// grouped by grade via the reverse index ("all inaccurate facts").
    Grade { level: String },
    /// A labelled piece of evidence in a confront/relate judgement (a fact
    /// breadcrumb or source name) that isn't a resolved node — the far side of
    /// a Judged stance edge until it's reconciled to a node.
    Evidence { label: String },
    /// A declared world entity a book establishes — a character, a symbol, a
    /// motif, a world tension (`kind` = which; `label` = its name). Lets the
    /// book's declared cast / symbol library / open tensions be queried from the
    /// graph.
    Declared { kind: String, label: String },
}

/// Where an edge starts or ends.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EndpointRef {
    /// Any manuscript node (the common case).
    Node(Uuid),
    /// An addressable non-node entity.
    Extern(ExternRef),
}

impl EndpointRef {
    /// The `(kind, ref)` column pair this endpoint is stored + indexed as.
    pub fn as_columns(&self) -> (&'static str, String) {
        match self {
            EndpointRef::Node(u) => ("node", u.to_string()),
            EndpointRef::Extern(x) => match x {
                ExternRef::Source { book_node, key } => ("source", format!("{book_node}{US}{key}")),
                ExternRef::Work { registry, id } => ("work", format!("{}{US}{id}", registry.as_str())),
                ExternRef::Locus { scheme, canonical } => ("locus", format!("{scheme}{US}{canonical}")),
                ExternRef::Sense { lang, synset } => ("sense", format!("{lang}{US}{synset}")),
                ExternRef::Ili { id } => ("ili", id.clone()),
                ExternRef::Grade { level } => ("grade", level.clone()),
                ExternRef::Evidence { label } => ("evidence", label.clone()),
                ExternRef::Declared { kind, label } => ("declared", format!("{kind}{US}{label}")),
            },
        }
    }

    /// Reconstruct an endpoint from its stored columns.
    pub fn from_columns(kind: &str, r: &str) -> Result<Self> {
        let split = |r: &str| -> Result<(String, String)> {
            r.split_once(US)
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .ok_or_else(|| anyhow!("malformed extern endpoint ref (no separator): {r:?}"))
        };
        Ok(match kind {
            "node" => EndpointRef::Node(
                Uuid::parse_str(r).map_err(|e| anyhow!("bad node endpoint uuid {r:?}: {e}"))?,
            ),
            "source" => {
                let (a, b) = split(r)?;
                EndpointRef::Extern(ExternRef::Source {
                    book_node: Uuid::parse_str(&a)
                        .map_err(|e| anyhow!("bad source book_node {a:?}: {e}"))?,
                    key: b,
                })
            }
            "work" => {
                let (a, b) = split(r)?;
                EndpointRef::Extern(ExternRef::Work {
                    registry: Registry::from_str(&a).unwrap_or(Registry::Other),
                    id: b,
                })
            }
            "locus" => {
                let (a, b) = split(r)?;
                EndpointRef::Extern(ExternRef::Locus { scheme: a, canonical: b })
            }
            "sense" => {
                let (a, b) = split(r)?;
                EndpointRef::Extern(ExternRef::Sense { lang: a, synset: b })
            }
            "ili" => EndpointRef::Extern(ExternRef::Ili { id: r.to_string() }),
            "grade" => EndpointRef::Extern(ExternRef::Grade { level: r.to_string() }),
            "evidence" => EndpointRef::Extern(ExternRef::Evidence { label: r.to_string() }),
            "declared" => {
                let (a, b) = split(r)?;
                EndpointRef::Extern(ExternRef::Declared { kind: a, label: b })
            }
            other => return Err(anyhow!("unknown endpoint kind: {other:?}")),
        })
    }

    /// Convenience for the common node endpoint.
    pub fn node(id: Uuid) -> Self {
        EndpointRef::Node(id)
    }
}

/// One typed, directed-or-symmetric relation between two endpoints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    pub id: Uuid,
    pub src: EndpointRef,
    pub dst: EndpointRef,
    pub kind: EdgeKind,
    #[serde(default)]
    pub directed: bool,
    #[serde(default = "default_weight")]
    pub weight: f32,
    #[serde(default)]
    pub reason: Option<String>,
    pub origin: EdgeOrigin,
    #[serde(default)]
    pub attrs: JsonValue,
    #[serde(default)]
    pub created_at: i64,
}

fn default_weight() -> f32 {
    1.0
}

impl Edge {
    /// A fresh edge: new UUIDv7, `weight = 1.0`, `directed` inferred from the
    /// kind's symmetry, empty reason/attrs, stamped now.
    pub fn new(src: EndpointRef, kind: EdgeKind, dst: EndpointRef, origin: EdgeOrigin) -> Self {
        Edge {
            id: Uuid::now_v7(),
            src,
            dst,
            kind,
            directed: !kind.is_symmetric(),
            weight: 1.0,
            reason: None,
            origin,
            attrs: JsonValue::Null,
            created_at: now_unix_secs(),
        }
    }

    pub fn with_reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    pub fn with_weight(mut self, weight: f32) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_attrs(mut self, attrs: JsonValue) -> Self {
        self.attrs = attrs;
        self
    }

    /// Given one endpoint of this edge, return the other side. If `ep` is
    /// neither endpoint, returns `dst` (the forward direction).
    pub fn other_endpoint(&self, ep: &EndpointRef) -> &EndpointRef {
        if &self.src == ep {
            &self.dst
        } else {
            &self.src
        }
    }
}

// ── EdgeStore ────────────────────────────────────────────────────────

/// DuckDB-backed edge table. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct EdgeStore {
    engine: Arc<StorageEngine>,
}

impl EdgeStore {
    pub fn new<P: AsRef<Path>>(path: P, pool_size: u32) -> Result<Self> {
        let engine = StorageEngine::new(path, EDGE_INIT_SQL, pool_size)?;
        let store = Self {
            engine: Arc::new(engine),
        };
        store.ensure_schema_version(EDGE_SCHEMA_VERSION)?;
        Ok(store)
    }

    /// Stamp or verify the on-disk schema version — refuse a store written by a
    /// newer inkhaven rather than silently mishandle its rows (mirrors
    /// `JsonStorage::ensure_schema_version`).
    fn ensure_schema_version(&self, current: i64) -> Result<()> {
        let rows = self
            .engine
            .select_all("SELECT version FROM _inkhaven_schema WHERE singleton = 1")?;
        let on_disk = rows.into_iter().next().and_then(|r| r.into_iter().next());
        let on_disk = match on_disk {
            Some(DuckValue::BigInt(v)) => Some(v),
            Some(DuckValue::Int(v)) => Some(v as i64),
            Some(DuckValue::HugeInt(v)) => Some(v as i64),
            _ => None,
        };
        match on_disk {
            Some(v) if v > current => Err(anyhow!(
                "edge store schema is v{v}, but this inkhaven only supports v{current} — \
                 upgrade inkhaven to open this project"
            )),
            Some(_) => Ok(()),
            None => self.engine.execute_with(
                "INSERT INTO _inkhaven_schema (singleton, version) VALUES (1, ?) \
                 ON CONFLICT (singleton) DO UPDATE SET version = excluded.version",
                &[&current],
            ),
        }
    }

    // ── writes ─────────────────────────────────────────────────────

    /// Insert one edge (a one-row transaction, so a `CAST`/constraint failure
    /// leaves no half-state).
    pub fn insert(&self, edge: &Edge) -> Result<()> {
        self.engine.transaction(|conn| bind_insert(conn, edge))
    }

    /// Insert many edges atomically — all land or none do (rolls back on the
    /// first failure, e.g. a duplicate id).
    pub fn insert_batch(&self, edges: &[Edge]) -> Result<()> {
        self.engine.transaction(|conn| {
            for e in edges {
                bind_insert(conn, e)?;
            }
            Ok(())
        })
    }

    // ── reads ──────────────────────────────────────────────────────

    pub fn by_id(&self, id: Uuid) -> Result<Option<Edge>> {
        let id_s = id.to_string();
        let sql = format!("{EDGE_SELECT} WHERE id = ?");
        let rows = self.engine.select_all_with(&sql, &[&id_s])?;
        match rows.into_iter().next() {
            None => Ok(None),
            Some(row) => Ok(Some(row_to_edge(row)?)),
        }
    }

    /// Edges leaving `ep` (matched on the `src` columns), filtered to `kinds`
    /// (empty = any kind).
    pub fn outgoing(&self, ep: &EndpointRef, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.select_side("src_kind", "src_ref", ep, kinds)
    }

    /// Edges arriving at `ep` (matched on the `dst` columns). This is the
    /// reverse-index query — "what points at this?".
    pub fn incoming(&self, ep: &EndpointRef, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        self.select_side("dst_kind", "dst_ref", ep, kinds)
    }

    /// Every edge touching `ep` on either side, deduped by id. Handles symmetric
    /// edges (which live on one row but are conceptually undirected).
    pub fn neighbors(&self, ep: &EndpointRef, kinds: &[EdgeKind]) -> Result<Vec<Edge>> {
        let mut out = self.outgoing(ep, kinds)?;
        let mut seen: HashSet<Uuid> = out.iter().map(|e| e.id).collect();
        for e in self.incoming(ep, kinds)? {
            if seen.insert(e.id) {
                out.push(e);
            }
        }
        Ok(out)
    }

    pub fn all(&self) -> Result<Vec<Edge>> {
        let rows = self.engine.select_all(EDGE_SELECT)?;
        rows.into_iter().map(row_to_edge).collect()
    }

    /// Every edge of one kind (e.g. all `Cites` edges, for a dedup pass).
    pub fn by_kind(&self, kind: EdgeKind) -> Result<Vec<Edge>> {
        let k = kind.as_str();
        let sql = format!("{EDGE_SELECT} WHERE kind = ?");
        let rows = self.engine.select_all_with(&sql, &[&k])?;
        rows.into_iter().map(row_to_edge).collect()
    }

    /// Every edge of one origin (e.g. all `Judged` edges — the advisory layer
    /// awaiting promote/dismiss triage: the edge inbox).
    pub fn by_origin(&self, origin: EdgeOrigin) -> Result<Vec<Edge>> {
        let o = origin.as_str();
        let sql = format!("{EDGE_SELECT} WHERE origin = ?");
        let rows = self.engine.select_all_with(&sql, &[&o])?;
        rows.into_iter().map(row_to_edge).collect()
    }

    pub fn count(&self) -> Result<usize> {
        scalar_count(&self.engine, "SELECT COUNT(*) FROM edges")
    }

    /// Per-kind counts, ascending by kind name — for `graph stats`.
    pub fn count_by_kind(&self) -> Result<Vec<(String, usize)>> {
        let rows = self
            .engine
            .select_all("SELECT kind, COUNT(*) FROM edges GROUP BY kind ORDER BY kind")?;
        let mut out = Vec::with_capacity(rows.len());
        for row in rows {
            let mut it = row.into_iter();
            let kind = take_text(it.next(), "kind")?;
            let n = take_i64(it.next(), "count")? as usize;
            out.push((kind, n));
        }
        Ok(out)
    }

    // ── deletes ────────────────────────────────────────────────────

    pub fn delete(&self, id: Uuid) -> Result<()> {
        let id_s = id.to_string();
        self.engine
            .execute_with("DELETE FROM edges WHERE id = ?", &[&id_s])
    }

    /// Cascade-GC: delete every edge with a `Node(id)` endpoint (either side)
    /// for any id in `ids`. Called from `Store::delete_subtree`, beside
    /// `scrub_linked_paragraphs`. Returns the number of edges removed.
    pub fn delete_nodes(&self, ids: &HashSet<Uuid>) -> Result<usize> {
        self.engine.transaction(|conn| {
            let mut n = 0usize;
            let node = "node";
            for id in ids {
                let r = id.to_string();
                let params: Vec<&dyn duckdb::ToSql> = vec![&node, &r, &node, &r];
                n += conn
                    .execute(
                        "DELETE FROM edges WHERE (src_kind = ? AND src_ref = ?) \
                         OR (dst_kind = ? AND dst_ref = ?)",
                        duckdb::params_from_iter(params),
                    )
                    .map_err(|e| anyhow!("edge node-GC failed: {e}"))?;
            }
            Ok(n)
        })
    }

    /// Drop every edge of a given origin (used by `graph rebuild` to clear the
    /// rebuildable `Derived`/`Imported` cache). Returns the count removed.
    pub fn delete_by_origin(&self, origin: EdgeOrigin) -> Result<usize> {
        let o = origin.as_str();
        self.engine.transaction(|conn| {
            let params: Vec<&dyn duckdb::ToSql> = vec![&o];
            let n = conn
                .execute("DELETE FROM edges WHERE origin = ?", duckdb::params_from_iter(params))
                .map_err(|e| anyhow!("edge origin-GC failed: {e}"))?;
            Ok(n)
        })
    }

    /// Delete every edge of the given kinds (e.g. clearing the lexical bridge
    /// before re-importing it). Returns the count removed.
    pub fn delete_by_kinds(&self, kinds: &[EdgeKind]) -> Result<usize> {
        if kinds.is_empty() {
            return Ok(0);
        }
        let names: Vec<&'static str> = kinds.iter().map(|k| k.as_str()).collect();
        let placeholders = vec!["?"; names.len()].join(",");
        let sql = format!("DELETE FROM edges WHERE kind IN ({placeholders})");
        self.engine.transaction(|conn| {
            let params: Vec<&dyn duckdb::ToSql> = names.iter().map(|n| n as &dyn duckdb::ToSql).collect();
            let n = conn
                .execute(&sql, duckdb::params_from_iter(params))
                .map_err(|e| anyhow!("edge kind-delete failed: {e}"))?;
            Ok(n)
        })
    }

    /// Change an edge's origin — the promote/demote primitive (e.g. a `Judged`
    /// stance edge the user accepts becomes `Promoted`). Returns the number of
    /// rows changed (0 if the id doesn't exist).
    pub fn set_origin(&self, id: Uuid, origin: EdgeOrigin) -> Result<usize> {
        let o = origin.as_str();
        let id_s = id.to_string();
        self.engine.transaction(|conn| {
            let params: Vec<&dyn duckdb::ToSql> = vec![&o, &id_s];
            let n = conn
                .execute("UPDATE edges SET origin = ? WHERE id = ?", duckdb::params_from_iter(params))
                .map_err(|e| anyhow!("edge set_origin failed: {e}"))?;
            Ok(n)
        })
    }

    // ── persistence ────────────────────────────────────────────────

    pub fn checkpoint(&self) -> Result<()> {
        self.engine.checkpoint()
    }

    pub fn integrity_check(&self) -> Result<String> {
        self.engine.integrity_check()
    }

    // ── internals ──────────────────────────────────────────────────

    fn select_side(
        &self,
        col_kind: &str,
        col_ref: &str,
        ep: &EndpointRef,
        kinds: &[EdgeKind],
    ) -> Result<Vec<Edge>> {
        // col_kind/col_ref are trusted constants (never user input).
        let (k, r) = ep.as_columns();
        let mut sql = format!("{EDGE_SELECT} WHERE {col_kind} = ? AND {col_ref} = ?");
        let kind_strs: Vec<&'static str> = kinds.iter().map(|x| x.as_str()).collect();
        if !kind_strs.is_empty() {
            sql.push_str(" AND kind IN (");
            for i in 0..kind_strs.len() {
                if i > 0 {
                    sql.push(',');
                }
                sql.push('?');
            }
            sql.push(')');
        }
        let mut args: Vec<&dyn duckdb::ToSql> = vec![&k, &r];
        for ks in &kind_strs {
            args.push(ks);
        }
        let rows = self.engine.select_all_with(&sql, &args)?;
        rows.into_iter().map(row_to_edge).collect()
    }
}

const EDGE_SELECT: &str = "SELECT id, src_kind, src_ref, dst_kind, dst_ref, kind, directed, weight, reason, origin, attrs, created_at FROM edges";

fn bind_insert(conn: &duckdb::Connection, e: &Edge) -> Result<()> {
    let id = e.id.to_string();
    let (src_kind, src_ref) = e.src.as_columns();
    let (dst_kind, dst_ref) = e.dst.as_columns();
    let kind = e.kind.as_str();
    let origin = e.origin.as_str();
    let weight = e.weight as f64;
    let attrs = serde_json::to_string(&e.attrs)
        .map_err(|err| anyhow!("edge attrs serialise failed: {err}"))?;
    let params: Vec<&dyn duckdb::ToSql> = vec![
        &id, &src_kind, &src_ref, &dst_kind, &dst_ref, &kind, &e.directed, &weight, &e.reason,
        &origin, &attrs, &e.created_at,
    ];
    conn.execute(EDGE_INSERT_SQL, duckdb::params_from_iter(params))
        .map_err(|err| anyhow!("edge insert failed: {err}"))?;
    Ok(())
}

fn row_to_edge(row: Vec<DuckValue>) -> Result<Edge> {
    let mut it = row.into_iter();
    let id = Uuid::parse_str(&take_text(it.next(), "id")?)
        .map_err(|e| anyhow!("invalid edge id: {e}"))?;
    let src = EndpointRef::from_columns(&take_text(it.next(), "src_kind")?, &take_text(it.next(), "src_ref")?)?;
    let dst = EndpointRef::from_columns(&take_text(it.next(), "dst_kind")?, &take_text(it.next(), "dst_ref")?)?;
    let kind = EdgeKind::from_str(&take_text(it.next(), "kind")?)
        .ok_or_else(|| anyhow!("unknown edge kind in row"))?;
    let directed = take_bool(it.next(), "directed")?;
    let weight = take_f32(it.next(), "weight")?;
    let reason = take_opt_text(it.next());
    let origin = EdgeOrigin::from_str(&take_text(it.next(), "origin")?)
        .ok_or_else(|| anyhow!("unknown edge origin in row"))?;
    let attrs = match take_opt_text(it.next()) {
        Some(s) => serde_json::from_str(&s).unwrap_or(JsonValue::Null),
        None => JsonValue::Null,
    };
    let created_at = take_i64(it.next(), "created_at")?;
    Ok(Edge { id, src, dst, kind, directed, weight, reason, origin, attrs, created_at })
}

// ── column extraction helpers ────────────────────────────────────────

fn take_text(v: Option<DuckValue>, col: &str) -> Result<String> {
    match v {
        Some(DuckValue::Text(s)) => Ok(s),
        Some(other) => Err(anyhow!("edge column {col}: expected text, got {other:?}")),
        None => Err(anyhow!("edge row missing column {col}")),
    }
}

fn take_opt_text(v: Option<DuckValue>) -> Option<String> {
    match v {
        Some(DuckValue::Text(s)) => Some(s),
        _ => None,
    }
}

fn take_bool(v: Option<DuckValue>, col: &str) -> Result<bool> {
    match v {
        Some(DuckValue::Boolean(b)) => Ok(b),
        Some(DuckValue::Int(i)) => Ok(i != 0),
        Some(DuckValue::BigInt(i)) => Ok(i != 0),
        Some(other) => Err(anyhow!("edge column {col}: expected bool, got {other:?}")),
        None => Err(anyhow!("edge row missing column {col}")),
    }
}

fn take_f32(v: Option<DuckValue>, col: &str) -> Result<f32> {
    match v {
        Some(DuckValue::Double(d)) => Ok(d as f32),
        Some(DuckValue::Float(f)) => Ok(f),
        Some(DuckValue::Int(i)) => Ok(i as f32),
        Some(DuckValue::BigInt(i)) => Ok(i as f32),
        Some(other) => Err(anyhow!("edge column {col}: expected float, got {other:?}")),
        None => Err(anyhow!("edge row missing column {col}")),
    }
}

fn take_i64(v: Option<DuckValue>, col: &str) -> Result<i64> {
    match v {
        Some(DuckValue::BigInt(i)) => Ok(i),
        Some(DuckValue::Int(i)) => Ok(i as i64),
        Some(DuckValue::HugeInt(i)) => Ok(i as i64),
        Some(other) => Err(anyhow!("edge column {col}: expected integer, got {other:?}")),
        None => Err(anyhow!("edge row missing column {col}")),
    }
}

fn scalar_count(engine: &StorageEngine, sql: &str) -> Result<usize> {
    let rows = engine.select_all(sql)?;
    Ok(match rows.first().and_then(|r| r.first()) {
        Some(DuckValue::BigInt(i)) => *i as usize,
        Some(DuckValue::Int(i)) => *i as usize,
        Some(DuckValue::HugeInt(i)) => *i as usize,
        _ => 0,
    })
}

fn now_unix_secs() -> i64 {
    chrono::Utc::now().timestamp()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn store() -> (TempDir, EdgeStore) {
        let dir = TempDir::new().unwrap();
        let s = EdgeStore::new(dir.path().join("edges.db"), 2).unwrap();
        (dir, s)
    }

    fn link(a: Uuid, b: Uuid) -> Edge {
        Edge::new(EndpointRef::Node(a), EdgeKind::LinksTo, EndpointRef::Node(b), EdgeOrigin::Structural)
    }

    #[test]
    fn edge_roundtrips_through_columns() {
        let (_d, s) = store();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let e = link(a, b)
            .with_reason("because")
            .with_weight(0.5)
            .with_attrs(json!({"cross_source": true}));
        s.insert(&e).unwrap();
        let got = s.by_id(e.id).unwrap().unwrap();
        assert_eq!(got.src, EndpointRef::Node(a));
        assert_eq!(got.dst, EndpointRef::Node(b));
        assert_eq!(got.kind, EdgeKind::LinksTo);
        assert!(got.directed, "LinksTo is asymmetric → directed");
        assert_eq!(got.reason.as_deref(), Some("because"));
        assert_eq!(got.origin, EdgeOrigin::Structural);
        assert_eq!(got.attrs, json!({"cross_source": true}));
        assert!((got.weight - 0.5).abs() < 1e-6);
    }

    #[test]
    fn by_kind_filters_project_wide() {
        // SENTINEL-1 (CT-P0) — the project-wide edges-of-one-kind sweep that
        // `Store::edges_of_kind` wraps.
        let (_d, s) = store();
        s.insert(&link(Uuid::now_v7(), Uuid::now_v7())).unwrap();
        s.insert(&link(Uuid::now_v7(), Uuid::now_v7())).unwrap();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        s.insert(&Edge::new(
            EndpointRef::Node(a),
            EdgeKind::Cites,
            EndpointRef::Node(b),
            EdgeOrigin::Structural,
        ))
        .unwrap();
        assert_eq!(s.by_kind(EdgeKind::LinksTo).unwrap().len(), 2);
        let cites = s.by_kind(EdgeKind::Cites).unwrap();
        assert_eq!(cites.len(), 1);
        assert_eq!(cites[0].kind, EdgeKind::Cites);
    }

    #[test]
    fn edge_survives_checkpoint_and_reopen() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("edges.db");
        let e = link(Uuid::now_v7(), Uuid::now_v7());
        {
            let s = EdgeStore::new(&path, 2).unwrap();
            s.insert(&e).unwrap();
            s.checkpoint().unwrap();
        }
        let s2 = EdgeStore::new(&path, 2).unwrap();
        assert_eq!(s2.by_id(e.id).unwrap().unwrap().id, e.id);
        assert_eq!(s2.count().unwrap(), 1);
    }

    #[test]
    fn insert_batch_commits_and_rolls_back() {
        let (_d, s) = store();
        let good = vec![
            link(Uuid::now_v7(), Uuid::now_v7()),
            link(Uuid::now_v7(), Uuid::now_v7()),
        ];
        s.insert_batch(&good).unwrap();
        assert_eq!(s.count().unwrap(), 2);

        // A batch containing a duplicate id violates the PK partway → the whole
        // batch rolls back, leaving no half-state.
        let dup = link(Uuid::now_v7(), Uuid::now_v7());
        let bad = vec![
            link(Uuid::now_v7(), Uuid::now_v7()),
            dup.clone(),
            dup, // same id → PK violation on the second insert
        ];
        assert!(s.insert_batch(&bad).is_err());
        assert_eq!(s.count().unwrap(), 2, "the failed batch must not have landed any row");
    }

    #[test]
    fn reverse_index_finds_incoming() {
        let (_d, s) = store();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        s.insert(&link(a, b)).unwrap();
        let ea = EndpointRef::Node(a);
        let eb = EndpointRef::Node(b);
        assert_eq!(s.outgoing(&ea, &[]).unwrap().len(), 1);
        assert_eq!(s.incoming(&ea, &[]).unwrap().len(), 0);
        assert_eq!(s.incoming(&eb, &[]).unwrap().len(), 1, "reverse index: b has one in-edge");
        assert_eq!(s.outgoing(&eb, &[]).unwrap().len(), 0);
        // Kind filter excludes non-matching kinds.
        assert_eq!(s.outgoing(&ea, &[EdgeKind::Cites]).unwrap().len(), 0);
        assert_eq!(s.outgoing(&ea, &[EdgeKind::LinksTo]).unwrap().len(), 1);
    }

    #[test]
    fn symmetric_edge_found_from_both_sides() {
        let (_d, s) = store();
        let (a, b) = (Uuid::now_v7(), Uuid::now_v7());
        let e = Edge::new(
            EndpointRef::Node(a),
            EdgeKind::Contradicts,
            EndpointRef::Node(b),
            EdgeOrigin::Judged,
        );
        assert!(!e.directed, "Contradicts is symmetric → stored directed=false");
        s.insert(&e).unwrap();
        assert_eq!(s.neighbors(&EndpointRef::Node(a), &[]).unwrap().len(), 1);
        assert_eq!(s.neighbors(&EndpointRef::Node(b), &[]).unwrap().len(), 1);
    }

    #[test]
    fn delete_nodes_cascades_both_sides_only_for_targets() {
        let (_d, s) = store();
        let (x, y, z) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        s.insert(&link(x, y)).unwrap(); // x as src
        s.insert(&link(y, x)).unwrap(); // x as dst
        s.insert(&link(y, z)).unwrap(); // unrelated to x
        assert_eq!(s.count().unwrap(), 3);
        let removed = s.delete_nodes(&HashSet::from([x])).unwrap();
        assert_eq!(removed, 2);
        assert_eq!(s.count().unwrap(), 1, "only the y→z edge survives");
    }

    #[test]
    fn set_origin_promotes_judged_to_promoted() {
        let (_d, s) = store();
        let e = Edge::new(
            EndpointRef::Node(Uuid::now_v7()),
            EdgeKind::Contradicts,
            EndpointRef::Node(Uuid::now_v7()),
            EdgeOrigin::Judged,
        );
        s.insert(&e).unwrap();
        assert_eq!(s.set_origin(e.id, EdgeOrigin::Promoted).unwrap(), 1);
        assert_eq!(s.by_id(e.id).unwrap().unwrap().origin, EdgeOrigin::Promoted);
        // A promoted stance edge survives the rebuild sweep (only Structural/
        // Derived/Imported are cleared).
        for o in [EdgeOrigin::Structural, EdgeOrigin::Derived, EdgeOrigin::Imported] {
            s.delete_by_origin(o).unwrap();
        }
        assert!(s.by_id(e.id).unwrap().is_some(), "promoted edge must survive rebuild");
        assert_eq!(s.set_origin(Uuid::now_v7(), EdgeOrigin::Promoted).unwrap(), 0);
    }

    #[test]
    fn by_origin_lists_the_judged_inbox() {
        let (_d, s) = store();
        let judged = Edge::new(
            EndpointRef::Node(Uuid::now_v7()),
            EdgeKind::Contradicts,
            EndpointRef::Node(Uuid::now_v7()),
            EdgeOrigin::Judged,
        );
        s.insert(&judged).unwrap();
        s.insert(&link(Uuid::now_v7(), Uuid::now_v7())).unwrap(); // Structural
        let pending = s.by_origin(EdgeOrigin::Judged).unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, judged.id);
        assert!(s.by_origin(EdgeOrigin::Authorial).unwrap().is_empty());
    }

    #[test]
    fn delete_by_origin_spares_durable_edges() {
        let (_d, s) = store();
        let durable = link(Uuid::now_v7(), Uuid::now_v7()); // Structural
        let derived = Edge::new(
            EndpointRef::Node(Uuid::now_v7()),
            EdgeKind::SimilarTo,
            EndpointRef::Node(Uuid::now_v7()),
            EdgeOrigin::Derived,
        );
        s.insert(&durable).unwrap();
        s.insert(&derived).unwrap();
        assert_eq!(s.delete_by_origin(EdgeOrigin::Derived).unwrap(), 1);
        assert_eq!(s.count().unwrap(), 1);
        assert!(s.by_id(durable.id).unwrap().is_some());
    }

    #[test]
    fn replacing_judged_outgoing_keeps_promoted() {
        // Models Store::replace_confront_edges: a re-confront drops the node's
        // prior Judged stance edges but keeps ones the user promoted.
        let (_d, s) = store();
        let node = Uuid::now_v7();
        let judged = Edge::new(
            EndpointRef::Node(node),
            EdgeKind::Contradicts,
            EndpointRef::Extern(ExternRef::Evidence { label: "fact: old".into() }),
            EdgeOrigin::Judged,
        );
        let promoted = Edge::new(
            EndpointRef::Node(node),
            EdgeKind::InTension,
            EndpointRef::Extern(ExternRef::Evidence { label: "fact: kept".into() }),
            EdgeOrigin::Promoted,
        );
        s.insert(&judged).unwrap();
        s.insert(&promoted).unwrap();

        // Replace: delete Judged outgoing stance edges, add a fresh one.
        let stance = [EdgeKind::Contradicts, EdgeKind::InTension, EdgeKind::Qualifies, EdgeKind::Agrees];
        for e in s.outgoing(&EndpointRef::Node(node), &stance).unwrap() {
            if e.origin == EdgeOrigin::Judged {
                s.delete(e.id).unwrap();
            }
        }
        let fresh = Edge::new(
            EndpointRef::Node(node),
            EdgeKind::Agrees,
            EndpointRef::Extern(ExternRef::Evidence { label: "fact: new".into() }),
            EdgeOrigin::Judged,
        );
        s.insert(&fresh).unwrap();

        assert!(s.by_id(judged.id).unwrap().is_none(), "old judged dropped");
        assert!(s.by_id(promoted.id).unwrap().is_some(), "promoted kept");
        assert!(s.by_id(fresh.id).unwrap().is_some(), "fresh judged added");
    }

    #[test]
    fn extern_endpoints_roundtrip_through_columns() {
        let cases = vec![
            EndpointRef::Extern(ExternRef::Source { book_node: Uuid::now_v7(), key: "smith2020".into() }),
            EndpointRef::Extern(ExternRef::Work { registry: Registry::OpenAlex, id: "W123".into() }),
            EndpointRef::Extern(ExternRef::Locus { scheme: "bible".into(), canonical: "John 3:16".into() }),
            EndpointRef::Extern(ExternRef::Sense { lang: "ru".into(), synset: "12345-n".into() }),
            EndpointRef::Extern(ExternRef::Ili { id: "i98765".into() }),
            EndpointRef::Extern(ExternRef::Grade { level: "inaccurate".into() }),
            EndpointRef::Extern(ExternRef::Evidence { label: "fact: Chapter 3 › para 5".into() }),
            EndpointRef::Extern(ExternRef::Declared { kind: "character".into(), label: "Mara".into() }),
        ];
        for ep in cases {
            let (k, r) = ep.as_columns();
            let back = EndpointRef::from_columns(k, &r).unwrap();
            assert_eq!(back, ep);
        }
    }

    #[test]
    fn from_columns_rejects_unknown_kind() {
        assert!(EndpointRef::from_columns("galaxy", "x").is_err());
        assert!(EndpointRef::from_columns("node", "not-a-uuid").is_err());
        assert!(EndpointRef::from_columns("source", "no-separator").is_err());
    }

    #[test]
    fn extern_endpoint_edge_persists() {
        // An edge with a non-node endpoint round-trips through the store.
        let (_d, s) = store();
        let fact = Uuid::now_v7();
        let e = Edge::new(
            EndpointRef::Node(fact),
            EdgeKind::SourcedFrom,
            EndpointRef::Extern(ExternRef::Work { registry: Registry::Arxiv, id: "2401.00001".into() }),
            EdgeOrigin::Structural,
        );
        s.insert(&e).unwrap();
        let out = s.outgoing(&EndpointRef::Node(fact), &[EdgeKind::SourcedFrom]).unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].dst,
            EndpointRef::Extern(ExternRef::Work { registry: Registry::Arxiv, id: "2401.00001".into() })
        );
    }
}
