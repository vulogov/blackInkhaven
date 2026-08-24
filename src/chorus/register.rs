//! CHORUS-1 (CH-P6) — register & diction.
//!
//! A narrator holds a register — formal or plain, contracted or measured, plain
//! or archaic. When it *drifts* ("the prose gets casual in Act III") that's
//! worth seeing. This computes a small, language-keyed register bundle per
//! chapter and flags chapters whose register moved from the opening — the same
//! baseline-drift shape NARR-1's `prose drift` uses, kept CHORUS-local so
//! NARR-1's stored profile schema is untouched.
//!
//! Heuristic and word-list based (no parser). All five project languages carry
//! curated lists (EN/RU/FR + DE/ES as of 3.3.0); any other language degrades to
//! what its lists cover rather than guessing. The latinate-diction proxy stays
//! English-only (its suffixes are English). Advisory.

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::{ProseLanguage, resolve_prose_language};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::scenes::chapter_texts;

/// One chapter's (or span's) register.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Register {
    /// Contractions as a fraction of words (EN/FR; ~0 where the language has none).
    pub contraction_rate: f32,
    /// Archaic words as a fraction of words.
    pub archaism_density: f32,
    /// Formal-minus-informal balance in `[-1, +1]` (+1 fully formal).
    pub formality: f32,
    /// Latinate diction as a fraction of words — an English suffix proxy
    /// (`-tion`, `-ity`, …); `0` for other languages.
    pub latinate_density: f32,
    pub word_count: u32,
}

/// Compute the register of a span of prose.
pub(crate) fn register(text: &str, lang: &ProseLanguage) -> Register {
    let l = lists(lang);
    let latinate_lang = matches!(lang, ProseLanguage::En);

    let (mut contractions, mut archaisms, mut formal, mut informal, mut latinate, mut words) =
        (0u32, 0u32, 0u32, 0u32, 0u32, 0u32);
    for raw in text.split_whitespace() {
        let t = norm(raw);
        if t.is_empty() {
            continue;
        }
        words += 1;
        if l.contractions.contains(&t.as_str()) {
            contractions += 1;
        }
        if l.archaisms.contains(&t.as_str()) {
            archaisms += 1;
        }
        if l.formal.contains(&t.as_str()) {
            formal += 1;
        } else if l.informal.contains(&t.as_str()) {
            informal += 1;
        }
        if latinate_lang && is_latinate(&t) {
            latinate += 1;
        }
    }
    let wf = words.max(1) as f32;
    let formality = if formal + informal == 0 {
        0.0
    } else {
        (formal as f32 - informal as f32) / (formal + informal) as f32
    };
    Register {
        contraction_rate: contractions as f32 / wf,
        archaism_density: archaisms as f32 / wf,
        formality,
        latinate_density: latinate as f32 / wf,
        word_count: words,
    }
}

/// Keep internal apostrophes (so `don't` / `c'est` survive), normalise the curly
/// apostrophe, strip surrounding punctuation, lowercase (Unicode-aware).
fn norm(tok: &str) -> String {
    tok.replace('\u{2019}', "'")
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase()
}

fn is_latinate(t: &str) -> bool {
    const SUF: [&str; 12] =
        ["tion", "sion", "ment", "ance", "ence", "ity", "ous", "ive", "ate", "ize", "ise", "ology"];
    t.len() >= 6 && SUF.iter().any(|s| t.ends_with(s))
}

struct Lists {
    contractions: &'static [&'static str],
    archaisms: &'static [&'static str],
    formal: &'static [&'static str],
    informal: &'static [&'static str],
}

fn lists(lang: &ProseLanguage) -> Lists {
    match lang {
        ProseLanguage::Ru => Lists {
            contractions: &[],
            archaisms: RU_ARCHAIC,
            formal: RU_FORMAL,
            informal: RU_INFORMAL,
        },
        ProseLanguage::Fr => Lists {
            contractions: FR_CONTRACTIONS,
            archaisms: &[],
            formal: FR_FORMAL,
            informal: &[],
        },
        ProseLanguage::De => Lists {
            contractions: DE_CONTRACTIONS,
            archaisms: DE_ARCHAIC,
            formal: DE_FORMAL,
            informal: DE_INFORMAL,
        },
        // Spanish has only the neutral al/del fusions (not a register signal), so
        // colloquial elisions (pa', na') live in the informal list instead.
        ProseLanguage::Es => Lists {
            contractions: &[],
            archaisms: ES_ARCHAIC,
            formal: ES_FORMAL,
            informal: ES_INFORMAL,
        },
        _ => Lists {
            contractions: EN_CONTRACTIONS,
            archaisms: EN_ARCHAIC,
            formal: EN_FORMAL,
            informal: EN_INFORMAL,
        },
    }
}

const EN_CONTRACTIONS: &[&str] = &[
    "don't", "can't", "won't", "i'm", "it's", "that's", "he's", "she's", "we're", "they're",
    "i've", "you're", "didn't", "wouldn't", "couldn't", "isn't", "aren't", "wasn't", "weren't",
    "doesn't", "i'll", "we'll", "you'll", "he'll", "she'll", "they'll", "let's", "there's",
    "what's", "who's", "i'd", "you'd", "he'd", "she'd", "we'd", "they'd", "shouldn't", "hadn't",
    "hasn't", "haven't", "mustn't", "wouldn't",
];
const EN_ARCHAIC: &[&str] = &[
    "thee", "thou", "thy", "thine", "hath", "doth", "dost", "art", "ere", "whilst", "betwixt",
    "oft", "tis", "twas", "hither", "thither", "whence", "wherefore", "forsooth", "nay", "yea",
    "verily", "mayhap", "perchance", "methinks", "prithee", "aught", "naught",
];
const EN_FORMAL: &[&str] = &[
    "however", "therefore", "moreover", "nevertheless", "furthermore", "thus", "hence",
    "regarding", "concerning", "subsequently", "consequently", "notwithstanding", "henceforth",
    "herein", "thereby", "wherein", "whereas", "albeit", "whereby",
];
const EN_INFORMAL: &[&str] = &[
    "gonna", "wanna", "gotta", "yeah", "yep", "nope", "ok", "okay", "stuff", "kinda", "sorta",
    "dunno", "ain't", "guy", "guys", "kids", "folks", "cuz", "gotcha",
];

const RU_ARCHAIC: &[&str] = &[
    "сей", "сия", "сие", "оный", "дабы", "ежели", "поелику", "токмо", "паче", "зело", "вельми",
    "коль", "дондеже", "поныне", "поприще", "чело", "выя", "длань",
];
const RU_FORMAL: &[&str] = &[
    "однако", "следовательно", "поэтому", "итак", "впрочем", "ибо", "посему", "вследствие",
    "таким", "равно", "ввиду",
];
const RU_INFORMAL: &[&str] = &["ну", "ага", "типа", "короче", "блин", "чё", "щас", "ладно", "мол"];

const FR_CONTRACTIONS: &[&str] = &[
    "c'est", "j'ai", "n'est", "qu'il", "d'un", "l'on", "s'il", "j'en", "m'a", "t'a", "qu'elle",
    "d'une", "l'a", "n'a",
];
const FR_FORMAL: &[&str] =
    &["néanmoins", "toutefois", "cependant", "ainsi", "donc", "or", "partant", "nonobstant"];

// German (3.3.0 M1) — colloquial elisions, literary archaisms, formal discourse
// connectives, and modal-particle informality. Single-word compounds are not
// decomposed (the token matcher is whole-word).
const DE_CONTRACTIONS: &[&str] = &[
    "gibt's", "geht's", "hab's", "wie's", "wenn's", "so'n", "'ne", "'nen", "'nem",
    "auf'm", "aufm", "unterm", "überm", "hinterm", "isses", "haste", "biste", "kannste",
    "willste", "machste",
];
const DE_ARCHAIC: &[&str] = &[
    "ward", "weiland", "alldieweil", "sintemal", "fürwahr", "gemach", "alsbald", "hernach",
    "vormals", "dieweil", "obzwar", "dero", "nimmer", "traun", "maßen",
];
const DE_FORMAL: &[&str] = &[
    "jedoch", "folglich", "daher", "somit", "dennoch", "ferner", "überdies", "mithin",
    "demzufolge", "infolgedessen", "hinsichtlich", "bezüglich", "nichtsdestotrotz",
    "nichtsdestoweniger", "gleichwohl", "indes", "indessen", "demgemäß", "desgleichen",
];
const DE_INFORMAL: &[&str] = &[
    "halt", "eh", "nö", "krass", "voll", "irgendwie", "quatsch", "kumpel", "naja", "tja",
    "hä", "mega", "total", "eben", "ne",
];

// Spanish (3.3.0 M1) — literary archaisms, formal connectives (single-token, since
// the matcher is whole-word — "no obstante" contributes via "obstante"), and
// pan-Hispanic colloquialisms incl. the elided forms (pa'/na'/to').
const ES_ARCHAIC: &[&str] = &[
    "otrora", "asaz", "cuan", "doquier", "doquiera", "mesmo", "agora", "ansí",
    "fuere", "hubiere", "aqueste", "aquese", "vuesa", "acullá", "maguer",
];
const ES_FORMAL: &[&str] = &[
    "asimismo", "además", "empero", "ergo", "consecuentemente", "ulteriormente",
    "consiguientemente", "igualmente", "ciertamente", "efectivamente", "seguidamente",
    "obstante", "consiguiente", "subsiguientemente",
];
const ES_INFORMAL: &[&str] = &[
    "pa", "pa'", "na'", "to'", "tío", "tía", "tipo", "chaval", "guay", "vale", "mola",
    "porfa", "nomás", "pos", "oye", "bueno",
];

#[allow(clippy::too_many_arguments)]
fn push_drift(
    out: &mut Vec<RegisterDrift>,
    chapter_ord: u32,
    metric: &'static str,
    value: f32,
    baseline: f32,
    threshold: f32,
) {
    let delta = value - baseline;
    if delta.abs() >= threshold {
        out.push(RegisterDrift { chapter_ord, metric, baseline, value, delta });
    }
}

/// A chapter's register.
pub(crate) struct ChapterRegister {
    pub chapter_ord: u32,
    pub register: Register,
}

/// One register metric that moved from the baseline chapter beyond the threshold.
pub(crate) struct RegisterDrift {
    pub chapter_ord: u32,
    pub metric: &'static str,
    pub baseline: f32,
    pub value: f32,
    pub delta: f32,
}

pub(crate) struct RegisterReport {
    pub chapters: Vec<ChapterRegister>,
    pub drifts: Vec<RegisterDrift>,
}

/// Compute each chapter's register (chapters under 100 words are skipped as too
/// small to characterise) and flag those that drift from the first chapter.
pub(crate) fn scan_register(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
) -> RegisterReport {
    let (lang, _) = resolve_prose_language(None, &cfg.language);
    let thr = cfg.chorus.register_drift_threshold;

    let chapters: Vec<ChapterRegister> = chapter_texts(layout, h, book)
        .into_iter()
        .map(|(ord, text)| ChapterRegister { chapter_ord: ord, register: register(&text, &lang) })
        .filter(|c| c.register.word_count >= 100)
        .collect();

    let mut drifts = Vec::new();
    if let Some(base) = chapters.first() {
        let b = base.register;
        for c in chapters.iter().skip(1) {
            let r = c.register;
            push_drift(&mut drifts, c.chapter_ord, "contraction_rate", r.contraction_rate, b.contraction_rate, thr);
            push_drift(&mut drifts, c.chapter_ord, "archaism_density", r.archaism_density, b.archaism_density, thr);
            push_drift(&mut drifts, c.chapter_ord, "formality", r.formality, b.formality, thr);
            push_drift(&mut drifts, c.chapter_ord, "latinate_density", r.latinate_density, b.latinate_density, thr);
        }
    }
    RegisterReport { chapters, drifts }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_metrics_on_fixtures() {
        let r = register("I don't know but it's fine", &ProseLanguage::En);
        // don't + it's out of 6 words.
        assert!((r.contraction_rate - 2.0 / 6.0).abs() < 1e-4);

        let a = register("Thou hath spoken", &ProseLanguage::En);
        assert!((a.archaism_density - 2.0 / 3.0).abs() < 1e-4);

        let f = register("However therefore gonna", &ProseLanguage::En);
        // formal 2, informal 1 → (2-1)/3.
        assert!((f.formality - 1.0 / 3.0).abs() < 1e-4);

        let lat = register("The information and organization", &ProseLanguage::En);
        assert!((lat.latinate_density - 2.0 / 4.0).abs() < 1e-4);
    }

    #[test]
    fn register_works_in_russian_archaism_and_formality() {
        let r = register("Сей однако дабы", &ProseLanguage::Ru);
        assert!(r.archaism_density > 0.0, "сей/дабы archaic");
        assert!(r.formality > 0.0, "однако formal");
        assert_eq!(r.latinate_density, 0.0, "latinate is English-only");
    }

    #[test]
    fn register_works_in_german() {
        // ward archaic · jedoch+folglich formal · gibt's contraction · halt informal.
        // Before M1 these fell through to the English lists and matched nothing.
        let r = register("Ward jedoch folglich gibt's halt", &ProseLanguage::De);
        assert!(r.archaism_density > 0.0, "ward archaic");
        assert!(r.formality > 0.0, "jedoch+folglich formal outweighs halt informal");
        assert!(r.contraction_rate > 0.0, "gibt's contraction");
        assert_eq!(r.latinate_density, 0.0, "latinate is English-only");
    }

    #[test]
    fn register_works_in_spanish() {
        // otrora archaic · asimismo+además formal · tío informal · no contractions.
        let r = register("Otrora asimismo además tío", &ProseLanguage::Es);
        assert!(r.archaism_density > 0.0, "otrora archaic");
        assert!(r.formality > 0.0, "asimismo+además formal outweighs tío informal");
        assert_eq!(r.contraction_rate, 0.0, "Spanish has no register contractions");
        assert_eq!(r.latinate_density, 0.0, "latinate is English-only");
    }

    #[test]
    fn drift_threshold_flags_a_shift() {
        // Baseline plain; a later chapter heavy with contractions drifts.
        let base = register("The wind moved across the wide grey sea at dawn", &ProseLanguage::En);
        let later = register("I don't and can't and won't and it's and that's", &ProseLanguage::En);
        let mut drifts = Vec::new();
        push_drift(&mut drifts, 5, "contraction_rate", later.contraction_rate, base.contraction_rate, 0.08);
        assert_eq!(drifts.len(), 1);
        assert_eq!(drifts[0].chapter_ord, 5);
        assert!(drifts[0].delta > 0.0);
    }
}
