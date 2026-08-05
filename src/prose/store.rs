//! NARR-1 — `.inkhaven/prose.duckdb`, the per-feature DuckDB store for voice
//! profiles. Built on the shared pooled [`StorageEngine`]. Numeric metric
//! columns are stored as TEXT (parsed in Rust) — the same robust pattern the
//! rest of the project's DuckDB usage follows — with INTEGER counts.

use std::path::Path;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::ProseLanguage;
use super::profile::{VoiceProfile, VoiceScope, VoiceTier2};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS prose_profiles (
    book_slug         TEXT    NOT NULL,
    scope             TEXT    NOT NULL,
    chapter_ord       INTEGER,
    prose_language    TEXT    NOT NULL DEFAULT 'en',
    word_count        INTEGER NOT NULL,
    sentence_count    INTEGER NOT NULL,
    sent_len_p10      TEXT    NOT NULL,
    sent_len_p25      TEXT    NOT NULL,
    sent_len_p50      TEXT    NOT NULL,
    sent_len_p75      TEXT    NOT NULL,
    sent_len_p90      TEXT    NOT NULL,
    sent_len_cv       TEXT    NOT NULL,
    burstiness_b      TEXT    NOT NULL,
    mattr             TEXT    NOT NULL,
    modal_density     TEXT,
    interiority_ratio TEXT,
    de_erlebte_rede_particle_density TEXT,
    sensory_visual       TEXT,
    sensory_auditory     TEXT,
    sensory_olfactory    TEXT,
    sensory_tactile      TEXT,
    sensory_kinesthetic  TEXT,
    active_passive_ratio TEXT,
    computed_at       TEXT    NOT NULL,
    text_hash         TEXT    NOT NULL,
    PRIMARY KEY (book_slug, scope)
);
CREATE TABLE IF NOT EXISTS prose_drift_cache (
    book_slug   TEXT NOT NULL,
    scope_a     TEXT NOT NULL,
    scope_b     TEXT NOT NULL,
    delta_json  TEXT NOT NULL,
    computed_at TEXT NOT NULL,
    PRIMARY KEY (book_slug, scope_a, scope_b)
);
CREATE INDEX IF NOT EXISTS idx_prose_profiles_book_chapter
    ON prose_profiles (book_slug, chapter_ord);
"#;

/// Column list shared by INSERT and SELECT so positional parsing stays aligned.
const COLS: &str = "book_slug, scope, chapter_ord, prose_language, word_count, \
    sentence_count, sent_len_p10, sent_len_p25, sent_len_p50, sent_len_p75, \
    sent_len_p90, sent_len_cv, burstiness_b, mattr, modal_density, \
    interiority_ratio, de_erlebte_rede_particle_density, sensory_visual, \
    sensory_auditory, sensory_olfactory, sensory_tactile, sensory_kinesthetic, \
    active_passive_ratio, computed_at, text_hash";

pub(crate) struct ProseStore {
    engine: StorageEngine,
}

impl ProseStore {
    /// Open (creating if needed) `<project_root>/.inkhaven/prose.duckdb`.
    pub(crate) fn open(project_root: &Path) -> Result<ProseStore> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)?;
        let engine = StorageEngine::new_versioned(dir.join("prose.duckdb"), SCHEMA, 4, 1)?;
        Ok(ProseStore { engine })
    }

    /// Insert or replace a profile.
    pub(crate) fn upsert(&self, book_slug: &str, p: &VoiceProfile, computed_at: &str) -> Result<()> {
        let book_slug = book_slug.to_string();
        let scope = p.scope.as_str();
        let chapter_ord: Option<i64> = p.scope.chapter_ord().map(|n| n as i64);
        let lang = p.prose_language.as_code().to_string();
        let wc = p.word_count as i64;
        let sc = p.sentence_count as i64;
        let f = |x: f32| x.to_string();
        let of = |x: Option<f32>| x.map(|v| v.to_string());
        let (p10, p25, p50, p75, p90) = (f(p.p10), f(p.p25), f(p.p50), f(p.p75), f(p.p90));
        let (cv, burst, mattr) = (f(p.cv), f(p.burstiness), f(p.mattr));
        let modal = of(p.modal_density);
        let inter = of(p.interiority_ratio);
        let de = of(p.de_erlebte_rede_particle_density);
        let (sv, sa, so, st, sk, apr) = match &p.tier2 {
            Some(t) => (
                Some(f(t.sensory[0])), Some(f(t.sensory[1])), Some(f(t.sensory[2])),
                Some(f(t.sensory[3])), Some(f(t.sensory[4])), Some(f(t.active_passive_ratio)),
            ),
            None => (None, None, None, None, None, None),
        };
        let computed_at = computed_at.to_string();
        let hash = p.text_hash.to_string();

        let sql = format!(
            "INSERT OR REPLACE INTO prose_profiles ({COLS}) VALUES \
             (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> = vec![
            &book_slug, &scope, &chapter_ord, &lang, &wc, &sc,
            &p10, &p25, &p50, &p75, &p90, &cv, &burst, &mattr,
            &modal, &inter, &de, &sv, &sa, &so, &st, &sk, &apr,
            &computed_at, &hash,
        ];
        self.engine.execute_with(&sql, &params)?;
        Ok(())
    }

    /// All *narrator* profiles for a book (book aggregate + chapters), ordered.
    /// CHORUS-1 character voices (scope `character:<name>`) share this table but
    /// are excluded here so the narrator surfaces (`prose profile` / `prose
    /// drift`) never see them; the `chorus` path recomputes character profiles
    /// from dialogue rather than reading them back.
    pub(crate) fn get_all(&self, book_slug: &str) -> Result<Vec<VoiceProfile>> {
        let sql = format!(
            "SELECT {COLS} FROM prose_profiles WHERE book_slug = ? \
             AND scope NOT LIKE 'character:%' ORDER BY chapter_ord NULLS FIRST"
        );
        let rows = self.engine.select_all_with(&sql, &[&book_slug])?;
        Ok(rows.iter().filter_map(|r| row_to_profile(r)).collect())
    }

    /// Distinct book slugs with stored profiles (used by `--reference`).
    pub(crate) fn book_slugs(&self) -> Result<Vec<String>> {
        let rows = self
            .engine
            .select_all("SELECT DISTINCT book_slug FROM prose_profiles ORDER BY book_slug")?;
        Ok(rows.iter().filter_map(|r| r.first().and_then(as_text)).collect())
    }

    /// The stored text hash for one scope, if present (the staleness key).
    pub(crate) fn stored_hash(&self, book_slug: &str, scope: &str) -> Result<Option<u64>> {
        let rows = self.engine.select_all_with(
            "SELECT text_hash FROM prose_profiles WHERE book_slug = ? AND scope = ?",
            &[&book_slug, &scope],
        )?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .and_then(as_text)
            .and_then(|s| s.parse().ok()))
    }

    /// Language-change invalidation: zero the hash of every row whose stored
    /// `prose_language` differs from `lang`, so the next refresh recomputes them
    /// (rhythm columns recompute to identical values — deterministic — so they
    /// are effectively preserved).
    pub(crate) fn mark_language_stale(&self, book_slug: &str, lang: &ProseLanguage) -> Result<()> {
        let code = lang.as_code().to_string();
        let bs = book_slug.to_string();
        self.engine.execute_with(
            "UPDATE prose_profiles SET text_hash = '0' \
             WHERE book_slug = ? AND prose_language <> ?",
            &[&bs, &code],
        )
    }
}

// ── DuckValue parsing ────────────────────────────────────────────────────────

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
fn as_f32(v: &DuckValue) -> Option<f32> {
    as_text(v).and_then(|s| s.parse().ok())
}

fn row_to_profile(r: &[DuckValue]) -> Option<VoiceProfile> {
    if r.len() < 25 {
        return None;
    }
    let scope = VoiceScope::parse(&as_text(&r[1])?)?;
    let tier2 = as_f32(&r[22]).map(|apr| VoiceTier2 {
        sensory: [
            as_f32(&r[17]).unwrap_or(0.0),
            as_f32(&r[18]).unwrap_or(0.0),
            as_f32(&r[19]).unwrap_or(0.0),
            as_f32(&r[20]).unwrap_or(0.0),
            as_f32(&r[21]).unwrap_or(0.0),
        ],
        active_passive_ratio: apr,
    });
    Some(VoiceProfile {
        scope,
        prose_language: ProseLanguage::from_label(&as_text(&r[3]).unwrap_or_default()),
        word_count: as_i64(&r[4]).unwrap_or(0) as u32,
        sentence_count: as_i64(&r[5]).unwrap_or(0) as u32,
        p10: as_f32(&r[6]).unwrap_or(0.0),
        p25: as_f32(&r[7]).unwrap_or(0.0),
        p50: as_f32(&r[8]).unwrap_or(0.0),
        p75: as_f32(&r[9]).unwrap_or(0.0),
        p90: as_f32(&r[10]).unwrap_or(0.0),
        cv: as_f32(&r[11]).unwrap_or(0.0),
        burstiness: as_f32(&r[12]).unwrap_or(0.0),
        mattr: as_f32(&r[13]).unwrap_or(0.0),
        modal_density: as_f32(&r[14]),
        interiority_ratio: as_f32(&r[15]),
        de_erlebte_rede_particle_density: as_f32(&r[16]),
        tier2,
        text_hash: as_text(&r[24]).and_then(|s| s.parse().ok()).unwrap_or(0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::{De, En};
    use crate::prose::profile::compute_profile;

    fn tmp_store() -> (tempfile::TempDir, ProseStore) {
        let dir = tempfile::tempdir().unwrap();
        let s = ProseStore::open(dir.path()).unwrap();
        (dir, s)
    }

    #[test]
    fn round_trip_with_nulls_and_tier2() {
        let (_d, s) = tmp_store();
        // EN deep profile (Tier-2 present).
        let p = compute_profile(
            "She might have known. The wind was cold. She thought so quietly.",
            VoiceScope::Chapter(1),
            &En,
            true,
            100,
        );
        s.upsert("book", &p, "2026-06-27T00:00:00Z").unwrap();
        // Book scope, shallow (Tier-2 null).
        let pb = compute_profile("A short book.", VoiceScope::Book, &En, false, 100);
        s.upsert("book", &pb, "2026-06-27T00:00:00Z").unwrap();

        let all = s.get_all("book").unwrap();
        assert_eq!(all.len(), 2);
        let chap = all.iter().find(|p| matches!(p.scope, VoiceScope::Chapter(1))).unwrap();
        assert_eq!(chap.sentence_count, 3);
        assert!(chap.modal_density.is_some());
        assert!(chap.tier2.is_some());
        assert!((chap.cv - p.cv).abs() < 1e-4);
        let book = all.iter().find(|p| p.scope == VoiceScope::Book).unwrap();
        assert!(book.tier2.is_none()); // shallow → NULL Tier-2 round-trips to None
    }

    #[test]
    fn character_scopes_are_isolated_from_the_narrator_get_all() {
        // CHORUS-1: character voices share the table but must not leak into the
        // narrator surfaces (`prose profile` / `prose drift` read `get_all`).
        let (_d, s) = tmp_store();
        let book = compute_profile("A short book.", VoiceScope::Book, &En, false, 100);
        s.upsert("book", &book, "2026-08-02T00:00:00Z").unwrap();
        let mara =
            compute_profile("Yes. No. Fine.", VoiceScope::Character("Mara".into()), &En, false, 100);
        s.upsert("book", &mara, "2026-08-02T00:00:00Z").unwrap();
        // get_all sees only the narrator's book profile…
        let all = s.get_all("book").unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].scope, VoiceScope::Book);
        // …but the character row IS persisted (its own stored hash is present).
        assert!(s.stored_hash("book", "character:Mara").unwrap().is_some());
    }

    #[test]
    fn de_particle_density_round_trips() {
        let (_d, s) = tmp_store();
        let p = compute_profile("Sie dachte nach. Das war ja doch klar.", VoiceScope::Book, &De, false, 100);
        s.upsert("buch", &p, "2026-06-27T00:00:00Z").unwrap();
        let back = &s.get_all("buch").unwrap()[0];
        assert!(back.de_erlebte_rede_particle_density.unwrap() > 0.0);
        assert_eq!(back.prose_language, De);
    }

    #[test]
    fn language_change_marks_stale_but_preserves_rhythm() {
        let (_d, s) = tmp_store();
        // Stored under EN.
        let p = compute_profile("She walked home. It was late.", VoiceScope::Chapter(1), &En, false, 100);
        s.upsert("b", &p, "t").unwrap();
        let cv_before = s.get_all("b").unwrap()[0].cv;
        assert_eq!(s.stored_hash("b", "chapter:1").unwrap(), Some(p.text_hash));

        // Resolve to DE → EN rows are now stale (hash zeroed).
        s.mark_language_stale("b", &De).unwrap();
        assert_eq!(s.stored_hash("b", "chapter:1").unwrap(), Some(0));
        // Rhythm value untouched in storage until a refresh recomputes it
        // (and recompute is deterministic, so it would be identical).
        assert!((s.get_all("b").unwrap()[0].cv - cv_before).abs() < 1e-6);
    }
}
