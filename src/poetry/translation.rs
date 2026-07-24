//! POEM-7 (PO-P8) — the translation trilemma.
//!
//! Translating verse forces a choice among three dimensions no translation can
//! fully keep: **Form** (metre and rhyme), **Meaning** (the sense), and **Sound**
//! (the texture — alliteration, assonance). This module measures the two
//! dimensions that are deterministic — Form (by scanning both texts) and Sound
//! (by comparing their alliterative density) — and builds the prompt for the
//! Meaning axis, which needs the LLM. It makes the translator's trade-off
//! *visible*; it never judges or rewrites.

use crate::poetry::form::PoemForm;
use crate::poetry::metre;
use crate::poetry::rhyme::{self, RhymeQuality};
use crate::poetry::syllabify;
use crate::prose::ProseLanguage;

/// POEM-TUI (PO-P15) — a `para:verse-translation` paragraph holds the source
/// stanza and its translation in one body, separated by a delimiter line: either
/// the thematic `⇄`, or a rule of three-or-more dashes (`---`) that authors can
/// actually type. Source is above the *first* delimiter, translation below.
///
/// Returns `None` when no delimiter is present (the paragraph isn't yet a paired
/// translation) — the caller shows a hint rather than guessing. Blank lines are
/// preserved within each half; only the halves are trimmed of leading/trailing
/// blank lines.
pub fn split_source_translation(body: &str) -> Option<(String, String)> {
    let is_delim = |line: &str| {
        let t = line.trim();
        t == "⇄" || (t.len() >= 3 && t.chars().all(|c| c == '-'))
    };
    let mut lines = body.lines();
    let mut source: Vec<&str> = Vec::new();
    let mut found = false;
    for line in lines.by_ref() {
        if is_delim(line) {
            found = true;
            break;
        }
        source.push(line);
    }
    if !found {
        return None;
    }
    let translation: Vec<&str> = lines.collect();
    let trim_blank = |v: Vec<&str>| {
        let s = v.join("\n");
        s.trim_matches('\n').to_string()
    };
    Some((trim_blank(source), trim_blank(translation)))
}

/// The measurable half of the trilemma: how much Form and Sound survived.
#[derive(Debug, Clone, PartialEq)]
pub struct Trilemma {
    /// 0..1 — metre and rhyme preservation.
    pub form_score: f64,
    /// 0..1 — alliterative-texture similarity.
    pub sound_score: f64,
    pub metre_note: String,
    pub rhyme_note: String,
    pub sound_note: String,
}

/// Compare a source stanza and its translation on the Form and Sound axes.
pub fn trilemma(
    source: &str,
    src_lang: &ProseLanguage,
    translation: &str,
    trans_lang: &ProseLanguage,
    form: &PoemForm,
) -> Trilemma {
    let (ms, metre_note) = metre_similarity(source, src_lang, translation, trans_lang);
    let (rs, rhyme_note) = rhyme_similarity(source, src_lang, translation, trans_lang, form);
    let (sound_score, sound_note) = sound_similarity(source, src_lang, translation, trans_lang);
    Trilemma { form_score: (ms + rs) / 2.0, sound_score, metre_note, rhyme_note, sound_note }
}

/// Build the LLM prompt for the Meaning axis and the overall trade-off summary.
/// Non-prescriptive: it asks which dimension was most preserved and which most
/// sacrificed, making the translator's choice visible. Consumed by the trilemma
/// slow track (the in-editor engage); exercised now by the tests.
#[allow(dead_code)]
pub fn build_trilemma_prompt(source: &str, translation: &str, form: &PoemForm) -> String {
    let declared = if form.form.is_empty() {
        String::new()
    } else {
        format!(" (declared form: {})", form.form)
    };
    format!(
        "Here is a stanza and its verse translation{declared}. Considering the three dimensions a \
         verse translation trades off — Form (metre and rhyme), Meaning (the sense), and Sound \
         (alliteration, assonance) — say which dimension has been most preserved and which most \
         sacrificed, and note any specific meaning shift (a lost plural, a changed image). Make the \
         trade-off visible; do not judge the choice or rewrite.\n\nSOURCE:\n{source}\n\nTRANSLATION:\n{translation}"
    )
}

fn lines_of(text: &str) -> Vec<&str> {
    text.lines().map(str::trim).filter(|l| !l.is_empty()).collect()
}

fn end_word(line: &str) -> String {
    line.split_whitespace()
        .next_back()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic() && c != '\u{301}').to_string())
        .unwrap_or_default()
}

fn metre_similarity(
    source: &str,
    src_lang: &ProseLanguage,
    translation: &str,
    trans_lang: &ProseLanguage,
) -> (f64, String) {
    let (s, t) = (lines_of(source), lines_of(translation));
    let n = s.len().min(t.len());
    if n == 0 {
        return (1.0, "no lines to compare".into());
    }
    let mut matches = 0;
    for i in 0..n {
        let sm = metre::detect(&metre::line_to_beats(s[i], src_lang.clone()));
        let tm = metre::detect(&metre::line_to_beats(t[i], trans_lang.clone()));
        match (sm, tm) {
            (Some(a), Some(b)) if a.foot == b.foot => matches += 1,
            (None, None) => matches += 1,
            _ => {}
        }
    }
    (matches as f64 / n as f64, format!("{matches}/{n} lines keep the foot"))
}

fn rhyme_similarity(
    source: &str,
    src_lang: &ProseLanguage,
    translation: &str,
    trans_lang: &ProseLanguage,
    form: &PoemForm,
) -> (f64, String) {
    let scheme = form.rhyme_scheme.trim();
    if scheme.is_empty() || scheme == "-" {
        return (1.0, "no rhyme scheme declared".into());
    }
    let labels: Vec<char> = scheme.chars().filter(|c| !c.is_whitespace()).collect();
    let (s_ends, t_ends): (Vec<String>, Vec<String>) =
        (lines_of(source).iter().map(|l| end_word(l)).collect(),
         lines_of(translation).iter().map(|l| end_word(l)).collect());

    use std::collections::BTreeMap;
    let mut groups: BTreeMap<char, Vec<usize>> = BTreeMap::new();
    for (i, &lab) in labels.iter().enumerate() {
        if lab != '-' {
            groups.entry(lab).or_default().push(i);
        }
    }
    let rhymes = |ends: &[String], lang: &ProseLanguage, a: usize, b: usize| -> bool {
        match (ends.get(a), ends.get(b)) {
            (Some(x), Some(y)) if !x.is_empty() && !y.is_empty() => matches!(
                rhyme::analyse_rhyme(x, y, lang.clone()).quality,
                RhymeQuality::Perfect | RhymeQuality::Near
            ),
            _ => false,
        }
    };

    let (mut src_rhyming, mut both) = (0usize, 0usize);
    for idxs in groups.values() {
        for w in idxs.windows(2) {
            let (a, b) = (w[0], w[1]);
            if rhymes(&s_ends, src_lang, a, b) {
                src_rhyming += 1;
                if rhymes(&t_ends, trans_lang, a, b) {
                    both += 1;
                }
            }
        }
    }
    if src_rhyming == 0 {
        (1.0, "the source does not rhyme at the scheme's positions".into())
    } else {
        (both as f64 / src_rhyming as f64, format!("{both}/{src_rhyming} source rhymes kept"))
    }
}

/// Fraction of adjacent word pairs that share a consonant onset — a rough
/// alliteration density.
fn alliteration_density(text: &str, lang: &ProseLanguage) -> f64 {
    let words: Vec<&str> = text.split_whitespace().collect();
    let onset = |w: &str| -> Option<char> {
        w.chars().find(|c| c.is_alphabetic()).and_then(|c| c.to_lowercase().next())
    };
    let (mut hits, mut total) = (0usize, 0usize);
    for w in words.windows(2) {
        if let (Some(a), Some(b)) = (onset(w[0]), onset(w[1])) {
            total += 1;
            if a == b && !syllabify::is_vowel_for(a, lang) {
                hits += 1;
            }
        }
    }
    if total == 0 { 0.0 } else { hits as f64 / total as f64 }
}

fn sound_similarity(
    source: &str,
    src_lang: &ProseLanguage,
    translation: &str,
    trans_lang: &ProseLanguage,
) -> (f64, String) {
    let s = alliteration_density(source, src_lang);
    let t = alliteration_density(translation, trans_lang);
    (1.0 - (s - t).abs(), format!("alliteration density {s:.2} → {t:.2}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poetry::form::PoemForm;
    use crate::prose::ProseLanguage::*;

    #[test]
    fn split_source_translation_on_both_delimiters() {
        // The typeable dash rule.
        let (s, t) = split_source_translation("мой дядя\nсамых честных\n---\nmy uncle\nof honest ways").unwrap();
        assert_eq!(s, "мой дядя\nсамых честных");
        assert_eq!(t, "my uncle\nof honest ways");
        // The thematic glyph, plus blank-line trimming around the halves.
        let (s2, t2) = split_source_translation("source\n\n⇄\n\ntranslation\n").unwrap();
        assert_eq!(s2, "source");
        assert_eq!(t2, "translation");
        // Only the FIRST delimiter splits; later dashes stay in the translation.
        let (_, t3) = split_source_translation("a\n---\nb\n---\nc").unwrap();
        assert_eq!(t3, "b\n---\nc");
        // No delimiter → not a paired translation.
        assert!(split_source_translation("just one block\nof lines").is_none());
    }

    #[test]
    fn identical_text_preserves_everything() {
        let form = PoemForm { rhyme_scheme: "AA".into(), ..Default::default() };
        let text = "the bright light\nthe silent night";
        let tri = trilemma(text, &En, text, &En, &form);
        assert!((tri.sound_score - 1.0).abs() < 1e-9);
        // both lines detect the same (or no) metre, and the AA rhyme is kept.
        assert!(tri.form_score > 0.9);
    }

    #[test]
    fn a_lost_rhyme_lowers_the_form_score() {
        let form = PoemForm { rhyme_scheme: "AA".into(), ..Default::default() };
        // source rhymes (light/night), translation does not (light/house).
        let src = "the bright light\nthe silent night";
        let trans = "the bright light\nthe silent house";
        let tri = trilemma(src, &En, trans, &En, &form);
        assert!(tri.rhyme_note.contains("0/1"));
    }

    #[test]
    fn prompt_carries_both_texts() {
        let form = PoemForm { form: "sonnet".into(), ..Default::default() };
        let p = build_trilemma_prompt("исходник", "the source", &form);
        assert!(p.contains("исходник"));
        assert!(p.contains("the source"));
        assert!(p.contains("Form"));
    }
}
