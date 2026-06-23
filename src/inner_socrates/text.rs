//! Small, dependency-free text helpers for the deterministic Fast track:
//! sentence splitting, word counts, opening words, and dialogue-segment counting.
//! Deliberately simple — the Fast track trades a little precision for speed and
//! zero dependencies; the Slow track (LLM) handles what these miss.

/// Split prose into trimmed, non-empty sentences on `.` / `!` / `?`.
pub fn sentences(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0usize;
    for (i, c) in text.char_indices() {
        if matches!(c, '.' | '!' | '?') {
            let end = i + c.len_utf8();
            let seg = text[start..end].trim();
            if !seg.is_empty() {
                out.push(seg);
            }
            start = end;
        }
    }
    let tail = text[start..].trim();
    if !tail.is_empty() {
        out.push(tail);
    }
    out
}

/// Whitespace-delimited word count.
pub fn word_count(s: &str) -> usize {
    s.split_whitespace().count()
}

/// The lowercased first word of a sentence, stripped of leading/trailing
/// punctuation (so `"Where` and `Where,` both yield `where`). `None` if empty.
pub fn first_word(s: &str) -> Option<String> {
    s.split_whitespace().next().map(|w| {
        w.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
    }).filter(|w| !w.is_empty())
}

/// A rough count of spoken segments — straight `"…"` pairs plus opening smart
/// quotes `\u{201c}`. Enough to tell "a run of dialogue" from ordinary prose.
pub fn dialogue_segment_count(text: &str) -> usize {
    let straight = text.matches('"').count() / 2;
    let smart = text.matches('\u{201c}').count();
    straight + smart
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_sentences() {
        let s = sentences("One. Two!  Three?  ");
        assert_eq!(s, vec!["One.", "Two!", "Three?"]);
        assert!(sentences("   ").is_empty());
        assert_eq!(sentences("No terminator").len(), 1);
    }

    #[test]
    fn counts_words_and_first_word() {
        assert_eq!(word_count("the regent had to act"), 5);
        assert_eq!(first_word("\u{201c}Where,\u{201d} she asked.").as_deref(), Some("where"));
        assert_eq!(first_word("   "), None);
    }

    #[test]
    fn counts_dialogue_segments() {
        assert_eq!(dialogue_segment_count("\"Here.\" \"There.\" \"Everywhere.\""), 3);
        assert_eq!(dialogue_segment_count("no quotes here"), 0);
    }
}
