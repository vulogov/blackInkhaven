//! Verse scansion (1.8.2) — read the metre of a line of poetry.
//!
//! Scanning a line means marking each syllable stressed or unstressed and then
//! recognising the repeating *foot* — iamb, trochee, dactyl — that the pattern
//! spells out. This composes two primitives already in the phonology layer:
//! [`syllabify`](crate::conlang::phonology::syllable::syllabify) splits a word
//! into syllables, and stress is resolved per word through a three-link chain
//! so the tool works for languages with lexical (unpredictable) stress, Russian
//! chief among them:
//!
//! 1. an **explicit mark** in the text — a combining acute over the stressed
//!    vowel (`лу́`), the way Russian verse is annotated;
//! 2. the lexicon's **`stress`** field for that word;
//! 3. the language's **stress rule** (initial / penult / …) as a last resort.
//!
//! A monosyllable with no explicit or lexical stress is left *flexible* — it may
//! promote or demote to fit the metre, exactly as a monosyllabic function word
//! does in real scansion. Pure and deterministic.

use crate::conlang::phonology::{stress_eval, syllable};
use crate::conlang::types::{Phonology, PhonemeKind};
use crate::language_entry::DictionaryEntry;

/// Combining acute accent — the standard stress mark over a vowel.
const ACUTE: char = '\u{0301}';

/// Whether a syllable bears the beat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Beat {
    /// Carries primary stress.
    Stressed,
    /// Unstressed.
    Unstressed,
    /// A monosyllable (or a word of unknown stress) that may take either value
    /// to fit the metre — promotion/demotion.
    Flexible,
}

impl Beat {
    /// The scansion glyph: `/` stressed, `×` unstressed, `·` flexible.
    pub fn glyph(self) -> char {
        match self {
            Beat::Stressed => '/',
            Beat::Unstressed => '×',
            Beat::Flexible => '·',
        }
    }
}

/// One syllable of a scanned word.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedSyllable {
    pub text: String,
    pub beat: Beat,
}

/// One word of a scanned line, split into syllables.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedWord {
    pub surface: String,
    pub syllables: Vec<ScannedSyllable>,
}

/// The recognised metre of a line.
#[derive(Debug, Clone, PartialEq)]
pub struct Meter {
    /// Foot name: `iamb`, `trochee`, `anapest`, `dactyl`, `amphibrach`.
    pub foot: &'static str,
    /// How many feet (→ dimeter, trimeter, tetrameter, …).
    pub feet: usize,
    /// The full name, e.g. `iambic tetrameter`.
    pub name: String,
    /// Fraction of fixed (non-flexible) syllables that conform, 0..1.
    pub conformance: f64,
}

/// A scanned line: its words, the flat beat pattern, and the best metre guess.
#[derive(Debug, Clone, PartialEq)]
pub struct ScannedLine {
    pub words: Vec<ScannedWord>,
    pub meter: Option<Meter>,
}

impl ScannedLine {
    /// The flat sequence of beats across the whole line.
    pub fn beats(&self) -> Vec<Beat> {
        self.words.iter().flat_map(|w| w.syllables.iter().map(|s| s.beat)).collect()
    }

}

/// The metrical foot templates we recognise, as stress patterns (true =
/// stressed). Binary feet first so they win ties over the rarer ternary ones.
const FEET: &[(&str, &[bool])] = &[
    ("iamb", &[false, true]),
    ("trochee", &[true, false]),
    ("anapest", &[false, false, true]),
    ("dactyl", &[true, false, false]),
    ("amphibrach", &[false, true, false]),
];

/// Scan one line against a language.
pub fn scan_line(phon: &Phonology, entries: &[DictionaryEntry], line: &str) -> ScannedLine {
    let vowels = vowel_graphemes(phon);
    let mut words = Vec::new();
    for raw in line.split_whitespace() {
        let word = trim_punct(raw);
        if word.chars().all(|c| !c.is_alphabetic()) {
            continue;
        }
        words.push(scan_word(phon, entries, &vowels, word));
    }
    let meter = detect_meter(&flatten(&words));
    ScannedLine { words, meter }
}

/// Scan every non-empty line of a (possibly multi-line) text.
pub fn scan_text(phon: &Phonology, entries: &[DictionaryEntry], text: &str) -> Vec<ScannedLine> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| scan_line(phon, entries, l))
        .collect()
}

/// The dominant metre across a set of scanned lines — the one most lines share.
pub fn dominant_meter(lines: &[ScannedLine]) -> Option<Meter> {
    use std::collections::HashMap;
    let mut tally: HashMap<(&str, usize), (usize, f64)> = HashMap::new();
    for l in lines {
        if let Some(m) = &l.meter {
            let e = tally.entry((m.foot, m.feet)).or_insert((0, 0.0));
            e.0 += 1;
            e.1 += m.conformance;
        }
    }
    tally.into_iter().max_by_key(|(_, (count, _))| *count).map(|((foot, feet), (count, sumc))| Meter {
        foot,
        feet,
        name: format!("{} {}", adjective(foot), length_name(feet)),
        conformance: sumc / count as f64,
    })
}

fn flatten(words: &[ScannedWord]) -> Vec<Beat> {
    words.iter().flat_map(|w| w.syllables.iter().map(|s| s.beat)).collect()
}

/// Scan a single word: syllabify it, resolve which syllable is stressed, and
/// assign a beat to each.
fn scan_word(
    phon: &Phonology,
    entries: &[DictionaryEntry],
    vowels: &[String],
    word: &str,
) -> ScannedWord {
    // The explicit mark, if any, and the word with the mark removed.
    let (clean, marked_syllable) = strip_stress_mark(word, vowels);

    let seq = phon.segment(&clean);
    let sylls = syllable::syllabify(phon, &seq);
    let n = sylls.len().max(1);

    // Resolve the stressed syllable index through the chain.
    let (stressed, lexical) = if let Some(i) = marked_syllable {
        (Some(i.min(n - 1)), true)
    } else if let Some(i) = lexicon_stress(entries, &clean) {
        (Some((i.saturating_sub(1)).min(n - 1)), true)
    } else if let Some(rule) = &phon.stress {
        (stress_eval::primary_stress(rule, &sylls), false)
    } else {
        (None, false)
    };

    let render = |s: &syllable::Syllable| -> String {
        s.onset
            .iter()
            .chain(&s.nucleus)
            .chain(&s.coda)
            .map(|ipa| phon.phoneme(ipa).map(|p| p.grapheme().to_string()).unwrap_or_else(|| ipa.clone()))
            .collect()
    };

    let mono = sylls.len() <= 1;
    let syllables = sylls
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let beat = if mono {
                // A monosyllable is only firmly stressed when we were told so
                // (explicit mark / lexicon); otherwise it flexes to the metre.
                if lexical && stressed == Some(i) { Beat::Stressed } else { Beat::Flexible }
            } else {
                match stressed {
                    Some(k) if k == i => Beat::Stressed,
                    Some(_) => Beat::Unstressed,
                    None => Beat::Flexible,
                }
            };
            ScannedSyllable { text: render(s), beat }
        })
        .collect();

    ScannedWord { surface: word.to_string(), syllables }
}

/// Find `word`'s lexical stress in the dictionary (case-insensitive on the
/// citation form).
fn lexicon_stress(entries: &[DictionaryEntry], word: &str) -> Option<usize> {
    entries
        .iter()
        .find(|e| e.word.eq_ignore_ascii_case(word) || unicode_eq(&e.word, word))
        .and_then(|e| e.stress)
        .filter(|&s| s > 0)
}

fn unicode_eq(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Remove combining-acute marks from a word; return the cleaned word and, if a
/// mark was present, the 0-based index of the syllable it fell on (counted as
/// the ordinal of the marked vowel among the word's vowels).
fn strip_stress_mark(word: &str, vowels: &[String]) -> (String, Option<usize>) {
    if !word.contains(ACUTE) {
        return (word.to_string(), None);
    }
    let mut clean = String::with_capacity(word.len());
    let mut vowel_ord = 0usize; // vowels seen so far (before the current char)
    let mut marked: Option<usize> = None;
    for ch in word.chars() {
        if ch == ACUTE {
            // The acute follows its vowel: that vowel's ordinal is vowel_ord-1.
            if marked.is_none() && vowel_ord > 0 {
                marked = Some(vowel_ord - 1);
            }
            continue;
        }
        clean.push(ch);
        if is_vowel_char(ch, vowels) {
            vowel_ord += 1;
        }
    }
    (clean, marked)
}

/// A char is a vowel if it is one of the language's vowel graphemes.
fn is_vowel_char(ch: char, vowels: &[String]) -> bool {
    let s = ch.to_lowercase().to_string();
    vowels.iter().any(|v| v.to_lowercase() == s)
}

/// The language's vowel graphemes (romanization, falling back to the IPA).
fn vowel_graphemes(phon: &Phonology) -> Vec<String> {
    phon.phonemes
        .iter()
        .filter(|p| p.kind == PhonemeKind::Vowel)
        .map(|p| p.grapheme().to_string())
        .collect()
}

fn trim_punct(s: &str) -> &str {
    s.trim_matches(|c: char| !c.is_alphabetic() && c != ACUTE && c != '\u{0300}')
}

/// Detect the best-fitting metre for a beat sequence. Each foot template is
/// tiled across the line; flexible beats match either value, fixed beats must
/// agree. The template with the highest conformance (over the fixed beats) wins,
/// requiring a clear majority to name a metre at all. Source-agnostic — the
/// poetry layer (POEM-5) feeds it beats from natural-language syllabification.
pub(crate) fn detect_meter(beats: &[Beat]) -> Option<Meter> {
    let n = beats.len();
    if n < 2 {
        return None;
    }
    let mut best: Option<Meter> = None;
    for (foot, pattern) in FEET {
        let f = pattern.len();
        if n < f {
            continue;
        }
        // Floor division: a rising line with a feminine (extra unstressed)
        // ending — 9 syllables of iambic verse — is tetrameter with a
        // hypermetrical syllable, not pentameter. Feminine endings are far
        // commoner than catalexis, so flooring is the better default.
        let feet = (n / f).max(1);
        let mut fixed = 0usize;
        let mut hits = 0usize;
        for (i, b) in beats.iter().enumerate() {
            let expect_stressed = pattern[i % f];
            match b {
                Beat::Flexible => {}
                Beat::Stressed => {
                    fixed += 1;
                    if expect_stressed {
                        hits += 1;
                    }
                }
                Beat::Unstressed => {
                    fixed += 1;
                    if !expect_stressed {
                        hits += 1;
                    }
                }
            }
        }
        if fixed == 0 {
            continue;
        }
        let conformance = hits as f64 / fixed as f64;
        let cand = Meter {
            foot,
            feet,
            name: format!("{} {}", adjective(foot), length_name(feet)),
            conformance,
        };
        if best.as_ref().is_none_or(|b| cand.conformance > b.conformance) {
            best = Some(cand);
        }
    }
    // Only name a metre when the fit is convincing.
    best.filter(|m| m.conformance >= 0.7)
}

/// Adjective form of a foot name (`iamb` → `iambic`).
fn adjective(foot: &str) -> &'static str {
    match foot {
        "iamb" => "iambic",
        "trochee" => "trochaic",
        "anapest" => "anapestic",
        "dactyl" => "dactylic",
        "amphibrach" => "amphibrachic",
        _ => "metrical",
    }
}

/// Line-length name from the foot count.
fn length_name(feet: usize) -> String {
    match feet {
        1 => "monometer".into(),
        2 => "dimeter".into(),
        3 => "trimeter".into(),
        4 => "tetrameter".into(),
        5 => "pentameter".into(),
        6 => "hexameter".into(),
        7 => "heptameter".into(),
        8 => "octameter".into(),
        n => format!("{n}-meter"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind, StressRule};
    use crate::conlang::types::stress::StressPlacement;

    fn ph(ipa: &str, roman: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: Some(roman.into()), kind, sonority: None }
    }

    // A tiny Russian-ish phonology: Cyrillic vowels + a few consonants, no
    // stress rule (so the chain must use marks / lexicon).
    fn ru() -> Phonology {
        Phonology {
            phonemes: vec![
                ph("a", "а", PhonemeKind::Vowel),
                ph("o", "о", PhonemeKind::Vowel),
                ph("u", "у", PhonemeKind::Vowel),
                ph("i", "и", PhonemeKind::Vowel),
                ph("e", "е", PhonemeKind::Vowel),
                ph("ɨ", "ы", PhonemeKind::Vowel),
                ph("d", "д", PhonemeKind::Consonant),
                ph("m", "м", PhonemeKind::Consonant),
                ph("k", "к", PhonemeKind::Consonant),
                ph("n", "н", PhonemeKind::Consonant),
                ph("r", "р", PhonemeKind::Consonant),
                ph("l", "л", PhonemeKind::Consonant),
                ph("t", "т", PhonemeKind::Consonant),
                ph("s", "с", PhonemeKind::Consonant),
                ph("v", "в", PhonemeKind::Consonant),
                ph("b", "б", PhonemeKind::Consonant),
                ph("g", "г", PhonemeKind::Consonant),
                ph("j", "й", PhonemeKind::Consonant),
                ph("z", "з", PhonemeKind::Consonant),
                ph("p", "п", PhonemeKind::Consonant),
                ph("x", "х", PhonemeKind::Consonant),
                ph("f", "ф", PhonemeKind::Consonant),
                ph("ʃ", "ш", PhonemeKind::Consonant),
                ph("ʒ", "ж", PhonemeKind::Consonant),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn explicit_mark_sets_the_stressed_syllable() {
        let p = ru();
        // окно́ — stress on the 2nd (final) syllable.
        let w = scan_word(&p, &[], &vowel_graphemes(&p), "окно\u{0301}");
        assert_eq!(w.syllables.len(), 2);
        assert_eq!(w.syllables[0].beat, Beat::Unstressed);
        assert_eq!(w.syllables[1].beat, Beat::Stressed);
    }

    #[test]
    fn lexicon_stress_is_used_when_text_is_unmarked() {
        let p = ru();
        let entries = vec![DictionaryEntry {
            word: "окно".into(),
            stress: Some(2),
            ..Default::default()
        }];
        let w = scan_word(&p, &entries, &vowel_graphemes(&p), "окно");
        assert_eq!(w.syllables[1].beat, Beat::Stressed);
        assert_eq!(w.syllables[0].beat, Beat::Unstressed);
    }

    #[test]
    fn stress_rule_is_the_last_resort() {
        let mut p = ru();
        p.stress = Some(StressRule { primary: StressPlacement::Initial });
        // молоко — three syllables; the Initial rule stresses the first.
        let w = scan_word(&p, &[], &vowel_graphemes(&p), "молоко");
        assert_eq!(w.syllables.len(), 3);
        assert_eq!(w.syllables[0].beat, Beat::Stressed);
        assert_eq!(w.syllables[1].beat, Beat::Unstressed);
        assert_eq!(w.syllables[2].beat, Beat::Unstressed);
    }

    #[test]
    fn unmarked_monosyllable_is_flexible() {
        let p = ru();
        let w = scan_word(&p, &[], &vowel_graphemes(&p), "дуб");
        assert_eq!(w.syllables.len(), 1);
        assert_eq!(w.syllables[0].beat, Beat::Flexible);
    }

    #[test]
    fn beats_render_the_glyph_row() {
        let p = ru();
        // окно́ дуб → [× /] [·]
        let line = scan_line(&p, &[], "окно\u{0301} дуб");
        let glyphs: String = line.beats().iter().map(|b| b.glyph()).collect();
        assert_eq!(glyphs, "×/·");
        assert_eq!(line.beats().len(), 3);
    }

    #[test]
    fn detects_iambic_tetrameter() {
        // × / × / × / × /  — four iambs.
        let beats = vec![
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
        ];
        let m = detect_meter(&beats).expect("a metre");
        assert_eq!(m.foot, "iamb");
        assert_eq!(m.feet, 4);
        assert_eq!(m.name, "iambic tetrameter");
        assert!((m.conformance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn flexible_monosyllables_promote_to_fit() {
        // A line of content polysyllables pinning iambic feet, with a flexible
        // monosyllable that must promote — still reads as iambic.
        let beats = vec![
            Beat::Flexible, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
        ];
        let m = detect_meter(&beats).expect("a metre");
        assert_eq!(m.foot, "iamb");
        assert!((m.conformance - 1.0).abs() < 1e-9); // only fixed beats scored
    }

    #[test]
    fn free_verse_names_no_metre() {
        // An all-stressed (spondaic) run fits none of the common feet above the
        // 0.7 threshold — no regular metre is claimed.
        let beats = vec![
            Beat::Stressed, Beat::Stressed, Beat::Stressed, Beat::Stressed, Beat::Stressed,
        ];
        assert!(detect_meter(&beats).map(|m| m.conformance < 0.7).unwrap_or(true));
    }
}
