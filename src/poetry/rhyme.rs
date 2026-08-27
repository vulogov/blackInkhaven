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

/// Analyse the rhyme between two words. For English, consults the pronouncing
/// dictionary if one is installed (see [`analyse_rhyme_with`]).
pub fn analyse_rhyme(w1: &str, w2: &str, lang: ProseLanguage) -> RhymeAnalysis {
    analyse_rhyme_with(
        w1,
        w2,
        lang.clone(),
        if matches!(lang, ProseLanguage::En) { crate::poetry::phonemes::en() } else { None },
    )
}

/// [`analyse_rhyme`] with an explicit (optional) English pronouncing dictionary.
/// When English and *both* words are in the dictionary, the rhyme is judged from
/// their phonemes — so `love`/`move` is correctly an *eye* rhyme (same spelling
/// tail, different sound) rather than the "perfect" the orthographic heuristic
/// reports. Otherwise it falls back to the per-language orthographic analysis.
/// Split out so the phoneme path is unit-testable without the process-global dict.
pub fn analyse_rhyme_with(
    w1: &str,
    w2: &str,
    lang: ProseLanguage,
    dict: Option<&crate::poetry::phonemes::PhonemeDict>,
) -> RhymeAnalysis {
    if matches!(lang, ProseLanguage::En) {
        if let Some(d) = dict {
            if let (Some(p1), Some(p2)) = (d.get(w1), d.get(w2)) {
                return phoneme_rhyme(w1, w2, p1, p2, &lang);
            }
        }
    }

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

/// Judge an English rhyme from two words' phonemes (both known to be in the
/// dictionary). Perfect iff the rhyme tails are equal; an *eye* rhyme when the
/// spelling tails match but the sounds don't; near when only the stressed vowels
/// agree; else none.
fn phoneme_rhyme(
    w1: &str,
    w2: &str,
    p1: &crate::poetry::phonemes::Pron,
    p2: &crate::poetry::phonemes::Pron,
    lang: &ProseLanguage,
) -> RhymeAnalysis {
    let rtype = rhyme_type_from_pron(p1);
    // Orthographic tails, for the eye-rhyme test + a readable `shared`.
    let (t1, _) = rhyme_tail(w1, lang);
    let (t2, _) = rhyme_tail(w2, lang);
    let (n1, n2) = (normalize(&t1, lang), normalize(&t2, lang));

    if !p1.rhyme_key.is_empty() && p1.rhyme_key == p2.rhyme_key {
        return RhymeAnalysis {
            quality: RhymeQuality::Perfect,
            rhyme_type: rtype,
            shared: common_suffix(&n1, &n2),
            note: None,
        };
    }
    // Spelling says rhyme, phonemes say no → eye rhyme (the love/move case).
    if !n1.is_empty() && n1 == n2 {
        return RhymeAnalysis {
            quality: RhymeQuality::Eye,
            rhyme_type: rtype,
            shared: common_suffix(&n1, &n2),
            note: Some("eye rhyme — looks alike, sounds different (pronouncing dictionary)".into()),
        };
    }
    // Stressed vowels agree, codas differ → near.
    let v1 = p1.rhyme_key.split_whitespace().next().unwrap_or("");
    let v2 = p2.rhyme_key.split_whitespace().next().unwrap_or("");
    if !v1.is_empty() && v1 == v2 {
        return RhymeAnalysis {
            quality: RhymeQuality::Near,
            rhyme_type: rtype,
            shared: String::new(),
            note: Some("near rhyme — same vowel, different ending (pronouncing dictionary)".into()),
        };
    }
    RhymeAnalysis { quality: RhymeQuality::None, rhyme_type: rtype, shared: String::new(), note: None }
}

/// Rhyme type from a pronunciation: syllables after the primary stress →
/// masculine (0), feminine (1), dactylic (≥2).
fn rhyme_type_from_pron(p: &crate::poetry::phonemes::Pron) -> RhymeType {
    match p.syllables.saturating_sub(1).saturating_sub(p.stress) {
        0 => RhymeType::Masculine,
        1 => RhymeType::Feminine,
        _ => RhymeType::Dactylic,
    }
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

    // D2 — French masculine/feminine is an orthographic distinction (a mute final
    // -e), NOT a stress-position count: French stress is phrase-final, so the
    // count below would call every French word masculine. Classify by the ending.
    let rtype = if matches!(lang, ProseLanguage::Fr) {
        french_rhyme_type(&clean)
    } else {
        match n - 1 - stressed {
            0 => RhymeType::Masculine,
            1 => RhymeType::Feminine,
            _ => RhymeType::Dactylic,
        }
    };
    (tail.to_lowercase(), rtype)
}

/// French rhyme gender from the spelling: a mute final `-e` (including `-es` and
/// the 3rd-person-plural `-ent`) is a **feminine** rhyme; anything else —
/// including an accented final `-é`/`-è` — is masculine. (The `-ent` of a noun
/// like *argent* is voiced, but distinguishing that needs part-of-speech; the
/// verb-ending convention is the standard prosodic reading.) French has no
/// dactylic category.
fn french_rhyme_type(word: &str) -> RhymeType {
    let w = word
        .trim_end_matches(|c: char| !c.is_alphabetic())
        .to_lowercase();
    if w.ends_with("ent") || w.ends_with("es") || w.ends_with('e') {
        RhymeType::Feminine
    } else {
        RhymeType::Masculine
    }
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
    fn french_rhyme_gender_is_orthographic_not_masculine_by_default() {
        // D2 — a mute final -e is a feminine rhyme; an accented / consonant ending
        // is masculine. Previously every French word came out masculine.
        let table_fable = analyse_rhyme("table", "fable", Fr);
        assert_eq!(table_fable.rhyme_type, RhymeType::Feminine, "mute -e → feminine");
        let pensee_aimee = analyse_rhyme("pensée", "aimée", Fr);
        assert_eq!(pensee_aimee.rhyme_type, RhymeType::Feminine, "-ée → feminine");
        let chantent = analyse_rhyme("chantent", "portent", Fr);
        assert_eq!(chantent.rhyme_type, RhymeType::Feminine, "verb -ent → feminine");
        // Accented / consonant endings stay masculine.
        let cafe_ete = analyse_rhyme("café", "été", Fr);
        assert_eq!(cafe_ete.rhyme_type, RhymeType::Masculine, "accented -é → masculine");
        let amour_toujours = analyse_rhyme("amour", "toujours", Fr);
        assert_eq!(amour_toujours.rhyme_type, RhymeType::Masculine, "consonant → masculine");
    }

    #[test]
    fn english_phoneme_dictionary_catches_the_eye_rhyme() {
        // POEM-PHON: with a pronouncing dictionary installed, `love`/`move` —
        // which the spelling heuristic calls a perfect rhyme — is correctly an
        // *eye* rhyme (same spelling tail `-ove`, different sound AH V vs UW V).
        let dict = crate::poetry::phonemes::parse(
            "LOVE  L AH1 V\n\
             MOVE  M UW1 V\n\
             DAY  D EY1\n\
             MAY  M EY1\n\
             MOTHER  M AH1 DH ER0\n\
             BROTHER  B R AH1 DH ER0\n",
        );
        let d = Some(&dict);

        let love_move = analyse_rhyme_with("love", "move", En, d);
        assert_eq!(love_move.quality, RhymeQuality::Eye, "love/move must be an eye rhyme");

        // A true phonetic rhyme still reads perfect, with the right type.
        let day_may = analyse_rhyme_with("day", "may", En, d);
        assert_eq!(day_may.quality, RhymeQuality::Perfect);
        assert_eq!(day_may.rhyme_type, RhymeType::Masculine);

        let mother_brother = analyse_rhyme_with("mother", "brother", En, d);
        assert_eq!(mother_brother.quality, RhymeQuality::Perfect);
        assert_eq!(mother_brother.rhyme_type, RhymeType::Feminine);

        // Out-of-dictionary words fall back to the orthographic heuristic (no panic).
        let oov = analyse_rhyme_with("zzykx", "qwbrs", En, d);
        assert!(matches!(oov.quality, RhymeQuality::None | RhymeQuality::Near));
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
