//! The per-project Inner Editor store (`<project>/inner_editor.db`). Persists
//! emitted Editor findings (so a re-engagement replaces a paragraph's prior
//! ones, and the snapshot log can track resolutions later), the same-paragraph
//! cooldown state, and the `inner_editor` LLM-usage tally. Built on the in-tree
//! `StorageEngine`, exactly like `InnerSocratesStore` / `WorldStore`.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;
use uuid::Uuid;

use crate::storage::engine::StorageEngine;
use crate::world::proposals::now_secs;

use super::types::{EditorCategory, EditorFinding, EditorSeverity};

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS editor_findings (
        id            TEXT   NOT NULL PRIMARY KEY,
        paragraph_id  TEXT,
        chapter_id    TEXT,
        severity      TEXT   NOT NULL,
        category      TEXT   NOT NULL,
        language      TEXT,
        observation   TEXT   NOT NULL,
        observation_en TEXT  NOT NULL,
        conditional   INTEGER NOT NULL,
        suppressed_by TEXT,
        snapshot_id   TEXT,
        emitted_at    BIGINT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_ef_para ON editor_findings(paragraph_id);

    -- Same-paragraph cooldown: last Editor engagement and last edit, so an edit
    -- during the cooldown window resets the timer (RFC §8.4).
    CREATE TABLE IF NOT EXISTS editor_cooldown_state (
        paragraph_id       TEXT   NOT NULL PRIMARY KEY,
        last_engagement_at BIGINT NOT NULL,
        last_edit_at       BIGINT NOT NULL
    );

    -- inner_editor LLM usage, per day per sub-budget (separate tally from
    -- Inner Socrates / WORLD-4; surfaces in `inkhaven cost`).
    CREATE TABLE IF NOT EXISTS inner_editor_llm_usage (
        day        TEXT   NOT NULL,
        sub_budget TEXT   NOT NULL,
        calls      BIGINT NOT NULL,
        PRIMARY KEY (day, sub_budget)
    );

    -- COMPANIONS-1 — dismissal tally feeding promotion-from-dismissal (parallel
    -- to Inner Socrates' `socratic_dismissals`; the Editor's category ids are
    -- not Socratic categories so it needs its own table).
    CREATE TABLE IF NOT EXISTS editor_dismissals (
        id           TEXT   NOT NULL PRIMARY KEY,
        category     TEXT   NOT NULL,
        chapter_id   TEXT   NOT NULL,
        dismissed_at BIGINT NOT NULL
    );
    CREATE TABLE IF NOT EXISTS editor_promotion_refused (
        category   TEXT NOT NULL,
        chapter_id TEXT NOT NULL,
        PRIMARY KEY (category, chapter_id)
    );
";

fn text(v: Option<&DuckValue>) -> String {
    match v {
        Some(DuckValue::Text(s)) => s.clone(),
        _ => String::new(),
    }
}

fn opt_text(v: Option<&DuckValue>) -> Option<String> {
    match v {
        Some(DuckValue::Text(s)) if !s.is_empty() => Some(s.clone()),
        _ => None,
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

/// A persisted finding plus the paragraph it was emitted against.
#[derive(Debug, Clone)]
pub struct StoredEditorFinding {
    pub id: Uuid,
    pub paragraph_id: Option<Uuid>,
    pub finding: EditorFinding,
}

/// Same-paragraph cooldown snapshot.
#[derive(Debug, Clone, Copy)]
pub struct CooldownRow {
    pub last_engagement_at: i64,
    pub last_edit_at: i64,
}

/// An accumulated dismissal pattern the author might want to declare deliberate.
#[derive(Debug, Clone, PartialEq)]
pub struct EditorPromotionCandidate {
    pub category: EditorCategory,
    /// The chapter the dismissals clustered in (`""` = unknown).
    pub chapter_id: String,
    pub count: i64,
}

/// Per-project Inner Editor store. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct InnerEditorStore {
    engine: Arc<StorageEngine>,
}

impl InnerEditorStore {
    /// Daily ceiling on Inner Editor LLM calls. **Informative, not a gate** —
    /// per Inkhaven's permissive principle the preflight warns and continues.
    pub const DAILY_CALL_CAP: i64 = 200;
    /// Sub-budget keys (parallel to Inner Socrates' `slow_track`).
    pub const ENGAGEMENT_SUB_BUDGET: &'static str = "editor_engagement";
    pub const CONVERSATION_SUB_BUDGET: &'static str = "conversation";

    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self { engine: Arc::new(StorageEngine::new(path, INIT_SQL, 2)?) })
    }

    /// `<project>/inner_editor.db`, beside `inner_socrates.db` / `world.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("inner_editor.db"))
    }

    // ── findings ────────────────────────────────────────────────────────────

    /// Persist an emitted finding; returns its new id.
    pub fn insert_finding(
        &self,
        f: &EditorFinding,
        paragraph_id: Option<Uuid>,
        chapter_id: Option<&str>,
        language: Option<&str>,
        snapshot_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        self.engine.execute_with(
            "INSERT INTO editor_findings \
             (id, paragraph_id, chapter_id, severity, category, language, \
              observation, observation_en, conditional, suppressed_by, snapshot_id, emitted_at) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?,?)",
            &[
                &id.to_string(),
                &paragraph_id.map(|p| p.to_string()).unwrap_or_default(),
                &chapter_id.unwrap_or_default(),
                &f.severity.id(),
                &f.category.id(),
                &language.unwrap_or_default(),
                &f.observation,
                &f.observation_en,
                &(f.conditional as i64),
                &f.suppressed_by.clone().unwrap_or_default(),
                &snapshot_id.map(|s| s.to_string()).unwrap_or_default(),
                &now_secs(),
            ],
        )?;
        Ok(id)
    }

    /// Drop a paragraph's persisted findings (a re-engagement replaces them).
    pub fn clear_findings_for_paragraph(&self, paragraph_id: Uuid) -> Result<()> {
        self.engine.execute_with(
            "DELETE FROM editor_findings WHERE paragraph_id = ?",
            &[&paragraph_id.to_string()],
        )
    }

    /// All persisted findings, newest first.
    pub fn list_findings(&self) -> Result<Vec<StoredEditorFinding>> {
        let rows = self.engine.select_all(
            "SELECT id, paragraph_id, severity, category, observation, observation_en, \
                    conditional, suppressed_by \
             FROM editor_findings ORDER BY emitted_at DESC, id",
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let category = EditorCategory::from_id(&text(r.get(3)))?;
                Some(StoredEditorFinding {
                    id: Uuid::parse_str(&text(r.first())).ok()?,
                    paragraph_id: opt_text(r.get(1)).and_then(|s| Uuid::parse_str(&s).ok()),
                    finding: EditorFinding {
                        category,
                        severity: EditorSeverity::from_id(&text(r.get(2))),
                        observation: text(r.get(4)),
                        observation_en: text(r.get(5)),
                        evidence: None,
                        conditional: int(r.get(6)) != 0,
                        suppressed_by: opt_text(r.get(7)),
                    },
                })
            })
            .collect())
    }

    /// A paragraph's findings in chronological order (oldest first) — the
    /// history of what the Editor has observed across re-engagements / drafts.
    pub fn findings_history(&self, paragraph_id: Uuid) -> Result<Vec<(i64, EditorFinding)>> {
        let rows = self.engine.select_all_with(
            "SELECT emitted_at, severity, category, observation, observation_en, \
                    conditional, suppressed_by \
             FROM editor_findings WHERE paragraph_id = ? ORDER BY emitted_at ASC, id",
            &[&paragraph_id.to_string()],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let category = EditorCategory::from_id(&text(r.get(2)))?;
                Some((
                    int(r.first()),
                    EditorFinding {
                        category,
                        severity: EditorSeverity::from_id(&text(r.get(1))),
                        observation: text(r.get(3)),
                        observation_en: text(r.get(4)),
                        evidence: None,
                        conditional: int(r.get(5)) != 0,
                        suppressed_by: opt_text(r.get(6)),
                    },
                ))
            })
            .collect())
    }

    // ── cooldown ────────────────────────────────────────────────────────────

    /// The cooldown row for a paragraph, if any engagement/edit was recorded.
    pub fn cooldown(&self, paragraph_id: Uuid) -> Result<Option<CooldownRow>> {
        let rows = self.engine.select_all_with(
            "SELECT last_engagement_at, last_edit_at FROM editor_cooldown_state \
             WHERE paragraph_id = ?",
            &[&paragraph_id.to_string()],
        )?;
        Ok(rows.first().map(|r| CooldownRow {
            last_engagement_at: int(r.first()),
            last_edit_at: int(r.get(1)),
        }))
    }

    /// Stamp the last Editor engagement for a paragraph (preserves last edit).
    pub fn record_engagement(&self, paragraph_id: Uuid, at: i64) -> Result<()> {
        self.engine.execute_with(
            "INSERT INTO editor_cooldown_state (paragraph_id, last_engagement_at, last_edit_at) \
             VALUES (?, ?, 0) \
             ON CONFLICT (paragraph_id) DO UPDATE SET last_engagement_at = excluded.last_engagement_at",
            &[&paragraph_id.to_string(), &at],
        )
    }

    /// Stamp the last edit for a paragraph (resets the cooldown window).
    pub fn record_edit(&self, paragraph_id: Uuid, at: i64) -> Result<()> {
        self.engine.execute_with(
            "INSERT INTO editor_cooldown_state (paragraph_id, last_engagement_at, last_edit_at) \
             VALUES (?, 0, ?) \
             ON CONFLICT (paragraph_id) DO UPDATE SET last_edit_at = excluded.last_edit_at",
            &[&paragraph_id.to_string(), &at],
        )
    }

    // ── LLM usage (sub-budgeted) ──────────────────────────────────────────────

    /// Record one LLM call against `(day, sub_budget)`; returns the new count.
    pub fn record_llm_call(&self, day: &str, sub_budget: &str) -> Result<i64> {
        self.engine.execute_with(
            "INSERT INTO inner_editor_llm_usage (day, sub_budget, calls) VALUES (?, ?, 1) \
             ON CONFLICT (day, sub_budget) DO UPDATE SET calls = calls + 1",
            &[&day, &sub_budget],
        )?;
        self.llm_calls_today(day, sub_budget)
    }

    /// How many LLM calls have run on `day` for `sub_budget`.
    pub fn llm_calls_today(&self, day: &str, sub_budget: &str) -> Result<i64> {
        let rows = self.engine.select_all_with(
            "SELECT calls FROM inner_editor_llm_usage WHERE day = ? AND sub_budget = ?",
            &[&day, &sub_budget],
        )?;
        Ok(rows.first().map(|r| int(r.first())).unwrap_or(0))
    }

    /// Every `(sub_budget, calls)` recorded on `day` — the cost dashboard
    /// enumerates them without a hardcoded list.
    pub fn llm_usage_today(&self, day: &str) -> Result<Vec<(String, i64)>> {
        let rows = self.engine.select_all_with(
            "SELECT sub_budget, calls FROM inner_editor_llm_usage WHERE day = ? ORDER BY sub_budget",
            &[&day],
        )?;
        Ok(rows.iter().map(|r| (text(r.get(0)), int(r.get(1)))).collect())
    }

    // ── dismissals + promotion (COMPANIONS-1) ────────────────────────────────

    /// Record one dismissal of a `category` finding in `chapter_id` (empty when
    /// the chapter is unknown — those aggregate under `""`).
    pub fn record_dismissal(&self, category: EditorCategory, chapter_id: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT INTO editor_dismissals (id, category, chapter_id, dismissed_at) VALUES (?,?,?,?)",
            &[&Uuid::new_v4().to_string(), &category.id(), &chapter_id, &now_secs()],
        )
    }

    /// Mark a `(category, chapter)` promotion suggestion refused — it won't
    /// re-surface as a candidate.
    pub fn refuse_promotion(&self, category: EditorCategory, chapter_id: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO editor_promotion_refused (category, chapter_id) VALUES (?,?)",
            &[&category.id(), &chapter_id],
        )
    }

    /// Promotion candidates: `(category, chapter, count)` groups whose dismissal
    /// count is at least `threshold` and which the author hasn't refused.
    pub fn promotion_candidates(&self, threshold: i64) -> Result<Vec<EditorPromotionCandidate>> {
        let rows = self.engine.select_all_with(
            "SELECT d.category, d.chapter_id, COUNT(*) AS n \
             FROM editor_dismissals d \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM editor_promotion_refused r \
                 WHERE r.category = d.category AND r.chapter_id = d.chapter_id) \
             GROUP BY d.category, d.chapter_id \
             HAVING COUNT(*) >= ? \
             ORDER BY n DESC",
            &[&threshold],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(EditorPromotionCandidate {
                    category: EditorCategory::from_id(&text(r.first()))?,
                    chapter_id: text(r.get(1)),
                    count: int(r.get(2)),
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn finding(cat: EditorCategory, sev: EditorSeverity, text: &str) -> EditorFinding {
        EditorFinding {
            category: cat,
            severity: sev,
            observation: text.into(),
            observation_en: text.into(),
            evidence: None,
            conditional: true,
            suppressed_by: None,
        }
    }

    fn temp_store() -> (InnerEditorStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let store = InnerEditorStore::open_for_project(dir.path()).unwrap();
        (store, dir)
    }

    #[test]
    fn findings_roundtrip_and_clear() {
        let (store, _d) = temp_store();
        let pid = Uuid::new_v4();
        store
            .insert_finding(
                &finding(EditorCategory::StyleObservation, EditorSeverity::Note, "the rhythm shifts"),
                Some(pid),
                Some("ch07"),
                Some("en"),
                None,
            )
            .unwrap();
        store
            .insert_finding(
                &finding(EditorCategory::CraftPraise, EditorSeverity::Praise, "earned cadence"),
                Some(pid),
                Some("ch07"),
                Some("en"),
                None,
            )
            .unwrap();

        let all = store.list_findings().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all.iter().any(|f| f.finding.severity == EditorSeverity::Praise));

        let hist = store.findings_history(pid).unwrap();
        assert_eq!(hist.len(), 2);

        store.clear_findings_for_paragraph(pid).unwrap();
        assert!(store.list_findings().unwrap().is_empty());
    }

    #[test]
    fn cooldown_edit_and_engagement_are_independent() {
        let (store, _d) = temp_store();
        let pid = Uuid::new_v4();
        assert!(store.cooldown(pid).unwrap().is_none());

        store.record_engagement(pid, 1000).unwrap();
        let c = store.cooldown(pid).unwrap().unwrap();
        assert_eq!(c.last_engagement_at, 1000);
        assert_eq!(c.last_edit_at, 0);

        // An edit during cooldown updates only last_edit_at (preserves engagement).
        store.record_edit(pid, 1050).unwrap();
        let c = store.cooldown(pid).unwrap().unwrap();
        assert_eq!(c.last_engagement_at, 1000);
        assert_eq!(c.last_edit_at, 1050);
    }

    #[test]
    fn promotion_candidates_cross_threshold_and_respect_refusal() {
        let (store, _d) = temp_store();
        // Three dismissals of dictionary_richness in ch07, two of tautology there.
        for _ in 0..3 {
            store.record_dismissal(EditorCategory::DictionaryRichness, "ch07").unwrap();
        }
        for _ in 0..2 {
            store.record_dismissal(EditorCategory::Tautology, "ch07").unwrap();
        }
        // Threshold 3 → only dictionary_richness qualifies.
        let cands = store.promotion_candidates(3).unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].category, EditorCategory::DictionaryRichness);
        assert_eq!(cands[0].count, 3);

        // Refusing it removes it from candidates.
        store.refuse_promotion(EditorCategory::DictionaryRichness, "ch07").unwrap();
        assert!(store.promotion_candidates(3).unwrap().is_empty());
        // Threshold 2 now surfaces tautology (not refused).
        let cands = store.promotion_candidates(2).unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].category, EditorCategory::Tautology);
    }

    #[test]
    fn usage_tallies_per_sub_budget() {
        let (store, _d) = temp_store();
        let day = "2026-06-29";
        assert_eq!(store.record_llm_call(day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET).unwrap(), 1);
        assert_eq!(store.record_llm_call(day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET).unwrap(), 2);
        assert_eq!(store.record_llm_call(day, InnerEditorStore::CONVERSATION_SUB_BUDGET).unwrap(), 1);
        let all = store.llm_usage_today(day).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(store.llm_calls_today(day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET).unwrap(), 2);
    }
}
