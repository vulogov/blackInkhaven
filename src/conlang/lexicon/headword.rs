//! Headword validation and alphabet bucketing (LANG-BUGFIX-1, LB-4 / LB-5 /
//! LB-6).
//!
//! Every door a headword enters through — `language add-word`, the CSV
//! import pre-flight, the Toolbox / PolyGlot importer and the TUI tree-pane
//! `+` chord — runs the same [`validate_headword`], so a word one door
//! accepts the others accept too, and `export` → `import` of a dictionary is
//! a no-op.
//!
//! Two checks, one authority:
//!
//! * **Shape** ([`validate_shape`]) — 1–64 characters, no control characters.
//!   This is structural (the headword becomes a node title and a file name)
//!   and is never bypassed.
//! * **Spelling** — when the language declares a `Phonology` block with an
//!   inventory, every segment of the word must be a declared phoneme
//!   grapheme (greedy longest-match via `Phonology::segment`). Only when no
//!   inventory is declared does `Meta/overview.alphabet` act as the
//!   whitelist, matched by *letter* (an entry such as `"Ch ch"` declares a
//!   digraph) with diacritics folded to their base letter, so the default
//!   A–Z alphabet accepts `éle` but not `ŋa`. The alphabet always drives
//!   bucket derivation ([`Alphabet::bucket_for`]); `--force` is the escape
//!   hatch on the CLI doors.

use std::fmt;

use unicode_segmentation::UnicodeSegmentation;

use crate::conlang::types::norm::{describe_segment, fold, strip_marks};
use crate::conlang::types::Phonology;

/// Longest headword accepted, in characters.
pub const MAX_HEADWORD_CHARS: usize = 64;

/// Why a headword was refused. `Display` reads as a predicate on the word
/// (“`ŋa` contains `ŋ`, …”), so callers prefix the word themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadwordError {
    Empty,
    TooLong(usize),
    ControlChar,
    /// A segment `Phonology::segment` could not map to a declared phoneme.
    UnknownSegment(String),
    /// A letter no `Meta/overview.alphabet` entry declares.
    UnknownLetter(String),
}

impl fmt::Display for HeadwordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "is empty — a headword must be 1–{MAX_HEADWORD_CHARS} characters"),
            Self::TooLong(n) => write!(
                f,
                "is {n} characters long — a headword must be 1–{MAX_HEADWORD_CHARS} characters"
            ),
            Self::ControlChar => write!(f, "contains a control character"),
            Self::UnknownSegment(s) => write!(
                f,
                "contains `{s}`, which is not a phoneme of the declared Phonology inventory"
            ),
            Self::UnknownLetter(l) => {
                write!(f, "contains `{l}`, which is not in Meta/overview.alphabet")
            }
        }
    }
}

impl std::error::Error for HeadwordError {}

/// Structural check only: non-empty after trimming, at most
/// [`MAX_HEADWORD_CHARS`] characters, no control characters.
pub fn validate_shape(word: &str) -> Result<(), HeadwordError> {
    let word = word.trim();
    if word.is_empty() {
        return Err(HeadwordError::Empty);
    }
    let n = word.chars().count();
    if n > MAX_HEADWORD_CHARS {
        return Err(HeadwordError::TooLong(n));
    }
    if word.chars().any(char::is_control) {
        return Err(HeadwordError::ControlChar);
    }
    Ok(())
}

/// The letter runs of a headword: maximal spans with whitespace and ASCII
/// punctuation removed, so `ta-sora` and `k'ai` validate as `ta`, `sora`
/// and `k`, `ai`.
pub fn letter_runs(word: &str) -> impl Iterator<Item = &str> {
    word.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|run| !run.is_empty())
}

/// Full check: shape, then spelling against the phonology inventory when
/// one is declared, else against the alphabet when one is declared.
pub fn validate_headword(
    word: &str,
    alphabet: &[String],
    phon: Option<&Phonology>,
) -> Result<(), HeadwordError> {
    validate_shape(word)?;
    if let Some(phon) = phon.filter(|p| !p.phonemes.is_empty()) {
        for run in letter_runs(word) {
            if let Some(bad) = phon
                .segment(run)
                .into_iter()
                .find(|seg| phon.phoneme(seg).is_none())
            {
                return Err(HeadwordError::UnknownSegment(describe_segment(&bad)));
            }
        }
        return Ok(());
    }
    if !alphabet.is_empty() {
        if let Some(bad) = Alphabet::new(alphabet).first_unknown(word) {
            return Err(HeadwordError::UnknownLetter(describe_segment(&bad)));
        }
    }
    Ok(())
}

/// `Meta/overview.alphabet` compiled for longest-match lookups.
///
/// Each entry contributes its whitespace-separated tokens as letter forms
/// (`"Ch ch"` → `ch`; `"Aa"` → `aa`) plus each of its characters
/// (`"Aa"` → `a`; `"Aleph"` → `a`, `l`, `e`, `p`, `h`), so the paired-case,
/// letter-name and digraph conventions the scaffold documents all work.
/// Forms are compared after NFC + case-folding, first exactly and then with
/// diacritics stripped on both sides, so an alphabet that declares `Ą`
/// keeps `ą` apart from `a`, while the default A–Z accepts `é` under `E`.
pub struct Alphabet<'a> {
    entries: &'a [String],
    /// `(grapheme clusters of the folded form, entry index)`, longest first;
    /// among equal lengths a whole token (`"C"`) outranks a character taken
    /// from a longer token (`"Ch ch"`), so `cata` files under `C`.
    forms: Vec<(Vec<String>, usize)>,
}

impl<'a> Alphabet<'a> {
    pub fn new(entries: &'a [String]) -> Self {
        // (form, entry index, rank: 0 = whole token, 1 = character of a token)
        let mut forms: Vec<(Vec<String>, usize, u8)> = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let folded = fold(entry);
            for token in folded.split_whitespace() {
                forms.push((clusters(token), i, 0));
                for c in token.chars() {
                    forms.push((clusters(&c.to_string()), i, 1));
                }
            }
        }
        forms.retain(|(f, _, _)| !f.is_empty());
        forms.sort_by_key(|(f, _, rank)| (std::cmp::Reverse(f.len()), *rank));
        let mut forms: Vec<(Vec<String>, usize)> =
            forms.into_iter().map(|(f, i, _)| (f, i)).collect();
        forms.dedup();
        Self { entries, forms }
    }

    /// The alphabet entry the word files under: the longest letter form
    /// matching the start of its first letter run.
    pub fn bucket_for(&self, word: &str) -> Option<&'a str> {
        let run = letter_runs(word).next()?;
        let run = clusters(&fold(run));
        self.match_at(&run, 0).map(|(idx, _)| self.entries[idx].as_str())
    }

    /// The first letter (grapheme cluster, folded) of `word` that no form
    /// covers, scanning every letter run with longest-match.
    pub fn first_unknown(&self, word: &str) -> Option<String> {
        for run in letter_runs(word) {
            let run = clusters(&fold(run));
            let mut i = 0;
            while i < run.len() {
                match self.match_at(&run, i) {
                    Some((_, n)) => i += n,
                    None => return Some(run[i].clone()),
                }
            }
        }
        None
    }

    /// `(entry index, clusters consumed)` of the longest form matching
    /// `run[at..]`, exact first and then diacritic-insensitive.
    fn match_at(&self, run: &[String], at: usize) -> Option<(usize, usize)> {
        let rest = &run[at..];
        let exact = self
            .forms
            .iter()
            .find(|(f, _)| rest.len() >= f.len() && rest[..f.len()] == f[..]);
        if let Some((f, idx)) = exact {
            return Some((*idx, f.len()));
        }
        let base: Vec<String> = rest.iter().map(|c| strip_marks(c)).collect();
        self.forms
            .iter()
            .find(|(f, _)| {
                base.len() >= f.len()
                    && f.iter().zip(&base).all(|(a, b)| !b.is_empty() && strip_marks(a) == *b)
            })
            .map(|(f, idx)| (*idx, f.len()))
    }
}

/// Bucket name when no alphabet entry covers the word: the base letter of
/// its first grapheme cluster, uppercased (`ą́tal` → `A`, `ŋa` → `Ŋ`), so a
/// diacritic never spawns a second bucket with a colliding slug (LB-6).
pub fn fallback_bucket(word: &str) -> Option<String> {
    let run = letter_runs(word).next()?;
    let first = run.graphemes(true).next()?;
    let base = strip_marks(first);
    let base = if base.is_empty() { first.to_string() } else { base };
    Some(base.to_uppercase())
}

fn clusters(s: &str) -> Vec<String> {
    s.graphemes(true).map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    fn phon() -> Phonology {
        Phonology::from_hjson(
            r#"{
            phonemes: [
                { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
                { ipa: "ŋ", kind: "consonant" }, { ipa: "tʃ", romanize: "ch", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "e", kind: "vowel" }
            ]
        }"#,
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn shape_rejects_empty_oversized_and_control() {
        assert_eq!(validate_shape("   "), Err(HeadwordError::Empty));
        assert_eq!(validate_shape(&"a".repeat(65)), Err(HeadwordError::TooLong(65)));
        assert_eq!(validate_shape(&"a".repeat(64)), Ok(()));
        assert_eq!(validate_shape("pa\nta"), Err(HeadwordError::ControlChar));
        assert_eq!(validate_shape("pata"), Ok(()));
        assert_eq!(
            HeadwordError::TooLong(600).to_string(),
            "is 600 characters long — a headword must be 1–64 characters"
        );
    }

    #[test]
    fn phonology_is_the_authority_when_declared() {
        let p = phon();
        let az: Vec<String> = ('A'..='Z').map(|c| c.to_string()).collect();
        // `ŋ` is a phoneme but not an A–Z letter: accepted.
        assert_eq!(validate_headword("ŋata", &az, Some(&p)), Ok(()));
        // Digraph grapheme + case/NFC insensitivity.
        assert_eq!(validate_headword("Chata", &az, Some(&p)), Ok(()));
        // A letter outside the inventory names the segment.
        assert_eq!(
            validate_headword("pazo", &az, Some(&p)),
            Err(HeadwordError::UnknownSegment("z".into()))
        );
        // A bare combining mark is named by codepoint, never printed bare.
        assert_eq!(
            validate_headword("t\u{0303}a", &[], Some(&p)),
            Err(HeadwordError::UnknownSegment("\u{0303} (U+0303)".into()))
        );
        // Hyphen / apostrophe / space separate runs and are not segments.
        assert_eq!(validate_headword("ta-pa 'ate", &az, Some(&p)), Ok(()));
    }

    #[test]
    fn alphabet_applies_only_without_an_inventory() {
        let az: Vec<String> = ('A'..='Z').map(|c| c.to_string()).collect();
        assert_eq!(validate_headword("atal", &az, None), Ok(()));
        // Diacritics fold to the base letter under the default alphabet.
        assert_eq!(validate_headword("\u{00E9}le", &az, None), Ok(()));
        assert_eq!(validate_headword("e\u{0301}le", &az, None), Ok(()));
        assert_eq!(validate_headword("\u{0105}\u{0301}", &az, None), Ok(()));
        assert_eq!(
            validate_headword("\u{014B}a", &az, None),
            Err(HeadwordError::UnknownLetter("\u{014B}".into()))
        );
        // No alphabet, no inventory: only the shape is checked.
        assert_eq!(validate_headword("\u{014B}a\u{0294}", &[], None), Ok(()));
        let empty = Phonology::default();
        assert_eq!(validate_headword("\u{014B}a", &[], Some(&empty)), Ok(()));
    }

    #[test]
    fn alphabet_matches_digraphs_longest_first() {
        let alpha = strings(&["A", "Ch ch", "C", "H"]);
        let a = Alphabet::new(&alpha);
        assert_eq!(a.bucket_for("chata"), Some("Ch ch"));
        assert_eq!(a.bucket_for("cata"), Some("C"));
        assert_eq!(a.bucket_for("hac"), Some("H"));
        assert_eq!(a.first_unknown("chach"), None);
        assert_eq!(a.first_unknown("chaz"), Some("z".into()));
        // Paired-case and letter-name conventions.
        let paired = strings(&["Aa", "Bb"]);
        assert_eq!(Alphabet::new(&paired).bucket_for("Bran"), Some("Bb"));
        assert_eq!(Alphabet::new(&paired).first_unknown("aBz"), Some("z".into()));
        let names = strings(&["Aleph", "Beth"]);
        assert_eq!(Alphabet::new(&names).bucket_for("Avraham"), Some("Aleph"));
        assert_eq!(Alphabet::new(&names).bucket_for("Beriah"), Some("Beth"));
        assert_eq!(Alphabet::new(&names).bucket_for("zzz"), None);
        let cyr = strings(&["А", "Б"]);
        assert_eq!(Alphabet::new(&cyr).first_unknown("аб"), None);
        assert_eq!(Alphabet::new(&cyr).first_unknown("абя"), Some("я".into()));
    }

    #[test]
    fn declared_diacritic_letters_stay_distinct() {
        let polish = strings(&["A", "Ą", "B"]);
        let a = Alphabet::new(&polish);
        assert_eq!(a.bucket_for("\u{0105}b"), Some("Ą"));
        assert_eq!(a.bucket_for("ab"), Some("A"));
        // An undeclared diacritic still folds to its base letter.
        assert_eq!(a.bucket_for("\u{00E1}b"), Some("A"));
    }

    #[test]
    fn fallback_bucket_uses_the_base_letter() {
        assert_eq!(fallback_bucket("atal"), Some("A".into()));
        assert_eq!(fallback_bucket("\u{0105}\u{0301}tal"), Some("A".into()));
        assert_eq!(fallback_bucket("e\u{0301}le"), Some("E".into()));
        assert_eq!(fallback_bucket("\u{014B}a"), Some("\u{014A}".into()));
        assert_eq!(fallback_bucket("ярости"), Some("Я".into()));
        assert_eq!(fallback_bucket("  zeta"), Some("Z".into()));
        assert_eq!(fallback_bucket("'ata"), Some("A".into()));
        assert_eq!(fallback_bucket("   "), None);
        assert_eq!(fallback_bucket(""), None);
    }
}
