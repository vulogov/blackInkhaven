//! INNER-THEOLOGIAN-1 (IT-P2) — the fast-track finding store over its own
//! `inner_theologian.db` (beside `inner_socrates.db` / `output.db`). Modelled on
//! `socratic_findings`; one table, no migration. The audit corrected the RFC's
//! "extend inner_socrates.duckdb with a kind column" — that store has no session
//! table; each Inner-family member owns its own DuckDB file.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::{SignalType, TheologianFinding};

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS theologian_findings (
        id           TEXT    NOT NULL PRIMARY KEY,
        book_slug    TEXT    NOT NULL,
        signal_type  TEXT    NOT NULL,
        chapter_ord  INTEGER NOT NULL,
        para_id      TEXT    NOT NULL,
        description  TEXT    NOT NULL,
        suppressed   INTEGER NOT NULL DEFAULT 0,
        text_hash    TEXT    NOT NULL,
        emitted_at   TEXT    NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_tf_book ON theologian_findings(book_slug);
";

fn as_text(v: Option<&DuckValue>) -> Option<String> {
    match v {
        Some(DuckValue::Text(s)) => Some(s.clone()),
        _ => None,
    }
}

fn as_i64(v: Option<&DuckValue>) -> Option<i64> {
    match v {
        Some(DuckValue::Int(i)) => Some(*i as i64),
        Some(DuckValue::BigInt(i)) => Some(*i),
        Some(DuckValue::HugeInt(i)) => Some(*i as i64),
        _ => None,
    }
}

pub(crate) struct TheologianStore {
    engine: Arc<StorageEngine>,
}

impl TheologianStore {
    pub(crate) fn open(project_root: &Path) -> Result<Self> {
        let path = project_root.join("inner_theologian.db");
        Ok(Self { engine: Arc::new(StorageEngine::new_versioned(&path, INIT_SQL, 2, 1)?) })
    }

    /// Drop a chapter's findings before a re-scan re-emits them.
    pub(crate) fn clear_chapter(&self, book_slug: &str, chapter_ord: u32) -> Result<()> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &ord];
        self.engine.execute_with(
            "DELETE FROM theologian_findings WHERE book_slug = ? AND chapter_ord = ?",
            &params,
        )
    }

    /// Insert one finding. `text_hash` ties suppression to the paragraph's
    /// content (suppression clears when the paragraph changes — IT-P8).
    pub(crate) fn upsert_finding(
        &self,
        book_slug: &str,
        f: &TheologianFinding,
        text_hash: u64,
        now: &str,
    ) -> Result<()> {
        let id = format!("{book_slug}:{}:{}", f.para_id, f.signal_type.as_code());
        let bs = book_slug.to_string();
        let st = f.signal_type.as_code().to_string();
        let ord = f.chapter_ord as i64;
        let pid = f.para_id.clone();
        let desc = f.description.clone();
        let supp = i64::from(f.suppressed);
        let th = text_hash.to_string();
        let at = now.to_string();
        let params: Vec<&dyn duckdb::ToSql> =
            vec![&id, &bs, &st, &ord, &pid, &desc, &supp, &th, &at];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO theologian_findings \
             (id, book_slug, signal_type, chapter_ord, para_id, description, suppressed, text_hash, emitted_at) \
             VALUES (?,?,?,?,?,?,?,?,?)",
            &params,
        )
    }

    /// All findings for a book, optionally including suppressed ones.
    pub(crate) fn findings(&self, book_slug: &str, include_suppressed: bool) -> Result<Vec<TheologianFinding>> {
        let bs = book_slug.to_string();
        let sql = if include_suppressed {
            "SELECT signal_type, chapter_ord, para_id, description, suppressed \
             FROM theologian_findings WHERE book_slug = ? ORDER BY chapter_ord, para_id"
        } else {
            "SELECT signal_type, chapter_ord, para_id, description, suppressed \
             FROM theologian_findings WHERE book_slug = ? AND suppressed = 0 ORDER BY chapter_ord, para_id"
        };
        let rows = self.engine.select_all_with(sql, &[&bs])?;
        Ok(rows.iter().filter_map(row_to_finding).collect())
    }

    /// Findings for one chapter (unsuppressed).
    pub(crate) fn findings_for_chapter(&self, book_slug: &str, chapter_ord: u32) -> Result<Vec<TheologianFinding>> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let rows = self.engine.select_all_with(
            "SELECT signal_type, chapter_ord, para_id, description, suppressed \
             FROM theologian_findings WHERE book_slug = ? AND chapter_ord = ? AND suppressed = 0 \
             ORDER BY para_id",
            &[&bs, &ord],
        )?;
        Ok(rows.iter().filter_map(row_to_finding).collect())
    }

    /// Mark every finding on a paragraph suppressed. Returns how many were
    /// affected (0 = no such finding). IT-P8 ties this to the intent ledger.
    pub(crate) fn suppress_paragraph(&self, book_slug: &str, para_id: &str) -> Result<usize> {
        let bs = book_slug.to_string();
        let pid = para_id.to_string();
        let before = self.findings(&bs, false)?.iter().filter(|f| f.para_id == para_id).count();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &pid];
        self.engine.execute_with(
            "UPDATE theologian_findings SET suppressed = 1 WHERE book_slug = ? AND para_id = ?",
            &params,
        )?;
        Ok(before)
    }
}

fn row_to_finding(r: &Vec<DuckValue>) -> Option<TheologianFinding> {
    Some(TheologianFinding {
        signal_type: SignalType::from_code(&as_text(r.first())?)?,
        chapter_ord: as_i64(r.get(1))? as u32,
        para_id: as_text(r.get(2))?,
        description: as_text(r.get(3))?,
        suppressed: as_i64(r.get(4)).unwrap_or(0) != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mk(sig: SignalType, ch: u32, para: &str) -> TheologianFinding {
        TheologianFinding {
            signal_type: sig,
            chapter_ord: ch,
            para_id: para.to_string(),
            description: format!("{} at {para}", sig.as_code()),
            suppressed: false,
        }
    }

    #[test]
    fn insert_query_clear_suppress_roundtrip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("it-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let s = TheologianStore::open(&dir)?;

        s.upsert_finding("bk", &mk(SignalType::MoralInvisibility, 12, "p1"), 1, "now")?;
        s.upsert_finding("bk", &mk(SignalType::ConsequenceGap, 12, "p2"), 2, "now")?;
        s.upsert_finding("bk", &mk(SignalType::SacredLevity, 7, "p3"), 3, "now")?;

        assert_eq!(s.findings("bk", false)?.len(), 3);
        assert_eq!(s.findings_for_chapter("bk", 12)?.len(), 2);

        // Suppress paragraph p1 → drops from unsuppressed, stays with include.
        assert_eq!(s.suppress_paragraph("bk", "p1")?, 1);
        assert_eq!(s.findings("bk", false)?.len(), 2);
        assert_eq!(s.findings("bk", true)?.len(), 3);

        // Re-scan clears chapter 12.
        s.clear_chapter("bk", 12)?;
        assert_eq!(s.findings("bk", true)?.iter().filter(|f| f.chapter_ord == 12).count(), 0);
        assert_eq!(s.findings("bk", true)?.len(), 1); // ch.7 sacred-levity remains

        // Idempotent insert (same id) replaces, doesn't duplicate.
        s.upsert_finding("bk", &mk(SignalType::SacredLevity, 7, "p3"), 9, "later")?;
        assert_eq!(s.findings("bk", true)?.iter().filter(|f| f.para_id == "p3").count(), 1);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }

    #[test]
    fn suppress_unknown_paragraph_returns_zero() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("it-store2-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let s = TheologianStore::open(&dir)?;
        assert_eq!(s.suppress_paragraph("bk", "nope")?, 0);
        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
