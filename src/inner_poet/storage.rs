//! The per-project Inner Poet store (`<project>/inner_poet.db`, POEM-TUI PO-P16).
//! Persists the latest fast-track findings per verse paragraph — so ambient mode
//! and the `poem.*` Bund words can read them across sessions — plus the set of
//! *suppressed* findings, the author's decision to stop being told about a
//! particular line. Built on the in-tree `StorageEngine`, exactly like
//! `InnerEditorStore` / `InnerSocratesStore`.
//!
//! Poet findings are deterministic (the fast track recomputes them from the poem
//! and its declared form any time), so this store never becomes a source of
//! truth for *what* the findings are — only a cache of the latest scan and a
//! record of which the author has silenced.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;
use uuid::Uuid;

use crate::inner_poet::fast::{Finding, Severity};
use crate::storage::engine::StorageEngine;
use crate::world::proposals::now_secs;

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS poet_findings (
        id           TEXT   NOT NULL PRIMARY KEY,
        paragraph_id TEXT   NOT NULL,
        severity     TEXT   NOT NULL,
        kind         TEXT   NOT NULL,
        line         BIGINT NOT NULL,
        message      TEXT   NOT NULL,
        emitted_at   BIGINT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_pf_para ON poet_findings(paragraph_id);

    -- A suppressed finding: the author asked not to be told about this
    -- (paragraph, finding-key) again. `finding_key` is severity|kind|line — a
    -- stable fingerprint of the complaint, independent of the exact message.
    CREATE TABLE IF NOT EXISTS poet_suppressions (
        paragraph_id  TEXT   NOT NULL,
        finding_key   TEXT   NOT NULL,
        suppressed_at BIGINT NOT NULL,
        PRIMARY KEY (paragraph_id, finding_key)
    );
";

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

/// The stable suppression fingerprint of a finding: `severity|kind|line`.
/// Silencing this key stops the same complaint about the same line, whatever the
/// exact wording of the message.
pub fn finding_key(severity: &str, kind: &str, line: usize) -> String {
    format!("{severity}|{kind}|L{line}")
}

fn severity_str(s: Severity) -> &'static str {
    match s {
        Severity::Praise => "praise",
        Severity::Note => "note",
        Severity::Concern => "concern",
    }
}

/// A persisted fast-track finding (owned fields — the DB is the source).
#[derive(Debug, Clone, PartialEq)]
pub struct StoredPoetFinding {
    pub paragraph_id: Uuid,
    pub severity: String,
    pub kind: String,
    pub line: usize,
    pub message: String,
}

impl StoredPoetFinding {
    /// This finding's suppression key.
    pub fn key(&self) -> String {
        finding_key(&self.severity, &self.kind, self.line)
    }
}

/// Per-project Inner Poet store. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct InnerPoetStore {
    engine: Arc<StorageEngine>,
}

impl InnerPoetStore {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self { engine: Arc::new(StorageEngine::new(path, INIT_SQL, 2)?) })
    }

    /// `<project>/inner_poet.db`, beside `inner_editor.db` / `inner_socrates.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("inner_poet.db"))
    }

    // ── findings ────────────────────────────────────────────────────────────

    /// Replace a paragraph's persisted findings with a fresh scan's — the fast
    /// track is deterministic, so an older set is never worth keeping. The
    /// DELETE + INSERTs run in one transaction, so a failure mid-write leaves the
    /// prior set intact rather than a partial or empty one, and a concurrent
    /// reader never sees the gap.
    pub fn replace_findings(&self, paragraph_id: Uuid, findings: &[Finding]) -> Result<()> {
        let pid = paragraph_id.to_string();
        let now = now_secs();
        self.engine.transaction(|conn| {
            conn.execute(
                "DELETE FROM poet_findings WHERE paragraph_id = ?",
                duckdb::params![pid],
            )?;
            for f in findings {
                conn.execute(
                    "INSERT INTO poet_findings \
                     (id, paragraph_id, severity, kind, line, message, emitted_at) \
                     VALUES (?,?,?,?,?,?,?)",
                    duckdb::params![
                        Uuid::new_v4().to_string(),
                        pid,
                        severity_str(f.severity),
                        f.kind.to_string(),
                        f.line as i64,
                        f.message,
                        now,
                    ],
                )?;
            }
            Ok(())
        })
    }

    fn rows_to_findings(&self, sql: &str, args: &[&dyn duckdb::ToSql]) -> Result<Vec<StoredPoetFinding>> {
        let rows = self.engine.select_all_with(sql, args)?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(StoredPoetFinding {
                    paragraph_id: Uuid::parse_str(&text(r.get(0))).ok()?,
                    severity: text(r.get(1)),
                    kind: text(r.get(2)),
                    line: int(r.get(3)).max(0) as usize,
                    message: text(r.get(4)),
                })
            })
            .collect())
    }

    /// A paragraph's persisted findings.
    pub fn findings_for(&self, paragraph_id: Uuid) -> Result<Vec<StoredPoetFinding>> {
        self.rows_to_findings(
            "SELECT paragraph_id, severity, kind, line, message FROM poet_findings \
             WHERE paragraph_id = ? ORDER BY line, id",
            &[&paragraph_id.to_string()],
        )
    }

    /// Every persisted finding across the project, newest scan first. Store API
    /// surface — exercised by tests / scripts; not yet called from the TUI.
    #[allow(dead_code)]
    pub fn all_findings(&self) -> Result<Vec<StoredPoetFinding>> {
        self.rows_to_findings(
            "SELECT paragraph_id, severity, kind, line, message FROM poet_findings \
             ORDER BY emitted_at DESC, line, id",
            &[],
        )
    }

    // ── suppressions ──────────────────────────────────────────────────────────

    /// Silence a (paragraph, finding-key) pair. Idempotent.
    pub fn suppress(&self, paragraph_id: Uuid, key: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO poet_suppressions (paragraph_id, finding_key, suppressed_at) \
             VALUES (?,?,?)",
            &[&paragraph_id.to_string(), &key.to_string(), &now_secs()],
        )
    }

    /// Un-silence a previously suppressed finding. Store API surface (tests +
    /// the un-suppress path); not yet wired to a key in the TUI.
    #[allow(dead_code)]
    pub fn unsuppress(&self, paragraph_id: Uuid, key: &str) -> Result<()> {
        self.engine.execute_with(
            "DELETE FROM poet_suppressions WHERE paragraph_id = ? AND finding_key = ?",
            &[&paragraph_id.to_string(), &key.to_string()],
        )
    }

    /// The suppressed finding-keys for a paragraph.
    pub fn suppressions_for(&self, paragraph_id: Uuid) -> Result<Vec<String>> {
        let rows = self.engine.select_all_with(
            "SELECT finding_key FROM poet_suppressions WHERE paragraph_id = ? ORDER BY finding_key",
            &[&paragraph_id.to_string()],
        )?;
        Ok(rows.iter().map(|r| text(r.first())).collect())
    }

    /// Whether a specific (paragraph, key) is suppressed. Store API surface
    /// (tests); the scan path reads the whole suppression set at once instead.
    #[allow(dead_code)]
    pub fn is_suppressed(&self, paragraph_id: Uuid, key: &str) -> Result<bool> {
        Ok(self.suppressions_for(paragraph_id)?.iter().any(|k| k == key))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_poet::fast::Finding;

    fn store() -> (InnerPoetStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (InnerPoetStore::open(&dir.path().join("inner_poet.db")).unwrap(), dir)
    }

    fn finding(sev: Severity, kind: &'static str, line: usize, msg: &str) -> Finding {
        Finding { severity: sev, kind, line, message: msg.to_string() }
    }

    #[test]
    fn replace_findings_is_last_write_wins() {
        let (s, _d) = store();
        let p = Uuid::new_v4();
        s.replace_findings(p, &[
            finding(Severity::Note, "Metre", 1, "short"),
            finding(Severity::Concern, "Rhyme", 2, "no rhyme"),
        ]).unwrap();
        assert_eq!(s.findings_for(p).unwrap().len(), 2);
        // A re-scan replaces, never appends.
        s.replace_findings(p, &[finding(Severity::Praise, "Metre", 1, "clean")]).unwrap();
        let got = s.findings_for(p).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].severity, "praise");
        assert_eq!(got[0].kind, "Metre");
        assert_eq!(got[0].line, 1);
    }

    #[test]
    fn suppression_round_trips_and_keys_are_stable() {
        let (s, _d) = store();
        let p = Uuid::new_v4();
        let key = finding_key("concern", "Rhyme", 2);
        assert!(!s.is_suppressed(p, &key).unwrap());
        s.suppress(p, &key).unwrap();
        s.suppress(p, &key).unwrap(); // idempotent
        assert!(s.is_suppressed(p, &key).unwrap());
        assert_eq!(s.suppressions_for(p).unwrap(), vec![key.clone()]);
        // The stored finding's key matches the suppression key.
        s.replace_findings(p, &[finding(Severity::Concern, "Rhyme", 2, "no rhyme")]).unwrap();
        assert_eq!(s.findings_for(p).unwrap()[0].key(), key);
        s.unsuppress(p, &key).unwrap();
        assert!(!s.is_suppressed(p, &key).unwrap());
    }
}
