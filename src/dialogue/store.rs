//! DIALOG-1 (D-P3) — `.inkhaven/dialogue.duckdb`, the per-feature DuckDB store
//! for detected spans, per-chapter stats, and per-character fingerprints. Built
//! on the shared pooled [`StorageEngine`], mirroring `ProseStore`: real-valued
//! columns stored as TEXT (parsed in Rust), counts as INTEGER, booleans as
//! INTEGER 0/1, `text_hash` (a `DefaultHasher` u64) as TEXT.

use std::path::Path;

use anyhow::Result;
use duckdb::types::Value as DuckValue;

use crate::storage::engine::StorageEngine;

use super::{
    AttributionConfidence, ChapterDialogueStats, CharacterDialogueFingerprint, DialogueSpan,
    SpanForm, TagVerbClass,
};

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS dialogue_spans (
    book_slug         TEXT    NOT NULL,
    chapter_ord       INTEGER NOT NULL,
    para_id           TEXT    NOT NULL,
    span_index        INTEGER NOT NULL,
    span_form         TEXT    NOT NULL,
    speech_text       TEXT    NOT NULL,
    word_count        INTEGER NOT NULL,
    attribution_name  TEXT,
    attribution_conf  TEXT    NOT NULL,
    tag_verb          TEXT,
    tag_verb_class    TEXT,
    ends_question     INTEGER NOT NULL,
    ends_exclamation  INTEGER NOT NULL,
    computed_at       TEXT    NOT NULL,
    text_hash         TEXT    NOT NULL,
    PRIMARY KEY (book_slug, para_id, span_index)
);
CREATE TABLE IF NOT EXISTS dialogue_chapter_stats (
    book_slug              TEXT    NOT NULL,
    chapter_ord            INTEGER NOT NULL,
    total_spans            INTEGER NOT NULL,
    zero_attribution_count INTEGER NOT NULL,
    said_bookism_count     INTEGER NOT NULL,
    neutral_tag_count      INTEGER NOT NULL,
    said_bookism_density   TEXT    NOT NULL,
    dialogue_word_count    INTEGER NOT NULL,
    total_word_count       INTEGER NOT NULL,
    dialogue_density_ratio TEXT    NOT NULL,
    talking_head_sequences INTEGER NOT NULL,
    computed_at            TEXT    NOT NULL,
    text_hash              TEXT    NOT NULL,
    PRIMARY KEY (book_slug, chapter_ord)
);
CREATE TABLE IF NOT EXISTS character_dialogue_fingerprints (
    book_slug            TEXT    NOT NULL,
    character_name       TEXT    NOT NULL,
    utterance_count      INTEGER NOT NULL,
    mean_utterance_words TEXT    NOT NULL,
    utterance_mattr      TEXT    NOT NULL,
    question_ratio       TEXT    NOT NULL,
    exclamation_ratio    TEXT    NOT NULL,
    hedge_density        TEXT    NOT NULL,
    last_chapter_seen    INTEGER NOT NULL,
    computed_at          TEXT    NOT NULL,
    PRIMARY KEY (book_slug, character_name)
);
CREATE INDEX IF NOT EXISTS idx_dialogue_spans_book_chapter
    ON dialogue_spans (book_slug, chapter_ord);
CREATE INDEX IF NOT EXISTS idx_dialogue_spans_character
    ON dialogue_spans (book_slug, attribution_name);
"#;

const SPAN_COLS: &str = "book_slug, chapter_ord, para_id, span_index, span_form, \
    speech_text, word_count, attribution_name, attribution_conf, tag_verb, \
    tag_verb_class, ends_question, ends_exclamation, computed_at, text_hash";

const STAT_COLS: &str = "book_slug, chapter_ord, total_spans, zero_attribution_count, \
    said_bookism_count, neutral_tag_count, said_bookism_density, dialogue_word_count, \
    total_word_count, dialogue_density_ratio, talking_head_sequences, computed_at, text_hash";

const FP_COLS: &str = "book_slug, character_name, utterance_count, mean_utterance_words, \
    utterance_mattr, question_ratio, exclamation_ratio, hedge_density, last_chapter_seen, \
    computed_at";

pub(crate) struct DialogueStore {
    engine: StorageEngine,
}

impl DialogueStore {
    /// Open (creating if needed) `<project_root>/.inkhaven/dialogue.duckdb`.
    pub(crate) fn open(project_root: &Path) -> Result<DialogueStore> {
        let dir = project_root.join(".inkhaven");
        std::fs::create_dir_all(&dir)?;
        let engine = StorageEngine::new(dir.join("dialogue.duckdb"), SCHEMA, 4)?;
        Ok(DialogueStore { engine })
    }

    /// Delete a chapter's spans + stats before re-detecting (a clean recompute).
    pub(crate) fn clear_chapter(&self, book_slug: &str, chapter_ord: u32) -> Result<()> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        self.engine.execute_with(
            "DELETE FROM dialogue_spans WHERE book_slug = ? AND chapter_ord = ?",
            &[&bs, &ord],
        )?;
        self.engine.execute_with(
            "DELETE FROM dialogue_chapter_stats WHERE book_slug = ? AND chapter_ord = ?",
            &[&bs, &ord],
        )?;
        Ok(())
    }

    pub(crate) fn upsert_span(
        &self,
        book_slug: &str,
        chapter_ord: u32,
        span: &DialogueSpan,
        computed_at: &str,
        text_hash: u64,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let para = span.para_id.clone();
        let idx = span.span_index as i64;
        let form = span.form.as_code().to_string();
        let speech = span.speech_text.clone();
        let wc = span.word_count as i64;
        let name = span.attribution_name.clone();
        let conf = span.attribution_conf.as_code().to_string();
        let verb = span.tag_verb.clone();
        let class = span.tag_verb_class.map(|c| c.as_code().to_string());
        let q = span.ends_question as i64;
        let ex = span.ends_exclamation as i64;
        let ca = computed_at.to_string();
        let hash = text_hash.to_string();
        let sql = format!(
            "INSERT OR REPLACE INTO dialogue_spans ({SPAN_COLS}) VALUES \
             (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> = vec![
            &bs, &ord, &para, &idx, &form, &speech, &wc, &name, &conf, &verb, &class, &q, &ex,
            &ca, &hash,
        ];
        self.engine.execute_with(&sql, &params)
    }

    pub(crate) fn upsert_chapter_stats(
        &self,
        book_slug: &str,
        s: &ChapterDialogueStats,
        computed_at: &str,
        text_hash: u64,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let ord = s.chapter_ord as i64;
        let total = s.total_spans as i64;
        let zero = s.zero_attribution_count as i64;
        let sb = s.said_bookism_count as i64;
        let neu = s.neutral_tag_count as i64;
        let sbd = s.said_bookism_density.to_string();
        let dwc = s.dialogue_word_count as i64;
        let twc = s.total_word_count as i64;
        let ddr = s.dialogue_density_ratio.to_string();
        let th = s.talking_head_sequences as i64;
        let ca = computed_at.to_string();
        let hash = text_hash.to_string();
        let sql = format!(
            "INSERT OR REPLACE INTO dialogue_chapter_stats ({STAT_COLS}) VALUES \
             (?,?,?,?,?,?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> = vec![
            &bs, &ord, &total, &zero, &sb, &neu, &sbd, &dwc, &twc, &ddr, &th, &ca, &hash,
        ];
        self.engine.execute_with(&sql, &params)
    }

    pub(crate) fn upsert_fingerprint(
        &self,
        book_slug: &str,
        fp: &CharacterDialogueFingerprint,
        last_chapter_seen: u32,
        computed_at: &str,
    ) -> Result<()> {
        let bs = book_slug.to_string();
        let name = fp.character_name.clone();
        let uc = fp.utterance_count as i64;
        let muw = fp.mean_utterance_words.to_string();
        let mattr = fp.utterance_mattr.to_string();
        let qr = fp.question_ratio.to_string();
        let er = fp.exclamation_ratio.to_string();
        let hd = fp.hedge_density.to_string();
        let lcs = last_chapter_seen as i64;
        let ca = computed_at.to_string();
        let sql = format!(
            "INSERT OR REPLACE INTO character_dialogue_fingerprints ({FP_COLS}) VALUES \
             (?,?,?,?,?,?,?,?,?,?)"
        );
        let params: Vec<&dyn duckdb::ToSql> =
            vec![&bs, &name, &uc, &muw, &mattr, &qr, &er, &hd, &lcs, &ca];
        self.engine.execute_with(&sql, &params)
    }

    pub(crate) fn spans_for_chapter(
        &self,
        book_slug: &str,
        chapter_ord: u32,
    ) -> Result<Vec<DialogueSpan>> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let sql = format!(
            "SELECT {SPAN_COLS} FROM dialogue_spans WHERE book_slug = ? AND chapter_ord = ? \
             ORDER BY para_id, span_index"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs, &ord])?;
        Ok(rows.iter().filter_map(|r| row_to_span(r)).collect())
    }

    /// All spans across the book whose attribution is `Certain` — the fingerprint
    /// corpus. (Filtered in Rust so callers can group by name.)
    pub(crate) fn certain_spans(&self, book_slug: &str) -> Result<Vec<(u32, DialogueSpan)>> {
        let bs = book_slug.to_string();
        let sql = format!(
            "SELECT chapter_ord, {SPAN_COLS} FROM dialogue_spans \
             WHERE book_slug = ? AND attribution_conf = 'certain' ORDER BY chapter_ord"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let ord = as_i64(r.first()?)? as u32;
                row_to_span(&r[1..]).map(|s| (ord, s))
            })
            .collect())
    }

    /// CHORUS-1 (CH-P0) — every span across the book that resolved to a speaker
    /// (`Certain` OR `Inferred`; `None` excluded), for the character-voice
    /// corpus. Unlike [`certain_spans`], this keeps pronoun/action-beat–inferred
    /// lines so a character's aggregated dialogue is as complete as attribution
    /// allows; each span carries its own `attribution_conf` so a caller can weigh
    /// by confidence.
    pub(crate) fn attributed_spans(&self, book_slug: &str) -> Result<Vec<(u32, DialogueSpan)>> {
        let bs = book_slug.to_string();
        let sql = format!(
            "SELECT chapter_ord, {SPAN_COLS} FROM dialogue_spans \
             WHERE book_slug = ? AND attribution_conf <> 'none' ORDER BY chapter_ord"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows
            .iter()
            .filter_map(|r| {
                let ord = as_i64(r.first()?)? as u32;
                row_to_span(&r[1..]).map(|s| (ord, s))
            })
            .collect())
    }

    pub(crate) fn chapter_stats(
        &self,
        book_slug: &str,
        chapter_ord: u32,
    ) -> Result<Option<ChapterDialogueStats>> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let sql = format!(
            "SELECT {STAT_COLS} FROM dialogue_chapter_stats WHERE book_slug = ? AND chapter_ord = ?"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs, &ord])?;
        Ok(rows.first().and_then(|r| row_to_stats(r)))
    }

    pub(crate) fn all_chapter_stats(&self, book_slug: &str) -> Result<Vec<ChapterDialogueStats>> {
        let bs = book_slug.to_string();
        let sql = format!(
            "SELECT {STAT_COLS} FROM dialogue_chapter_stats WHERE book_slug = ? ORDER BY chapter_ord"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows.iter().filter_map(|r| row_to_stats(r)).collect())
    }

    pub(crate) fn fingerprint(
        &self,
        book_slug: &str,
        character_name: &str,
    ) -> Result<Option<CharacterDialogueFingerprint>> {
        let bs = book_slug.to_string();
        // Case-insensitive match against the Characters-book entry.
        let name = character_name.to_string();
        let sql = format!(
            "SELECT {FP_COLS} FROM character_dialogue_fingerprints \
             WHERE book_slug = ? AND lower(character_name) = lower(?)"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs, &name])?;
        Ok(rows.first().and_then(|r| row_to_fingerprint(r)))
    }

    pub(crate) fn all_fingerprints(
        &self,
        book_slug: &str,
    ) -> Result<Vec<CharacterDialogueFingerprint>> {
        let bs = book_slug.to_string();
        let sql = format!(
            "SELECT {FP_COLS} FROM character_dialogue_fingerprints \
             WHERE book_slug = ? ORDER BY utterance_count DESC"
        );
        let rows = self.engine.select_all_with(&sql, &[&bs])?;
        Ok(rows.iter().filter_map(|r| row_to_fingerprint(r)).collect())
    }

    /// The stored chapter-stats text hash (staleness key), if present.
    pub(crate) fn stored_chapter_hash(&self, book_slug: &str, chapter_ord: u32) -> Result<Option<u64>> {
        let bs = book_slug.to_string();
        let ord = chapter_ord as i64;
        let rows = self.engine.select_all_with(
            "SELECT text_hash FROM dialogue_chapter_stats WHERE book_slug = ? AND chapter_ord = ?",
            &[&bs, &ord],
        )?;
        Ok(rows
            .first()
            .and_then(|r| r.first())
            .and_then(as_text)
            .and_then(|s| s.parse().ok()))
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

fn row_to_span(r: &[DuckValue]) -> Option<DialogueSpan> {
    if r.len() < 15 {
        return None;
    }
    Some(DialogueSpan {
        para_id: as_text(&r[2])?,
        span_index: as_i64(&r[3]).unwrap_or(0) as u32,
        form: SpanForm::from_code(&as_text(&r[4]).unwrap_or_default()),
        char_start: 0,
        char_end: 0,
        speech_text: as_text(&r[5]).unwrap_or_default(),
        word_count: as_i64(&r[6]).unwrap_or(0) as u32,
        attribution_name: as_text(&r[7]),
        attribution_conf: AttributionConfidence::from_code(&as_text(&r[8]).unwrap_or_default()),
        has_attribution_signal: false,
        tag_verb: as_text(&r[9]),
        tag_verb_class: as_text(&r[10]).and_then(|s| TagVerbClass::from_code(&s)),
        ends_question: as_i64(&r[11]).unwrap_or(0) != 0,
        ends_exclamation: as_i64(&r[12]).unwrap_or(0) != 0,
    })
}

fn row_to_stats(r: &[DuckValue]) -> Option<ChapterDialogueStats> {
    if r.len() < 13 {
        return None;
    }
    Some(ChapterDialogueStats {
        chapter_ord: as_i64(&r[1]).unwrap_or(0) as u32,
        total_spans: as_i64(&r[2]).unwrap_or(0) as u32,
        zero_attribution_count: as_i64(&r[3]).unwrap_or(0) as u32,
        said_bookism_count: as_i64(&r[4]).unwrap_or(0) as u32,
        neutral_tag_count: as_i64(&r[5]).unwrap_or(0) as u32,
        said_bookism_density: as_f32(&r[6]).unwrap_or(0.0),
        dialogue_word_count: as_i64(&r[7]).unwrap_or(0) as u32,
        total_word_count: as_i64(&r[8]).unwrap_or(0) as u32,
        dialogue_density_ratio: as_f32(&r[9]).unwrap_or(0.0),
        talking_head_sequences: as_i64(&r[10]).unwrap_or(0) as u32,
    })
}

fn row_to_fingerprint(r: &[DuckValue]) -> Option<CharacterDialogueFingerprint> {
    if r.len() < 10 {
        return None;
    }
    Some(CharacterDialogueFingerprint {
        character_name: as_text(&r[1])?,
        utterance_count: as_i64(&r[2]).unwrap_or(0) as u32,
        mean_utterance_words: as_f32(&r[3]).unwrap_or(0.0),
        utterance_mattr: as_f32(&r[4]).unwrap_or(0.0),
        question_ratio: as_f32(&r[5]).unwrap_or(0.0),
        exclamation_ratio: as_f32(&r[6]).unwrap_or(0.0),
        hedge_density: as_f32(&r[7]).unwrap_or(0.0),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(idx: u32, name: Option<&str>, conf: AttributionConfidence) -> DialogueSpan {
        DialogueSpan {
            para_id: "para-1".into(),
            span_index: idx,
            form: SpanForm::QuotePair,
            char_start: 0,
            char_end: 0,
            speech_text: "Hello there".into(),
            word_count: 2,
            attribution_name: name.map(|s| s.to_string()),
            attribution_conf: conf,
            has_attribution_signal: true,
            tag_verb: Some("said".into()),
            tag_verb_class: Some(TagVerbClass::Neutral),
            ends_question: false,
            ends_exclamation: false,
        }
    }

    #[test]
    fn span_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        st.upsert_span("book", 3, &span(0, Some("Mara"), AttributionConfidence::Certain), "now", 42)
            .unwrap();
        st.upsert_span("book", 3, &span(1, None, AttributionConfidence::None), "now", 42)
            .unwrap();
        let back = st.spans_for_chapter("book", 3).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back[0].attribution_name.as_deref(), Some("Mara"));
        assert_eq!(back[0].attribution_conf, AttributionConfidence::Certain);
        assert_eq!(back[0].tag_verb_class, Some(TagVerbClass::Neutral));
        // Only the Certain span is in the fingerprint corpus.
        let certain = st.certain_spans("book").unwrap();
        assert_eq!(certain.len(), 1);
        assert_eq!(certain[0].0, 3);
    }

    #[test]
    fn attributed_spans_includes_inferred_excludes_none() {
        // CHORUS-1 (CH-P0): the character-voice corpus keeps Certain AND
        // Inferred lines, dropping only unattributed (None) ones.
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        st.upsert_span("book", 2, &span(0, Some("Mara"), AttributionConfidence::Certain), "now", 1)
            .unwrap();
        st.upsert_span("book", 2, &span(1, Some("Joren"), AttributionConfidence::Inferred), "now", 1)
            .unwrap();
        st.upsert_span("book", 2, &span(2, None, AttributionConfidence::None), "now", 1)
            .unwrap();
        // certain_spans keeps only the Certain line…
        assert_eq!(st.certain_spans("book").unwrap().len(), 1);
        // …attributed_spans keeps Certain + Inferred, drops None.
        let attributed = st.attributed_spans("book").unwrap();
        assert_eq!(attributed.len(), 2);
        let mut names: Vec<String> = attributed
            .iter()
            .filter_map(|(_, s)| s.attribution_name.clone())
            .collect();
        names.sort();
        assert_eq!(names, vec!["Joren".to_string(), "Mara".to_string()]);
    }

    #[test]
    fn stats_and_hash_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        let s = ChapterDialogueStats {
            chapter_ord: 12,
            total_spans: 34,
            zero_attribution_count: 3,
            said_bookism_count: 10,
            neutral_tag_count: 24,
            said_bookism_density: 0.29,
            dialogue_word_count: 540,
            total_word_count: 1000,
            dialogue_density_ratio: 0.54,
            talking_head_sequences: 1,
        };
        st.upsert_chapter_stats("book", &s, "now", 99).unwrap();
        assert_eq!(st.chapter_stats("book", 12).unwrap().as_ref(), Some(&s));
        assert_eq!(st.stored_chapter_hash("book", 12).unwrap(), Some(99));
        assert_eq!(st.stored_chapter_hash("book", 13).unwrap(), None);
    }

    #[test]
    fn fingerprint_round_trip_and_case_insensitive() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        let fp = CharacterDialogueFingerprint {
            character_name: "Mara".into(),
            utterance_count: 47,
            mean_utterance_words: 11.3,
            utterance_mattr: 0.74,
            question_ratio: 0.31,
            exclamation_ratio: 0.08,
            hedge_density: 0.019,
        };
        st.upsert_fingerprint("book", &fp, 12, "now").unwrap();
        assert_eq!(st.fingerprint("book", "mara").unwrap().as_ref(), Some(&fp));
        assert_eq!(st.all_fingerprints("book").unwrap().len(), 1);
    }

    #[test]
    fn clear_chapter_removes_spans_and_stats() {
        let dir = tempfile::tempdir().unwrap();
        let st = DialogueStore::open(dir.path()).unwrap();
        st.upsert_span("book", 3, &span(0, Some("Mara"), AttributionConfidence::Certain), "now", 1)
            .unwrap();
        st.clear_chapter("book", 3).unwrap();
        assert!(st.spans_for_chapter("book", 3).unwrap().is_empty());
    }
}
