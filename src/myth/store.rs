//! MYTH-1 (M-P2) — the store over its own `.inkhaven/myth.duckdb`. Holds the
//! declared inventory (symbols / motifs / archetypes), the compiled highlight
//! vocab, the per-chapter density scan, motif occurrences, and check findings.
//! Reads `char.duckdb` / `dialogue.duckdb` at query time elsewhere; never writes
//! them. Audit: TEXT/INTEGER columns, `DefaultHasher` u64 hashes, RFC3339 TEXT
//! timestamps — not the RFC's `UBIGINT`/`TIMESTAMPTZ`/`BOOLEAN`.

use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::{
    ArchetypeRole, FindingType, MythArchetype, MythFinding, MythMotif, MythSymbol, MythValence,
};

const INIT_SQL: &str = "
    CREATE TABLE IF NOT EXISTS myth_symbols (
        book_slug       TEXT NOT NULL,
        para_id         TEXT NOT NULL,
        vocabulary_json TEXT NOT NULL,
        meaning         TEXT NOT NULL,
        valence         TEXT NOT NULL,
        traditions_json TEXT NOT NULL,
        content_hash    TEXT NOT NULL,
        PRIMARY KEY (book_slug, para_id)
    );
    CREATE TABLE IF NOT EXISTS myth_motifs (
        book_slug    TEXT NOT NULL,
        para_id      TEXT NOT NULL,
        name         TEXT NOT NULL,
        description  TEXT NOT NULL,
        valence      TEXT NOT NULL,
        content_hash TEXT NOT NULL,
        PRIMARY KEY (book_slug, para_id)
    );
    CREATE TABLE IF NOT EXISTS myth_archetypes (
        book_slug      TEXT NOT NULL,
        para_id        TEXT NOT NULL,
        role           TEXT NOT NULL,
        character_name TEXT NOT NULL,
        function_desc  TEXT NOT NULL,
        content_hash   TEXT NOT NULL,
        PRIMARY KEY (book_slug, para_id)
    );
    CREATE TABLE IF NOT EXISTS myth_highlight_vocab (
        book_slug      TEXT NOT NULL,
        token          TEXT NOT NULL,
        symbol_para_id TEXT NOT NULL,
        PRIMARY KEY (book_slug, token)
    );
    CREATE TABLE IF NOT EXISTS myth_symbol_density (
        book_slug        TEXT    NOT NULL,
        symbol_para_id   TEXT    NOT NULL,
        chapter_ord      INTEGER NOT NULL,
        occurrence_count INTEGER NOT NULL,
        prose_hash       TEXT    NOT NULL,
        computed_at      TEXT    NOT NULL,
        PRIMARY KEY (book_slug, symbol_para_id, chapter_ord)
    );
    CREATE TABLE IF NOT EXISTS myth_motif_occurrences (
        book_slug     TEXT NOT NULL,
        motif_para_id TEXT NOT NULL,
        chapter_ord   INTEGER NOT NULL,
        prose_para_id TEXT NOT NULL,
        source        TEXT NOT NULL,
        computed_at   TEXT NOT NULL,
        PRIMARY KEY (book_slug, motif_para_id, prose_para_id)
    );
    CREATE TABLE IF NOT EXISTS myth_findings (
        book_slug     TEXT NOT NULL,
        finding_id    TEXT NOT NULL,
        finding_type  TEXT NOT NULL,
        description   TEXT NOT NULL,
        evidence      TEXT,
        entry_para_id TEXT,
        chapter_ord   INTEGER,
        suppressed    INTEGER NOT NULL DEFAULT 0,
        computed_at   TEXT NOT NULL,
        PRIMARY KEY (book_slug, finding_id)
    );
    CREATE INDEX IF NOT EXISTS idx_myth_density ON myth_symbol_density (book_slug, chapter_ord);
    CREATE INDEX IF NOT EXISTS idx_myth_occ ON myth_motif_occurrences (book_slug, chapter_ord);
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

pub(crate) struct MythStore {
    engine: Arc<StorageEngine>,
}

impl MythStore {
    pub(crate) fn open(project_root: &Path) -> Result<MythStore> {
        let path = project_root.join("myth.duckdb");
        Ok(MythStore { engine: Arc::new(StorageEngine::new(&path, INIT_SQL, 2)?) })
    }

    // ── declared inventory ──────────────────────────────────────────────────────

    /// Replace the whole declared inventory for a book (symbols / motifs /
    /// archetypes are rebuilt wholesale from the Mythology book on save).
    pub(crate) fn replace_inventory(
        &self,
        book_slug: &str,
        symbols: &[MythSymbol],
        motifs: &[MythMotif],
        archetypes: &[MythArchetype],
    ) -> Result<()> {
        let bs = book_slug.to_string();
        for t in ["myth_symbols", "myth_motifs", "myth_archetypes"] {
            self.engine.execute_with(&format!("DELETE FROM {t} WHERE book_slug = ?"), &[&bs])?;
        }
        let now = chrono::Utc::now().to_rfc3339();
        for s in symbols {
            let vocab = serde_json::to_string(&s.vocabulary).unwrap_or_else(|_| "[]".into());
            let trad = serde_json::to_string(&s.traditions).unwrap_or_else(|_| "[]".into());
            let val = s.valence.as_code().to_string();
            let hash = hash_str(&format!("{vocab}{}{val}{trad}", s.meaning));
            let params: Vec<&dyn duckdb::ToSql> =
                vec![&bs, &s.para_id, &vocab, &s.meaning, &val, &trad, &hash];
            self.engine.execute_with(
                "INSERT OR REPLACE INTO myth_symbols \
                 (book_slug, para_id, vocabulary_json, meaning, valence, traditions_json, content_hash) \
                 VALUES (?,?,?,?,?,?,?)",
                &params,
            )?;
        }
        for m in motifs {
            let val = m.valence.as_code().to_string();
            let hash = hash_str(&format!("{}{}{val}", m.name, m.description));
            let params: Vec<&dyn duckdb::ToSql> =
                vec![&bs, &m.para_id, &m.name, &m.description, &val, &hash];
            self.engine.execute_with(
                "INSERT OR REPLACE INTO myth_motifs \
                 (book_slug, para_id, name, description, valence, content_hash) VALUES (?,?,?,?,?,?)",
                &params,
            )?;
        }
        for a in archetypes {
            let role = a.role.as_code().to_string();
            let hash = hash_str(&format!("{role}{}{}", a.character_name, a.function_desc));
            let params: Vec<&dyn duckdb::ToSql> =
                vec![&bs, &a.para_id, &role, &a.character_name, &a.function_desc, &hash];
            self.engine.execute_with(
                "INSERT OR REPLACE INTO myth_archetypes \
                 (book_slug, para_id, role, character_name, function_desc, content_hash) \
                 VALUES (?,?,?,?,?,?)",
                &params,
            )?;
        }
        // The highlight vocab mirrors the symbols — rebuild it here.
        self.rebuild_highlight_vocab(book_slug, symbols)?;
        let _ = now;
        Ok(())
    }

    pub(crate) fn symbols(&self, book_slug: &str) -> Result<Vec<MythSymbol>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT para_id, vocabulary_json, meaning, valence, traditions_json \
             FROM myth_symbols WHERE book_slug = ? ORDER BY para_id",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(row_to_symbol).collect())
    }

    pub(crate) fn motifs(&self, book_slug: &str) -> Result<Vec<MythMotif>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT para_id, name, description, valence FROM myth_motifs WHERE book_slug = ? ORDER BY name",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(row_to_motif).collect())
    }

    pub(crate) fn archetypes(&self, book_slug: &str) -> Result<Vec<MythArchetype>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT para_id, role, character_name, function_desc FROM myth_archetypes \
             WHERE book_slug = ? ORDER BY role",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(row_to_archetype).collect())
    }

    // ── highlight vocab ─────────────────────────────────────────────────────────

    fn rebuild_highlight_vocab(&self, book_slug: &str, symbols: &[MythSymbol]) -> Result<()> {
        let bs = book_slug.to_string();
        self.engine
            .execute_with("DELETE FROM myth_highlight_vocab WHERE book_slug = ?", &[&bs])?;
        for s in symbols {
            for tok in &s.vocabulary {
                let token = tok.trim().to_lowercase();
                if token.is_empty() {
                    continue;
                }
                let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &token, &s.para_id];
                self.engine.execute_with(
                    "INSERT OR REPLACE INTO myth_highlight_vocab (book_slug, token, symbol_para_id) \
                     VALUES (?,?,?)",
                    &params,
                )?;
            }
        }
        Ok(())
    }

    /// The compiled, lowercased highlight tokens for a book (sorted).
    pub(crate) fn highlight_tokens(&self, book_slug: &str) -> Result<Vec<String>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT token FROM myth_highlight_vocab WHERE book_slug = ? ORDER BY token",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(|r| as_text(r.first())).collect())
    }

    // ── symbol density ──────────────────────────────────────────────────────────

    pub(crate) fn clear_density_chapter(&self, book_slug: &str, chapter_ord: u32) -> Result<()> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &ord];
        self.engine.execute_with(
            "DELETE FROM myth_symbol_density WHERE book_slug = ? AND chapter_ord = ?",
            &params,
        )
    }

    pub(crate) fn upsert_density(
        &self,
        book_slug: &str,
        symbol_para_id: &str,
        chapter_ord: u32,
        count: u32,
        prose_hash: u64,
        now: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let sp = symbol_para_id.to_string();
        let ord = chapter_ord as i64;
        let c = count as i64;
        let ph = prose_hash.to_string();
        let at = now.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &sp, &ord, &c, &ph, &at];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO myth_symbol_density \
             (book_slug, symbol_para_id, chapter_ord, occurrence_count, prose_hash, computed_at) \
             VALUES (?,?,?,?,?,?)",
            &params,
        )
    }

    /// Per-chapter `(chapter_ord, count)` for one symbol.
    pub(crate) fn density_for_symbol(&self, book_slug: &str, symbol_para_id: &str) -> Result<Vec<(u32, u32)>> {
        let (bs, sp) = (book_slug.to_string(), symbol_para_id.to_string());
        let rows = self.engine.select_all_with(
            "SELECT chapter_ord, occurrence_count FROM myth_symbol_density \
             WHERE book_slug = ? AND symbol_para_id = ? ORDER BY chapter_ord",
            &[&bs, &sp],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| Some((as_i64(r.first())? as u32, as_i64(r.get(1))? as u32)))
            .collect())
    }

    // ── motif occurrences ───────────────────────────────────────────────────────

    /// Drop a source's occurrences for a book before re-collecting them.
    pub(crate) fn clear_motif_occurrences(&self, book_slug: &str, source: &str) -> Result<()> {
        let (bs, src) = (book_slug.to_string(), source.to_string());
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &src];
        self.engine
            .execute_with("DELETE FROM myth_motif_occurrences WHERE book_slug = ? AND source = ?", &params)
    }

    pub(crate) fn upsert_motif_occurrence(
        &self,
        book_slug: &str,
        motif_para_id: &str,
        chapter_ord: u32,
        prose_para_id: &str,
        source: &str,
        now: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let mp = motif_para_id.to_string();
        let ord = chapter_ord as i64;
        let pp = prose_para_id.to_string();
        let src = source.to_string();
        let at = now.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &mp, &ord, &pp, &src, &at];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO myth_motif_occurrences \
             (book_slug, motif_para_id, chapter_ord, prose_para_id, source, computed_at) \
             VALUES (?,?,?,?,?,?)",
            &params,
        )
    }

    /// Distinct chapter ordinals where a motif occurs (any source).
    pub(crate) fn motif_chapters(&self, book_slug: &str, motif_para_id: &str) -> Result<Vec<u32>> {
        let (bs, mp) = (book_slug.to_string(), motif_para_id.to_string());
        let rows = self.engine.select_all_with(
            "SELECT DISTINCT chapter_ord FROM myth_motif_occurrences \
             WHERE book_slug = ? AND motif_para_id = ? ORDER BY chapter_ord",
            &[&bs, &mp],
        )?;
        Ok(rows.iter().filter_map(|r| as_i64(r.first()).map(|o| o as u32)).collect())
    }

    // ── findings ────────────────────────────────────────────────────────────────

    /// Clear a book's findings of a given type (a re-check replaces them).
    pub(crate) fn clear_findings_of_type(&self, book_slug: &str, finding_type: FindingType) -> Result<()> {
        let (bs, ft) = (book_slug.to_string(), finding_type.as_code().to_string());
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &ft];
        self.engine
            .execute_with("DELETE FROM myth_findings WHERE book_slug = ? AND finding_type = ?", &params)
    }

    pub(crate) fn upsert_finding(&self, book_slug: &str, id: &str, f: &MythFinding, now: &str) -> Result<()> {
        let bs = book_slug.to_string();
        let fid = id.to_string();
        let ft = f.finding_type.as_code().to_string();
        let desc = f.description.clone();
        let ev = f.evidence.clone().unwrap_or_default();
        let ep = f.entry_para_id.clone().unwrap_or_default();
        let ord = f.chapter_ord.map(|o| o as i64).unwrap_or(-1);
        let supp = i64::from(f.suppressed);
        let at = now.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &fid, &ft, &desc, &ev, &ep, &ord, &supp, &at];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO myth_findings \
             (book_slug, finding_id, finding_type, description, evidence, entry_para_id, chapter_ord, suppressed, computed_at) \
             VALUES (?,?,?,?,?,?,?,?,?)",
            &params,
        )
    }

    pub(crate) fn findings(&self, book_slug: &str, include_suppressed: bool) -> Result<Vec<MythFinding>> {
        let bs = book_slug.to_string();
        let sql = if include_suppressed {
            "SELECT finding_type, description, evidence, entry_para_id, chapter_ord, suppressed \
             FROM myth_findings WHERE book_slug = ? ORDER BY finding_type, chapter_ord"
        } else {
            "SELECT finding_type, description, evidence, entry_para_id, chapter_ord, suppressed \
             FROM myth_findings WHERE book_slug = ? AND suppressed = 0 ORDER BY finding_type, chapter_ord"
        };
        let rows = self.engine.select_all_with(sql, &[&bs])?;
        Ok(rows.iter().filter_map(row_to_finding).collect())
    }

    /// Mark a finding suppressed by id. Returns whether a row matched.
    pub(crate) fn suppress_finding(&self, book_slug: &str, finding_id: &str) -> Result<bool> {
        let before = {
            let (bs, fid) = (book_slug.to_string(), finding_id.to_string());
            self.engine
                .select_all_with(
                    "SELECT 1 FROM myth_findings WHERE book_slug = ? AND finding_id = ? AND suppressed = 0",
                    &[&bs, &fid],
                )?
                .len()
        };
        let (bs, fid) = (book_slug.to_string(), finding_id.to_string());
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &fid];
        self.engine.execute_with(
            "UPDATE myth_findings SET suppressed = 1 WHERE book_slug = ? AND finding_id = ?",
            &params,
        )?;
        Ok(before > 0)
    }
}

fn row_to_symbol(r: &Vec<DuckValue>) -> Option<MythSymbol> {
    let vocab: Vec<String> =
        serde_json::from_str(&as_text(r.get(1))?).unwrap_or_default();
    let traditions: Vec<String> =
        serde_json::from_str(&as_text(r.get(4))?).unwrap_or_default();
    Some(MythSymbol {
        para_id: as_text(r.first())?,
        vocabulary: vocab,
        meaning: as_text(r.get(2))?,
        valence: MythValence::from_code(&as_text(r.get(3))?),
        traditions,
    })
}

fn row_to_motif(r: &Vec<DuckValue>) -> Option<MythMotif> {
    Some(MythMotif {
        para_id: as_text(r.first())?,
        name: as_text(r.get(1))?,
        description: as_text(r.get(2))?,
        valence: MythValence::from_code(&as_text(r.get(3))?),
    })
}

fn row_to_archetype(r: &Vec<DuckValue>) -> Option<MythArchetype> {
    Some(MythArchetype {
        para_id: as_text(r.first())?,
        role: ArchetypeRole::from_code(&as_text(r.get(1))?),
        character_name: as_text(r.get(2))?,
        function_desc: as_text(r.get(3))?,
    })
}

fn row_to_finding(r: &Vec<DuckValue>) -> Option<MythFinding> {
    let ord = as_i64(r.get(4));
    let ev = as_text(r.get(2)).filter(|s| !s.is_empty());
    let ep = as_text(r.get(3)).filter(|s| !s.is_empty());
    Some(MythFinding {
        finding_type: FindingType::from_code(&as_text(r.first())?)?,
        description: as_text(r.get(1))?,
        evidence: ev,
        entry_para_id: ep,
        chapter_ord: ord.filter(|o| *o >= 0).map(|o| o as u32),
        suppressed: as_i64(r.get(5)).unwrap_or(0) != 0,
    })
}

fn hash_str(s: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sym(para: &str, vocab: &[&str], val: MythValence, trad: &[&str]) -> MythSymbol {
        MythSymbol {
            para_id: para.into(),
            vocabulary: vocab.iter().map(|s| s.to_string()).collect(),
            meaning: "m".into(),
            valence: val,
            traditions: trad.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn inventory_highlight_density_findings_roundtrip() -> Result<()> {
        let dir = std::env::temp_dir().join(format!("myth-store-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let s = MythStore::open(&dir)?;

        let symbols = vec![
            sym("s1", &["Raven", "ravens"], MythValence::Negative, &["Norse"]),
            sym("s2", &["white rose"], MythValence::Ambiguous, &[]),
        ];
        let motifs = vec![MythMotif {
            para_id: "m1".into(),
            name: "locked door".into(),
            description: "d".into(),
            valence: MythValence::Ambiguous,
        }];
        let arch = vec![MythArchetype {
            para_id: "a1".into(),
            role: ArchetypeRole::Herald,
            character_name: "Seren".into(),
            function_desc: "announces".into(),
        }];
        s.replace_inventory("bk", &symbols, &motifs, &arch)?;

        assert_eq!(s.symbols("bk")?.len(), 2);
        assert_eq!(s.motifs("bk")?.len(), 1);
        assert_eq!(s.archetypes("bk")?.len(), 1);
        // Highlight vocab lowercased + includes the phrase.
        let toks = s.highlight_tokens("bk")?;
        assert!(toks.contains(&"raven".to_string()) && toks.contains(&"white rose".to_string()));

        // Density.
        let now = "now";
        s.upsert_density("bk", "s1", 3, 4, 99, now)?;
        s.upsert_density("bk", "s1", 7, 1, 99, now)?;
        assert_eq!(s.density_for_symbol("bk", "s1")?, vec![(3, 4), (7, 1)]);

        // Motif occurrences.
        s.upsert_motif_occurrence("bk", "m1", 5, "p10", "explicit_tag", now)?;
        s.upsert_motif_occurrence("bk", "m1", 9, "p20", "llm_discovered", now)?;
        assert_eq!(s.motif_chapters("bk", "m1")?, vec![5, 9]);

        // Findings + suppression.
        let f = MythFinding {
            finding_type: FindingType::ArchetypeVacant,
            description: "shadow vacant".into(),
            evidence: None,
            entry_para_id: Some("a2".into()),
            chapter_ord: None,
            suppressed: false,
        };
        s.upsert_finding("bk", "f1", &f, now)?;
        assert_eq!(s.findings("bk", false)?.len(), 1);
        assert!(s.suppress_finding("bk", "f1")?);
        assert_eq!(s.findings("bk", false)?.len(), 0);
        assert_eq!(s.findings("bk", true)?.len(), 1);
        assert!(!s.suppress_finding("bk", "nope")?);

        // Replacing inventory clears prior symbols.
        s.replace_inventory("bk", &symbols[..1], &[], &[])?;
        assert_eq!(s.symbols("bk")?.len(), 1);

        let _ = std::fs::remove_dir_all(&dir);
        Ok(())
    }
}
