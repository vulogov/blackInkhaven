//! CHAR-1 (C-P1) — `.inkhaven/char.duckdb`, the per-feature store for character
//! state chains, arc declarations, arc checks, scene-card links, and planning
//! findings. House pattern (reals/hashes/timestamps TEXT, bools INTEGER 0/1,
//! DefaultHasher u64 stringified). `character_states` has **two writers** — the
//! deterministic agency pass and the LLM state extraction — so each upserts only
//! its own columns via `ON CONFLICT DO UPDATE`.

use std::path::Path;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::{ArcCheckType, ArcDeclaration, ArcType, ArcVerdict, CharacterArcCheck, CharacterState};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS character_states (
    book_slug                 TEXT    NOT NULL,
    character_name            TEXT    NOT NULL,
    chapter_ord               INTEGER NOT NULL,
    state_summary             TEXT    NOT NULL DEFAULT '',
    changed                   INTEGER NOT NULL DEFAULT 0,
    change_description        TEXT,
    agency_score              TEXT,
    active_count              INTEGER NOT NULL DEFAULT 0,
    passive_count             INTEGER NOT NULL DEFAULT 0,
    utterance_count           INTEGER,
    chapter_hedge_density     TEXT,
    chapter_interiority_ratio TEXT,
    computed_at               TEXT    NOT NULL DEFAULT '',
    text_hash                 TEXT    NOT NULL DEFAULT '0',
    PRIMARY KEY (book_slug, character_name, chapter_ord)
);
CREATE TABLE IF NOT EXISTS character_arc_declarations (
    book_slug              TEXT    NOT NULL,
    character_name         TEXT    NOT NULL,
    arc_type               TEXT    NOT NULL,
    desired_state_start    TEXT    NOT NULL,
    desired_midpoint_state TEXT,
    desired_state_end      TEXT    NOT NULL,
    declaration_hash       TEXT    NOT NULL,
    PRIMARY KEY (book_slug, character_name)
);
CREATE TABLE IF NOT EXISTS character_arc_checks (
    book_slug      TEXT    NOT NULL,
    character_name TEXT    NOT NULL,
    check_type     TEXT    NOT NULL,
    verdict        TEXT    NOT NULL,
    description    TEXT    NOT NULL,
    chapter_ord    INTEGER,
    computed_at    TEXT    NOT NULL,
    PRIMARY KEY (book_slug, character_name, check_type)
);
CREATE TABLE IF NOT EXISTS scene_card_characters (
    book_slug      TEXT NOT NULL,
    scene_card_id  TEXT NOT NULL,
    character_name TEXT NOT NULL,
    arc_function   TEXT,
    chapter_ord    INTEGER,
    PRIMARY KEY (book_slug, scene_card_id, character_name)
);
CREATE TABLE IF NOT EXISTS arc_planning_findings (
    book_slug      TEXT NOT NULL,
    character_name TEXT NOT NULL,
    finding_type   TEXT NOT NULL,
    description    TEXT NOT NULL,
    computed_at    TEXT NOT NULL,
    PRIMARY KEY (book_slug, character_name, finding_type)
);
CREATE INDEX IF NOT EXISTS idx_states_book_character
    ON character_states (book_slug, character_name);
CREATE INDEX IF NOT EXISTS idx_states_book_chapter
    ON character_states (book_slug, chapter_ord);
"#;

const STATE_COLS: &str = "book_slug, character_name, chapter_ord, state_summary, changed, \
    change_description, agency_score, active_count, passive_count, utterance_count, \
    chapter_hedge_density, chapter_interiority_ratio, computed_at, text_hash";

pub(crate) struct CharStore {
    engine: StorageEngine,
}

impl CharStore {
    pub(crate) fn open(project_root: &Path) -> Result<CharStore> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)?;
        let engine = StorageEngine::new(dir.join("char.duckdb"), SCHEMA, 4)?;
        Ok(CharStore { engine })
    }

    // ── states ────────────────────────────────────────────────────────────────

    /// Upsert the deterministic agency columns (preserves the LLM state).
    pub(crate) fn upsert_agency(
        &self,
        book_slug: &str,
        character_name: &str,
        chapter_ord: u32,
        agency_score: Option<f32>,
        active_count: u32,
        passive_count: u32,
        computed_at: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let nm = character_name.to_string();
        let ord = chapter_ord as i64;
        let agency = agency_score.map(|a| a.to_string());
        let ac = active_count as i64;
        let pc = passive_count as i64;
        let ca = computed_at.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &nm, &ord, &agency, &ac, &pc, &ca];
        self.engine.execute_with(
            "INSERT INTO character_states \
             (book_slug, character_name, chapter_ord, agency_score, active_count, passive_count, computed_at) \
             VALUES (?,?,?,?,?,?,?) \
             ON CONFLICT (book_slug, character_name, chapter_ord) DO UPDATE SET \
               agency_score = excluded.agency_score, active_count = excluded.active_count, \
               passive_count = excluded.passive_count",
            &params,
        )
    }

    /// Upsert the LLM-extracted state columns + enrichment (preserves agency).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn upsert_state(
        &self,
        book_slug: &str,
        s: &CharacterState,
        computed_at: &str,
        text_hash: u64,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let nm = s.character_name.clone();
        let ord = s.chapter_ord as i64;
        let summary = s.state_summary.clone();
        let changed = s.changed as i64;
        let desc = s.change_description.clone();
        let utt = s.utterance_count.map(|u| u as i64);
        let hedge = s.chapter_hedge_density.map(|h| h.to_string());
        let inter = s.chapter_interiority_ratio.map(|i| i.to_string());
        let ca = computed_at.to_string();
        let hash = text_hash.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![
            &bs, &nm, &ord, &summary, &changed, &desc, &utt, &hedge, &inter, &ca, &hash,
        ];
        self.engine.execute_with(
            "INSERT INTO character_states \
             (book_slug, character_name, chapter_ord, state_summary, changed, change_description, \
              utterance_count, chapter_hedge_density, chapter_interiority_ratio, computed_at, text_hash) \
             VALUES (?,?,?,?,?,?,?,?,?,?,?) \
             ON CONFLICT (book_slug, character_name, chapter_ord) DO UPDATE SET \
               state_summary = excluded.state_summary, changed = excluded.changed, \
               change_description = excluded.change_description, utterance_count = excluded.utterance_count, \
               chapter_hedge_density = excluded.chapter_hedge_density, \
               chapter_interiority_ratio = excluded.chapter_interiority_ratio, \
               computed_at = excluded.computed_at, text_hash = excluded.text_hash",
            &params,
        )
    }

    pub(crate) fn states_for_character(
        &self,
        book_slug: &str,
        character_name: &str,
    ) -> Result<Vec<CharacterState>> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let sql = format!(
            "SELECT {STATE_COLS} FROM character_states \
             WHERE book_slug = ? AND character_name = ? ORDER BY chapter_ord"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs, &nm])?;
        Ok(rows.iter().filter_map(row_to_state).collect())
    }

    /// Distinct character names with any stored state.
    pub(crate) fn characters_with_states(&self, book_slug: &str) -> Result<Vec<String>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT DISTINCT character_name FROM character_states WHERE book_slug = ? ORDER BY character_name",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(|r| r.first().and_then(as_text)).collect())
    }

    /// The stored state text hash for one character+chapter (the staleness key).
    pub(crate) fn stored_state_hash(
        &self,
        book_slug: &str,
        character_name: &str,
        chapter_ord: u32,
    ) -> Result<Option<u64>> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let ord = chapter_ord as i64;
        let rows = self.engine.select_all_with(
            "SELECT text_hash FROM character_states \
             WHERE book_slug = ? AND character_name = ? AND chapter_ord = ?",
            &[&bs, &nm, &ord],
        )?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .and_then(as_text)
            .and_then(|s| s.parse().ok())
            .filter(|h| *h != 0))
    }

    /// Drop state rows for a character from `from_chapter` onward (incremental
    /// invalidation propagates forward because the sliding window feeds N→N+1).
    pub(crate) fn clear_states_from(
        &self,
        book_slug: &str,
        character_name: &str,
        from_chapter: u32,
    ) -> Result<()> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let ord = from_chapter as i64;
        self.engine.execute_with(
            "DELETE FROM character_states \
             WHERE book_slug = ? AND character_name = ? AND chapter_ord >= ?",
            &[&bs, &nm, &ord],
        )
    }

    // ── declarations ───────────────────────────────────────────────────────────

    pub(crate) fn upsert_declaration(
        &self,
        book_slug: &str,
        d: &ArcDeclaration,
        declaration_hash: u64,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let nm = d.character_name.clone();
        let at = d.arc_type.as_code().to_string();
        let start = d.desired_state_start.clone();
        let mid = d.desired_midpoint_state.clone();
        let end = d.desired_state_end.clone();
        let hash = declaration_hash.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &nm, &at, &start, &mid, &end, &hash];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO character_arc_declarations \
             (book_slug, character_name, arc_type, desired_state_start, desired_midpoint_state, \
              desired_state_end, declaration_hash) VALUES (?,?,?,?,?,?,?)",
            &params,
        )
    }

    pub(crate) fn declaration(
        &self,
        book_slug: &str,
        character_name: &str,
    ) -> Result<Option<ArcDeclaration>> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let rows = self.engine.select_all_with(
            "SELECT character_name, arc_type, desired_state_start, desired_midpoint_state, desired_state_end \
             FROM character_arc_declarations WHERE book_slug = ? AND character_name = ?",
            &[&bs, &nm],
        )?;
        Ok(rows.first().and_then(row_to_declaration))
    }

    pub(crate) fn all_declarations(&self, book_slug: &str) -> Result<Vec<ArcDeclaration>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT character_name, arc_type, desired_state_start, desired_midpoint_state, desired_state_end \
             FROM character_arc_declarations WHERE book_slug = ? ORDER BY character_name",
            &[&bs],
        )?;
        Ok(rows.iter().filter_map(row_to_declaration).collect())
    }

    // ── checks ─────────────────────────────────────────────────────────────────

    pub(crate) fn clear_checks(&self, book_slug: &str, character_name: &str) -> Result<()> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        self.engine.execute_with(
            "DELETE FROM character_arc_checks WHERE book_slug = ? AND character_name = ?",
            &[&bs, &nm],
        )
    }

    pub(crate) fn upsert_check(
        &self,
        book_slug: &str,
        c: &CharacterArcCheck,
        computed_at: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let nm = c.character_name.clone();
        let ct = c.check_type.as_code().to_string();
        let v = c.verdict.as_code().to_string();
        let desc = c.description.clone();
        let ord = c.chapter_ord.map(|o| o as i64);
        let ca = computed_at.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &nm, &ct, &v, &desc, &ord, &ca];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO character_arc_checks \
             (book_slug, character_name, check_type, verdict, description, chapter_ord, computed_at) \
             VALUES (?,?,?,?,?,?,?)",
            &params,
        )
    }

    pub(crate) fn checks_for_character(
        &self,
        book_slug: &str,
        character_name: &str,
    ) -> Result<Vec<CharacterArcCheck>> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let rows = self.engine.select_all_with(
            "SELECT character_name, check_type, verdict, description, chapter_ord \
             FROM character_arc_checks WHERE book_slug = ? AND character_name = ? ORDER BY check_type",
            &[&bs, &nm],
        )?;
        Ok(rows.iter().filter_map(row_to_check).collect())
    }

    // ── scene cards + planning findings ─────────────────────────────────────────

    pub(crate) fn clear_scene_links(&self, book_slug: &str) -> Result<()> {
        let bs = book_slug.to_string();
        self.engine
            .execute_with("DELETE FROM scene_card_characters WHERE book_slug = ?", &[&bs])
    }

    pub(crate) fn upsert_scene_link(
        &self,
        book_slug: &str,
        scene_card_id: &str,
        character_name: &str,
        arc_function: Option<&str>,
        chapter_ord: Option<u32>,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let sc = scene_card_id.to_string();
        let nm = character_name.to_string();
        let af = arc_function.map(str::to_string);
        let ord = chapter_ord.map(|o| o as i64);
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &sc, &nm, &af, &ord];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO scene_card_characters \
             (book_slug, scene_card_id, character_name, arc_function, chapter_ord) VALUES (?,?,?,?,?)",
            &params,
        )
    }

    /// The chapter ordinals of a character's linked scene cards.
    pub(crate) fn scene_chapters(&self, book_slug: &str, character_name: &str) -> Result<Vec<u32>> {
        let (bs, nm) = (book_slug.to_string(), character_name.to_string());
        let rows = self.engine.select_all_with(
            "SELECT chapter_ord FROM scene_card_characters \
             WHERE book_slug = ? AND character_name = ? AND chapter_ord IS NOT NULL ORDER BY chapter_ord",
            &[&bs, &nm],
        )?;
        Ok(rows.iter().filter_map(|r| r.first().and_then(as_i64).map(|o| o as u32)).collect())
    }

    pub(crate) fn clear_planning_findings(&self, book_slug: &str) -> Result<()> {
        let bs = book_slug.to_string();
        self.engine
            .execute_with("DELETE FROM arc_planning_findings WHERE book_slug = ?", &[&bs])
    }

    pub(crate) fn upsert_planning_finding(
        &self,
        book_slug: &str,
        character_name: &str,
        finding_type: &str,
        description: &str,
        computed_at: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let nm = character_name.to_string();
        let ft = finding_type.to_string();
        let d = description.to_string();
        let ca = computed_at.to_string();
        let params: Vec<&dyn duckdb::ToSql> = vec![&bs, &nm, &ft, &d, &ca];
        self.engine.execute_with(
            "INSERT OR REPLACE INTO arc_planning_findings \
             (book_slug, character_name, finding_type, description, computed_at) VALUES (?,?,?,?,?)",
            &params,
        )
    }

    pub(crate) fn planning_findings(&self, book_slug: &str) -> Result<Vec<(String, String, String)>> {
        let bs = book_slug.to_string();
        let rows = self.engine.select_all_with(
            "SELECT character_name, finding_type, description FROM arc_planning_findings \
             WHERE book_slug = ? ORDER BY character_name, finding_type",
            &[&bs],
        )?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                Some((as_text(r.first()?)?, as_text(r.get(1)?)?, as_text(r.get(2)?)?))
            })
            .collect())
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
fn as_f32(v: &DuckValue) -> Option<f32> {
    as_text(v).and_then(|s| s.parse().ok())
}

fn row_to_state(r: &Vec<DuckValue>) -> Option<CharacterState> {
    if r.len() < 14 {
        return None;
    }
    Some(CharacterState {
        character_name: as_text(&r[1])?,
        chapter_ord: as_i64(&r[2]).unwrap_or(0) as u32,
        state_summary: as_text(&r[3]).unwrap_or_default(),
        changed: as_i64(&r[4]).unwrap_or(0) != 0,
        change_description: as_text(&r[5]),
        agency_score: as_f32(&r[6]),
        active_count: as_i64(&r[7]).unwrap_or(0) as u32,
        passive_count: as_i64(&r[8]).unwrap_or(0) as u32,
        utterance_count: as_i64(&r[9]).map(|u| u as u32),
        chapter_hedge_density: as_f32(&r[10]),
        chapter_interiority_ratio: as_f32(&r[11]),
    })
}

fn row_to_declaration(r: &Vec<DuckValue>) -> Option<ArcDeclaration> {
    if r.len() < 5 {
        return None;
    }
    Some(ArcDeclaration {
        character_name: as_text(&r[0])?,
        arc_type: ArcType::from_label(&as_text(&r[1]).unwrap_or_default()),
        desired_state_start: as_text(&r[2]).unwrap_or_default(),
        desired_midpoint_state: as_text(&r[3]),
        desired_state_end: as_text(&r[4]).unwrap_or_default(),
    })
}

fn row_to_check(r: &Vec<DuckValue>) -> Option<CharacterArcCheck> {
    if r.len() < 5 {
        return None;
    }
    Some(CharacterArcCheck {
        character_name: as_text(&r[0])?,
        check_type: ArcCheckType::from_code(&as_text(&r[1])?)?,
        verdict: ArcVerdict::from_code(&as_text(&r[2]).unwrap_or_default()),
        description: as_text(&r[3]).unwrap_or_default(),
        chapter_ord: as_i64(&r[4]).map(|o| o as u32),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str, ch: u32, changed: bool) -> CharacterState {
        CharacterState {
            character_name: name.into(),
            chapter_ord: ch,
            state_summary: format!("state at {ch}"),
            changed,
            change_description: changed.then(|| "shifted".to_string()),
            agency_score: None,
            active_count: 0,
            passive_count: 0,
            utterance_count: Some(3),
            chapter_hedge_density: Some(0.02),
            chapter_interiority_ratio: None,
        }
    }

    #[test]
    fn dual_writer_state_and_agency_preserve_each_other() {
        let dir = tempfile::tempdir().unwrap();
        let st = CharStore::open(dir.path()).unwrap();
        // LLM state first, then agency — each preserves the other's columns.
        st.upsert_state("b", &state("Mara", 1, true), "now", 7).unwrap();
        st.upsert_agency("b", "Mara", 1, Some(0.7), 5, 2, "now").unwrap();
        let rows = st.states_for_character("b", "Mara").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].state_summary, "state at 1"); // preserved by agency upsert
        assert_eq!(rows[0].agency_score, Some(0.7)); // set by agency upsert
        assert_eq!(rows[0].active_count, 5);
        assert!(rows[0].changed);
        // Order independence: agency first, then state.
        st.upsert_agency("b", "Aldric", 2, Some(0.3), 1, 4, "now").unwrap();
        st.upsert_state("b", &state("Aldric", 2, false), "now", 9).unwrap();
        let a = st.states_for_character("b", "Aldric").unwrap();
        assert_eq!(a[0].agency_score, Some(0.3));
        assert_eq!(a[0].state_summary, "state at 2");
    }

    #[test]
    fn state_hash_and_incremental_clear() {
        let dir = tempfile::tempdir().unwrap();
        let st = CharStore::open(dir.path()).unwrap();
        for ch in 1..=5 {
            st.upsert_state("b", &state("Mara", ch, true), "now", ch as u64).unwrap();
        }
        assert_eq!(st.stored_state_hash("b", "Mara", 3).unwrap(), Some(3));
        assert_eq!(st.stored_state_hash("b", "Mara", 9).unwrap(), None);
        // Edit in ch.3 → drop 3..=5, keep 1..2.
        st.clear_states_from("b", "Mara", 3).unwrap();
        let rows = st.states_for_character("b", "Mara").unwrap();
        assert_eq!(rows.iter().map(|r| r.chapter_ord).collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn declarations_and_checks() {
        let dir = tempfile::tempdir().unwrap();
        let st = CharStore::open(dir.path()).unwrap();
        let d = ArcDeclaration {
            character_name: "Mara".into(),
            arc_type: ArcType::PositiveChange,
            desired_state_start: "defers".into(),
            desired_midpoint_state: None,
            desired_state_end: "acts".into(),
        };
        st.upsert_declaration("b", &d, 42).unwrap();
        assert_eq!(st.declaration("b", "Mara").unwrap().as_ref(), Some(&d));
        assert_eq!(st.all_declarations("b").unwrap().len(), 1);

        let c = CharacterArcCheck {
            character_name: "Mara".into(),
            check_type: ArcCheckType::ArcEarned,
            verdict: ArcVerdict::Gap,
            description: "no preparation".into(),
            chapter_ord: None,
        };
        st.upsert_check("b", &c, "now").unwrap();
        assert_eq!(st.checks_for_character("b", "Mara").unwrap(), vec![c]);
    }

    #[test]
    fn scene_links_and_planning_findings() {
        let dir = tempfile::tempdir().unwrap();
        let st = CharStore::open(dir.path()).unwrap();
        st.upsert_scene_link("b", "sc1", "Mara", Some("midpoint"), Some(6)).unwrap();
        st.upsert_scene_link("b", "sc2", "Mara", None, Some(20)).unwrap();
        assert_eq!(st.scene_chapters("b", "Mara").unwrap(), vec![6, 20]);
        st.upsert_planning_finding("b", "Vorath", "no_scenes", "no linked scenes", "now").unwrap();
        let f = st.planning_findings("b").unwrap();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].1, "no_scenes");
    }
}
