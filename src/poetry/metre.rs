//! POEM-5 (PO-P3) — metre scanning for verse.
//!
//! Builds a line's *beat sequence* from the POEM-2 syllabifier, then either
//! detects its metre (reusing the 1.8.2 scansion engine's `detect_meter`) or
//! checks it against a *declared* foot pattern from the `poem:` block, reporting
//! feminine endings, catalexis, and per-line conformance. Also covers the
//! syllabic tradition (French/Japanese — count only), an accentual stress count,
//! and free-verse observation.
//!
//! Stress per word comes from an explicit acute mark in the text if present
//! (the way Russian verse is annotated), else the syllabifier's rule; a
//! monosyllabic word is *flexible* (promotes/demotes to fit the metre).

use crate::conlang::scansion::{self, Beat};
use crate::poetry::syllabify::{self, StressLevel};
use crate::prose::ProseLanguage;

/// A metrical foot, as a pattern of strong (`true`) / weak (`false`) positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foot {
    Iamb,
    Trochee,
    Dactyl,
    Anapest,
    Amphibrach,
    Spondee,
}

impl Foot {
    /// Parse a `poem.metre` value (`iambic`, `trochaic`, …).
    pub fn parse(metre: &str) -> Option<Foot> {
        Some(match metre.trim().to_lowercase().as_str() {
            "iambic" | "iamb" => Foot::Iamb,
            "trochaic" | "trochee" => Foot::Trochee,
            "dactylic" | "dactyl" => Foot::Dactyl,
            "anapestic" | "anapest" => Foot::Anapest,
            "amphibrachic" | "amphibrach" => Foot::Amphibrach,
            "spondaic" | "spondee" => Foot::Spondee,
            _ => return None,
        })
    }

    fn pattern(self) -> &'static [bool] {
        match self {
            Foot::Iamb => &[false, true],
            Foot::Trochee => &[true, false],
            Foot::Dactyl => &[true, false, false],
            Foot::Anapest => &[false, false, true],
            Foot::Amphibrach => &[false, true, false],
            Foot::Spondee => &[true, true],
        }
    }
}

/// The beats of one word: its stressed syllable strong, the rest weak; a
/// monosyllable flexible unless explicitly marked. An acute mark overrides the
/// rule-based stress.
fn word_beats(word: &str, lang: ProseLanguage) -> Vec<Beat> {
    let marked = syllabify::marked_syllable_index(word, lang.clone());
    // The syllabifier works on the mark-free word.
    let clean: String = word.chars().filter(|&c| c != '\u{301}').collect();
    let sylls = syllabify::syllabify(&clean, lang);
    let n = sylls.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        // A marked monosyllable is firmly stressed; an unmarked one flexes.
        return vec![if marked.is_some() { Beat::Stressed } else { Beat::Flexible }];
    }
    let stressed = marked.map(|m| m.min(n - 1)).or_else(|| {
        sylls.iter().position(|s| s.stress == StressLevel::Primary)
    });
    (0..n)
        .map(|i| if Some(i) == stressed { Beat::Stressed } else { Beat::Unstressed })
        .collect()
}

/// The full beat sequence of a line of verse.
pub fn line_to_beats(line: &str, lang: ProseLanguage) -> Vec<Beat> {
    let mut beats = Vec::new();
    for raw in line.split_whitespace() {
        let word: String = raw
            .chars()
            .filter(|&c| c.is_alphabetic() || c == '\u{301}')
            .collect();
        if word.chars().any(|c| c.is_alphabetic()) {
            beats.extend(word_beats(&word, lang.clone()));
        }
    }
    beats
}

/// Detect the metre of a line's beats (reuses the scansion engine). `None` for
/// free verse / an unclear pattern.
pub fn detect(beats: &[Beat]) -> Option<scansion::Meter> {
    scansion::detect_meter(beats)
}

/// The result of checking a line against a declared accentual-syllabic metre.
#[derive(Debug, Clone, PartialEq)]
pub struct LineScan {
    /// Syllables in the line.
    pub syllables: usize,
    /// Expected syllables (feet × foot length).
    pub expected_syllables: usize,
    /// Fraction of fixed (non-flexible) beats that match the tiled foot pattern.
    pub conformance: f64,
    /// One extra unstressed syllable past the last full foot — a rising line's
    /// *feminine ending* (normal in Russian and English iambic verse).
    pub feminine_ending: bool,
    /// One syllable short of the last full foot (catalexis).
    pub catalectic: bool,
}

/// Check a beat sequence against a declared foot × feet. Flexible beats match
/// either value; the tolerance for length is ±1 (feminine ending / catalexis).
pub fn scan_line(beats: &[Beat], foot: Foot, feet: usize) -> LineScan {
    let pat = foot.pattern();
    let flen = pat.len();
    let expected = feet * flen;
    let n = beats.len();

    let mut fixed = 0usize;
    let mut hits = 0usize;
    for (i, b) in beats.iter().enumerate() {
        let expect_strong = pat[i % flen];
        match b {
            Beat::Flexible => {}
            Beat::Stressed => {
                fixed += 1;
                if expect_strong {
                    hits += 1;
                }
            }
            Beat::Unstressed => {
                fixed += 1;
                if !expect_strong {
                    hits += 1;
                }
            }
        }
    }
    let conformance = if fixed == 0 { 1.0 } else { hits as f64 / fixed as f64 };

    LineScan {
        syllables: n,
        expected_syllables: expected,
        conformance,
        feminine_ending: n == expected + 1,
        catalectic: n + 1 == expected,
    }
}

/// The result of a syllabic-tradition scan (French classical, Japanese) — count
/// only. Consumed by the Inner Poet (PO-P5).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct SyllabicScan {
    pub syllables: usize,
    pub target: usize,
    pub delta: i64,
}

/// Scan a line of syllable-counted verse against a target count.
#[allow(dead_code)]
pub fn scan_syllabic(line: &str, lang: ProseLanguage, target: usize) -> SyllabicScan {
    let n = line_to_beats(line, lang).len();
    SyllabicScan { syllables: n, target, delta: n as i64 - target as i64 }
}

/// Count primary stresses in a line (accentual tradition — OE, ballad).
/// Consumed by the Inner Poet (PO-P5); exercised now by the accentual path.
#[allow(dead_code)]
pub fn scan_accentual(beats: &[Beat]) -> usize {
    beats.iter().filter(|b| matches!(b, Beat::Stressed)).count()
}

/// A profile of a free-verse passage — observations, never a verdict.
/// Consumed by the Inner Poet (PO-P5).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub struct FreeVerseProfile {
    pub lines: usize,
    pub mean_syllables: f64,
    /// Coefficient of variation of line length; < 0.2 is rhythmically monotone.
    pub rhythm_cv: f64,
    pub longest: usize,
    pub shortest: usize,
}

/// Observe a free-verse passage (line beat-sequences).
#[allow(dead_code)]
pub fn observe_free_verse(lines: &[Vec<Beat>]) -> FreeVerseProfile {
    let counts: Vec<usize> = lines.iter().map(|l| l.len()).filter(|&n| n > 0).collect();
    if counts.is_empty() {
        return FreeVerseProfile {
            lines: 0,
            mean_syllables: 0.0,
            rhythm_cv: 0.0,
            longest: 0,
            shortest: 0,
        };
    }
    let n = counts.len();
    let sum: usize = counts.iter().sum();
    let mean = sum as f64 / n as f64;
    let var = counts.iter().map(|&c| (c as f64 - mean).powi(2)).sum::<f64>() / n as f64;
    let cv = if mean > 0.0 { var.sqrt() / mean } else { 0.0 };
    FreeVerseProfile {
        lines: n,
        mean_syllables: mean,
        rhythm_cv: cv,
        longest: *counts.iter().max().unwrap(),
        shortest: *counts.iter().min().unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage;

    #[test]
    fn foot_parses_from_poem_metre() {
        assert_eq!(Foot::parse("iambic"), Some(Foot::Iamb));
        assert_eq!(Foot::parse("Trochee"), Some(Foot::Trochee));
        assert_eq!(Foot::parse("free"), None);
    }

    #[test]
    fn marked_russian_stress_drives_the_beats() {
        // окно́ (stress marked on the 2nd syllable) → weak-strong = one iamb.
        let beats = line_to_beats("окно\u{301}", ProseLanguage::Ru);
        assert_eq!(beats, vec![Beat::Unstressed, Beat::Stressed]);
    }

    #[test]
    fn monosyllable_is_flexible_unless_marked() {
        let b = line_to_beats("дуб", ProseLanguage::Ru);
        assert_eq!(b, vec![Beat::Flexible]);
        let m = line_to_beats("ду\u{301}б", ProseLanguage::Ru);
        assert_eq!(m, vec![Beat::Stressed]);
    }

    #[test]
    fn scan_line_detects_feminine_ending() {
        // Nine beats of iambic verse = tetrameter with a feminine ending.
        let beats = vec![
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
            Beat::Unstressed,
        ];
        let scan = scan_line(&beats, Foot::Iamb, 4);
        assert_eq!(scan.expected_syllables, 8);
        assert!(scan.feminine_ending);
        assert!(!scan.catalectic);
        assert!((scan.conformance - 1.0).abs() < 1e-9);
    }

    #[test]
    fn detect_reuses_scansion() {
        let beats = vec![
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
            Beat::Unstressed, Beat::Stressed, Beat::Unstressed, Beat::Stressed,
        ];
        let m = detect(&beats).expect("a metre");
        assert_eq!(m.foot, "iamb");
        assert_eq!(m.feet, 4);
    }

    #[test]
    fn syllabic_scan_reports_delta() {
        // A 5-syllable haiku line against a target of 5.
        let s = scan_syllabic("розы у окна", ProseLanguage::Ru, 5);
        assert_eq!(s.syllables, 5);
        assert_eq!(s.delta, 0);
    }

    #[test]
    fn free_verse_flags_monotone_rhythm() {
        // Four lines of identical length → CV 0 (monotone).
        let line = || vec![Beat::Unstressed; 6];
        let p = observe_free_verse(&[line(), line(), line(), line()]);
        assert_eq!(p.lines, 4);
        assert!((p.mean_syllables - 6.0).abs() < 1e-9);
        assert!(p.rhythm_cv < 0.2);
    }
}
