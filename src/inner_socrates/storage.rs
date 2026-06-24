//! The per-project Inner Socrates store (`<project>/inner_socrates.db`). Persists
//! emitted Socratic findings (so a re-check can replace a paragraph's prior ones,
//! and the snapshot log can track resolutions later) and the **intent ledger**.
//! Built on the in-tree `StorageEngine`, exactly like `WorldStore`.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;
use uuid::Uuid;

use crate::storage::engine::StorageEngine;
use crate::world::proposals::now_secs;

use super::intent::{IntentEntry, IntentKind, IntentLedger, IntentScope, ScopeLevel};
use super::types::{Category, Severity, SocraticFinding, Track};

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS socratic_findings (
        id           TEXT   NOT NULL PRIMARY KEY,
        paragraph_id TEXT,
        chapter_id   TEXT,
        track        TEXT   NOT NULL,
        category     TEXT   NOT NULL,
        severity     TEXT   NOT NULL,
        persona_id   TEXT   NOT NULL,
        question     TEXT   NOT NULL,
        question_en  TEXT   NOT NULL,
        suppressed_by TEXT,
        emitted_at   BIGINT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_sf_para ON socratic_findings(paragraph_id);

    -- Dismissals — each time the author dismisses a finding (the raw material the
    -- promotion mechanism aggregates).
    CREATE TABLE IF NOT EXISTS socratic_dismissals (
        id           TEXT   NOT NULL PRIMARY KEY,
        category     TEXT   NOT NULL,
        chapter_id   TEXT   NOT NULL,
        dismissed_at BIGINT NOT NULL
    );
    -- Promotion suggestions the author refused (won't re-suggest).
    CREATE TABLE IF NOT EXISTS socratic_promotion_refused (
        category   TEXT NOT NULL,
        chapter_id TEXT NOT NULL,
        PRIMARY KEY (category, chapter_id)
    );

    -- The active Reader Persona for this project (one row).
    CREATE TABLE IF NOT EXISTS active_persona (
        singleton  INTEGER NOT NULL PRIMARY KEY DEFAULT 1,
        persona_id TEXT    NOT NULL,
        selected_at BIGINT NOT NULL
    );

    -- Slow-track LLM usage, per day per sub-budget (RFC §3.14 — separate from
    -- WORLD-4's tally).
    CREATE TABLE IF NOT EXISTS inner_socrates_llm_usage (
        day        TEXT   NOT NULL,
        sub_budget TEXT   NOT NULL,
        calls      BIGINT NOT NULL,
        PRIMARY KEY (day, sub_budget)
    );

    -- The intent ledger — deliberate authorial choices the interrogator respects.
    CREATE TABLE IF NOT EXISTS intent_entries (
        id          TEXT   NOT NULL PRIMARY KEY,
        kind        TEXT   NOT NULL,
        description TEXT   NOT NULL,
        scope_type  TEXT   NOT NULL,
        scope_data  TEXT   NOT NULL,
        coverage    TEXT   NOT NULL,
        scope_level TEXT   NOT NULL,
        created_at  BIGINT NOT NULL
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

/// An accumulated dismissal pattern the author might want to declare as intent.
#[derive(Debug, Clone, PartialEq)]
pub struct PromotionCandidate {
    pub category: Category,
    /// The chapter the dismissals clustered in (`""` = unknown).
    pub chapter_id: String,
    pub count: i64,
}

/// A persisted finding plus the paragraph it was emitted against.
#[derive(Debug, Clone)]
pub struct StoredFinding {
    pub id: Uuid,
    pub paragraph_id: Option<Uuid>,
    pub finding: SocraticFinding,
}

/// Per-project Inner Socrates store. Cloneable; clones share the pool.
#[derive(Clone)]
pub struct InnerSocratesStore {
    engine: Arc<StorageEngine>,
}

impl InnerSocratesStore {
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self { engine: Arc::new(StorageEngine::new(path, INIT_SQL, 2)?) })
    }

    /// `<project>/inner_socrates.db`, beside `world.db` / `output.db`.
    pub fn open_for_project(project_root: &Path) -> Result<Self> {
        Self::open(&project_root.join("inner_socrates.db"))
    }

    // ── findings ──────────────────────────────────────────────────────────────

    /// Persist an emitted finding; returns its new id.
    pub fn insert_finding(
        &self,
        f: &SocraticFinding,
        paragraph_id: Option<Uuid>,
        chapter_id: Option<&str>,
    ) -> Result<Uuid> {
        let id = Uuid::new_v4();
        let track = match f.category.track() {
            Track::Fast => "fast",
            Track::Slow => "slow",
        };
        self.engine.execute_with(
            "INSERT INTO socratic_findings \
             (id, paragraph_id, chapter_id, track, category, severity, persona_id, \
              question, question_en, suppressed_by, emitted_at) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?)",
            &[
                &id.to_string(),
                &paragraph_id.map(|p| p.to_string()).unwrap_or_default(),
                &chapter_id.unwrap_or_default(),
                &track,
                &f.category.id(),
                &severity_id(f.severity),
                &f.persona_id,
                &f.question,
                &f.question_en,
                &f.suppressed_by.clone().unwrap_or_default(),
                &now_secs(),
            ],
        )?;
        Ok(id)
    }

    /// Drop a paragraph's persisted findings (a re-check replaces them).
    pub fn clear_findings_for_paragraph(&self, paragraph_id: Uuid) -> Result<()> {
        self.engine.execute_with(
            "DELETE FROM socratic_findings WHERE paragraph_id = ?",
            &[&paragraph_id.to_string()],
        )
    }

    /// All persisted findings, newest first.
    pub fn list_findings(&self) -> Result<Vec<StoredFinding>> {
        let rows = self.engine.select_all(
            "SELECT id, paragraph_id, category, severity, persona_id, question, question_en, suppressed_by \
             FROM socratic_findings ORDER BY emitted_at DESC, id",
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let category = Category::from_id(&text(r.get(2)))?;
                Some(StoredFinding {
                    id: Uuid::parse_str(&text(r.first())).ok()?,
                    paragraph_id: opt_text(r.get(1)).and_then(|s| Uuid::parse_str(&s).ok()),
                    finding: SocraticFinding {
                        category,
                        severity: severity_from_id(&text(r.get(3))),
                        persona_id: text(r.get(4)),
                        question: text(r.get(5)),
                        question_en: text(r.get(6)),
                        suppressed_by: opt_text(r.get(7)),
                    },
                })
            })
            .collect())
    }

    /// A paragraph's findings in chronological order (oldest first) — the history
    /// of what the interrogator has asked about it across re-checks / drafts.
    pub fn findings_history(&self, paragraph_id: Uuid) -> Result<Vec<(i64, SocraticFinding)>> {
        let rows = self.engine.select_all_with(
            "SELECT emitted_at, category, severity, persona_id, question, question_en, suppressed_by \
             FROM socratic_findings WHERE paragraph_id = ? ORDER BY emitted_at ASC, id",
            &[&paragraph_id.to_string()],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let category = Category::from_id(&text(r.get(1)))?;
                Some((
                    int(r.first()),
                    SocraticFinding {
                        category,
                        severity: severity_from_id(&text(r.get(2))),
                        persona_id: text(r.get(3)),
                        question: text(r.get(4)),
                        question_en: text(r.get(5)),
                        suppressed_by: opt_text(r.get(6)),
                    },
                ))
            })
            .collect())
    }

    // ── intent ledger ──────────────────────────────────────────────────────────

    /// Insert or replace an intent entry.
    pub fn add_intent(&self, e: &IntentEntry) -> Result<()> {
        let (scope_type, scope_data) = scope_to_row(&e.scope);
        let coverage = serde_json::to_string(&e.coverage.iter().map(|c| c.id()).collect::<Vec<_>>())
            .unwrap_or_else(|_| "[]".into());
        self.engine.execute_with(
            "INSERT OR REPLACE INTO intent_entries \
             (id, kind, description, scope_type, scope_data, coverage, scope_level, created_at) \
             VALUES (?,?,?,?,?,?,?,?)",
            &[
                &e.id,
                &e.kind.id(),
                &e.description,
                &scope_type,
                &scope_data,
                &coverage,
                &scope_level_id(e.scope_level),
                &now_secs(),
            ],
        )
    }

    pub fn remove_intent(&self, id: &str) -> Result<()> {
        self.engine.execute_with("DELETE FROM intent_entries WHERE id = ?", &[&id])
    }

    /// All intent entries.
    pub fn list_intents(&self) -> Result<Vec<IntentEntry>> {
        let rows = self.engine.select_all(
            "SELECT id, kind, description, scope_type, scope_data, coverage, scope_level \
             FROM intent_entries ORDER BY created_at DESC, id",
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let kind = IntentKind::from_id(&text(r.get(1)))?;
                let scope = row_to_scope(&text(r.get(3)), &text(r.get(4)))?;
                let coverage: Vec<Category> =
                    serde_json::from_str::<Vec<String>>(&text(r.get(5)))
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|s| Category::from_id(s))
                        .collect();
                Some(IntentEntry {
                    id: text(r.first()),
                    kind,
                    description: text(r.get(2)),
                    scope,
                    coverage,
                    scope_level: scope_level_from_id(&text(r.get(6))),
                })
            })
            .collect())
    }

    /// The intent ledger, loaded for consultation.
    pub fn load_ledger(&self) -> Result<IntentLedger> {
        Ok(IntentLedger { entries: self.list_intents()? })
    }

    // ── dismissals + promotion ──────────────────────────────────────────────────

    /// Record one dismissal of a `category` finding in `chapter_id` (empty when
    /// the chapter is unknown — those aggregate under `""`).
    pub fn record_dismissal(&self, category: Category, chapter_id: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT INTO socratic_dismissals (id, category, chapter_id, dismissed_at) VALUES (?,?,?,?)",
            &[&Uuid::new_v4().to_string(), &category.id(), &chapter_id, &now_secs()],
        )
    }

    /// Mark a `(category, chapter)` promotion suggestion as refused — it won't
    /// re-surface as a candidate.
    pub fn refuse_promotion(&self, category: Category, chapter_id: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO socratic_promotion_refused (category, chapter_id) VALUES (?,?)",
            &[&category.id(), &chapter_id],
        )
    }

    /// Promotion candidates: `(category, chapter, count)` groups whose dismissal
    /// count is at least `threshold` and which the author hasn't refused.
    pub fn promotion_candidates(&self, threshold: i64) -> Result<Vec<PromotionCandidate>> {
        let rows = self.engine.select_all_with(
            "SELECT d.category, d.chapter_id, COUNT(*) AS n \
             FROM socratic_dismissals d \
             WHERE NOT EXISTS ( \
                 SELECT 1 FROM socratic_promotion_refused r \
                 WHERE r.category = d.category AND r.chapter_id = d.chapter_id) \
             GROUP BY d.category, d.chapter_id \
             HAVING COUNT(*) >= ? \
             ORDER BY n DESC",
            &[&threshold],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some(PromotionCandidate {
                    category: Category::from_id(&text(r.first()))?,
                    chapter_id: text(r.get(1)),
                    count: int(r.get(2)),
                })
            })
            .collect())
    }

    // ── active persona ──────────────────────────────────────────────────────────

    /// The active persona id for this project, if one has been chosen.
    pub fn active_persona_id(&self) -> Result<Option<String>> {
        let rows = self.engine.select_all("SELECT persona_id FROM active_persona WHERE singleton = 1")?;
        Ok(rows.first().map(|r| text(r.first())).filter(|s| !s.is_empty()))
    }

    /// Set the active persona (idempotent on the singleton row).
    pub fn set_active_persona(&self, persona_id: &str) -> Result<()> {
        self.engine.execute_with(
            "INSERT OR REPLACE INTO active_persona (singleton, persona_id, selected_at) VALUES (1, ?, ?)",
            &[&persona_id, &now_secs()],
        )
    }

    // ── LLM usage (sub-budgeted) ────────────────────────────────────────────────

    /// Daily ceiling on Inner Socrates slow-track LLM calls (shared by the
    /// slow-track preflight and the cost dashboard).
    pub const DAILY_CALL_CAP: i64 = 150;
    /// The slow-track usage sub-budget key.
    pub const SLOW_SUB_BUDGET: &'static str = "slow_track";

    /// Record one LLM call against `(day, sub_budget)`; returns the new count.
    pub fn record_llm_call(&self, day: &str, sub_budget: &str) -> Result<i64> {
        self.engine.execute_with(
            "INSERT INTO inner_socrates_llm_usage (day, sub_budget, calls) VALUES (?, ?, 1) \
             ON CONFLICT (day, sub_budget) DO UPDATE SET calls = calls + 1",
            &[&day, &sub_budget],
        )?;
        self.llm_calls_today(day, sub_budget)
    }

    /// How many LLM calls have run on `day` for `sub_budget`.
    pub fn llm_calls_today(&self, day: &str, sub_budget: &str) -> Result<i64> {
        let rows = self.engine.select_all_with(
            "SELECT calls FROM inner_socrates_llm_usage WHERE day = ? AND sub_budget = ?",
            &[&day, &sub_budget],
        )?;
        Ok(rows.first().map(|r| int(r.first())).unwrap_or(0))
    }

    /// Every `(sub_budget, calls)` recorded on `day`. Each analytical thread that
    /// runs a cost-capped slow pass records under its own sub-budget key, so the
    /// cost dashboard enumerates them all here without a hardcoded list — a new
    /// thread shows up automatically once it records its first call.
    pub fn llm_usage_today(&self, day: &str) -> Result<Vec<(String, i64)>> {
        let rows = self.engine.select_all_with(
            "SELECT sub_budget, calls FROM inner_socrates_llm_usage WHERE day = ? ORDER BY sub_budget",
            &[&day],
        )?;
        Ok(rows.iter().map(|r| (text(r.get(0)), int(r.get(1)))).collect())
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

fn severity_id(s: Severity) -> &'static str {
    match s {
        Severity::Notice => "notice",
        Severity::Inquiry => "inquiry",
        Severity::Probe => "probe",
    }
}

fn severity_from_id(s: &str) -> Severity {
    match s {
        "probe" => Severity::Probe,
        "inquiry" => Severity::Inquiry,
        _ => Severity::Notice,
    }
}

fn scope_level_id(s: ScopeLevel) -> &'static str {
    match s {
        ScopeLevel::Project => "project",
        ScopeLevel::Series => "series",
    }
}

fn scope_level_from_id(s: &str) -> ScopeLevel {
    match s {
        "series" => ScopeLevel::Series,
        _ => ScopeLevel::Project,
    }
}

/// Serialize a scope to its `(type, json-data)` row form.
fn scope_to_row(scope: &IntentScope) -> (String, String) {
    use serde_json::json;
    let (ty, data) = match scope {
        IntentScope::Project => ("project", json!({})),
        IntentScope::Chapter(c) => ("chapter", json!({ "chapter": c })),
        IntentScope::ParagraphRange { from, to } => {
            ("paragraph_range", json!({ "from": from, "to": to }))
        }
        IntentScope::Character(id) => ("character", json!({ "character": id })),
        IntentScope::Scene(s) => ("scene", json!({ "scene": s })),
        IntentScope::TimelineRange { from, to } => {
            ("timeline_range", json!({ "from": from, "to": to }))
        }
    };
    (ty.to_string(), data.to_string())
}

/// Parse a scope back from its row form.
fn row_to_scope(scope_type: &str, scope_data: &str) -> Option<IntentScope> {
    let v: serde_json::Value = serde_json::from_str(scope_data).unwrap_or(serde_json::json!({}));
    let s = |k: &str| v.get(k).and_then(|x| x.as_str()).unwrap_or("").to_string();
    Some(match scope_type {
        "project" => IntentScope::Project,
        "chapter" => IntentScope::Chapter(s("chapter")),
        "paragraph_range" => IntentScope::ParagraphRange { from: s("from"), to: s("to") },
        "character" => IntentScope::Character(s("character")),
        "scene" => IntentScope::Scene(s("scene")),
        "timeline_range" => IntentScope::TimelineRange { from: s("from"), to: s("to") },
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> InnerSocratesStore {
        InnerSocratesStore::open(Path::new(":memory:")).unwrap()
    }

    fn finding() -> SocraticFinding {
        SocraticFinding {
            category: Category::ModalClaims,
            severity: Severity::Inquiry,
            persona_id: "inner-socrates".into(),
            question: "What alternatives did you leave out?".into(),
            question_en: "What alternatives did you leave out?".into(),
            suppressed_by: None,
        }
    }

    #[test]
    fn findings_roundtrip_and_clear() {
        let s = store();
        let p = Uuid::new_v4();
        s.insert_finding(&finding(), Some(p), Some("ch07")).unwrap();
        let listed = s.list_findings().unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].paragraph_id, Some(p));
        assert_eq!(listed[0].finding.category, Category::ModalClaims);
        assert_eq!(listed[0].finding.severity, Severity::Inquiry);

        s.clear_findings_for_paragraph(p).unwrap();
        assert!(s.list_findings().unwrap().is_empty());
    }

    #[test]
    fn intent_entries_roundtrip_with_scope_and_coverage() {
        let s = store();
        let e = IntentEntry {
            id: "e1".into(),
            kind: IntentKind::DeliberateAmbiguity,
            description: "Mara's loyalty is intentionally unresolved".into(),
            scope: IntentScope::ParagraphRange { from: "ch07-p042".into(), to: "ch07-p051".into() },
            coverage: vec![Category::AssumptionSurfacing, Category::TensionDetection],
            scope_level: ScopeLevel::Series,
        };
        s.add_intent(&e).unwrap();
        let back = s.list_intents().unwrap();
        assert_eq!(back.len(), 1);
        let r = &back[0];
        assert_eq!(r.id, "e1");
        assert_eq!(r.kind, IntentKind::DeliberateAmbiguity);
        assert_eq!(r.scope_level, ScopeLevel::Series);
        assert_eq!(r.coverage, vec![Category::AssumptionSurfacing, Category::TensionDetection]);
        assert!(matches!(&r.scope, IntentScope::ParagraphRange { from, to }
            if from == "ch07-p042" && to == "ch07-p051"));

        // The loaded ledger consults correctly.
        let ledger = s.load_ledger().unwrap();
        let ctx = super::super::intent::FindingContext {
            paragraph_id: Some("ch07-p045".into()),
            ..Default::default()
        };
        assert!(matches!(
            ledger.consult(Category::AssumptionSurfacing, &ctx),
            super::super::intent::ConsultationResult::Suppress { .. }
        ));

        s.remove_intent("e1").unwrap();
        assert!(s.list_intents().unwrap().is_empty());
    }

    #[test]
    fn dismissals_become_promotion_candidates() {
        let s = store();
        // Four dismissals of framing in ch12 — below the threshold of 5.
        for _ in 0..4 {
            s.record_dismissal(Category::FramingInterrogation, "ch12").unwrap();
        }
        assert!(s.promotion_candidates(5).unwrap().is_empty());
        // The fifth crosses the threshold.
        s.record_dismissal(Category::FramingInterrogation, "ch12").unwrap();
        let cands = s.promotion_candidates(5).unwrap();
        assert_eq!(cands.len(), 1);
        assert_eq!(cands[0].category, Category::FramingInterrogation);
        assert_eq!(cands[0].chapter_id, "ch12");
        assert_eq!(cands[0].count, 5);
        // Refusing it removes it from the candidate list.
        s.refuse_promotion(Category::FramingInterrogation, "ch12").unwrap();
        assert!(s.promotion_candidates(5).unwrap().is_empty());
    }

    #[test]
    fn active_persona_roundtrips() {
        let s = store();
        assert_eq!(s.active_persona_id().unwrap(), None);
        s.set_active_persona("skeptical-reader").unwrap();
        assert_eq!(s.active_persona_id().unwrap().as_deref(), Some("skeptical-reader"));
    }

    #[test]
    fn timeline_scope_roundtrips() {
        let s = store();
        let e = IntentEntry {
            id: "t1".into(),
            kind: IntentKind::DeliberateTemporalAmbiguity,
            description: "Years 90-95 intentionally unresolved".into(),
            scope: IntentScope::TimelineRange { from: "1A.090".into(), to: "1A.095".into() },
            coverage: vec![Category::DramatizationGap],
            scope_level: ScopeLevel::Project,
        };
        s.add_intent(&e).unwrap();
        let back = s.list_intents().unwrap();
        assert!(matches!(&back[0].scope, IntentScope::TimelineRange { from, to }
            if from == "1A.090" && to == "1A.095"));
    }
}
