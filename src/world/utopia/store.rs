//! WORLD-6 (W-P1) — `.inkhaven/utopia.duckdb`, the per-feature store for
//! extracted claims, pairwise compatibility, coherence findings, and stage
//! cache metadata. Built on the shared pooled [`StorageEngine`], mirroring
//! `DialogueStore`/`ProseStore`: reals/hashes/timestamps as TEXT (hashes are
//! `DefaultHasher` u64 stringified), booleans as INTEGER 0/1.

use std::path::Path;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::{ClaimType, FindingDomain, FindingType, UtopiaClaim, UtopiaFinding};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS utopia_claims (
    book_slug      TEXT NOT NULL,
    premise_group  TEXT NOT NULL,
    claim_id       TEXT NOT NULL,
    claim_type     TEXT NOT NULL,
    claim_text     TEXT NOT NULL,
    source_para_id TEXT NOT NULL,
    computed_at    TEXT NOT NULL,
    content_hash   TEXT NOT NULL,
    PRIMARY KEY (book_slug, claim_id)
);
CREATE TABLE IF NOT EXISTS utopia_pairs (
    book_slug     TEXT NOT NULL,
    premise_group TEXT NOT NULL,
    claim_id_a    TEXT NOT NULL,
    claim_id_b    TEXT NOT NULL,
    compatible    INTEGER,
    reasoning     TEXT,
    computed_at   TEXT NOT NULL,
    PRIMARY KEY (book_slug, claim_id_a, claim_id_b)
);
CREATE TABLE IF NOT EXISTS utopia_findings (
    book_slug            TEXT NOT NULL,
    finding_id           TEXT NOT NULL,
    premise_group        TEXT NOT NULL,
    finding_type         TEXT NOT NULL,
    finding_domain       TEXT NOT NULL DEFAULT 'systemic',
    severity             TEXT NOT NULL DEFAULT 'info',
    description          TEXT NOT NULL,
    evidence             TEXT,
    chapter_ord          INTEGER,
    para_id              TEXT,
    suppressed           INTEGER NOT NULL DEFAULT 0,
    suppression_reason   TEXT,
    grounded_by_research INTEGER NOT NULL DEFAULT 0,
    computed_at          TEXT NOT NULL,
    PRIMARY KEY (book_slug, finding_id)
);
CREATE TABLE IF NOT EXISTS utopia_cache (
    book_slug               TEXT NOT NULL,
    premise_group           TEXT NOT NULL,
    stage1_hash             TEXT NOT NULL DEFAULT '0',
    stage2_complete         INTEGER NOT NULL DEFAULT 0,
    stage3_prose_hash       TEXT,
    research_corpus_version TEXT,
    last_stage1_at          TEXT,
    last_stage2_at          TEXT,
    last_stage3_at          TEXT,
    PRIMARY KEY (book_slug, premise_group)
);
CREATE TABLE IF NOT EXISTS utopia_chapter_scan (
    book_slug     TEXT NOT NULL,
    premise_group TEXT NOT NULL,
    chapter_ord   INTEGER NOT NULL,
    chapter_hash  TEXT NOT NULL,
    scanned_at    TEXT NOT NULL,
    PRIMARY KEY (book_slug, premise_group, chapter_ord)
);
CREATE INDEX IF NOT EXISTS idx_utopia_findings_book_chapter
    ON utopia_findings (book_slug, chapter_ord);
CREATE INDEX IF NOT EXISTS idx_utopia_findings_suppressed
    ON utopia_findings (book_slug, suppressed);
"#;

const CLAIM_COLS: &str =
    "book_slug, premise_group, claim_id, claim_type, claim_text, source_para_id, \
     computed_at, content_hash";

const FINDING_COLS: &str = "book_slug, finding_id, premise_group, finding_type, \
    finding_domain, severity, description, evidence, chapter_ord, para_id, suppressed, \
    suppression_reason, grounded_by_research, computed_at";

pub(crate) struct UtopiaStore {
    engine: StorageEngine,
}

impl UtopiaStore {
    pub(crate) fn open(project_root: &Path) -> Result<UtopiaStore> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)?;
        let engine = StorageEngine::new_versioned(dir.join("utopia.duckdb"), SCHEMA, 4, 1)?;
        Ok(UtopiaStore { engine })
    }

    // ── claims ────────────────────────────────────────────────────────────────

    /// Replace a premise group's claims (Stage 1 re-extraction).
    pub(crate) fn clear_group_claims(&self, book_slug: &str, group: &str) -> Result<()> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        self.engine.execute_with(
            "DELETE FROM utopia_claims WHERE book_slug = ? AND premise_group = ?",
            &[&bs, &g],
        )
    }

    pub(crate) fn upsert_claim(
        &self,
        book_slug: &str,
        c: &UtopiaClaim,
        computed_at: &str,
        content_hash: u64,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let g = c.premise_group.clone();
        let id = c.claim_id.clone();
        let ct = c.claim_type.as_code().to_string();
        let text = c.claim_text.clone();
        let para = c.source_para_id.clone();
        let ca = computed_at.to_string();
        let hash = content_hash.to_string();
        let sql = format!(
            "INSERT OR REPLACE INTO utopia_claims ({CLAIM_COLS}) VALUES (?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> =
            vec![&bs, &g, &id, &ct, &text, &para, &ca, &hash];
        self.engine.execute_with(&sql, &params)
    }

    pub(crate) fn claims_for_group(&self, book_slug: &str, group: &str) -> Result<Vec<UtopiaClaim>> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let sql = format!(
            "SELECT {CLAIM_COLS} FROM utopia_claims WHERE book_slug = ? AND premise_group = ? \
             ORDER BY claim_type"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs, &g])?;
        Ok(rows.iter().filter_map(row_to_claim).collect())
    }

    pub(crate) fn all_claims(&self, book_slug: &str) -> Result<Vec<UtopiaClaim>> {
        let bs = book_slug.to_string();
        let sql = format!(
            "SELECT {CLAIM_COLS} FROM utopia_claims WHERE book_slug = ? ORDER BY premise_group, claim_type"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows.iter().filter_map(row_to_claim).collect())
    }

    // ── pairs ─────────────────────────────────────────────────────────────────

    pub(crate) fn upsert_pair(
        &self,
        book_slug: &str,
        group: &str,
        a: &str,
        b: &str,
        compatible: bool,
        reasoning: &str,
        computed_at: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let g = group.to_string();
        let (a, b) = (a.to_string(), b.to_string());
        let comp = compatible as i64;
        let r = reasoning.to_string();
        let ca = computed_at.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &g, &a, &b, &comp, &r, &ca];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO utopia_pairs \
             (book_slug, premise_group, claim_id_a, claim_id_b, compatible, reasoning, computed_at) \
             VALUES (?,?,?,?,?,?,?)",
            &params,
        )
    }

    // ── findings ──────────────────────────────────────────────────────────────

    /// Delete a group's findings of a given type (before re-detecting). Pass
    /// `None` to clear all of the group's findings.
    pub(crate) fn clear_group_findings(
        &self,
        book_slug: &str,
        group: &str,
        finding_type: Option<FindingType>,
    ) -> Result<()> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        match finding_type {
            Some(ft) => {
                let ftc = ft.as_code().to_string();
                self.engine.execute_with(
                    "DELETE FROM utopia_findings WHERE book_slug = ? AND premise_group = ? AND finding_type = ?",
                    &[&bs, &g, &ftc],
                )
            }
            None => self.engine.execute_with(
                "DELETE FROM utopia_findings WHERE book_slug = ? AND premise_group = ?",
                &[&bs, &g],
            ),
        }
    }

    /// Delete a group's findings of a given reasoning domain — keeps the
    /// Factual cross-reference (domain `factual`) and the Stage-2 chain logic
    /// (domain `systemic`/`logical`) from clobbering each other, since both can
    /// be `ChainBreak`.
    pub(crate) fn clear_group_findings_by_domain(
        &self,
        book_slug: &str,
        group: &str,
        domain: FindingDomain,
    ) -> Result<()> {
        let (bs, g, d) = (book_slug.to_string(), group.to_string(), domain.as_code().to_string());
        self.engine.execute_with(
            "DELETE FROM utopia_findings WHERE book_slug = ? AND premise_group = ? AND finding_domain = ?",
            &[&bs, &g, &d],
        )
    }

    /// Delete a group's entailment-violation findings for one chapter (before
    /// re-scanning that chapter). Per-chapter so a re-scan of one chapter
    /// doesn't drop the others' violations.
    pub(crate) fn clear_chapter_entailment(
        &self,
        book_slug: &str,
        group: &str,
        chapter_ord: u32,
    ) -> Result<()> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let ord = chapter_ord as i64;
        self.engine.execute_with(
            "DELETE FROM utopia_findings WHERE book_slug = ? AND premise_group = ? \
             AND chapter_ord = ? AND finding_type = 'entailment_violation'",
            &[&bs, &g, &ord],
        )
    }

    pub(crate) fn upsert_finding(
        &self,
        book_slug: &str,
        f: &UtopiaFinding,
        computed_at: &str,
        suppression_reason: Option<&str>,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let id = f.finding_id.clone();
        let g = f.premise_group.clone();
        let ft = f.finding_type.as_code().to_string();
        let fd = f.finding_domain.as_code().to_string();
        let sev = "info".to_string();
        let desc = f.description.clone();
        let ev = f.evidence.clone();
        let ch = f.chapter_ord.map(|c| c as i64);
        let para = f.para_id.clone();
        let sup = f.suppressed as i64;
        let reason = suppression_reason.map(str::to_string);
        let gbr = f.grounded_by_research as i64;
        let ca = computed_at.to_string();
        let sql = format!(
            "INSERT OR REPLACE INTO utopia_findings ({FINDING_COLS}) VALUES \
             (?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> = vec![
            &bs, &id, &g, &ft, &fd, &sev, &desc, &ev, &ch, &para, &sup, &reason, &gbr, &ca,
        ];
        self.engine.execute_with(&sql, &params)
    }

    /// All findings for a book (optionally only unsuppressed). Ordered chain
    /// logic first (chain_break, consequence_gap, internal_conflict) then
    /// entailment.
    pub(crate) fn findings(
        &self,
        book_slug: &str,
        unsuppressed_only: bool,
    ) -> Result<Vec<UtopiaFinding>> {
        let bs = book_slug.to_string();
        let where_sup = if unsuppressed_only { " AND suppressed = 0" } else { "" };
        let sql = format!(
            "SELECT {FINDING_COLS} FROM utopia_findings WHERE book_slug = ?{where_sup} \
             ORDER BY (finding_type = 'entailment_violation'), finding_type, chapter_ord"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows.iter().filter_map(row_to_finding).collect())
    }

    pub(crate) fn suppress_finding(
        &self,
        book_slug: &str,
        finding_id: &str,
        reason: &str,
    ) -> Result<bool> {
        let (bs, id, r) = (book_slug.to_string(), finding_id.to_string(), reason.to_string());
        // Verify it exists first (so the CLI can report unknown ids).
        let exists = !self
            .engine
            .select_all_with(
                "SELECT finding_id FROM utopia_findings WHERE book_slug = ? AND finding_id = ?",
                &[&bs, &id],
            )?
            .is_empty();
        if !exists {
            return Ok(false);
        }
        self.engine.execute_with(
            "UPDATE utopia_findings SET suppressed = 1, suppression_reason = ? \
             WHERE book_slug = ? AND finding_id = ?",
            &[&r, &bs, &id],
        )?;
        Ok(true)
    }

    // ── cache ─────────────────────────────────────────────────────────────────

    pub(crate) fn stage1_hash(&self, book_slug: &str, group: &str) -> Result<Option<u64>> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let rows = self.engine.select_all_with(
            "SELECT stage1_hash FROM utopia_cache WHERE book_slug = ? AND premise_group = ?",
            &[&bs, &g],
        )?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .and_then(as_text)
            .and_then(|s| s.parse().ok()))
    }

    pub(crate) fn set_stage1(&self, book_slug: &str, group: &str, hash: u64, at: &str) -> Result<()> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let h = hash.to_string();
        let at = at.to_string();
        // Upsert the cache row, resetting stage2_complete (claims changed).
        self.engine.execute_with(
            "INSERT INTO utopia_cache (book_slug, premise_group, stage1_hash, stage2_complete, last_stage1_at) \
             VALUES (?,?,?,0,?) \
             ON CONFLICT (book_slug, premise_group) DO UPDATE SET \
               stage1_hash = excluded.stage1_hash, stage2_complete = 0, last_stage1_at = excluded.last_stage1_at",
            &[&bs, &g, &h, &at],
        )
    }

    pub(crate) fn stage2_complete(&self, book_slug: &str, group: &str) -> Result<bool> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let rows = self.engine.select_all_with(
            "SELECT stage2_complete FROM utopia_cache WHERE book_slug = ? AND premise_group = ?",
            &[&bs, &g],
        )?;
        Ok(rows.first().and_then(|r| r.first()).and_then(as_i64).unwrap_or(0) != 0)
    }

    pub(crate) fn set_stage2_complete(&self, book_slug: &str, group: &str, at: &str) -> Result<()> {
        let (bs, g, at) = (book_slug.to_string(), group.to_string(), at.to_string());
        self.engine.execute_with(
            "UPDATE utopia_cache SET stage2_complete = 1, last_stage2_at = ? \
             WHERE book_slug = ? AND premise_group = ?",
            &[&at, &bs, &g],
        )
    }

    /// Force Stage 1 re-extraction for every group (zero the cache hash).
    pub(crate) fn invalidate_all_stage1(&self, book_slug: &str) -> Result<()> {
        let bs = book_slug.to_string();
        self.engine.execute_with(
            "UPDATE utopia_cache SET stage1_hash = '0', stage2_complete = 0 WHERE book_slug = ?",
            &[&bs],
        )
    }

    /// Force Stage 3 re-scan (drop every per-chapter scan record).
    pub(crate) fn clear_chapter_scans(&self, book_slug: &str) -> Result<()> {
        let bs = book_slug.to_string();
        self.engine.execute_with(
            "DELETE FROM utopia_chapter_scan WHERE book_slug = ?",
            &[&bs],
        )
    }

    /// Per-chapter Stage 3 cache: the stored chapter hash, if scanned.
    pub(crate) fn chapter_scan_hash(
        &self,
        book_slug: &str,
        group: &str,
        chapter_ord: u32,
    ) -> Result<Option<u64>> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let ord = chapter_ord as i64;
        let rows = self.engine.select_all_with(
            "SELECT chapter_hash FROM utopia_chapter_scan \
             WHERE book_slug = ? AND premise_group = ? AND chapter_ord = ?",
            &[&bs, &g, &ord],
        )?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .and_then(as_text)
            .and_then(|s| s.parse().ok()))
    }

    pub(crate) fn set_chapter_scanned(
        &self,
        book_slug: &str,
        group: &str,
        chapter_ord: u32,
        hash: u64,
        at: &str,
    ) -> Result<()> {
        let (bs, g) = (book_slug.to_string(), group.to_string());
        let ord = chapter_ord as i64;
        let h = hash.to_string();
        let at = at.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &g, &ord, &h, &at];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO utopia_chapter_scan \
             (book_slug, premise_group, chapter_ord, chapter_hash, scanned_at) VALUES (?,?,?,?,?)",
            &params,
        )
    }
}

// ── DuckValue parsing ─────────────────────────────────────────────────────────

fn as_text(v: &DuckValue) -> Option<String> {
    match v {
        DuckValue::Text(s) => Some(s.clone()),
        _ => None,
    }
}
fn as_i64(v: &DuckValue) -> Option<i64> {
    match v {
        DuckValue::Int(i) => Some(*i as i64),
        DuckValue::BigInt(i) => Some(*i),
        DuckValue::HugeInt(i) => Some(*i as i64),
        _ => None,
    }
}

fn row_to_claim(r: &Vec<DuckValue>) -> Option<UtopiaClaim> {
    if r.len() < 8 {
        return None;
    }
    Some(UtopiaClaim {
        premise_group: as_text(&r[1])?,
        claim_id: as_text(&r[2])?,
        claim_type: ClaimType::from_code(&as_text(&r[3])?)?,
        claim_text: as_text(&r[4]).unwrap_or_default(),
        source_para_id: as_text(&r[5]).unwrap_or_default(),
    })
}

fn row_to_finding(r: &Vec<DuckValue>) -> Option<UtopiaFinding> {
    if r.len() < 14 {
        return None;
    }
    Some(UtopiaFinding {
        finding_id: as_text(&r[1])?,
        premise_group: as_text(&r[2]).unwrap_or_default(),
        finding_type: FindingType::from_code(&as_text(&r[3])?)?,
        finding_domain: FindingDomain::from_code(&as_text(&r[4]).unwrap_or_default()),
        description: as_text(&r[6]).unwrap_or_default(),
        evidence: as_text(&r[7]),
        chapter_ord: as_i64(&r[8]).map(|c| c as u32),
        para_id: as_text(&r[9]),
        suppressed: as_i64(&r[10]).unwrap_or(0) != 0,
        grounded_by_research: as_i64(&r[12]).unwrap_or(0) != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(id: &str, group: &str, ct: ClaimType) -> UtopiaClaim {
        UtopiaClaim {
            claim_id: id.into(),
            premise_group: group.into(),
            claim_type: ct,
            claim_text: format!("text {id}"),
            source_para_id: format!("para-{id}"),
        }
    }
    fn finding(id: &str, group: &str, ft: FindingType) -> UtopiaFinding {
        UtopiaFinding {
            finding_id: id.into(),
            premise_group: group.into(),
            finding_type: ft,
            finding_domain: FindingDomain::Systemic,
            description: format!("desc {id}"),
            evidence: Some("ev".into()),
            chapter_ord: None,
            para_id: None,
            suppressed: false,
            grounded_by_research: false,
        }
    }

    #[test]
    fn claims_round_trip_and_clear() {
        let dir = tempfile::tempdir().unwrap();
        let st = UtopiaStore::open(dir.path()).unwrap();
        st.upsert_claim("b", &claim("c1", "g1", ClaimType::Premise), "now", 7).unwrap();
        st.upsert_claim("b", &claim("c2", "g1", ClaimType::Mechanism), "now", 7).unwrap();
        assert_eq!(st.claims_for_group("b", "g1").unwrap().len(), 2);
        st.clear_group_claims("b", "g1").unwrap();
        assert!(st.claims_for_group("b", "g1").unwrap().is_empty());
    }

    #[test]
    fn findings_suppress_and_filter() {
        let dir = tempfile::tempdir().unwrap();
        let st = UtopiaStore::open(dir.path()).unwrap();
        st.upsert_finding("b", &finding("f1", "g1", FindingType::ChainBreak), "now", None).unwrap();
        st.upsert_finding("b", &finding("f2", "g1", FindingType::InternalConflict), "now", None).unwrap();
        assert_eq!(st.findings("b", true).unwrap().len(), 2);
        assert!(st.suppress_finding("b", "f1", "deliberate irony").unwrap());
        assert_eq!(st.findings("b", true).unwrap().len(), 1); // f1 hidden
        assert_eq!(st.findings("b", false).unwrap().len(), 2); // both with all
        assert!(!st.suppress_finding("b", "nope", "x").unwrap()); // unknown id
    }

    #[test]
    fn stage_cache_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let st = UtopiaStore::open(dir.path()).unwrap();
        st.set_stage1("b", "g1", 42, "now").unwrap();
        assert_eq!(st.stage1_hash("b", "g1").unwrap(), Some(42));
        assert!(!st.stage2_complete("b", "g1").unwrap());
        st.set_stage2_complete("b", "g1", "now").unwrap();
        assert!(st.stage2_complete("b", "g1").unwrap());
        // A new stage1 hash resets stage2_complete.
        st.set_stage1("b", "g1", 99, "later").unwrap();
        assert_eq!(st.stage1_hash("b", "g1").unwrap(), Some(99));
        assert!(!st.stage2_complete("b", "g1").unwrap());
    }

    #[test]
    fn chapter_scan_cache() {
        let dir = tempfile::tempdir().unwrap();
        let st = UtopiaStore::open(dir.path()).unwrap();
        assert_eq!(st.chapter_scan_hash("b", "g1", 12).unwrap(), None);
        st.set_chapter_scanned("b", "g1", 12, 555, "now").unwrap();
        assert_eq!(st.chapter_scan_hash("b", "g1", 12).unwrap(), Some(555));
    }
}
