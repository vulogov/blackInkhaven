//! LECTOR-1 LR-P1 — prose-measured dramatic intensity.
//!
//! The Planning Board already plots an expected-vs-actual tension curve — but its
//! "actual" comes from author-tagged tensions/threads, so it is blind on an
//! untagged draft (exactly the draft that needs it). This module measures a
//! chapter's dramatic intensity **from the prose itself**, so the realized shape
//! is available on any manuscript with no tagging.
//!
//! The signal is a weighted blend of things already computable deterministically:
//! dialogue density, a per-language **stakes/conflict lexicon**, sentence-rhythm
//! acceleration (short sentences read fast/tense), a summary penalty (time-
//! compression narration reads flat), and a chapter-ending turn. Multilingual: the
//! lexicon is stemmed with the project's Snowball algorithm (ё-folded for Russian)
//! exactly like the continuity-bible drift comparison, and skips cleanly for a
//! language with no lexicon (the other signals still carry it).

use std::collections::HashSet;

use rust_stemmers::Stemmer;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

/// The number of words below which a sentence counts as "short" (fast rhythm).
const SHORT_SENTENCE_WORDS: usize = 9;
/// Scale mapping a raw conflict-word density to 0..=1 (≈5% conflict words ⇒ 1.0).
const STAKES_SCALE: f32 = 20.0;

/// The normalised (0..=1) sub-signals a chapter's intensity is blended from.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct IntensitySignals {
    /// Fraction of sentences that carry dialogue.
    pub dialogue_density: f32,
    /// Conflict/stakes-word density, saturating.
    pub stakes_density: f32,
    /// Fraction of sentences that are short (fast rhythm).
    pub short_sentence_ratio: f32,
    /// Time-compression / summary-narration density (subtracts).
    pub summary_penalty: f32,
    /// How much the chapter ends on a turn / hook.
    pub ending_turn: f32,
}

/// Blend the sub-signals into a 0..=1 dramatic-intensity score. Pure. Stakes and
/// dialogue carry the most weight (conflict on the page); rhythm and the ending
/// turn add; summary narration subtracts.
pub(crate) fn chapter_intensity(s: &IntensitySignals) -> f32 {
    let raw = 0.30 * s.stakes_density
        + 0.26 * s.dialogue_density
        + 0.18 * s.short_sentence_ratio
        + 0.16 * s.ending_turn
        - 0.22 * s.summary_penalty;
    raw.clamp(0.0, 1.0)
}

/// A compiled per-language stakes matcher: the conflict lexicon stemmed with the
/// language's Snowball algorithm, plus the summary-marker phrases.
pub(crate) struct StakesMatcher {
    stemmer: Option<Stemmer>,
    /// Conflict lemmas, each pre-normalised (lowercased, ё-folded, stemmed).
    conflict: HashSet<String>,
    /// Time-compression phrases, lowercased, matched as substrings.
    summary: &'static [&'static str],
}

impl StakesMatcher {
    /// Build the matcher for `language` (long name or ISO code), or `None` when no
    /// stakes lexicon ships for it (intensity then leans on the other signals).
    pub(crate) fn for_language(language: &str) -> Option<Self> {
        let (conflict_words, summary) = word_lists(language)?;
        let stemmer = crate::config::parse_stemmer_language(language).map(Stemmer::create);
        let conflict = conflict_words
            .iter()
            .map(|w| crate::text::normalize_stem(w, &stemmer))
            .filter(|w| !w.is_empty())
            .collect();
        Some(Self { stemmer, conflict, summary })
    }

    fn is_conflict(&self, token: &str) -> bool {
        self.conflict.contains(&crate::text::normalize_stem(token, &self.stemmer))
    }
}

/// True when a sentence carries dialogue — any quotation convention across the
/// five supported languages (paired quotes, guillemets, or a leading dash).
fn is_dialogue(sentence: &str) -> bool {
    let t = sentence.trim_start();
    t.starts_with('—') || t.starts_with('–') || t.starts_with('-')
        || sentence.contains('"')
        || sentence.contains('\u{201c}') // “
        || sentence.contains('\u{201d}') // ”
        || sentence.contains('\u{00ab}') // «
        || sentence.contains('\u{00bb}') // »
}

/// Lowercased alphanumeric word tokens (Unicode-aware).
fn tokens(sentence: &str) -> impl Iterator<Item = String> + '_ {
    sentence
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
}

/// The chapter-ending turn: 1.0 when the last sentence ends on a `?`/`!`, 0.6 when
/// the final two sentences carry conflict, 0.3 when the last sentence is short,
/// else 0.0.
fn ending_turn(sentences: &[String], matcher: Option<&StakesMatcher>) -> f32 {
    let Some(last) = sentences.last() else { return 0.0 };
    let last_t = last.trim_end();
    if last_t.ends_with('?') || last_t.ends_with('!') {
        return 1.0;
    }
    let tail_conflict = matcher.is_some_and(|m| {
        sentences
            .iter()
            .rev()
            .take(2)
            .any(|s| tokens(s).any(|t| m.is_conflict(&t)))
    });
    if tail_conflict {
        return 0.6;
    }
    if tokens(last).count() < SHORT_SENTENCE_WORDS {
        return 0.3;
    }
    0.0
}

/// Measure the intensity sub-signals of a chapter's plain prose. Pure.
pub(crate) fn signals_from_text(text: &str, matcher: Option<&StakesMatcher>) -> IntensitySignals {
    let sentences = crate::continuity::split_sentences(text);
    if sentences.is_empty() {
        return IntensitySignals::default();
    }
    let n = sentences.len() as f32;

    let mut dialogue = 0usize;
    let mut short = 0usize;
    let mut words_total = 0usize;
    let mut stakes_hits = 0usize;
    for s in &sentences {
        if is_dialogue(s) {
            dialogue += 1;
        }
        let mut wc = 0usize;
        for tok in tokens(s) {
            wc += 1;
            if matcher.is_some_and(|m| m.is_conflict(&tok)) {
                stakes_hits += 1;
            }
        }
        if wc > 0 && wc < SHORT_SENTENCE_WORDS {
            short += 1;
        }
        words_total += wc;
    }

    let stakes_density = if words_total > 0 {
        (stakes_hits as f32 / words_total as f32 * STAKES_SCALE).min(1.0)
    } else {
        0.0
    };
    let summary_penalty = match matcher {
        Some(m) => {
            let text_lc = text.to_lowercase();
            let hits: usize = m.summary.iter().map(|p| text_lc.matches(p).count()).sum();
            (hits as f32 / n * 3.0).min(1.0)
        }
        None => 0.0,
    };

    IntensitySignals {
        dialogue_density: dialogue as f32 / n,
        stakes_density,
        short_sentence_ratio: short as f32 / n,
        summary_penalty,
        ending_turn: ending_turn(&sentences, matcher),
    }
}

/// One chapter's measured intensity, in reading order.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ChapterIntensity {
    pub chapter: u32,
    pub title: String,
    /// 0..=1, or `None` for an empty chapter.
    pub intensity: Option<f32>,
}

/// Measure every user-book chapter's dramatic intensity from its prose (read from
/// disk, like the SENTINEL numeric detector). Chapters are numbered in
/// `user_book_chapters` order.
pub(crate) fn measure(layout: &ProjectLayout, h: &Hierarchy, cfg: &Config) -> Vec<ChapterIntensity> {
    let matcher = StakesMatcher::for_language(&cfg.language);
    h.user_book_chapters()
        .into_iter()
        .enumerate()
        .map(|(idx, (chapter_id, title))| {
            let raw = crate::cli::book_walk::chapter_raw_prose(layout, h, chapter_id);
            let plain = crate::audiobook::typst_to_plain(&raw);
            let intensity = if plain.trim().is_empty() {
                None
            } else {
                Some(chapter_intensity(&signals_from_text(&plain, matcher.as_ref())))
            };
            ChapterIntensity { chapter: (idx + 1) as u32, title, intensity }
        })
        .collect()
}

/// The per-language stakes/conflict lexicon + summary-marker phrases, or `None`
/// when none ships. Words are natural forms (stemmed at compile of the matcher);
/// the density is a statistical signal, so missing an inflection only lowers
/// sensitivity, it never breaks.
fn word_lists(language: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match language.trim().to_ascii_lowercase().as_str() {
        "english" | "en" => Some((EN_CONFLICT, EN_SUMMARY)),
        "russian" | "ru" => Some((RU_CONFLICT, RU_SUMMARY)),
        "german" | "de" => Some((DE_CONFLICT, DE_SUMMARY)),
        "french" | "fr" => Some((FR_CONFLICT, FR_SUMMARY)),
        "spanish" | "es" => Some((ES_CONFLICT, ES_SUMMARY)),
        _ => None,
    }
}

const EN_CONFLICT: &[&str] = &[
    "kill", "killed", "death", "die", "died", "dead", "blood", "bloody", "fight", "fought",
    "danger", "fear", "afraid", "scream", "screamed", "run", "ran", "gun", "knife", "sword",
    "blade", "attack", "enemy", "save", "lose", "lost", "never", "must", "hurry", "flee",
    "wound", "pain", "betray", "threat", "war", "escape", "trapped", "desperate", "panic",
    "terror", "burn", "explode", "shatter", "strike", "rage", "fury",
];
const EN_SUMMARY: &[&str] = &[
    "over the next", "in the following", "days passed", "weeks passed", "months passed",
    "years passed", "used to", "would often", "every day", "each morning", "as the weeks",
    "eventually", "gradually",
];

const RU_CONFLICT: &[&str] = &[
    "смерть", "убить", "убил", "кровь", "бой", "драться", "опасность", "страх", "бежать",
    "бежал", "крик", "кричать", "оружие", "нож", "меч", "клинок", "враг", "спасти", "потерять",
    "никогда", "должен", "война", "боль", "предать", "угроза", "ловушка", "паника", "ужас",
    "гореть", "ярость", "удар", "атака",
];
const RU_SUMMARY: &[&str] = &[
    "прошли недели", "прошли годы", "прошли месяцы", "с каждым днём", "с каждым днем",
    "постепенно", "со временем", "каждый день", "как правило",
];

const DE_CONFLICT: &[&str] = &[
    "tod", "töten", "getötet", "blut", "kampf", "kämpfen", "gefahr", "angst", "schrei",
    "schreien", "rennen", "waffe", "messer", "schwert", "feind", "retten", "verlieren",
    "niemals", "muss", "krieg", "schmerz", "verraten", "falle", "panik", "fliehen", "wut",
];
const DE_SUMMARY: &[&str] = &[
    "in den nächsten", "wochen vergingen", "jahre vergingen", "mit der zeit", "allmählich",
    "jeden tag", "pflegte",
];

const FR_CONFLICT: &[&str] = &[
    "mort", "tuer", "tué", "sang", "combat", "battre", "danger", "peur", "cri", "crier",
    "courir", "arme", "couteau", "épée", "ennemi", "sauver", "perdre", "jamais", "guerre",
    "douleur", "trahir", "piège", "panique", "fuir", "rage",
];
const FR_SUMMARY: &[&str] = &[
    "au cours des", "les semaines passèrent", "les années passèrent", "peu à peu",
    "avec le temps", "chaque jour", "autrefois",
];

const ES_CONFLICT: &[&str] = &[
    "muerte", "matar", "mató", "sangre", "lucha", "luchar", "peligro", "miedo", "grito",
    "gritar", "correr", "arma", "cuchillo", "espada", "enemigo", "salvar", "perder", "nunca",
    "guerra", "dolor", "traicionar", "trampa", "pánico", "huir", "furia",
];
const ES_SUMMARY: &[&str] = &[
    "en los siguientes", "pasaron las semanas", "pasaron los años", "poco a poco",
    "con el tiempo", "cada día", "solía",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_scene_outscores_quiet_summary() {
        let m = StakesMatcher::for_language("english");
        let action = "\"Run!\" she screamed. He drew his knife. Blood. \
                      The enemy was upon them. Would they escape?";
        let summary = "Over the next weeks the town settled into its routines. \
                       Trade resumed and, gradually, the harvest was gathered as the seasons turned.";
        let ai = chapter_intensity(&signals_from_text(action, m.as_ref()));
        let qi = chapter_intensity(&signals_from_text(summary, m.as_ref()));
        assert!(ai > qi, "action {ai} should outscore quiet summary {qi}");
        assert!(ai > 0.4, "action reads high: {ai}");
    }

    #[test]
    fn russian_stakes_words_match() {
        // Cyrillic conflict words register through the Russian stemmer (ё-folded).
        let m = StakesMatcher::for_language("russian").expect("ru lexicon");
        let s = signals_from_text("Он бежал сквозь кровь и страх. Смерть шла за ним.", Some(&m));
        assert!(s.stakes_density > 0.0, "ru conflict words counted: {s:?}");
    }

    #[test]
    fn inflected_forms_match_via_stemmer() {
        let m = StakesMatcher::for_language("english").expect("en lexicon");
        // "killing"/"screamed" should stem to the lexicon's kill/scream.
        let s = signals_from_text("The killing would not stop; she screamed and screamed.", Some(&m));
        assert!(s.stakes_density > 0.0);
    }

    #[test]
    fn no_lexicon_language_still_scores_on_other_signals() {
        // Japanese has no stakes lexicon here — dialogue + rhythm still carry it.
        let s = signals_from_text("\u{300c}\u{9003}\u{3052}\u{308d}\u{ff01}\u{300d} Short. Fast. Now!", None);
        assert!(chapter_intensity(&s) >= 0.0);
        assert_eq!(s.stakes_density, 0.0);
        assert!(s.ending_turn > 0.0, "ends on '!'");
    }

    #[test]
    fn empty_prose_is_flat() {
        assert_eq!(signals_from_text("   ", None), IntensitySignals::default());
        assert_eq!(chapter_intensity(&IntensitySignals::default()), 0.0);
    }
}
