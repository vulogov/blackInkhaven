//! DIALOG-1 — embedded dialogue-tag verb lists, five languages. Two classes per
//! language: **neutral** tags (invisible — said/asked) and **said-bookisms**
//! (non-neutral — whispered/growled), the distinction the density metric counts.
//!
//! Lists are stored lowercase and matched case-insensitively via a linear scan
//! (≈30–45 entries each — a HashSet/binary-search would be premature). Forms are
//! the dominant narrative tense per language (EN past; RU perfective past with
//! gender pairs; DE Präteritum; FR passé simple; ES pretérito). EN/`Other` add a
//! 6-char-stem fallback so non-past inflections (whisper/whispering) still match.
//!
//! Genre-specific verbs (`transmitted`, `intoned`, …) are folded in at runtime
//! from `dialogue.extra_neutral_verbs` / `extra_said_bookisms` (D-P10), not here.

use super::TagVerbClass;
use crate::prose::ProseLanguage;

/// A language's two tag-verb classes plus whether the 6-char-stem fallback
/// applies (EN/`Other` only — the other languages list inflected forms).
pub(crate) struct DialogueLexicon {
    pub neutral: &'static [&'static str],
    pub said_bookism: &'static [&'static str],
    pub stem_fallback: bool,
}

// ── English ──────────────────────────────────────────────────────────
static EN: DialogueLexicon = DialogueLexicon {
    neutral: &["added", "answered", "asked", "began", "continued", "replied", "said"],
    said_bookism: &[
        "admitted", "barked", "bellowed", "breathed", "chuckled", "cried", "croaked",
        "declared", "drawled", "ejaculated", "exclaimed", "gasped", "giggled", "growled",
        "grunted", "hissed", "howled", "huffed", "intoned", "laughed", "moaned",
        "murmured", "muttered", "purred", "rasped", "retorted", "roared", "scoffed",
        "screamed", "shouted", "sighed", "simpered", "snapped", "sneered", "snorted",
        "sobbed", "spat", "sputtered", "squeaked", "taunted", "thundered", "wailed",
        "wheezed", "whimpered", "whispered", "yelled",
    ],
    stem_fallback: true,
};

// ── Russian (perfective past; gender pairs) ──────────────────────────
static RU: DialogueLexicon = DialogueLexicon {
    neutral: &[
        "сказал", "сказала", "спросил", "спросила", "ответил", "ответила",
        "продолжил", "продолжила", "добавил", "добавила", "произнёс", "произнесла",
    ],
    said_bookism: &[
        "воскликнул", "воскликнула", "прошептал", "прошептала", "прошипел", "прошипела",
        "пробормотал", "пробормотала", "пробурчал", "пробурчала", "прорычал", "прорычала",
        "простонал", "простонала", "выдохнул", "выдохнула", "усмехнулся", "усмехнулась",
        "хихикнул", "хихикнула", "взвыл", "взвыла", "завопил", "завопила",
        "проворчал", "проворчала", "огрызнулся", "огрызнулась", "выкрикнул", "выкрикнула",
        "буркнул", "буркнула",
    ],
    stem_fallback: false,
};

// ── German (Präteritum) ──────────────────────────────────────────────
static DE: DialogueLexicon = DialogueLexicon {
    neutral: &["sagte", "fragte", "antwortete", "erwiderte", "begann", "meinte", "rief"],
    said_bookism: &[
        "bellte", "brüllte", "fauchte", "flüsterte", "grollte", "gurgelte", "hustete",
        "jammerte", "jaulte", "kicherte", "knurrte", "kreischte", "lachte", "murmelte",
        "röchelte", "schimpfte", "schrie", "schnaufte", "schnauzte", "seufzte",
        "stöhnte", "stotterte", "wimmerte", "winselte", "wisperte", "zischte", "ächzte",
        "brummte", "gluckste",
    ],
    stem_fallback: false,
};

// ── French (passé simple; inverted forms included as neutral) ────────
static FR: DialogueLexicon = DialogueLexicon {
    neutral: &[
        "dit", "demanda", "répondit", "continua", "ajouta", "reprit", "commença",
        "dit-il", "dit-elle", "répondit-il", "répondit-elle",
    ],
    said_bookism: &[
        "bégaya", "chuchota", "cria", "gémit", "grommela", "gronda", "grogna", "haleta",
        "hurla", "murmura", "pleura", "pouffa", "rit", "ricana", "rugit", "sanglota",
        "siffla", "soupira", "souffla", "bredouilla", "balbutia", "vociféra", "cracha",
        "glapit", "chevrota",
    ],
    stem_fallback: false,
};

// ── Spanish (pretérito) ──────────────────────────────────────────────
static ES: DialogueLexicon = DialogueLexicon {
    neutral: &["dijo", "preguntó", "respondió", "contestó", "continuó", "añadió", "comenzó"],
    said_bookism: &[
        "balbuceó", "bramó", "chilló", "exclamó", "farfulló", "gritó", "gruñó", "jadeó",
        "lloró", "masculló", "murmuró", "refunfuñó", "rio", "rugió", "sollozó", "suspiró",
        "susurró", "tartamudeó", "tembló", "vociferó", "siseó", "escupió", "bufó",
        "gimió", "aulló",
    ],
    stem_fallback: false,
};

/// Like [`lexicon_for`], but with the config's genre verb extras folded in
/// (`dialogue.extra_neutral_verbs` / `extra_said_bookisms`). Leaks the folded
/// lists to `'static` — the same pattern `prose`'s lexicon uses; called once
/// per refresh, so the leak is bounded by the (tiny) config size. Falls back to
/// the static lexicon when there are no extras.
pub(crate) fn lexicon_for_with(
    lang: &ProseLanguage,
    extra_neutral: &[String],
    extra_bookism: &[String],
) -> &'static DialogueLexicon {
    let base = lexicon_for(lang);
    if extra_neutral.is_empty() && extra_bookism.is_empty() {
        return base;
    }
    let mut neutral: Vec<&'static str> = base.neutral.to_vec();
    for v in extra_neutral {
        neutral.push(Box::leak(v.to_lowercase().into_boxed_str()));
    }
    let mut bookism: Vec<&'static str> = base.said_bookism.to_vec();
    for v in extra_bookism {
        bookism.push(Box::leak(v.to_lowercase().into_boxed_str()));
    }
    Box::leak(Box::new(DialogueLexicon {
        neutral: Box::leak(neutral.into_boxed_slice()),
        said_bookism: Box::leak(bookism.into_boxed_slice()),
        stem_fallback: base.stem_fallback,
    }))
}

/// The verb lexicon for a language. `Other` falls back to the EN lists (per
/// RFC §17 language fallback).
pub(crate) fn lexicon_for(lang: &ProseLanguage) -> &'static DialogueLexicon {
    match lang {
        ProseLanguage::En => &EN,
        ProseLanguage::Ru => &RU,
        ProseLanguage::De => &DE,
        ProseLanguage::Fr => &FR,
        ProseLanguage::Es => &ES,
        ProseLanguage::Other(_) => &EN,
    }
}

/// Classify a tag verb as neutral / said-bookism / unrecognised. Case-
/// insensitive; for EN/`Other`, an exact miss falls back to a 6-char stem match
/// against the said-bookism list (so `whispering` matches `whispered`).
pub(crate) fn classify_tag_verb(verb: &str, lex: &DialogueLexicon) -> Option<TagVerbClass> {
    let q = verb.trim().to_lowercase();
    if q.is_empty() {
        return None;
    }
    if lex.neutral.iter().any(|&v| v == q) {
        return Some(TagVerbClass::Neutral);
    }
    if lex.said_bookism.iter().any(|&v| v == q) {
        return Some(TagVerbClass::SaidBookism);
    }
    if lex.stem_fallback && stem_match(&q, lex.said_bookism) {
        return Some(TagVerbClass::SaidBookism);
    }
    None
}

/// 6-char (char, not byte) prefix match against a list — the EN inflection
/// fallback. Requires a ≥5-char shared prefix to avoid spurious hits.
fn stem_match(q: &str, list: &[&str]) -> bool {
    let qp: String = q.chars().take(6).collect();
    if qp.chars().count() < 5 {
        return false;
    }
    list.iter().any(|&v| {
        let vp: String = v.chars().take(6).collect();
        vp == qp
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> [(&'static str, &'static DialogueLexicon); 5] {
        [
            ("en", &EN),
            ("ru", &RU),
            ("de", &DE),
            ("fr", &FR),
            ("es", &ES),
        ]
    }

    #[test]
    fn lists_are_lowercase_and_disjoint() {
        for (code, lex) in all() {
            for &v in lex.neutral.iter().chain(lex.said_bookism.iter()) {
                assert_eq!(v, v.to_lowercase(), "{code}: `{v}` must be lowercase");
            }
            for &n in lex.neutral {
                assert!(
                    !lex.said_bookism.contains(&n),
                    "{code}: `{n}` in both neutral and said-bookism"
                );
            }
        }
    }

    #[test]
    fn classifies_each_language() {
        // Neutral vs said-bookism per language.
        assert_eq!(classify_tag_verb("said", &EN), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("WHISPERED", &EN), Some(TagVerbClass::SaidBookism));
        assert_eq!(classify_tag_verb("сказал", &RU), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("прошептала", &RU), Some(TagVerbClass::SaidBookism));
        assert_eq!(classify_tag_verb("sagte", &DE), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("flüsterte", &DE), Some(TagVerbClass::SaidBookism));
        assert_eq!(classify_tag_verb("dit", &FR), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("dit-il", &FR), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("murmura", &FR), Some(TagVerbClass::SaidBookism));
        assert_eq!(classify_tag_verb("dijo", &ES), Some(TagVerbClass::Neutral));
        assert_eq!(classify_tag_verb("susurró", &ES), Some(TagVerbClass::SaidBookism));
        // Unrecognised verb.
        assert_eq!(classify_tag_verb("walked", &EN), None);
    }

    #[test]
    fn en_stem_fallback_catches_inflections() {
        // Not in the list verbatim, but shares a stem with `whispered`.
        assert_eq!(classify_tag_verb("whispering", &EN), Some(TagVerbClass::SaidBookism));
        assert_eq!(classify_tag_verb("whispers", &EN), Some(TagVerbClass::SaidBookism));
        // Non-EN languages do not stem-fallback.
        assert_eq!(classify_tag_verb("flüster", &DE), None);
    }

    #[test]
    fn fallback_language_uses_en_lists() {
        let lex = lexicon_for(&ProseLanguage::Other("pl".into()));
        assert_eq!(classify_tag_verb("said", lex), Some(TagVerbClass::Neutral));
    }
}
