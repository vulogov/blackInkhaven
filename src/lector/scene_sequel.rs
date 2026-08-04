//! LECTOR-1 LR-P2 — scene / sequel micro-structure.
//!
//! The Swain/Bickham axis nobody measures: fiction alternates **scenes** (goal →
//! conflict → disaster — forward, external, high-energy) with **sequels**
//! (reaction → dilemma → decision — reflective, internal, lower-energy). A
//! healthy read breathes between the two; an all-scene stretch reads breathless,
//! an all-sequel stretch sags.
//!
//! LECTOR classifies each chapter from prose signals — the LR-P1 intensity +
//! dialogue (scene evidence) versus interiority verbs + deliberation markers
//! (sequel evidence) — and flags the arrhythmia. Multilingual, same stemmed-lexicon
//! plumbing as the intensity signal.

// `chapter_kinds`/`scan` are consumed by LR-P3 (the walk) / LR-P5 (the report) /
// LR-P6 (the rails); scaffolding until then. Drop this allow at LR-P3.
#![allow(dead_code)]

use std::collections::HashSet;

use rust_stemmers::Stemmer;

use super::intensity;
use super::{ReaderFinding, Severity};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;

/// Saturating scales mapping a raw interiority / deliberation word density to 0..1.
const REFLECTION_SCALE: f32 = 16.0;
const DELIBERATION_SCALE: f32 = 20.0;
/// Consecutive same-kind runs that read as arrhythmia.
const BREATHLESS_MIN: usize = 4; // scenes with no sequel
const SAG_MIN: usize = 3; // sequels with no scene

/// A chapter's place on the scene ⇄ sequel axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SceneKind {
    Scene,
    Sequel,
    /// Transitional / summary / balanced — breaks a run of either.
    Mixed,
}

impl SceneKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SceneKind::Scene => "scene",
            SceneKind::Sequel => "sequel",
            SceneKind::Mixed => "mixed",
        }
    }
}

/// The normalised evidence for the classification.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct SceneSignals {
    /// LR-P1 dramatic intensity (scene evidence).
    pub intensity: f32,
    /// Dialogue density (scene evidence).
    pub dialogue_density: f32,
    /// Interiority / thought-verb density (sequel evidence).
    pub reflection_density: f32,
    /// Deliberation / decision-marker density (sequel evidence).
    pub deliberation: f32,
}

/// Classify from the evidence. Pure. Scene = forward + external; sequel =
/// reflective + internal; Mixed when neither clearly wins.
pub(crate) fn classify(s: &SceneSignals) -> SceneKind {
    const MARGIN: f32 = 0.12;
    const FLOOR: f32 = 0.20;
    let scene = 0.55 * s.intensity + 0.45 * s.dialogue_density;
    let sequel = 0.65 * s.reflection_density + 0.35 * s.deliberation;
    if scene >= sequel + MARGIN && scene >= FLOOR {
        SceneKind::Scene
    } else if sequel >= scene + MARGIN && sequel >= FLOOR {
        SceneKind::Sequel
    } else {
        SceneKind::Mixed
    }
}

/// Normalise (lowercase, ё-fold, stem) each lexicon word into a lookup set.
fn compile_set(words: &[&str], stemmer: &Option<Stemmer>) -> HashSet<String> {
    words.iter().map(|w| crate::text::normalize_stem(w, stemmer)).filter(|w| !w.is_empty()).collect()
}

/// Lowercased alphanumeric word tokens (Unicode-aware).
fn word_tokens(text: &str) -> impl Iterator<Item = String> + '_ {
    text.split(|c: char| !c.is_alphanumeric()).filter(|w| !w.is_empty()).map(|w| w.to_lowercase())
}

/// The per-language interiority + deliberation lexicons, stemmed like the stakes
/// matcher.
pub(crate) struct ReflectionMatcher {
    stemmer: Option<Stemmer>,
    reflection: HashSet<String>,
    deliberation: HashSet<String>,
}

impl ReflectionMatcher {
    pub(crate) fn for_language(language: &str) -> Option<Self> {
        let (reflection, deliberation) = word_lists(language)?;
        let stemmer = crate::config::parse_stemmer_language(language).map(Stemmer::create);
        let reflection = compile_set(reflection, &stemmer);
        let deliberation = compile_set(deliberation, &stemmer);
        Some(Self { stemmer, reflection, deliberation })
    }

    /// `(reflection_density, deliberation)` over the plain text, saturating.
    pub(crate) fn density(&self, text: &str) -> (f32, f32) {
        let (mut words, mut refl, mut delib) = (0usize, 0usize, 0usize);
        for tok in word_tokens(text) {
            words += 1;
            let stem = crate::text::normalize_stem(&tok, &self.stemmer);
            if self.reflection.contains(&stem) {
                refl += 1;
            }
            if self.deliberation.contains(&stem) {
                delib += 1;
            }
        }
        if words == 0 {
            return (0.0, 0.0);
        }
        let w = words as f32;
        (
            (refl as f32 / w * REFLECTION_SCALE).min(1.0),
            (delib as f32 / w * DELIBERATION_SCALE).min(1.0),
        )
    }
}

/// Classify every user-book chapter in reading order.
pub(crate) fn chapter_kinds(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
) -> Vec<(u32, String, SceneKind)> {
    let stakes = intensity::StakesMatcher::for_language(&cfg.language);
    let refl = ReflectionMatcher::for_language(&cfg.language);
    h.user_book_chapters()
        .into_iter()
        .enumerate()
        .map(|(idx, (chapter_id, title))| {
            let raw = crate::cli::book_walk::chapter_raw_prose(layout, h, chapter_id);
            let plain = crate::audiobook::typst_to_plain(&raw);
            let is = intensity::signals_from_text(&plain, stakes.as_ref());
            let (reflection_density, deliberation) =
                refl.as_ref().map(|m| m.density(&plain)).unwrap_or((0.0, 0.0));
            let kind = classify(&SceneSignals {
                intensity: intensity::chapter_intensity(&is),
                dialogue_density: is.dialogue_density,
                reflection_density,
                deliberation,
            });
            ((idx + 1) as u32, title, kind)
        })
        .collect()
}

/// Flag arrhythmia in the scene/sequel sequence: a long run of scenes with no
/// sequel (breathless), or of sequels with no scene (sag). Pure; a Mixed chapter
/// breaks either run (it gives the reader room). `kinds` is `(chapter, kind)` in
/// reading order.
pub(crate) fn arrhythmia(kinds: &[(u32, SceneKind)]) -> Vec<ReaderFinding> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < kinds.len() {
        let (start_ch, kind) = kinds[i];
        if kind == SceneKind::Mixed {
            i += 1;
            continue;
        }
        let mut j = i;
        while j < kinds.len() && kinds[j].1 == kind {
            j += 1;
        }
        let run = j - i;
        let end_ch = kinds[j - 1].0;
        let flag = match kind {
            SceneKind::Scene if run >= BREATHLESS_MIN => Some((
                "breathless",
                if run >= BREATHLESS_MIN + 2 { Severity::Concern } else { Severity::Notice },
                format!(
                    "ch. {start_ch}\u{2013}{end_ch} run all scene ({run} in a row) with no sequel — \
                     the reader gets no room to breathe."
                ),
            )),
            SceneKind::Sequel if run >= SAG_MIN => Some((
                "sag",
                if run >= SAG_MIN + 2 { Severity::Concern } else { Severity::Notice },
                format!(
                    "ch. {start_ch}\u{2013}{end_ch} run all sequel ({run} in a row) — reflection \
                     without forward motion; the pace sags."
                ),
            )),
            _ => None,
        };
        if let Some((kind_str, severity, message)) = flag {
            out.push(ReaderFinding {
                kind: kind_str,
                severity,
                chapter: start_ch,
                anchor: None,
                dedup_key: ReaderFinding::make_dedup_key(kind_str, &[], start_ch),
                entities: Vec::new(),
                message,
                source: "walk",
            });
        }
        i = j;
    }
    out
}

/// Classify the book and flag its arrhythmia in one call.
pub(crate) fn scan(layout: &ProjectLayout, h: &Hierarchy, cfg: &Config) -> Vec<ReaderFinding> {
    let kinds: Vec<(u32, SceneKind)> =
        chapter_kinds(layout, h, cfg).into_iter().map(|(c, _, k)| (c, k)).collect();
    arrhythmia(&kinds)
}

/// Interiority (thought/feeling verbs) and deliberation (decision/weighing)
/// lexicons per language. Single tokens; stemmed at matcher build.
fn word_lists(language: &str) -> Option<(&'static [&'static str], &'static [&'static str])> {
    match language.trim().to_ascii_lowercase().as_str() {
        "english" | "en" => Some((EN_REFLECT, EN_DELIB)),
        "russian" | "ru" => Some((RU_REFLECT, RU_DELIB)),
        "german" | "de" => Some((DE_REFLECT, DE_DELIB)),
        "french" | "fr" => Some((FR_REFLECT, FR_DELIB)),
        "spanish" | "es" => Some((ES_REFLECT, ES_DELIB)),
        _ => None,
    }
}

const EN_REFLECT: &[&str] = &[
    "felt", "feel", "wondered", "wonder", "realized", "realize", "remembered", "remember",
    "thought", "think", "knew", "considered", "consider", "understood", "understand",
    "imagined", "sensed", "recalled", "reflected", "wished", "hoped", "doubted", "believed",
    "supposed", "regretted", "mused", "pondered",
];
const EN_DELIB: &[&str] = &[
    "perhaps", "maybe", "should", "could", "whether", "decision", "choice", "choose", "decide",
    "weigh", "option", "either", "consider",
];

const RU_REFLECT: &[&str] = &[
    "чувствовал", "думал", "думать", "понял", "понимать", "вспомнил", "помнил", "знал",
    "казалось", "представил", "надеялся", "сомневался", "размышлял", "осознал", "верил",
    "жалел", "мечтал",
];
const RU_DELIB: &[&str] = &[
    "возможно", "может", "если", "выбор", "решение", "решить", "наверное", "стоит",
];

const DE_REFLECT: &[&str] = &[
    "fühlte", "dachte", "wusste", "erkannte", "erinnerte", "verstand", "überlegte", "glaubte",
    "hoffte", "spürte", "ahnte", "bedauerte", "sann",
];
const DE_DELIB: &[&str] = &[
    "vielleicht", "sollte", "könnte", "würde", "entscheidung", "wahl", "wählen", "ob",
];

const FR_REFLECT: &[&str] = &[
    "sentit", "pensa", "savait", "comprit", "réalisa", "souvint", "croyait", "espérait",
    "songea", "imaginait", "doutait", "réfléchit", "regretta",
];
const FR_DELIB: &[&str] = &[
    "peut", "devrait", "pourrait", "choix", "décision", "décider", "choisir", "possible",
];

const ES_REFLECT: &[&str] = &[
    "sintió", "pensó", "sabía", "comprendió", "recordó", "creía", "esperaba", "imaginó",
    "dudaba", "reflexionó", "supuso", "lamentó",
];
const ES_DELIB: &[&str] = &[
    "quizás", "debería", "podría", "elección", "decisión", "decidir", "elegir", "posible",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_scene_sequel_mixed() {
        let scene = classify(&SceneSignals {
            intensity: 0.6,
            dialogue_density: 0.6,
            reflection_density: 0.05,
            deliberation: 0.02,
        });
        assert_eq!(scene, SceneKind::Scene);

        let sequel = classify(&SceneSignals {
            intensity: 0.1,
            dialogue_density: 0.05,
            reflection_density: 0.6,
            deliberation: 0.3,
        });
        assert_eq!(sequel, SceneKind::Sequel);

        let mixed = classify(&SceneSignals {
            intensity: 0.3,
            dialogue_density: 0.2,
            reflection_density: 0.2,
            deliberation: 0.1,
        });
        assert_eq!(mixed, SceneKind::Mixed);
    }

    #[test]
    fn reflection_lexicon_registers_including_russian() {
        let en = ReflectionMatcher::for_language("english").unwrap();
        let (r, d) = en.density("She wondered and remembered, and thought perhaps she should decide.");
        assert!(r > 0.0 && d > 0.0, "en reflection {r} + deliberation {d}");

        let ru = ReflectionMatcher::for_language("russian").unwrap();
        let (rr, _) = ru.density("Он думал и вспоминал, размышлял о случившемся.");
        assert!(rr > 0.0, "ru reflection {rr}");
    }

    #[test]
    fn arrhythmia_flags_breathless_and_sag() {
        use SceneKind::*;
        // Four scenes in a row → breathless.
        let breathless = arrhythmia(&[(1, Scene), (2, Scene), (3, Scene), (4, Scene)]);
        assert_eq!(breathless.len(), 1);
        assert_eq!(breathless[0].kind, "breathless");
        assert_eq!(breathless[0].chapter, 1);

        // Three sequels in a row → sag.
        let sag = arrhythmia(&[(1, Sequel), (2, Sequel), (3, Sequel)]);
        assert_eq!(sag.len(), 1);
        assert_eq!(sag[0].kind, "sag");
    }

    #[test]
    fn healthy_alternation_and_short_runs_are_clean() {
        use SceneKind::*;
        assert!(arrhythmia(&[(1, Scene), (2, Sequel), (3, Scene), (4, Sequel)]).is_empty());
        // A Mixed breaks the run before it reaches the threshold.
        assert!(arrhythmia(&[(1, Scene), (2, Scene), (3, Mixed), (4, Scene), (5, Scene)]).is_empty());
    }
}
