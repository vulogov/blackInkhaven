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
use crate::world::proposals::{now_secs, PlaceProposal};

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

    pub fn count(&self, status: &str) -> Result<usize> {
        Ok(self.list(Some(status))?.len())
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
