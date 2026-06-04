//! Shared typst-prose helpers for the reader-exports.
//!
//! The ePub and audiobook exporters both pre-process a
//! paragraph body the same way before their format-
//! specific rendering: drop a leading `= …` org-title
//! heading, then split the remainder into blank-line-
//! separated blocks.  Before 1.2.20 each carried its own
//! identical copy; this is now the one home.
//!
//! The book-assembly path (`crate::assemble`) keeps its
//! own heading strip — it matches any `=`-prefixed line
//! (including `==` section headings) and is whitespace-
//! aware, because it serves typst compilation rather than
//! reader-export prose.

/// Drop a leading level-1 `= …` heading (the org chapter
/// title) and the blank lines that follow it.  A `==`
/// subheading is kept — it's content, not the title.
/// Bodies without a leading `= ` heading pass through
/// unchanged.
pub(crate) fn strip_leading_heading(body: &str) -> String {
    let mut lines = body.lines();
    if let Some(first) = lines.clone().next() {
        if first.trim_start().starts_with("= ") {
            lines.next();
            let rest: Vec<&str> = lines.collect();
            return rest.join("\n").trim_start_matches('\n').to_string();
        }
    }
    body.to_string()
}

/// Split prose into blank-line-separated blocks, each a
/// paragraph.  Leading/trailing blanks and runs of blank
/// lines collapse to block boundaries; whitespace-only
/// blocks are dropped.
pub(crate) fn split_blocks(s: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut cur = String::new();
    for line in s.lines() {
        if line.trim().is_empty() {
            if !cur.trim().is_empty() {
                blocks.push(std::mem::take(&mut cur));
            }
        } else {
            if !cur.is_empty() {
                cur.push('\n');
            }
            cur.push_str(line);
        }
    }
    if !cur.trim().is_empty() {
        blocks.push(cur);
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_drops_leading_equals_heading() {
        let body = "= 001. Approach\n\nHelena paused.";
        assert_eq!(strip_leading_heading(body), "Helena paused.");
    }

    #[test]
    fn strip_keeps_body_without_heading() {
        let body = "Just prose here.";
        assert_eq!(strip_leading_heading(body), "Just prose here.");
    }

    #[test]
    fn strip_keeps_subheadings() {
        // `==` is a subheading, not the org title — keep it.
        let body = "== A scene\n\nProse.";
        assert_eq!(strip_leading_heading(body), "== A scene\n\nProse.");
    }

    #[test]
    fn split_blocks_separates_on_blank_lines() {
        let s = "Para one.\nstill one.\n\n\nPara two.\n";
        assert_eq!(
            split_blocks(s),
            vec![
                "Para one.\nstill one.".to_string(),
                "Para two.".to_string()
            ]
        );
    }

    #[test]
    fn split_blocks_empty_input_is_empty() {
        assert!(split_blocks("").is_empty());
        assert!(split_blocks("   \n\n  ").is_empty());
    }
}
