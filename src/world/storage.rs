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
        resolved_at  BIGINT
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
        created_at      BIGINT NOT NULL
    );

    -- WORLD-4 P5 — slow-track LLM usage, for the cost caps. One row per day.
    CREATE TABLE IF NOT EXISTS world_llm_usage (
        day   TEXT   NOT NULL PRIMARY KEY,   -- YYYY-MM-DD
        calls INTEGER NOT NULL DEFAULT 0
    );
";

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
        Ok(Self { engine: Arc::new(StorageEngine::new(path, INIT_SQL, 2)?) })
    }

    /// `<project>/world.db`, beside `output.db` / `progress.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("world.db"))
    }

    /// Insert a proposal (one INSERT).
    pub fn insert(&self, p: &PlaceProposal) -> Result<()> {
        let id = p.id.to_string();
        let payload = p.payload.to_string();
        self.engine.execute_with(
            "INSERT INTO world_proposals \
             (id, signature, kind, name, payload_json, rationale, status, created_at, resolved_at) \
             VALUES (?,?,?,?,?,?,?,?,NULL)",
            &[&id, &p.signature, &p.kind, &p.name, &payload, &p.rationale, &p.status, &p.created_at],
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
    /// re-compile doesn't re-propose them.
    pub fn resolved_signatures(&self) -> Result<HashSet<String>> {
        let rows = self.engine.select_all(
            "SELECT signature FROM world_proposals WHERE status IN ('accepted','rejected')",
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

    /// Record a Place ↔ World cross-reference (idempotent on place_id).
    pub fn insert_place_link(&self, link: &PlaceLink) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO world_place_links \
             (place_id, name, biome, climate_zone, hydrology_basis, population, x, y, created_at) \
             VALUES (?,?,?,?,?,?,?,?,?)",
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
            ],
        )
    }

    /// Update a place link's grid coordinates (used when plakat resolves a
    /// landmark to a refined map position).
    pub fn update_place_link_coords(&self, place_id: Uuid, x: usize, y: usize) -> Result<()> {
        self.engine.execute_with(
            "UPDATE world_place_links SET x = ?, y = ? WHERE place_id = ?",
            &[&(x as i64), &(y as i64), &place_id.to_string()],
        )
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
        s.insert(&p).unwrap();
        let listed = s.list(Some("pending")).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "Laevokorel");
        assert_eq!(listed[0].signature, "place:60:69");
        // Accept it → it leaves the pending set and joins the resolved signatures.
        s.set_status(p.id, "accepted").unwrap();
        assert!(s.list(Some("pending")).unwrap().is_empty());
        assert!(s.resolved_signatures().unwrap().contains("place:60:69"));
    }

    #[test]
    fn place_link_round_trip() {
        let s = store();
        let p = proposal("place:60:69", "Laevokorel");
        let link = PlaceLink::from_proposal(p.id, &p);
        assert_eq!(link.climate_zone, "tropical_seasonal");
        assert_eq!(link.hydrology_basis, "river_mouth");
        s.insert_place_link(&link).unwrap();
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
