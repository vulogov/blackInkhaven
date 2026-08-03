//! CHORUS-1 (CH-P5) — tense discipline.
//!
//! A manuscript keeps one narrative tense; an accidental slip (a past-tense book
//! that lapses into present) survives line edits because it's structural. This
//! flags it — heuristically, because the tree has no parser: each narrative
//! sentence is classified past/present from **copula/auxiliary anchors**
//! (`was/were/had/did` vs `is/are/am/has/have/does/do`) plus common irregular and
//! regular `-ed` pasts; the scene's dominant tense is the majority; sentences
//! that break it are flagged.
//!
//! ## The language gate (the RFC's honest decision)
//!
//! Covered: **English, German, French, Spanish** — languages that share the "keep
//! one narrative tense" convention, each with its own copula/auxiliary anchors +
//! irregular and regular past-suffix markers. **Russian is excluded by design**:
//! its narrative tense is governed by *aspect* — the historical present and
//! perfective/imperfective interleaving are legitimate devices, not slips, and
//! nothing in the tree models aspect — so a past→present heuristic is *wrong* for
//! Russian. CHORUS says so plainly rather than false-flagging. Other languages
//! are not covered.

use uuid::Uuid;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::{ProseLanguage, resolve_prose_language};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::scenes::book_scenes;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tense {
    Past,
    Present,
}

impl Tense {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Tense::Past => "past",
            Tense::Present => "present",
        }
    }
}

/// One sentence whose tense breaks the scene's dominant tense.
#[derive(Debug, Clone)]
pub(crate) struct TenseSlip {
    pub excerpt: String,
    pub tense: Tense,
}

/// The outcome of scanning one span of prose for tense.
pub(crate) enum TenseScan {
    /// The language isn't covered — carries the honest reason (shown, not hidden).
    Unsupported(&'static str),
    /// Scanned: the dominant tense and the sentences that break it.
    Scanned { dominant: Tense, slips: Vec<TenseSlip> },
}

/// Why a language is (not) covered by the tense heuristic.
pub(crate) fn tense_unsupported(lang: &ProseLanguage) -> Option<&'static str> {
    match lang {
        // English + German + French + Spanish share the "keep one narrative
        // tense" convention, so the past↔present heuristic applies.
        ProseLanguage::En | ProseLanguage::De | ProseLanguage::Fr | ProseLanguage::Es => None,
        ProseLanguage::Ru => Some(
            "Russian narrative tense is governed by aspect — the historical present and \
             perfective/imperfective interleaving are legitimate, not slips — so CHORUS does \
             not flag Russian tense.",
        ),
        ProseLanguage::Other(_) => {
            Some("tense-slip detection covers English, German, French, and Spanish.")
        }
    }
}

/// Classify a span of narration into a dominant tense + the sentences that break
/// it. Dialogue is stripped first (a character's speech tense is not the
/// narration's). Too little classifiable narration → no slips.
pub(crate) fn tense_scan(text: &str, lang: &ProseLanguage) -> TenseScan {
    if let Some(reason) = tense_unsupported(lang) {
        return TenseScan::Unsupported(reason);
    }
    let narration = strip_dialogue(text);
    let classified: Vec<(String, Tense)> = split_sentences(&narration)
        .into_iter()
        .filter_map(|s| classify(&s, lang).map(|t| (s, t)))
        .collect();
    if classified.len() < 3 {
        return TenseScan::Scanned { dominant: Tense::Past, slips: Vec::new() };
    }
    let past = classified.iter().filter(|(_, t)| *t == Tense::Past).count();
    let dominant = if past >= classified.len() - past { Tense::Past } else { Tense::Present };
    let slips = classified
        .iter()
        .filter(|(_, t)| *t != dominant)
        .map(|(s, t)| TenseSlip { excerpt: excerpt(s), tense: *t })
        .collect();
    TenseScan::Scanned { dominant, slips }
}

/// A scene whose narration contains tense slips.
pub(crate) struct SceneTense {
    pub chapter_ord: u32,
    pub scene_index: u32,
    pub first_para: Uuid,
    pub dominant: Tense,
    pub slips: Vec<TenseSlip>,
}

/// The book-wide tense outcome.
pub(crate) enum TenseSummary {
    /// The project language isn't covered — the honest reason.
    Unsupported(&'static str),
    /// Scanned: the scenes that contain slips (empty ⇒ consistent).
    Scanned(Vec<SceneTense>),
}

/// Scan every scene of `book` for tense slips. Gated once on the project
/// language; Russian (and non-baseline languages) return `Unsupported`.
pub(crate) fn scan_tense(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
) -> TenseSummary {
    let (lang, _) = resolve_prose_language(None, &cfg.language);
    // Probe the gate once (also covers an empty book) — reads the reason.
    if let TenseScan::Unsupported(reason) = tense_scan("", &lang) {
        return TenseSummary::Unsupported(reason);
    }
    let mut out = Vec::new();
    for s in book_scenes(layout, h, book) {
        if let TenseScan::Scanned { dominant, slips } = tense_scan(&s.text, &lang) {
            if !slips.is_empty() {
                out.push(SceneTense {
                    chapter_ord: s.chapter_ord,
                    scene_index: s.scene_index,
                    first_para: s.first_para,
                    dominant,
                    slips,
                });
            }
        }
    }
    TenseSummary::Scanned(out)
}

fn excerpt(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() <= 90 {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(88).collect::<String>().trim_end())
    }
}

/// Remove double-quoted dialogue (straight + curly) so only narration is scored.
fn strip_dialogue(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut inside = false;
    for c in text.chars() {
        if c == '"' || c == '\u{201C}' || c == '\u{201D}' {
            inside = !inside;
            out.push(' ');
        } else if !inside {
            out.push(c);
        }
    }
    out
}

fn split_sentences(text: &str) -> Vec<String> {
    text.split(|c| c == '.' || c == '!' || c == '?')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn norm(tok: &str) -> String {
    tok.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

/// Classify one sentence past/present, or `None` when the signal is too weak or
/// balanced. Copula/auxiliary anchors carry the most weight (they are the least
/// ambiguous tense markers); irregular pasts less; a language's regular past
/// suffix least. Language-keyed: English + German + French + Spanish share the
/// "keep one narrative tense" convention (Russian is excluded — its tense is
/// aspect — before this is ever reached).
fn classify(sentence: &str, lang: &ProseLanguage) -> Option<Tense> {
    let m = markers(lang);
    let (mut past, mut present) = (0i32, 0i32);
    for raw in sentence.split_whitespace() {
        let t = norm(raw);
        if t.is_empty() {
            continue;
        }
        if m.past_anchor.contains(&t.as_str()) {
            past += 3;
        } else if m.present_anchor.contains(&t.as_str()) {
            present += 3;
        } else if m.irregular_past.contains(&t.as_str()) {
            past += 2;
        } else if regular_past(&t, lang) {
            past += 1;
        }
    }
    // Need a real anchor/irregular (weight ≥ 2) and a clear winner.
    if past == present || past.max(present) < 2 {
        None
    } else if past > present {
        Some(Tense::Past)
    } else {
        Some(Tense::Present)
    }
}

struct Markers {
    past_anchor: &'static [&'static str],
    present_anchor: &'static [&'static str],
    irregular_past: &'static [&'static str],
}

fn markers(lang: &ProseLanguage) -> Markers {
    match lang {
        ProseLanguage::De => Markers { past_anchor: DE_PAST, present_anchor: DE_PRESENT, irregular_past: DE_IRREGULAR },
        ProseLanguage::Fr => Markers { past_anchor: FR_PAST, present_anchor: FR_PRESENT, irregular_past: FR_IRREGULAR },
        ProseLanguage::Es => Markers { past_anchor: ES_PAST, present_anchor: ES_PRESENT, irregular_past: ES_IRREGULAR },
        _ => Markers { past_anchor: EN_PAST, present_anchor: EN_PRESENT, irregular_past: EN_IRREGULAR },
    }
}

/// A language's *regular* past-tense verb suffix — the weakest signal, so a stray
/// noun that happens to share the ending only nudges, never decides, a sentence
/// that has no stronger anchor.
fn regular_past(t: &str, lang: &ProseLanguage) -> bool {
    match lang {
        // English weak past `-ed`.
        ProseLanguage::En | ProseLanguage::Other(_) => {
            t.len() >= 4 && t.ends_with("ed") && !EN_ED_STOPLIST.contains(&t)
        }
        // German weak preterite `-te`/`-ten` (sagte, machten).
        ProseLanguage::De => {
            t.len() >= 5 && (t.ends_with("te") || t.ends_with("ten")) && !DE_TE_STOPLIST.contains(&t)
        }
        // French imparfait `-ait`/`-aient` (marchait, parlaient) — reliable.
        ProseLanguage::Fr => t.len() >= 5 && (t.ends_with("ait") || t.ends_with("aient")),
        // Spanish imperfecto `-aba`/`-aban` + `-ía`/`-ían` (with a stoplist for
        // common `-ía` nouns) + pretérito `-ó`/`-ió`/`-aron`/`-ieron`.
        ProseLanguage::Es => {
            (t.len() >= 4
                && (t.ends_with("aba")
                    || t.ends_with("aban")
                    || ((t.ends_with("ía") || t.ends_with("ían")) && !ES_IA_STOPLIST.contains(&t))))
                || (t.len() >= 5 && (t.ends_with("aron") || t.ends_with("ieron")))
                || (t.len() >= 3 && (t.ends_with('ó') || t.ends_with("ió")))
        }
        ProseLanguage::Ru => false,
    }
}

const EN_PAST: &[&str] = &["was", "were", "had", "did"];
const EN_PRESENT: &[&str] = &["is", "are", "am", "has", "have", "does", "do"];
const EN_IRREGULAR: &[&str] = &[
    "went", "came", "saw", "said", "knew", "felt", "thought", "took", "made", "found", "gave",
    "told", "ran", "stood", "sat", "became", "held", "heard", "kept", "left", "met", "brought",
    "began", "spoke", "wrote", "drove", "rode", "fell", "rose", "broke", "chose", "grew", "threw",
    "caught", "taught", "bought", "fought", "sought", "understood", "won", "lost", "sent", "spent",
];
const EN_ED_STOPLIST: &[&str] = &[
    "red", "bed", "fed", "led", "wed", "need", "indeed", "instead", "seed", "deed", "speed",
    "bleed", "greed", "freed", "embed", "ahead", "sacred", "hundred", "hatred", "naked", "wicked",
];

// German — sein/haben/werden + modals in Präteritum vs Präsens.
const DE_PAST: &[&str] = &[
    "war", "warst", "waren", "wart", "hatte", "hattest", "hatten", "hattet", "wurde", "wurdest",
    "wurden", "konnte", "konnten", "wollte", "wollten", "musste", "mussten", "sollte", "sollten",
    "durfte", "durften",
];
const DE_PRESENT: &[&str] = &[
    "ist", "bist", "sind", "seid", "bin", "hat", "habe", "haben", "hast", "habt", "wird", "werden",
    "wirst", "kann", "können", "will", "wollen", "muss", "müssen", "soll", "sollen",
];
const DE_IRREGULAR: &[&str] = &[
    "ging", "kam", "sah", "gab", "nahm", "fand", "stand", "saß", "blieb", "hielt", "sprach",
    "dachte", "wusste", "ließ", "trug", "fuhr", "rief", "schrieb", "zog", "schlug", "fiel", "hieß",
    "las", "trank", "sang", "fing",
];
const DE_TE_STOPLIST: &[&str] =
    &["heute", "mitte", "seite", "liste", "karte", "worte", "gute", "harte", "kette", "ernte"];

// French — être/avoir in imparfait / passé simple vs présent.
const FR_PAST: &[&str] = &[
    "était", "étaient", "étais", "étiez", "étions", "fut", "furent", "fus", "avait", "avaient",
    "avais", "aviez", "avions", "eut", "eurent", "eus",
];
const FR_PRESENT: &[&str] =
    &["est", "sont", "es", "suis", "êtes", "sommes", "a", "ont", "as", "ai", "avez", "avons"];
const FR_IRREGULAR: &[&str] = &[
    "fit", "firent", "dit", "dirent", "vint", "vinrent", "prit", "prirent", "vit", "virent",
    "alla", "allèrent", "sut", "surent", "put", "purent", "voulut", "mit", "mirent",
];

// Spanish — ser/estar/haber in pretérito / imperfecto vs presente.
const ES_PAST: &[&str] = &[
    "era", "eran", "eras", "fue", "fueron", "fui", "estaba", "estaban", "estabas", "estuvo",
    "estuvieron", "había", "habían", "habías",
];
const ES_PRESENT: &[&str] =
    &["es", "son", "eres", "soy", "somos", "está", "están", "estás", "estoy", "ha", "han", "has", "he", "hemos"];
const ES_IRREGULAR: &[&str] = &[
    "dijo", "dijeron", "hizo", "hicieron", "vino", "vinieron", "tuvo", "tuvieron", "pudo",
    "pudieron", "quiso", "puso", "pusieron", "vio", "vieron", "dio", "dieron",
];
// Common `-ía`/`-ían` words that are NOT imperfecto verbs.
const ES_IA_STOPLIST: &[&str] = &[
    "día", "días", "todavía", "maría", "policía", "alegría", "melodía", "compañía", "sabiduría",
    "cortesía", "energía", "vía", "guía", "fantasía", "poesía",
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_past_and_present() {
        use ProseLanguage::En;
        assert_eq!(classify("She walked home and it was cold", &En), Some(Tense::Past));
        assert_eq!(classify("She walks home and it is cold", &En), Some(Tense::Present));
        // No verb signal → unclassified.
        assert_eq!(classify("The wide grey sea", &En), None);
        // A lone -ed adjective doesn't pass the weight floor.
        assert_eq!(classify("The red door", &En), None);
    }

    #[test]
    fn classifies_german_french_spanish() {
        use ProseLanguage::{De, Es, Fr};
        // German: Präteritum sagte/war vs Präsens ist.
        assert_eq!(classify("Sie ging nach Hause und es war kalt", &De), Some(Tense::Past));
        assert_eq!(classify("Sie geht nach Hause und es ist kalt", &De), Some(Tense::Present));
        // French: imparfait marchait/était vs présent est.
        assert_eq!(classify("Elle marchait vers la maison et il faisait froid", &Fr), Some(Tense::Past));
        assert_eq!(classify("Elle est là et il fait froid maintenant", &Fr), Some(Tense::Present));
        // Spanish: pretérito/imperfecto caminaba/estaba vs presente está.
        assert_eq!(classify("Ella caminaba a casa y hacía frío", &Es), Some(Tense::Past));
        assert_eq!(classify("Ella está en casa y hace frío ahora", &Es), Some(Tense::Present));
    }

    #[test]
    fn german_scans_a_present_slip() {
        let text = "Sie ging zum Fenster. Der Regen hatte aufgehört. \
                    Sie ist jetzt an der Tür. Er kam leise herein. Sie saßen am Feuer.";
        match tense_scan(text, &ProseLanguage::De) {
            TenseScan::Scanned { dominant, slips } => {
                assert_eq!(dominant, Tense::Past);
                assert_eq!(slips.len(), 1);
                assert_eq!(slips[0].tense, Tense::Present);
            }
            TenseScan::Unsupported(_) => panic!("German should be supported"),
        }
    }

    #[test]
    fn a_present_slip_in_a_past_scene_is_flagged() {
        // The slip is copula-anchored (`is`) — the detector is copula/auxiliary
        // based, so it catches present narration reliably where a present-tense
        // passage lives (a bare present-simple verb with no copula can be missed).
        let text = "She walked to the window. The rain had stopped. \
                    She is at the door now, waiting. He came inside quietly. \
                    They sat by the fire and said nothing.";
        match tense_scan(text, &ProseLanguage::En) {
            TenseScan::Scanned { dominant, slips } => {
                assert_eq!(dominant, Tense::Past);
                assert_eq!(slips.len(), 1);
                assert_eq!(slips[0].tense, Tense::Present);
                assert!(slips[0].excerpt.contains("door"));
            }
            TenseScan::Unsupported(_) => panic!("English should be supported"),
        }
    }

    #[test]
    fn dialogue_tense_is_not_scored() {
        // The present-tense dialogue must not count as a narration slip.
        let text = "She walked in. \"I am here now,\" she said. He was waiting by the fire. \
                    They stood together and looked out.";
        match tense_scan(text, &ProseLanguage::En) {
            TenseScan::Scanned { dominant, slips } => {
                assert_eq!(dominant, Tense::Past);
                assert!(slips.is_empty(), "dialogue tense leaked: {slips:?}");
            }
            TenseScan::Unsupported(_) => panic!(),
        }
    }

    #[test]
    fn russian_is_not_analysed() {
        let text = "Она подумала. Он стоит у окна. Они сидели молча.";
        match tense_scan(text, &ProseLanguage::Ru) {
            TenseScan::Unsupported(reason) => assert!(reason.contains("aspect")),
            TenseScan::Scanned { .. } => panic!("Russian tense must not be scanned"),
        }
    }
}
