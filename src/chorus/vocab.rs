//! CHORUS-1 — language-keyed style word-lists. CH-P4 uses the **interiority
//! verbs**: verbs that mark accessing a character's inner life (`thought`,
//! `wondered`, `felt`, …). They're derived in spirit from NARR-1's per-language
//! interiority *markers* (`src/prose/lexicon.rs`), but as bare verbs so a
//! *named* subject can be resolved — a named character who is the subject of an
//! interiority verb is the head-hop signal (the markers themselves are
//! pronoun-based, which can't be attributed to a name without antecedent
//! resolution the tree doesn't do).
//!
//! Heuristic and non-exhaustive by nature — a lexicon, not a parser. EN/RU are
//! the solid baseline; DE/FR/ES cover the common forms.

use crate::prose::ProseLanguage;

/// The interiority verbs for a language (lowercased). Any other language → the
/// English set (a best-effort default; head-hop stays advisory).
pub(crate) fn interiority_verbs(lang: &ProseLanguage) -> &'static [&'static str] {
    match lang {
        ProseLanguage::Ru => RU,
        ProseLanguage::De => DE,
        ProseLanguage::Fr => FR,
        ProseLanguage::Es => ES,
        _ => EN,
    }
}

const EN: &[&str] = &[
    "thought", "wondered", "realised", "realized", "knew", "felt", "remembered",
    "decided", "noticed", "understood", "believed", "sensed", "imagined",
    "recalled", "supposed", "reflected", "mused", "wished",
];

// Russian verbs carry gender/number; include the common past-tense forms
// (impersonal `казалось`/`показалось` are excluded — they take no named subject).
const RU: &[&str] = &[
    "подумал", "подумала", "подумали", "почувствовал", "почувствовала",
    "почувствовали", "вспомнил", "вспомнила", "вспомнили", "знал", "знала",
    "знали", "понял", "поняла", "поняли", "решил", "решила", "решили",
    "заметил", "заметила", "заметили", "осознал", "осознала", "задумался",
    "задумалась",
];

const DE: &[&str] = &[
    "dachte", "überlegte", "wusste", "fühlte", "erinnerte", "bemerkte",
    "verstand", "glaubte", "spürte", "erkannte", "ahnte",
];

const FR: &[&str] = &[
    "pensa", "pensait", "savait", "sentait", "sentit", "remarqua", "comprit",
    "crut", "songea", "réalisa",
];

const ES: &[&str] = &[
    "pensó", "pensaba", "sabía", "sintió", "recordó", "notó", "comprendió",
    "creyó", "supo", "imaginó",
];
