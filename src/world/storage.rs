//! WORLD-4 — the per-project world store (`<project>/world.db`). Persists the
//! proposal queue so proposals survive across runs and the CLI / TUI manage the
//! same set. Built on the in-tree `StorageEngine`, exactly like the Output and
//! progress stores (unix-secs timestamps, scope-by-file, no project_id column).

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;
use uuid::Uuid;

use crate::storage::engine::StorageEngine;
use crate::world::proposals::{now_secs, PlaceLink, PlaceProposal};

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS world_proposals (
        id           TEXT   NOT NULL PRIMARY KEY,
        signature    TEXT   NOT NULL,
        kind         TEXT   NOT NULL,
        name         TEXT   NOT NULL,
        payload_json TEXT   NOT NULL,
        rationale    TEXT   NOT NULL,
        status       TEXT   NOT NULL,
        created_at   BIGINT NOT NULL,
        resolved_at  BIGINT,
        seed         TEXT              -- v2: the world seed it was proposed under (NULL = pre-v2)
    );
    CREATE INDEX IF NOT EXISTS idx_wp_status ON world_proposals(status);
    CREATE INDEX IF NOT EXISTS idx_wp_sig    ON world_proposals(signature);

    -- WORLD-4 P2 — Place ↔ World cross-references. One row per accepted
    -- compiler-proposed Place, linking the Place record (by its node id) back to
    -- the world data that generated it (climate zone / biome / hydrology basis /
    -- coordinates). The fact-checker (P4) joins through here.
    CREATE TABLE IF NOT EXISTS world_place_links (
        place_id        TEXT   NOT NULL PRIMARY KEY,
        name            TEXT   NOT NULL,
        biome           TEXT,
        climate_zone    TEXT,
        hydrology_basis TEXT,
        population      BIGINT,
        x               INTEGER,
        y               INTEGER,
        created_at      BIGINT NOT NULL,
        coords_source   TEXT DEFAULT 'compiled'  -- v2: compiled | author | map
    );

    -- WORLD-4 P5 — slow-track LLM usage, for the cost caps. One row per day.
    CREATE TABLE IF NOT EXISTS world_llm_usage (
        day   TEXT   NOT NULL PRIMARY KEY,   -- YYYY-MM-DD
        calls INTEGER NOT NULL DEFAULT 0
    );
";

/// The world store's schema version. v1 = the 3.0.0 freeze; v2 (3.16,
/// WORLD-KEEP-2) adds `world_proposals.seed` and `world_place_links.coords_source`.
const SCHEMA_VERSION: i64 = 2;

/// Forward-only, idempotent steps from an older `world.db` (see
/// `StorageEngine::new_migrating`). v1 → v2 only ADDS columns: existing proposal
/// rows keep `seed = NULL` (they keep applying under every seed — the decisions
/// already made stay made) and existing place links read as `compiled`.
const MIGRATIONS: &[(i64, &[&str])] = &[(
    2,
    &[
        "ALTER TABLE world_proposals ADD COLUMN IF NOT EXISTS seed TEXT",
        "ALTER TABLE world_place_links ADD COLUMN IF NOT EXISTS coords_source TEXT DEFAULT 'compiled'",
    ],
)];

/// Who last set a Place's map coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CoordsSource {
    /// From the compiler (the settlement the Place was accepted from).
    Compiled,
    /// The author, with `realworld set-coords` — never overridden by the map.
    Author,
    /// Refined by a map render's resolved landmark position.
    Map,
}

impl CoordsSource {
    fn as_str(self) -> &'static str {
        match self {
            CoordsSource::Compiled => "compiled",
            CoordsSource::Author => "author",
            CoordsSource::Map => "map",
        }
    }
}

/// Per-project world store. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct WorldStore {
    engine: Arc<StorageEngine>,
}

fn text(v: Option<&DuckValue>) -> String {
    match v {
        Some(DuckValue::Text(s)) => s.clone(),
        _ => String::new(),
    }
}

fn int(v: Option<&DuckValue>) -> i64 {
    match v {
        Some(DuckValue::BigInt(i)) => *i,
        Some(DuckValue::Int(i)) => *i as i64,
        Some(DuckValue::HugeInt(i)) => *i as i64,
        _ => 0,
    }
}

impl WorldStore {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            engine: Arc::new(StorageEngine::new_migrating(path, INIT_SQL, 2, SCHEMA_VERSION, MIGRATIONS)?),
        })
    }

    /// `<project>/world.db`, beside `output.db` / `progress.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("world.db"))
    }

    /// Insert a proposal (one INSERT), recording the world `seed` it was
    /// proposed under so its eventual accept/reject can be scoped to that seed.
    pub fn insert(&self, p: &PlaceProposal, seed: Option<u64>) -> Result<()> {
        let id = p.id.to_string();
        let payload = p.payload.to_string();
        let seed = seed.map(|s| format!("{s:x}"));
        self.engine.execute_with(
            "INSERT INTO world_proposals \
             (id, signature, kind, name, payload_json, rationale, status, created_at, resolved_at, seed) \
             VALUES (?,?,?,?,?,?,?,?,NULL,?)",
            &[&id, &p.signature, &p.kind, &p.name, &payload, &p.rationale, &p.status, &p.created_at, &seed],
        )?;
        Ok(())
    }

    /// List proposals, optionally filtered by status, newest first.
    pub fn list(&self, status: Option<&str>) -> Result<Vec<PlaceProposal>> {
        let rows = match status {
            Some(s) => self.engine.select_all_with(
                "SELECT id, signature, kind, name, payload_json, rationale, status, created_at \
                 FROM world_proposals WHERE status = ? ORDER BY created_at DESC, id",
                &[&s],
            )?,
            None => self.engine.select_all(
                "SELECT id, signature, kind, name, payload_json, rationale, status, created_at \
                 FROM world_proposals ORDER BY created_at DESC, id",
            )?,
        };
        Ok(rows.iter().filter_map(row_to_proposal).collect())
    }

    pub fn get(&self, id: Uuid) -> Result<Option<PlaceProposal>> {
        let rows = self.engine.select_all_with(
            "SELECT id, signature, kind, name, payload_json, rationale, status, created_at \
             FROM world_proposals WHERE id = ?",
            &[&id.to_string()],
        )?;
        Ok(rows.first().and_then(row_to_proposal))
    }

    /// Signatures already resolved (accepted or rejected) — the dedup set so a
    /// re-compile doesn't re-propose them — as they apply under world `seed`.
    ///
    /// A **place** signature is a map cell (`place:60:69`), and a different seed
    /// grows a different settlement on the same cell, so a place decision made
    /// under one seed does not suppress a proposal under another. Rows written
    /// before v2 carry no seed and keep applying everywhere (the decisions
    /// already made stay made). Every other kind is keyed by a name
    /// (`character:ruler:karon`) and stays global.
    pub fn resolved_signatures(&self, seed: u64) -> Result<HashSet<String>> {
        let rows = self.engine.select_all_with(
            "SELECT signature FROM world_proposals \
             WHERE status IN ('accepted','rejected') \
               AND (kind <> 'place' OR seed IS NULL OR seed = ?)",
            &[&format!("{seed:x}")],
        )?;
        Ok(rows.iter().map(|r| text(r.first())).collect())
    }

    pub fn set_status(&self, id: Uuid, status: &str) -> Result<()> {
        self.engine.execute_with(
            "UPDATE world_proposals SET status = ?, resolved_at = ? WHERE id = ?",
            &[&status, &now_secs(), &id.to_string()],
        )
    }

    /// Drop all still-pending proposals (a fresh `propose` re-seeds them).
    pub fn clear_pending(&self) -> Result<()> {
        self.engine.execute_with("DELETE FROM world_proposals WHERE status = 'pending'", &[])
    }

    /// Drop only the pending proposals whose `kind` matches a SQL LIKE pattern
    /// (e.g. `"place"` or `"myth-%"`). Lets `propose` and `propose-myth` re-seed
    /// their own queue without clobbering the other's pending set.
    pub fn clear_pending_kinds(&self, kind_like: &str) -> Result<()> {
        self.engine.execute_with(
            "DELETE FROM world_proposals WHERE status = 'pending' AND kind LIKE ?",
            &[&kind_like],
        )
    }

    pub fn count(&self, status: &str) -> Result<usize> {
        Ok(self.list(Some(status))?.len())
    }

    /// Record a Place ↔ World cross-reference (idempotent on place_id). `source`
    /// says who placed it: the compiler (an accepted proposal) or the author
    /// (`set-coords`).
    pub fn insert_place_link(&self, link: &PlaceLink, source: CoordsSource) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO world_place_links \
             (place_id, name, biome, climate_zone, hydrology_basis, population, x, y, created_at, coords_source) \
             VALUES (?,?,?,?,?,?,?,?,?,?)",
            &[
                &link.place_id.to_string(),
                &link.name,
                &link.biome,
                &link.climate_zone,
                &link.hydrology_basis,
                &(link.population as i64),
                &(link.x as i64),
                &(link.y as i64),
                &now_secs(),
                &source.as_str(),
            ],
        )
    }

    /// Who last set this Place's coordinates (`None` when it has no link).
    pub fn coords_source(&self, place_id: Uuid) -> Result<Option<CoordsSource>> {
        let rows = self.engine.select_all_with(
            "SELECT coords_source FROM world_place_links WHERE place_id = ?",
            &[&place_id.to_string()],
        )?;
        Ok(rows.first().map(|r| match text(r.first()).as_str() {
            "author" => CoordsSource::Author,
            "map" => CoordsSource::Map,
            _ => CoordsSource::Compiled, // incl. NULL on a row older than v2
        }))
    }

    /// Refine a place link's grid coordinates from a map render's resolved
    /// landmark position — unless the author set them (`set-coords`), which the
    /// map never overrides. Returns whether the coordinates were changed.
    pub fn refine_place_link_coords(&self, place_id: Uuid, x: usize, y: usize) -> Result<bool> {
        match self.coords_source(place_id)? {
            None | Some(CoordsSource::Author) => Ok(false),
            Some(_) => {
                self.engine.execute_with(
                    "UPDATE world_place_links SET x = ?, y = ?, coords_source = 'map' WHERE place_id = ?",
                    &[&(x as i64), &(y as i64), &place_id.to_string()],
                )?;
                Ok(true)
            }
        }
    }

    /// Daily ceiling on world slow-track LLM calls (shared by the slow-track
    /// preflight and the cost dashboard, so the two never drift).
    pub const DAILY_CALL_CAP: i64 = 200;

    /// Record one slow-track LLM call against today's tally; returns the new count.
    pub fn record_llm_call(&self, day: &str) -> Result<i64> {
        self.engine.execute_with(
            "INSERT INTO world_llm_usage (day, calls) VALUES (?, 1) \
             ON CONFLICT (day) DO UPDATE SET calls = calls + 1",
            &[&day],
        )?;
        Ok(self.llm_calls_today(day)?)
    }

    /// How many slow-track LLM calls have run on `day` (YYYY-MM-DD).
    pub fn llm_calls_today(&self, day: &str) -> Result<i64> {
        let rows = self
            .engine
            .select_all_with("SELECT calls FROM world_llm_usage WHERE day = ?", &[&day])?;
        Ok(rows.first().map(|r| int(r.first())).unwrap_or(0))
    }

    /// All Place ↔ World cross-references, newest first.
    pub fn list_place_links(&self) -> Result<Vec<PlaceLink>> {
        let rows = self.engine.select_all(
            "SELECT place_id, name, biome, climate_zone, hydrology_basis, population, x, y \
             FROM world_place_links ORDER BY created_at DESC, name",
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(PlaceLink {
                    place_id: Uuid::parse_str(&text(r.first())).ok()?,
                    name: text(r.get(1)),
                    biome: text(r.get(2)),
                    climate_zone: text(r.get(3)),
                    hydrology_basis: text(r.get(4)),
                    population: int(r.get(5)).max(0) as u64,
                    x: int(r.get(6)).max(0) as usize,
                    y: int(r.get(7)).max(0) as usize,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::proposals::PlaceProposal;

    fn store() -> WorldStore {
        WorldStore::open(Path::new(":memory:")).unwrap()
    }

    fn proposal(sig: &str, name: &str) -> PlaceProposal {
        PlaceProposal {
            id: Uuid::new_v4(),
            signature: sig.into(),
            kind: "place".into(),
            name: name.into(),
            payload: serde_json::json!({"x": 60, "y": 69, "population": 40000, "class": "city", "basis": "river_mouth", "biome": "tropical_seasonal"}),
            rationale: "a city".into(),
            status: "pending".into(),
            created_at: 1,
        }
    }

    #[test]
    fn proposal_round_trip_and_dedup() {
        let s = store();
        let p = proposal("place:60:69", "Laevokorel");
        s.insert(&p, Some(0x1A)).unwrap();
        let listed = s.list(Some("pending")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Laevokorel");
        assert_eq!(listed[0].signature, "place:60:69");
        // Accept it → it leaves the pending set and joins the resolved signatures.
        s.set_status(p.id, "accepted").unwrap();
        assert!(s.list(Some("pending")).unwrap().is_empty());
        assert!(s.resolved_signatures(0x1A).unwrap().contains("place:60:69"));
        // A place decision is scoped to the seed it was made under…
        assert!(!s.resolved_signatures(0x2B).unwrap().contains("place:60:69"));
        // …a pre-v2 row (no seed) applies under every seed…
        let legacy = proposal("place:1:1", "Old");
        s.insert(&legacy, None).unwrap();
        s.set_status(legacy.id, "rejected").unwrap();
        assert!(s.resolved_signatures(0x2B).unwrap().contains("place:1:1"));
        // …and a name-keyed kind stays global whatever the seed.
        let mut ruler = proposal("character:ruler:karon", "Karon's ruler");
        ruler.kind = "character".into();
        s.insert(&ruler, Some(0x1A)).unwrap();
        s.set_status(ruler.id, "accepted").unwrap();
        assert!(s.resolved_signatures(0x2B).unwrap().contains("character:ruler:karon"));
    }

    #[test]
    fn a_v1_world_db_is_migrated_forward_without_losing_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("world.db");
        // A store exactly as 3.15.0 created it: the v1 tables, stamped 1.
        {
            let v1 = "
                CREATE TABLE world_proposals (
                    id TEXT NOT NULL PRIMARY KEY, signature TEXT NOT NULL, kind TEXT NOT NULL,
                    name TEXT NOT NULL, payload_json TEXT NOT NULL, rationale TEXT NOT NULL,
                    status TEXT NOT NULL, created_at BIGINT NOT NULL, resolved_at BIGINT);
                CREATE TABLE world_place_links (
                    place_id TEXT NOT NULL PRIMARY KEY, name TEXT NOT NULL, biome TEXT,
                    climate_zone TEXT, hydrology_basis TEXT, population BIGINT,
                    x INTEGER, y INTEGER, created_at BIGINT NOT NULL);
                CREATE TABLE world_llm_usage (day TEXT NOT NULL PRIMARY KEY, calls INTEGER NOT NULL DEFAULT 0);
            ";
            let e = StorageEngine::new_versioned(&path, v1, 1, 1).unwrap();
            e.execute("INSERT INTO world_proposals VALUES ('00000000-0000-0000-0000-000000000001','place:5:5','place','Oldtown','{}','r','accepted',1,2)").unwrap();
            e.execute("INSERT INTO world_place_links VALUES ('00000000-0000-0000-0000-000000000002','Oldtown','b','c','h',10,5,5,1)").unwrap();
            assert_eq!(e.schema_version().unwrap(), Some(1));
        }
        let s = WorldStore::open(&path).unwrap();
        assert_eq!(s.engine.schema_version().unwrap(), Some(SCHEMA_VERSION));
        // The old rows are intact and read with the new columns' defaults.
        assert!(s.resolved_signatures(0xABC).unwrap().contains("place:5:5"), "a pre-v2 decision applies under any seed");
        let links = s.list_place_links().unwrap();
        assert_eq!((links.len(), links[0].name.as_str(), links[0].x), (1, "Oldtown", 5));
        assert_eq!(s.coords_source(links[0].place_id).unwrap(), Some(CoordsSource::Compiled));
        // New writes work, and re-opening is a no-op.
        s.insert(&proposal("place:7:7", "Newtown"), Some(0xABC)).unwrap();
        drop(s);
        let again = WorldStore::open(&path).unwrap();
        assert_eq!(again.list(Some("pending")).unwrap().len(), 1);
        assert_eq!(again.engine.schema_version().unwrap(), Some(SCHEMA_VERSION));
    }

    #[test]
    fn the_map_never_overrides_coordinates_the_author_set() {
        let s = store();
        let (a, b) = (proposal("place:1:1", "Compiled"), proposal("place:2:2", "Authored"));
        let la = PlaceLink::from_proposal(a.id, &a);
        let lb = PlaceLink::from_proposal(b.id, &b);
        s.insert_place_link(&la, CoordsSource::Compiled).unwrap();
        s.insert_place_link(&lb, CoordsSource::Author).unwrap();
        assert!(s.refine_place_link_coords(a.id, 10, 11).unwrap(), "a compiled position is refined");
        assert!(!s.refine_place_link_coords(b.id, 20, 21).unwrap(), "an author-set one is kept");
        assert!(!s.refine_place_link_coords(Uuid::new_v4(), 1, 1).unwrap(), "no link → nothing to refine");
        let by_name = |n: &str| s.list_place_links().unwrap().into_iter().find(|l| l.name == n).unwrap();
        assert_eq!((by_name("Compiled").x, by_name("Compiled").y), (10, 11));
        assert_eq!(by_name("Authored").x, 60, "untouched");
        assert_eq!(s.coords_source(a.id).unwrap(), Some(CoordsSource::Map));
        // A map-refined position can be refined again; only `author` is sticky.
        assert!(s.refine_place_link_coords(a.id, 12, 13).unwrap());
    }

    #[test]
    fn place_link_round_trip() {
        let s = store();
        let p = proposal("place:60:69", "Laevokorel");
        let link = PlaceLink::from_proposal(p.id, &p);
        assert_eq!(link.climate_zone, "tropical_seasonal");
        assert_eq!(link.hydrology_basis, "river_mouth");
        s.insert_place_link(&link, CoordsSource::Compiled).unwrap();
        let back = s.list_place_links().unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].name, "Laevokorel");
        assert_eq!(back[0].population, 40000);
        assert_eq!(back[0].x, 60);
    }
}

fn row_to_proposal(r: &Vec<DuckValue>) -> Option<PlaceProposal> {
    let id = Uuid::parse_str(&text(r.first())).ok()?;
    let payload: serde_json::Value =
        serde_json::from_str(&text(r.get(4))).unwrap_or(serde_json::Value::Null);
    Some(PlaceProposal {
        id,
        signature: text(r.get(1)),
        kind: text(r.get(2)),
        name: text(r.get(3)),
        payload,
        rationale: text(r.get(5)),
        status: text(r.get(6)),
        created_at: int(r.get(7)),
    })
}
