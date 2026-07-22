//! POEM-2 (PO-P2) — natural-language syllabification for verse scanning.
//!
//! The conlang layer (1.7.0) syllabifies invented words against a declared
//! phoneme inventory. This adds syllabification for the five *natural* project
//! languages (EN/RU/DE/FR/ES), which everything downstream — metre (PO-P5),
//! rhyme (PO-P4), the Inner Poet (PO-P3) — depends on.
//!
//! Boundaries come from **`hypher`** (Typst's Knuth–Liang hyphenation crate,
//! already in the tree). But Liang hyphenation returns *typographic* break
//! opportunities and under-counts syllables (it suppresses word-edge single-vowel
//! breaks — `hyphenate("hello")` gives one segment for a two-syllable word), so
//! the syllable *count* is taken from **vowel groups** (reliable, and ~exact for
//! Russian). Each hypher segment is then sub-split so there is exactly one
//! syllable per vowel nucleus. Stress is a rule-based layer on top; a lexical
//! stress dictionary is a later refinement.

use crate::prose::ProseLanguage;

/// The stress carried by a syllable. `Secondary` is reserved for the Inner
/// Poet's later analysis of long words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StressLevel {
    Primary,
    Secondary,
    Unstressed,
}

/// Syllable weight for quantitative metre (heavy = long/closed, light = short/open).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoraWeight {
    Heavy,
    Light,
}

/// One syllable: its orthographic text, stress, and weight.
#[derive(Debug, Clone, PartialEq)]
pub struct Syllable {
    pub text: String,
    pub stress: StressLevel,
    pub mora_weight: MoraWeight,
}

/// Count syllables by vowel nuclei (English silent final-e excluded) — the
/// reliable measure metre needs. Consumed by the metre scanner (PO-P5).
#[allow(dead_code)]
pub fn syllable_count(word: &str, lang: ProseLanguage) -> usize {
    let vowels = vowels_for(&lang);
    let chars: Vec<char> = word.chars().collect();
    nucleus_starts(&chars, &vowels, &lang).len()
}

/// Syllabify a natural-language word: one syllable per vowel nucleus, cut at the
/// consonant runs between them, preferring a hypher break where one falls in the
/// run. Stress and mora weight are annotated by rule.
pub fn syllabify(word: &str, lang: ProseLanguage) -> Vec<Syllable> {
    let vowels = vowels_for(&lang);
    let chars: Vec<char> = word.chars().collect();
    let nuclei = nucleus_starts(&chars, &vowels, &lang);

    let texts: Vec<String> = if nuclei.len() <= 1 {
        if word.is_empty() {
            Vec::new()
        } else {
            vec![word.to_string()]
        }
    } else {
        let hbreaks = hypher_breaks(word, &lang);
        let cuts = compute_cuts(&chars, &vowels, &nuclei, &hbreaks);
        let mut out = Vec::new();
        let mut prev = 0;
        for &c in &cuts {
            out.push(chars[prev..c].iter().collect::<String>());
            prev = c;
        }
        out.push(chars[prev..].iter().collect::<String>());
        out
    };

    let stressed = default_stress_index(&lang, &texts, &vowels);
    texts
        .iter()
        .enumerate()
        .map(|(i, t)| Syllable {
            stress: if Some(i) == stressed { StressLevel::Primary } else { StressLevel::Unstressed },
            mora_weight: mora_weight_of(t, &vowels),
            text: t.clone(),
        })
        .collect()
}

/// Count morae (heavy = 2, light = 1) — for quantitative metre or mora-counted
/// forms. Consumed by the metre scanner (PO-P5).
#[allow(dead_code)]
pub fn mora_count(word: &str, lang: ProseLanguage) -> usize {
    syllabify(word, lang)
        .iter()
        .map(|s| match s.mora_weight {
            MoraWeight::Heavy => 2,
            MoraWeight::Light => 1,
        })
        .sum()
}

/// Syllabify an invented-language word against its phoneme inventory, mapping
/// the conlang layer's syllables into the poetry [`Syllable`] shape. Stress is
/// left `Unstressed` here — for conlang verse the scansion engine resolves stress
/// through its own mark → lexicon → rule chain. Consumed when conlang metre
/// scanning lands (PO-P5); exercised now by the tests.
#[allow(dead_code)]
pub fn syllabify_conlang(word: &str, phon: &crate::conlang::types::Phonology) -> Vec<Syllable> {
    let seq = phon.segment(word);
    let sylls = crate::conlang::phonology::syllable::syllabify(phon, &seq);
    sylls
        .iter()
        .map(|s| {
            let text: String = s
                .onset
                .iter()
                .chain(&s.nucleus)
                .chain(&s.coda)
                .map(|ipa| {
                    phon.phoneme(ipa).map(|p| p.grapheme().to_string()).unwrap_or_else(|| ipa.clone())
                })
                .collect();
            let mora_weight = if crate::conlang::phonology::stress_eval::is_heavy(s) {
                MoraWeight::Heavy
            } else {
                MoraWeight::Light
            };
            Syllable { text, stress: StressLevel::Unstressed, mora_weight }
        })
        .collect()
}

/// If the word carries a combining acute accent (`\u{301}`) over a vowel — the
/// way verse is stress-marked — return the 0-based syllable index it marks (the
/// nucleus ordinal of the accented vowel), else `None`. Used by the metre scanner
/// to override rule-based stress with the author's marks.
pub fn marked_syllable_index(word: &str, lang: ProseLanguage) -> Option<usize> {
    let vowels = vowels_for(&lang);
    let mut group_index: isize = -1;
    let mut in_v = false;
    for c in word.chars() {
        if c == '\u{301}' {
            return if group_index >= 0 { Some(group_index as usize) } else { None };
        }
        let v = is_vowel(c, &vowels);
        if v && !in_v {
            group_index += 1;
        }
        in_v = v;
    }
    None
}

// ── internals ───────────────────────────────────────────────────────

fn hyph_lang(lang: &ProseLanguage) -> Option<hypher::Lang> {
    use ProseLanguage::*;
    Some(match lang {
        En => hypher::Lang::English,
        Ru => hypher::Lang::Russian,
        De => hypher::Lang::German,
        Fr => hypher::Lang::French,
        Es => hypher::Lang::Spanish,
        Other(_) => return None,
    })
}

fn vowels_for(lang: &ProseLanguage) -> String {
    use ProseLanguage::*;
    match lang {
        En => "aeiouy",
        Ru => "аеёиоуыэюя",
        De => "aeiouyäöü",
        Fr => "aeiouyàâäéèêëîïôöùûüœ",
        Es => "aeiouyáéíóúü",
        Other(_) => "aeiou",
    }
    .to_string()
}

fn is_vowel(c: char, vowels: &str) -> bool {
    let lc = c.to_lowercase().next().unwrap_or(c);
    vowels.chars().any(|v| v == lc)
}

/// Whether `c` is a vowel in `lang` — for the rhyme engine's tail extraction.
pub fn is_vowel_for(c: char, lang: &ProseLanguage) -> bool {
    is_vowel(c, &vowels_for(lang))
}

/// The char index where each syllable nucleus begins. English silent final-e is
/// dropped (a lone word-final `e` after a consonant, when the word has more than
/// one nucleus), so "extensive" is three syllables, not four.
fn nucleus_starts(chars: &[char], vowels: &str, lang: &ProseLanguage) -> Vec<usize> {
    let mut starts: Vec<usize> = Vec::new();
    let mut in_v = false;
    for (i, &c) in chars.iter().enumerate() {
        let v = is_vowel(c, vowels);
        if v && !in_v {
            starts.push(i);
        }
        in_v = v;
    }
    if matches!(lang, ProseLanguage::En) && starts.len() > 1 {
        let last = *starts.last().unwrap();
        let is_final = last + 1 == chars.len();
        let is_e = chars[last].to_lowercase().next() == Some('e');
        let after_consonant = last > 0 && !is_vowel(chars[last - 1], vowels);
        if is_final && is_e && after_consonant {
            starts.pop();
        }
    }
    starts
}

/// Char indices where hypher places a break (Typst's Liang hyphenation). These
/// are typographic break opportunities — used to refine cut placement, not to
/// count syllables (hypher under-counts).
fn hypher_breaks(word: &str, lang: &ProseLanguage) -> Vec<usize> {
    let Some(hl) = hyph_lang(lang) else { return Vec::new() };
    let segs: Vec<String> = hypher::hyphenate(word, hl).map(|s| s.to_string()).collect();
    let mut breaks = Vec::new();
    let mut acc = 0usize;
    for seg in segs.iter().take(segs.len().saturating_sub(1)) {
        acc += seg.chars().count();
        breaks.push(acc);
    }
    breaks
}

/// Cut positions between successive nuclei: a hypher break inside the consonant
/// run if one is there, else the heuristic (0 consonants → hiatus; 1 → the
/// consonant onsets the next syllable; ≥2 → the first consonant closes this one).
fn compute_cuts(chars: &[char], vowels: &str, nuclei: &[usize], hbreaks: &[usize]) -> Vec<usize> {
    let group_end = |start: usize| -> usize {
        let mut j = start;
        while j < chars.len() && is_vowel(chars[j], vowels) {
            j += 1;
        }
        j
    };
    let mut cuts = Vec::new();
    for w in nuclei.windows(2) {
        let (a, b) = (w[0], w[1]);
        let end_a = group_end(a);
        let cut = match hbreaks.iter().copied().find(|&x| x > end_a && x <= b) {
            Some(x) => x,
            None => match b - end_a {
                0 => b,
                1 => end_a,
                _ => end_a + 1,
            },
        };
        cuts.push(cut);
    }
    cuts
}

/// The default primary-stress syllable index for a language (no dictionary yet).
fn default_stress_index(lang: &ProseLanguage, texts: &[String], vowels: &str) -> Option<usize> {
    let n = texts.len();
    if n == 0 {
        return None;
    }
    use ProseLanguage::*;
    Some(match lang {
        // English: monosyllable stresses itself, else penultimate.
        En => if n == 1 { 0 } else { n - 2 },
        // Russian is lexical; the rule fallback is penult for short words,
        // antepenult for longer ones (the marks in verse text override this).
        Ru => match n {
            1 | 2 => 0,   // penult of a ≤2-syllable word (or the sole syllable)
            _ => n - 3,   // antepenult
        },
        // German: root-initial (first syllable) for the native majority.
        De => 0,
        // French: syllabic tradition — approximate the phrase-final accent.
        Fr => n - 1,
        // Spanish: penultimate unless the word ends in a consonant other than n/s.
        Es => {
            let ends_open = texts
                .last()
                .and_then(|s| s.chars().last())
                .map(|c| {
                    is_vowel(c, vowels)
                        || matches!(c.to_lowercase().next().unwrap_or(c), 'n' | 's')
                })
                .unwrap_or(true);
            if ends_open {
                n.saturating_sub(2)
            } else {
                n - 1
            }
        }
        Other(_) => n.saturating_sub(2),
    })
}

fn mora_weight_of(t: &str, vowels: &str) -> MoraWeight {
    let ends_consonant = t.chars().last().map(|c| !is_vowel(c, vowels)).unwrap_or(false);
    let vowel_count = t.chars().filter(|c| is_vowel(*c, vowels)).count();
    // Closed syllable or diphthong nucleus → heavy.
    if ends_consonant || vowel_count >= 2 {
        MoraWeight::Heavy
    } else {
        MoraWeight::Light
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage;

    #[test]
    fn russian_syllable_count_is_vowel_exact() {
        // Russian syllable count == vowel count, always.
        assert_eq!(syllable_count("молоко", ProseLanguage::Ru), 3);
        assert_eq!(syllable_count("дом", ProseLanguage::Ru), 1);
        assert_eq!(syllable_count("лукоморье", ProseLanguage::Ru), 4);
    }

    #[test]
    fn english_count_beats_hyphers_undercount() {
        // hypher gives 0 breaks for "hello" (1 segment); vowel groups give 2.
        assert_eq!(syllable_count("hello", ProseLanguage::En), 2);
        assert_eq!(syllable_count("away", ProseLanguage::En), 2);
        assert_eq!(syllable_count("extensive", ProseLanguage::En), 3);
    }

    #[test]
    fn syllabify_yields_one_per_vowel_group() {
        let s = syllabify("молоко", ProseLanguage::Ru);
        assert_eq!(s.len(), 3);
        assert_eq!(s.iter().map(|x| x.text.clone()).collect::<Vec<_>>().concat(), "молоко");
    }

    #[test]
    fn russian_stress_rule_is_penult_or_antepenult() {
        // 3 syllables → antepenult (index 0).
        let s = syllabify("молоко", ProseLanguage::Ru);
        assert_eq!(s[0].stress, StressLevel::Primary);
        // 2 syllables → penult (index 0).
        let s2 = syllabify("окно", ProseLanguage::Ru);
        assert_eq!(s2.len(), 2);
        assert_eq!(s2[0].stress, StressLevel::Primary);
    }

    #[test]
    fn english_stress_is_penultimate() {
        let s = syllabify("extensive", ProseLanguage::En); // ex-ten-sive
        assert_eq!(s.len(), 3);
        assert_eq!(s[1].stress, StressLevel::Primary); // penult
    }

    #[test]
    fn german_stress_is_initial() {
        let s = syllabify("Wandern", ProseLanguage::De);
        assert!(!s.is_empty());
        assert_eq!(s[0].stress, StressLevel::Primary);
    }

    #[test]
    fn spanish_penult_vs_final_by_ending() {
        // ends in vowel → penultimate
        let casa = syllabify("casa", ProseLanguage::Es);
        assert_eq!(casa.len(), 2);
        assert_eq!(casa[0].stress, StressLevel::Primary);
        // ends in a consonant other than n/s → final
        let reloj = syllabify("reloj", ProseLanguage::Es);
        assert_eq!(reloj.last().unwrap().stress, StressLevel::Primary);
    }

    #[test]
    fn mora_weight_and_count() {
        // "dom" is a single closed (heavy) syllable → 2 morae.
        assert_eq!(mora_count("дом", ProseLanguage::Ru), 2);
        // an open light syllable is 1 mora.
        let a = syllabify("а", ProseLanguage::Ru);
        assert_eq!(a[0].mora_weight, MoraWeight::Light);
    }

    #[test]
    fn unsupported_language_still_counts_vowels() {
        let n = syllable_count("saluton", ProseLanguage::Other("eo".into()));
        assert_eq!(n, 3); // sa-lu-ton
    }
}
