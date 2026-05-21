//! Cheap word-count for Typst-flavoured paragraph bodies.
//!
//! The intent is "what a human would count" rather than "tokens
//! a tokeniser sees", so:
//!
//! * Whitespace-separated runs of non-whitespace are words.
//! * `=` heading markers and `#` Typst directives are NOT
//!   counted as words — they're structure, not content.
//! * Lines starting with `//` (typst comments) are skipped.
//!
//! Output is in the same ballpark as Microsoft Word's count for
//! plain prose; minor drift on heavy markup is acceptable for
//! the progress widget.

/// Count words in a paragraph body. See module docs for the
/// rules. Empty / whitespace-only input returns 0.
pub fn count_words(s: &str) -> i64 {
    let mut total: i64 = 0;
    for raw in s.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with("//") {
            continue;
        }
        // Skip lone heading markers like `= Heading` — the
        // leading `=`+ is structure, but the heading text IS
        // content the user wrote, so we count words AFTER the
        // run of `=`s.
        let stripped = strip_heading_prefix(line);
        // Drop `#…(…)` Typst function calls inline. Cheap
        // approximation: chunks that start with `#` and contain
        // `(` are skipped as one token. Plain `#tag` words stay
        // (they're typically inline markup, not directives the
        // user thinks of as "content").
        for tok in stripped.split_whitespace() {
            if tok.starts_with('#') && tok.contains('(') {
                continue;
            }
            total += 1;
        }
    }
    total
}

fn strip_heading_prefix(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() && bytes[i] == b'=' {
        i += 1;
    }
    if i > 0 && i < bytes.len() && bytes[i] == b' ' {
        return &line[i + 1..];
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("   \n  \n"), 0);
    }

    #[test]
    fn plain_prose() {
        assert_eq!(count_words("hello world"), 2);
        assert_eq!(count_words("one two three four five"), 5);
    }

    #[test]
    fn heading_text_counts() {
        // "= Title" → heading marker drops, "Title" counts as 1.
        assert_eq!(count_words("= Chapter one\n\nThe morning was cold."), 6);
    }

    #[test]
    fn typst_directives_dont_count_as_words() {
        // "#image(\"foo.png\")" is structure, not content.
        assert_eq!(count_words("#image(\"foo.png\")"), 0);
        assert_eq!(
            count_words("Before the storm. #image(\"img.png\") After.\n"),
            4
        );
    }

    #[test]
    fn comments_are_skipped() {
        let body = "// TODO: rewrite this paragraph\nThe sun rose.\n";
        assert_eq!(count_words(body), 3);
    }
}
