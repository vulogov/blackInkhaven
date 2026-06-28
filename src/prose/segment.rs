//! NARR-1 — prose sentence segmentation, multilingual.
//!
//! `continuity::split_sentences` is deliberately naive (splits on every `.!?`)
//! and `tui::sentence_rhythm`'s splitter is English-abbreviation-only and
//! TUI-coupled. Voice metrics need abbreviation-aware, ellipsis-aware,
//! dialogue-aware boundaries across all five embedded languages, so NARR-1
//! ships its own splitter here. Heuristic, deterministic, zero-dep.
//!
//! Boundaries: `.`/`!`/`?` (and `…`), except when —
//!   * the `.` follows a known abbreviation or a single-letter initial,
//!   * the run is an ellipsis (`…`, `...`) — treated as non-terminal,
//! and a boundary consumes any trailing closing quote/bracket so a dialogue
//! tag stays with its sentence. Em-dash (`—`) is never a terminator.

use std::collections::HashSet;

use super::ProseLanguage;

/// Abbreviations (lowercase, **without** the trailing dot — they're matched
/// against the word immediately preceding a `.`). Multi-dot forms like `z.b`
/// and `т.д` are stored as they appear before the final dot.
fn abbreviations(lang: &ProseLanguage) -> &'static [&'static str] {
    match lang {
        ProseLanguage::En => &[
            "mr", "mrs", "ms", "dr", "prof", "st", "vs", "etc", "e.g", "i.e",
            "no", "vol", "pp", "fig", "ch", "jr", "sr",
        ],
        ProseLanguage::Ru => &[
            "т.д", "т.е", "т.п", "др", "см", "рис", "стр", "гл", "г", "обл",
            "пр", "напр",
        ],
        ProseLanguage::De => &[
            "usw", "z.b", "d.h", "bzw", "ca", "evtl", "ggf", "u.a", "vgl", "nr",
            "abb", "bspw", "etc",
        ],
        ProseLanguage::Fr => &[
            "etc", "cf", "pp", "ex", "env", "no", "vol", "st", "ste", "mme",
            "dr", "m", "p",
        ],
        ProseLanguage::Es => &[
            "etc", "pp", "cap", "núm", "sr", "sra", "dr", "vol", "ej", "pág",
            "p", "cf",
        ],
        ProseLanguage::Other(_) => &["mr", "mrs", "dr", "etc", "e.g", "i.e"],
    }
}

/// The last whitespace-delimited token of `chars` (may contain internal dots),
/// lowercased.
fn trailing_word(chars: &[char]) -> String {
    let mut word: Vec<char> = Vec::new();
    for &c in chars.iter().rev() {
        if c.is_whitespace() {
            break;
        }
        word.push(c);
    }
    word.reverse();
    word.iter().collect::<String>().to_lowercase()
}

/// Split `text` into sentences for `lang`.
pub(crate) fn split_sentences(text: &str, lang: &ProseLanguage) -> Vec<String> {
    let abbr: HashSet<&str> = abbreviations(lang).iter().copied().collect();
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let mut out: Vec<String> = Vec::new();
    let mut start = 0usize;
    let mut i = 0usize;

    while i < n {
        let c = chars[i];
        if c == '.' || c == '!' || c == '?' || c == '…' {
            // Collapse a run of terminator chars (`?!`, `...`, `…`).
            let mut j = i;
            while j < n && matches!(chars[j], '.' | '!' | '?' | '…') {
                j += 1;
            }
            let run_len = j - i;
            let run: String = chars[i..j].iter().collect();
            let is_ellipsis = run.contains('…') || run_len >= 3 || run == "..";

            let is_abbrev = c == '.' && run_len == 1 && {
                let w = trailing_word(&chars[start..i]);
                // word minus a leading-dot artefact, e.g. "e.g" stays "e.g"
                let bare = w.trim_end_matches('.');
                abbr.contains(bare) || is_initial(bare)
            };

            if is_ellipsis || is_abbrev {
                i = j;
                continue;
            }

            // Pull in trailing closing quotes / brackets so a dialogue tag isn't
            // orphaned onto the next sentence.
            let mut k = j;
            while k < n && matches!(chars[k], '"' | '»' | '”' | '’' | '\'' | ')' | ']') {
                k += 1;
            }
            // Dialogue / mid-clause continuation: a terminator followed (after any
            // closing quote + whitespace) by a LOWERCASE word is a dialogue tag or
            // aside, not a real boundary — `"Run!" she cried.` is one sentence.
            // Sentences proper begin uppercase / with new opening punctuation.
            let mut m = k;
            while m < n && chars[m].is_whitespace() {
                m += 1;
            }
            if m < n && chars[m].is_alphabetic() && chars[m].is_lowercase() {
                i = k;
                continue;
            }
            let sent: String = chars[start..k].iter().collect::<String>().trim().to_string();
            if !sent.is_empty() {
                out.push(sent);
            }
            start = k;
            i = k;
            continue;
        }
        i += 1;
    }

    let tail: String = chars[start..].iter().collect::<String>().trim().to_string();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// A single-letter token (an initial like `J.` / `г.`) — never a boundary.
fn is_initial(word: &str) -> bool {
    word.chars().count() == 1 && word.chars().all(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage::*;

    #[test]
    fn basic_split() {
        let s = split_sentences("The sun rose. It was warm! Was it? Yes.", &En);
        assert_eq!(s.len(), 4);
        assert_eq!(s[0], "The sun rose.");
    }

    #[test]
    fn abbreviations_do_not_split() {
        // EN: Dr. / e.g. / St. stay inline.
        let s = split_sentences("Dr. Smith met Mr. Vane at St. Paul, e.g. there. Then left.", &En);
        assert_eq!(s.len(), 2, "{s:?}");
    }

    #[test]
    fn de_and_ru_abbreviations() {
        // z.B. stays inline; the real break is after "Brot." (→ uppercase "Dann").
        let de = split_sentences("Er kaufte z.B. Brot. Dann ging er.", &De);
        assert_eq!(de.len(), 2, "{de:?}");
        // т.д. stays inline; the real break is after "магазине." (→ uppercase "Потом").
        let ru = split_sentences("Купил хлеб, и т.д., в магазине. Потом ушёл.", &Ru);
        assert_eq!(ru.len(), 2, "{ru:?}");
    }

    #[test]
    fn ellipsis_is_not_a_boundary() {
        let s = split_sentences("She paused… then spoke. Done.", &En);
        assert_eq!(s.len(), 2, "{s:?}");
        let dots = split_sentences("Wait... what now. Ok.", &En);
        assert_eq!(dots.len(), 2, "{dots:?}");
    }

    #[test]
    fn dialogue_closing_quote_stays_with_sentence() {
        let s = split_sentences("\"Run!\" she cried. He ran.", &En);
        assert_eq!(s.len(), 2, "{s:?}");
        // The `?!` cluster is one boundary.
        let q = split_sentences("Really?! I doubt it.", &En);
        assert_eq!(q.len(), 2, "{q:?}");
    }

    #[test]
    fn initials_do_not_split() {
        let s = split_sentences("J. R. R. Tolkien wrote it. We read it.", &En);
        assert_eq!(s.len(), 2, "{s:?}");
    }
}
