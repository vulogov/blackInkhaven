//! NARR-1 — per-language passive-voice detection. Each language needs its own
//! heuristic; no language's passive can be found with another's pattern.
//! Accuracy is trend-grade (~75–90%), which is all the drift metric needs.
//! `regex` is already in the tree; patterns compile once via `OnceLock`.

use std::sync::OnceLock;

use regex::Regex;

use super::{CompiledLexicon, ProseLanguage, tokenize};

fn re(cell: &'static OnceLock<Regex>, pat: &str) -> &'static Regex {
    cell.get_or_init(|| Regex::new(pat).expect("valid prose passive regex"))
}

/// English: `(be-aux) + V-ed/en`, plus `(be-aux) + <irregular participle>` from
/// the lexicon's EN list (`held`, `kept`, `thought`, …).
fn detect_passive_en(sentence: &str, lx: &CompiledLexicon) -> bool {
    static REG: OnceLock<Regex> = OnceLock::new();
    let regular = re(
        &REG,
        r"(?i)\b(was|were|is|are|been|being|be)\s+\w+(ed|en)\b",
    );
    if regular.is_match(sentence) {
        return true;
    }
    // be-aux immediately followed by an irregular past participle.
    let toks = tokenize(sentence);
    toks.windows(2).any(|w| {
        matches!(
            w[0].as_str(),
            "was" | "were" | "is" | "are" | "been" | "being" | "be"
        ) && lx.is_passive_exception(&w[1])
    })
}

/// Russian: reflexive `-ся`/`-сь` construction, excluding the lexicon's
/// reflexive-only (non-passive) verbs (`казалось`, `смеялся`, …).
fn detect_passive_ru(sentence: &str, lx: &CompiledLexicon) -> bool {
    tokenize(sentence).iter().any(|w| {
        (w.ends_with("ся") || w.ends_with("сь")) && !lx.is_passive_exception(w)
    })
}

/// German: *Vorgangspassiv* (`werden` + participle) or *Zustandspassiv* (`sein`
/// + participle), minus `sein` + predicative-adjective collocations.
fn detect_passive_de(sentence: &str) -> bool {
    static VORGANG: OnceLock<Regex> = OnceLock::new();
    static ZUSTAND: OnceLock<Regex> = OnceLock::new();
    static EXCL: OnceLock<Regex> = OnceLock::new();
    let vorgang = re(
        &VORGANG,
        r"(?i)\b(wird|wurde|werden|wurden|worden|werde|werdet)\s+\w{3,}(t|en)\b",
    );
    if vorgang.is_match(sentence) {
        return true;
    }
    let zustand = re(
        &ZUSTAND,
        r"(?i)\b(ist|war|sind|waren|sei|wäre)\s+\w{3,}(t|en)\b",
    );
    let excl = re(
        &EXCL,
        r"(?i)\b(ist|war|sind|waren|sei|wäre)\s+(klar|bekannt|bereit|fertig|möglich|nötig)\b",
    );
    zustand.is_match(sentence) && !excl.is_match(sentence)
}

/// French: `être` (incl. compound `a été` / `avait été`) + past participle.
fn detect_passive_fr(sentence: &str) -> bool {
    static REG: OnceLock<Regex> = OnceLock::new();
    // NOTE: the RFC's sample regex omits the common `-it` family (`construit`,
    // `détruit`) and the `-eint`/`-aint`/`-oint` families; added here.
    let r = re(
        &REG,
        r"(?i)\b(est|était|fut|sera|serait|a été|avait été|aura été)\s+\w+(é|ée|és|ées|i|ie|is|ies|it|ite|its|ites|u|ue|us|ues|ert|erte|eint|eints|aint|aints|oint|oints)\b",
    );
    r.is_match(sentence)
}

/// Spanish: *pasiva perifrástica* — `ser` (incl. `ha sido` / `había sido`) +
/// past participle. The *pasiva refleja* (`se` + verb) is intentionally not
/// counted (indistinguishable from a true reflexive without a parser).
fn detect_passive_es(sentence: &str) -> bool {
    static REG: OnceLock<Regex> = OnceLock::new();
    let r = re(
        &REG,
        r"(?i)\b(es|era|fue|será|sería|ha sido|había sido)\s+\w+(ado|ada|ados|adas|ido|ida|idos|idas)\b",
    );
    r.is_match(sentence)
}

/// Whether `sentence` contains a passive construction in `lang`.
pub(crate) fn detect_passive(sentence: &str, lang: &ProseLanguage, lx: &CompiledLexicon) -> bool {
    match lang {
        ProseLanguage::En => detect_passive_en(sentence, lx),
        ProseLanguage::Ru => detect_passive_ru(sentence, lx),
        ProseLanguage::De => detect_passive_de(sentence),
        ProseLanguage::Fr => detect_passive_fr(sentence),
        ProseLanguage::Es => detect_passive_es(sentence),
        ProseLanguage::Other(_) => false,
    }
}

/// Active/passive ratio: passive sentences ÷ active (non-passive) sentences.
/// `None` for unsupported languages. `0.0` for empty input.
pub(crate) fn passive_ratio(
    sentences: &[String],
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
) -> Option<f32> {
    if !lang.is_supported() {
        return None;
    }
    if sentences.is_empty() {
        return Some(0.0);
    }
    let passive = sentences
        .iter()
        .filter(|s| detect_passive(s, lang, lx))
        .count();
    let active = sentences.len() - passive;
    Some(passive as f32 / active.max(1) as f32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::CompiledLexicon;
    use crate::prose::ProseLanguage::*;

    fn lx(l: &crate::prose::ProseLanguage) -> CompiledLexicon {
        CompiledLexicon::for_language(l)
    }

    #[test]
    fn passive_per_language() {
        assert!(detect_passive("The bridge was built last year.", &En, &lx(&En)));
        assert!(detect_passive("The door was opened slowly.", &En, &lx(&En)));
        assert!(detect_passive("Дверь была закрыта, и свет погасился.", &Ru, &lx(&Ru)));
        assert!(detect_passive("Die Brücke wurde gebaut.", &De, &lx(&De)));
        assert!(detect_passive("Le pont a été construit.", &Fr, &lx(&Fr)));
        assert!(detect_passive("El puente fue construido.", &Es, &lx(&Es)));
    }

    #[test]
    fn active_and_exclusions_not_passive() {
        assert!(!detect_passive("She built the bridge.", &En, &lx(&En)));
        // RU reflexive-only verb is excluded.
        assert!(!detect_passive("Ему казалось странно.", &Ru, &lx(&Ru)));
        // DE sein + predicative adjective is not Zustandspassiv.
        assert!(!detect_passive("Es war klar.", &De, &lx(&De)));
        assert!(!detect_passive("Elle marchait vite.", &Fr, &lx(&Fr)));
    }

    #[test]
    fn ratio_none_for_unsupported() {
        let other = Other("it".into());
        assert_eq!(passive_ratio(&["qualcosa".into()], &other, &lx(&other)), None);
    }

    #[test]
    fn ratio_counts() {
        let sents: Vec<String> = vec![
            "The wall was painted.".into(), // passive
            "She walked home.".into(),      // active
            "He read the book.".into(),     // active
        ];
        // 1 passive / 2 active = 0.5
        assert!((passive_ratio(&sents, &En, &lx(&En)).unwrap() - 0.5).abs() < 1e-6);
    }
}
