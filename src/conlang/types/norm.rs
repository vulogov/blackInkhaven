//! Unicode normalisation for phonological strings (LANG-BUGFIX-1, LB-3 / LB-9).
//!
//! Every string the phonology compares — inventory graphemes and IPA, class
//! members, rule atoms, lexicon headwords and user input — passes through
//! [`nfc`] (canonical composition) so a precomposed `ã` (U+00E3) and the
//! decomposed `a` + U+0303 are the same segment. [`fold`] adds Unicode
//! case-folding on top for the segmenter, which treats `Pata` and `pata` as
//! the same word (LB-9). [`describe_segment`] renders a segment for an error
//! message, appending the codepoint when the segment is (or contains) a
//! combining mark that would otherwise print as a bare accent.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Canonical composition (NFC) of `s`.
pub fn nfc(s: &str) -> String {
    s.nfc().collect()
}

/// Lowercase + NFC — the key the segmenter matches on.
pub fn fold(s: &str) -> String {
    s.to_lowercase().nfc().collect()
}

/// `s` with every combining mark removed, recomposed: the *base letters* of
/// a string (`ą́` → `a`, `é` → `e`, `ŋ` → `ŋ`). Used where a diacritic is a
/// detail rather than a distinction — alphabet-bucket derivation and the
/// alphabet check of a headword (LB-4 / LB-6).
pub fn strip_marks(s: &str) -> String {
    s.nfd().filter(|c| !is_combining_mark(*c)).nfc().collect()
}

/// A file-system-safe slug of a user-facing name (LB-11): case-folded NFC,
/// restricted to `[a-z0-9._-]` with every other run collapsed to `-`, no
/// leading / trailing `-` or `.`, at most 64 characters, never empty
/// (`unnamed`). `Old Avesha/Sea` → `old-avesha-sea`; `../x` → `x`.
pub fn fs_safe_name(name: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in strip_marks(&fold(name)).chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '.' | '_') {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    let trimmed: String = out.trim_matches(|c| c == '-' || c == '.').chars().take(64).collect();
    let trimmed = trimmed.trim_matches(|c| c == '-' || c == '.').to_string();
    if trimmed.is_empty() { "unnamed".to_string() } else { trimmed }
}

/// NFC applied in place.
pub fn nfc_in_place(s: &mut String) {
    use unicode_normalization::{is_nfc_quick, IsNormalized};
    if !matches!(is_nfc_quick(s.chars()), IsNormalized::Yes) {
        *s = nfc(s);
    }
}

/// A segment for an error message: `ã`, or `̃ (U+0303)` when it contains a
/// combining mark (which would otherwise render as a bare accent).
pub fn describe_segment(seg: &str) -> String {
    if seg.chars().any(is_combining_mark) {
        let cps: Vec<String> = seg.chars().map(|c| format!("U+{:04X}", c as u32)).collect();
        format!("{seg} ({})", cps.join(" "))
    } else {
        seg.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nfc_composes_a_decomposed_tilde() {
        assert_eq!(nfc("a\u{0303}ta"), "\u{00E3}ta");
        assert_eq!(nfc("\u{00E3}ta"), "\u{00E3}ta");
    }

    #[test]
    fn fold_lowercases_then_composes() {
        assert_eq!(fold("PATA"), "pata");
        assert_eq!(fold("E\u{0301}le"), "\u{00E9}le");
    }

    #[test]
    fn fs_safe_name_slugs_to_a_single_path_component() {
        assert_eq!(fs_safe_name("Avesha"), "avesha");
        assert_eq!(fs_safe_name("Old Avesha/Sea"), "old-avesha-sea");
        assert_eq!(fs_safe_name("../x"), "x");
        assert_eq!(fs_safe_name("a.b_c"), "a.b_c");
        assert_eq!(fs_safe_name("\u{00C9}l\u{00E9}"), "ele");
        assert_eq!(fs_safe_name("///"), "unnamed");
        assert_eq!(fs_safe_name(&"x".repeat(100)).chars().count(), 64);
        assert!(!fs_safe_name("k/../../etc").contains('/'));
    }

    #[test]
    fn strip_marks_keeps_base_letters_only() {
        assert_eq!(strip_marks("\u{0105}\u{0301}tal"), "atal"); // ą́tal
        assert_eq!(strip_marks("\u{00E9}le"), "ele");
        assert_eq!(strip_marks("\u{014B}a"), "\u{014B}a"); // ŋ has no decomposition
        assert_eq!(strip_marks("k\u{0303}a"), "ka");
    }

    #[test]
    fn nfc_in_place_only_rewrites_when_needed() {
        let mut s = String::from("tʰa");
        let before = s.as_ptr();
        nfc_in_place(&mut s);
        assert_eq!(s.as_ptr(), before);
        let mut d = String::from("a\u{0303}");
        nfc_in_place(&mut d);
        assert_eq!(d, "\u{00E3}");
    }

    #[test]
    fn describe_segment_names_a_combining_mark() {
        assert_eq!(describe_segment("k"), "k");
        assert_eq!(describe_segment("\u{0303}"), "\u{0303} (U+0303)");
        // A composed character carries no combining mark and prints plainly.
        assert_eq!(describe_segment("\u{00E3}"), "\u{00E3}");
    }
}
