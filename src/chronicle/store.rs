//! CHRONICLE-1 (CH-P0) — the draft-milestone store.
//!
//! DuckDB-backed, mirroring `progress::store`: one `milestones` row per captured
//! draft (its metric vector serialised to `metrics_json`) plus a
//! `milestone_findings` row per finding (the fingerprint set the cleared/introduced
//! diff walks). Cloneable; clones share the pool. Read-only from the writer's
//! side — CHRONICLE never mutates the manuscript.
#![allow(dead_code)]

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;
use uuid::Uuid;

use super::{FindingRef, Milestone, MetricVector};
use crate::storage::engine::StorageEngine;

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS milestones (
        id           TEXT    PRIMARY KEY,
        label        TEXT    NOT NULL,
        day          BIGINT  NOT NULL,   -- days since epoch
        ts           BIGINT  NOT NULL,   -- unix-seconds (ordering key)
        book_slug    TEXT,               -- NULL = whole project
        git_ref      TEXT,               -- verbatim, never resolved
        metrics_json TEXT    NOT NULL
    );

    CREATE INDEX IF NOT EXISTS idx_milestones_ts ON milestones(ts);

    CREATE TABLE IF NOT EXISTS milestone_findings (
        milestone_id TEXT NOT NULL,
        fingerprint  TEXT NOT NULL,
        category     TEXT NOT NULL,
        severity     TEXT NOT NULL,      -- error | warn | info
        location     TEXT,
        paragraph    TEXT
    );

    CREATE INDEX IF NOT EXISTS idx_mf_milestone ON milestone_findings(milestone_id);
";

/// Per-project draft-history store. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct ChronicleStore {
    engine: Arc<StorageEngine>,
}

/// Pull a `String` out of a row column (NULL / non-text → `None`).
fn text(v: Option<&DuckValue>) -> Option<String> {
    match v {
        Some(DuckValue::Text(s)) => Some(s.clone()),
        _ => None,
    }
}

/// Pull an `i64` out of a row column (DuckDB widths → `i64`).
fn int(v: Option<&DuckValue>) -> Option<i64> {
    match v {
        Some(DuckValue::BigInt(i)) => Some(*i),
        Some(DuckValue::Int(i)) => Some(*i as i64),
        Some(DuckValue::HugeInt(i)) => Some(*i as i64),
        _ => None,
    }
}

fn row_to_milestone(row: &[DuckValue]) -> Option<Milestone> {
    let id = Uuid::parse_str(&text(row.get(0))?).ok()?;
    let label = text(row.get(1))?;
    let day = int(row.get(2))?;
    let ts = int(row.get(3))?;
    let book_slug = text(row.get(4)).filter(|s| !s.is_empty());
    let git_ref = text(row.get(5)).filter(|s| !s.is_empty());
    let metrics: MetricVector = serde_json::from_str(&text(row.get(6))?).ok()?;
    Some(Milestone { id, label, day, ts, book_slug, git_ref, metrics })
}

fn row_to_finding(row: &[DuckValue]) -> Option<FindingRef> {
    Some(FindingRef {
        fingerprint: text(row.get(0))?,
        category: text(row.get(1))?,
        severity: text(row.get(2))?,
        location: text(row.get(3)).filter(|s| !s.is_empty()),
        paragraph: text(row.get(4)).and_then(|s| Uuid::parse_str(&s).ok()),
    })
}

impl ChronicleStore {
    /// Open (creating if needed) the store at `path`.
    pub fn open(path: &Path) -> Result<Self> {
        let engine = StorageEngine::new(path, INIT_SQL, 2)?;
        Ok(Self { engine: Arc::new(engine) })
    }

    /// The conventional per-project store path (`<project>/chronicle.db`), beside
    /// `progress.db` / `output.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("chronicle.db"))
    }

    /// Persist a milestone and its finding set (one transaction's worth of INSERTs).
    pub fn insert_milestone(&self, m: &Milestone, findings: &[FindingRef]) -> Result<()> {
        let id = m.id.to_string();
        let metrics_json = serde_json::to_string(&m.metrics)?;
        self.engine.execute_with(
            "INSERT INTO milestones (id, label, day, ts, book_slug, git_ref, metrics_json)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
            &[&id, &m.label, &m.day, &m.ts, &m.book_slug, &m.git_ref, &metrics_json],
        )?;
        for f in findings {
            let para = f.paragraph.map(|p| p.to_string());
            self.engine.execute_with(
                "INSERT INTO milestone_findings
                 (milestone_id, fingerprint, category, severity, location, paragraph)
                 VALUES (?, ?, ?, ?, ?, ?)",
                &[&id, &f.fingerprint, &f.category, &f.severity, &f.location, &para],
            )?;
        }
        Ok(())
    }

    /// Every milestone (newest first). `book_slug = Some(..)` restricts to that
    /// book; `None` returns them all.
    pub fn list_milestones(&self, book_slug: Option<&str>) -> Result<Vec<Milestone>> {
        let rows = match book_slug {
            Some(b) => self.engine.select_all_with(
                "SELECT id, label, day, ts, book_slug, git_ref, metrics_json
                 FROM milestones WHERE book_slug = ? ORDER BY ts DESC",
                &[&b],
            )?,
            None => self.engine.select_all_with(
                "SELECT id, label, day, ts, book_slug, git_ref, metrics_json
                 FROM milestones ORDER BY ts DESC",
                &[],
            )?,
        };
        Ok(rows.iter().filter_map(|r| row_to_milestone(r)).collect())
    }

    /// The finding set recorded for a milestone (the cleared/introduced diff input).
    pub fn findings_for(&self, milestone_id: Uuid) -> Result<Vec<FindingRef>> {
        let id = milestone_id.to_string();
        let rows = self.engine.select_all_with(
            "SELECT fingerprint, category, severity, location, paragraph
             FROM milestone_findings WHERE milestone_id = ?",
            &[&id],
        )?;
        Ok(rows.iter().filter_map(|r| row_to_finding(r)).collect())
    }

    /// The most recent milestone (the trend baseline), scoped as in `list_milestones`.
    pub fn latest(&self, book_slug: Option<&str>) -> Result<Option<Milestone>> {
        Ok(self.list_milestones(book_slug)?.into_iter().next())
    }

    /// The milestone with the given label, scoped as in `list_milestones`
    /// (newest wins if a label was reused).
    pub fn by_label(&self, label: &str, book_slug: Option<&str>) -> Result<Option<Milestone>> {
        Ok(self.list_milestones(book_slug)?.into_iter().find(|m| m.label == label))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mv() -> MetricVector {
        let mut v = MetricVector { total: 3, errors: 1, warnings: 2, ..Default::default() };
        v.by_category.insert("echo".into(), 2);
        v.by_category.insert("shape_sag".into(), 1);
        v.sag_count = 1;
        v
    }

    #[test]
    fn insert_list_findings_and_scope_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChronicleStore::open(&dir.path().join("chronicle.db")).unwrap();
        let id = Uuid::from_u128(1);
        let metrics = mv();
        let m = Milestone {
            id,
            label: "draft-1".into(),
            day: 100,
            ts: 8_640_000,
            book_slug: Some("tower".into()),
            git_ref: Some("v0.1".into()),
            metrics: metrics.clone(),
        };
        let fs = vec![
            FindingRef {
                fingerprint: "echo\u{1}about ×5".into(),
                category: "echo".into(),
                severity: "warn".into(),
                location: Some("ch. 3".into()),
                paragraph: Some(Uuid::from_u128(9)),
            },
            FindingRef {
                fingerprint: "shape_sag\u{1}ch.5 flat".into(),
                category: "shape_sag".into(),
                severity: "info".into(),
                location: Some("ch. 5".into()),
                paragraph: None,
            },
        ];
        store.insert_milestone(&m, &fs).unwrap();

        // list + metric vector survives the JSON round-trip.
        let list = store.list_milestones(Some("tower")).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "draft-1");
        assert_eq!(list[0].git_ref.as_deref(), Some("v0.1"));
        assert_eq!(list[0].metrics, metrics);

        // latest + by_label find it.
        assert_eq!(store.latest(Some("tower")).unwrap().unwrap().id, id);
        assert_eq!(store.by_label("draft-1", Some("tower")).unwrap().unwrap().id, id);
        assert!(store.by_label("nope", Some("tower")).unwrap().is_none());

        // the finding set round-trips with its jump target.
        let got = store.findings_for(id).unwrap();
        assert_eq!(got.len(), 2);
        let echo = got.iter().find(|f| f.category == "echo").unwrap();
        assert_eq!(echo.paragraph, Some(Uuid::from_u128(9)));
        assert_eq!(echo.location.as_deref(), Some("ch. 3"));

        // book scoping isolates.
        assert!(store.list_milestones(Some("other")).unwrap().is_empty());
        assert_eq!(store.list_milestones(None).unwrap().len(), 1);
    }

    #[test]
    fn project_scoped_milestone_has_null_book() {
        let dir = tempfile::tempdir().unwrap();
        let store = ChronicleStore::open(&dir.path().join("chronicle.db")).unwrap();
        let m = Milestone {
            id: Uuid::from_u128(2),
            label: "whole-project".into(),
            day: 101,
            ts: 8_726_400,
            book_slug: None,
            git_ref: None,
            metrics: MetricVector::default(),
        };
        store.insert_milestone(&m, &[]).unwrap();
        let all = store.list_milestones(None).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].book_slug, None);
        assert_eq!(all[0].git_ref, None);
    }
}
