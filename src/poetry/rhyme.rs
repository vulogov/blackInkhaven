//! POEM-4 (PO-P4) — the multilingual rhyme engine.
//!
//! Classifies the rhyme between two words: its *quality* (perfect / near) and
//! *type* (masculine / feminine / dactylic). Without a phonemic transcription for
//! the natural languages, it works on the *orthographic rhyme tail* — everything
//! from the stressed vowel onward — with per-language normalization: German
//! final-devoicing (so *Hund* and *bunt* rhyme on `-unt`), French mute-e elision,
//! light Russian vowel reduction, and Spanish assonance (vowel-only matches).
//! This is accurate for the regular orthographies (Russian above all); English
//! eye-rhyme (`love`/`prove` — same spelling, different sound) can't be told from
//! true rhyme without the lexical phoneme dictionary, a later refinement.

use crate::poetry::syllabify::{self, StressLevel};
use crate::prose::ProseLanguage;

/// How well two words rhyme. `Eye` (orthographic match, phonetic mismatch) is
/// reserved for when phonemic data is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum RhymeQuality {
    Perfect,
    Near,
    Eye,
    None,
}

/// Where the rhyme's stress falls — masculine (final), feminine (penultimate),
/// dactylic (antepenultimate).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RhymeType {
    Masculine,
    Feminine,
    Dactylic,
}

/// The rhyme analysis of a word pair.
#[derive(Debug, Clone, PartialEq)]
pub struct RhymeAnalysis {
    pub quality: RhymeQuality,
    pub rhyme_type: RhymeType,
    /// The shared terminal string (the common rhyme tail).
    pub shared: String,
    pub note: Option<String>,
}

/// Analyse the rhyme between two words.
pub fn analyse_rhyme(w1: &str, w2: &str, lang: ProseLanguage) -> RhymeAnalysis {
    let (t1, rtype) = rhyme_tail(w1, &lang);
    let (t2, _) = rhyme_tail(w2, &lang);
    let n1 = normalize(&t1, &lang);
    let n2 = normalize(&t2, &lang);

    let vowels_only = |s: &str| -> String {
        s.chars().filter(|&c| syllabify::is_vowel_for(c, &lang)).collect()
    };

    let (quality, note) = if !n1.is_empty() && n1 == n2 {
        (RhymeQuality::Perfect, None)
    } else {
        let (v1, v2) = (vowels_only(&n1), vowels_only(&n2));
        if !v1.is_empty() && v1 == v2 {
            let note = if matches!(lang, ProseLanguage::Es) {
                "assonant rhyme"
            } else {
                "assonance — vowels match, consonants differ"
            };
            (RhymeQuality::Near, Some(note.to_string()))
        } else if !n1.is_empty() && levenshtein(&n1, &n2) <= 1 {
            (RhymeQuality::Near, Some("near-rhyme — differs by one sound".to_string()))
        } else {
            (RhymeQuality::None, None)
        }
    };

    RhymeAnalysis { quality, rhyme_type: rtype, shared: common_suffix(&n1, &n2), note }
}

/// The rhyme tail (from the stressed vowel to the end of the word, lower-cased)
/// and the rhyme type, using the acute mark if present else the syllabifier's
/// rule for stress.
fn rhyme_tail(word: &str, lang: &ProseLanguage) -> (String, RhymeType) {
    let marked = syllabify::marked_syllable_index(word, lang.clone());
    let clean: String = word.chars().filter(|&c| c != '\u{301}').collect();
    let sylls = syllabify::syllabify(&clean, lang.clone());
    if sylls.is_empty() {
        return (String::new(), RhymeType::Masculine);
    }
    let n = sylls.len();
    let stressed = marked
        .map(|m| m.min(n - 1))
        .or_else(|| sylls.iter().position(|s| s.stress == StressLevel::Primary))
        .unwrap_or(n - 1);

    let mut tail = String::new();
    for (i, s) in sylls.iter().enumerate() {
        if i < stressed {
            continue;
        }
        if i == stressed {
            // Drop the onset — the rhyme runs from the nucleus vowel.
            let mut started = false;
            for c in s.text.chars() {
                if !started && !syllabify::is_vowel_for(c, lang) {
                    continue;
                }
                started = true;
                tail.push(c);
            }
            if !started {
                tail.push_str(&s.text);
            }
        } else {
            tail.push_str(&s.text);
        }
    }

    let after = n - 1 - stressed;
    let rtype = match after {
        0 => RhymeType::Masculine,
        1 => RhymeType::Feminine,
        _ => RhymeType::Dactylic,
    };
    (tail.to_lowercase(), rtype)
}

/// Per-language phonetic-ish normalization of a rhyme tail.
fn normalize(tail: &str, lang: &ProseLanguage) -> String {
    let mut chars: Vec<char> = tail.chars().collect();
    match lang {
        // Final devoicing: a voiced obstruent at word end is voiceless.
        ProseLanguage::De => {
            if let Some(last) = chars.last_mut() {
                *last = match *last {
                    'd' => 't',
                    'b' => 'p',
                    'g' => 'k',
                    'v' => 'f',
                    'z' => 's',
                    c => c,
                };
            }
        }
        // Mute final -e is silent.
        ProseLanguage::Fr => {
            if chars.len() > 1 && chars.last() == Some(&'e') {
                chars.pop();
            }
        }
        // Light reduction: a trailing unstressed о reads as а.
        ProseLanguage::Ru => {
            if chars.len() > 1 && chars.last() == Some(&'о') {
                *chars.last_mut().unwrap() = 'а';
            }
        }
        _ => {}
    }
    chars.into_iter().collect()
}

fn common_suffix(a: &str, b: &str) -> String {
    let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut out: Vec<char> = Vec::new();
    let (mut i, mut j) = (ac.len(), bc.len());
    while i > 0 && j > 0 && ac[i - 1] == bc[j - 1] {
        out.push(ac[i - 1]);
        i -= 1;
        j -= 1;
    }
    out.reverse();
    out.into_iter().collect()
}

fn levenshtein(a: &str, b: &str) -> usize {
    let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    let mut prev: Vec<usize> = (0..=bc.len()).collect();
    for (i, &ca) in ac.iter().enumerate() {
        let mut cur = vec![i + 1];
        for (j, &cb) in bc.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            cur.push((prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost));
        }
        prev = cur;
    }
    prev[bc.len()]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::*;

    #[test]
    fn russian_perfect_masculine() {
        // дом / том — single stressed syllables, tail "ом".
        let r = analyse_rhyme("дом", "том", Ru);
        assert_eq!(r.quality, RhymeQuality::Perfect);
        assert_eq!(r.rhyme_type, RhymeType::Masculine);
        assert_eq!(r.shared, "ом");
    }

    #[test]
    fn russian_perfect_feminine_with_marks() {
        // мо́ре / го́ре — stress on the first syllable, tail "оре", feminine.
        let r = analyse_rhyme("мо\u{301}ре", "го\u{301}ре", Ru);
        assert_eq!(r.quality, RhymeQuality::Perfect);
        assert_eq!(r.rhyme_type, RhymeType::Feminine);
    }

    #[test]
    fn german_final_devoicing_makes_a_rhyme() {
        // Hund / bunt — tails "und"/"unt"; devoicing → "unt"/"unt".
        let r = analyse_rhyme("Hund", "bunt", De);
        assert_eq!(r.quality, RhymeQuality::Perfect);
    }

    #[test]
    fn spanish_assonance_is_near() {
        // cielo / viejo — vowels ie-o match, consonants differ → assonant.
        let r = analyse_rhyme("cielo", "viejo", Es);
        assert_eq!(r.quality, RhymeQuality::Near);
        assert_eq!(r.note.as_deref(), Some("assonant rhyme"));
    }

    #[test]
    fn english_perfect_and_non_rhyme() {
        assert_eq!(analyse_rhyme("light", "night", En).quality, RhymeQuality::Perfect);
        assert_eq!(analyse_rhyme("cat", "dog", En).quality, RhymeQuality::None);
    }
}
