//! Pure string / time / wrap helpers used by the TUI render
//! paths. Extracted from `tui::app` in the 1.2.7 cycle to
//! tame the 26K-line monolith. All free functions; no
//! reference to App state or ratatui types beyond what the
//! signatures show.

/// Maximum number of characters a node title is allowed to
/// occupy in the tree pane. Beyond that the title is truncated
/// with an ellipsis so trailing pips (status / progress / tag
/// chips) stay visible on a single row. Currently consulted
/// only by [`extract_first_sentence`] for the auto-rename
/// length cap; the tree itself uses real wrapping (1.2.6+) so
/// the constant no longer drives a hard truncate there.
pub(super) const TITLE_MAX_DISPLAY: usize = 60;

/// Placeholder title for paragraphs added without one. The
/// next save replaces it with the first sentence of the body.
pub(super) const PARAGRAPH_PLACEHOLDER_TITLE: &str = "Untitled paragraph";

/// Greedy word-wrap (`text` over a `width`-wide column),
/// falling back to char-break when a single word doesn't fit.
/// Empty input returns one empty line; zero width returns the
/// text unchanged. Used by the tree pane's hanging-indent
/// renderer to wrap long node titles.
pub(super) fn wrap_words_or_chars(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_owned()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;
    for word in text.split_whitespace() {
        let word_w = word.chars().count();
        if word_w > width {
            // Word doesn't fit even on a line of its own —
            // flush whatever's pending, then hard-break by
            // character.
            if !current.is_empty() {
                lines.push(std::mem::take(&mut current));
            }
            let mut buf = String::new();
            let mut bw = 0;
            for c in word.chars() {
                if bw == width {
                    lines.push(std::mem::take(&mut buf));
                    bw = 0;
                }
                buf.push(c);
                bw += 1;
            }
            current = buf;
            current_w = bw;
        } else {
            let needed = if current.is_empty() {
                word_w
            } else {
                current_w + 1 + word_w
            };
            if needed > width {
                lines.push(std::mem::take(&mut current));
                current = word.to_owned();
                current_w = word_w;
            } else {
                if !current.is_empty() {
                    current.push(' ');
                    current_w += 1;
                }
                current.push_str(word);
                current_w += word_w;
            }
        }
    }
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

/// 1.2.6+ — truncate a track label to `max_chars`, appending
/// `…` when the value was actually shortened. Returns the
/// original on short strings.
pub(super) fn truncate_label(label: &str, max_chars: usize) -> String {
    if label.chars().count() <= max_chars {
        return label.to_owned();
    }
    let take = max_chars.saturating_sub(1);
    let mut s: String = label.chars().take(take).collect();
    s.push('…');
    s
}

/// Format an "active time" duration (seconds since the editor
/// last took focus) as `Xh YYm` / `Nm` / `0m`. Stays compact
/// for the status bar's right-side chip.
pub(super) fn format_active_duration(seconds: i64) -> String {
    let s = seconds.max(0);
    if s < 60 {
        return "0m".to_string();
    }
    let minutes = s / 60;
    if minutes < 60 {
        return format!("{minutes}m");
    }
    let h = minutes / 60;
    let m = minutes % 60;
    format!("{h}h {m:02}m")
}

/// Estimated reading time at 250 wpm. Single-letter prefix
/// (`~`) so the status bar reads it as an estimate, not a
/// hard count. Matches the Ctrl+B I info-panel rounding so
/// both surfaces agree.
pub(super) fn format_reading_time(words: usize) -> String {
    if words == 0 {
        return "<1m".to_string();
    }
    let minutes = ((words as f64) / 250.0).ceil() as u64;
    if minutes < 60 {
        format!("~{minutes}m")
    } else {
        let h = minutes / 60;
        let m = minutes % 60;
        if m == 0 {
            format!("~{h}h")
        } else {
            format!("~{h}h {m}m")
        }
    }
}

/// Format a `Duration` as a coarse "N units ago" string using
/// only the largest two units (days+hours, hours+minutes,
/// etc.). humantime's default formatter prints every non-zero
/// unit down to nanoseconds, which is too noisy for a "how
/// old is this PDF" read-out.
pub(super) fn format_age_humantime(dur: std::time::Duration) -> String {
    let total_secs = dur.as_secs();
    if total_secs < 60 {
        return format!("{total_secs}s");
    }
    let days = total_secs / 86_400;
    let hours = (total_secs % 86_400) / 3600;
    let minutes = (total_secs % 3600) / 60;
    if days > 0 {
        if hours > 0 {
            format!("{days}d {hours}h")
        } else {
            format!("{days}d")
        }
    } else if hours > 0 {
        if minutes > 0 {
            format!("{hours}h {minutes}m")
        } else {
            format!("{hours}h")
        }
    } else {
        format!("{minutes}m")
    }
}

/// Normalise CRLF / bare-CR / LF line endings into a `Vec<String>`
/// suitable for the tui-textarea's `lines` constructor. Imported
/// text dumps (Windows / DOS / pre-OS-X Mac) survive without
/// vertical-bar control-character renderings.
pub(super) fn body_to_lines(body: &str) -> Vec<String> {
    if body.is_empty() {
        return vec![String::new()];
    }
    // CRLF first so we don't double-split, then any remaining
    // bare CR (pre-OS-X Mac files). After this every line
    // break is one `\n`.
    let normalised = body.replace("\r\n", "\n").replace('\r', "\n");
    normalised.split('\n').map(String::from).collect()
}

/// Extract the first sentence of a paragraph body, used by
/// the rename-to-first-sentence chord and the placeholder
/// title fallback. Strips typst headings (`= …`) and line
/// comments before sentence detection; caps the result at
/// [`TITLE_MAX_DISPLAY`] chars with an ellipsis.
pub(super) fn extract_first_sentence(content: &str) -> Option<String> {
    let prose: String = content
        .lines()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() || t.starts_with("=") || t.starts_with("//") {
                None
            } else {
                Some(t.to_string())
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    if prose.is_empty() {
        return None;
    }

    let chars: Vec<char> = prose.chars().collect();
    let mut end = chars.len();
    for (i, c) in chars.iter().enumerate() {
        if matches!(*c, '.' | '!' | '?') {
            let next_is_space_or_end = i + 1 >= chars.len() || chars[i + 1].is_whitespace();
            if next_is_space_or_end {
                end = i + 1;
                break;
            }
        }
    }
    let sentence: String = chars.iter().take(end).collect();
    let sentence = sentence.trim();
    if sentence.is_empty() {
        return None;
    }

    let s_chars: Vec<char> = sentence.chars().collect();
    if s_chars.len() > TITLE_MAX_DISPLAY {
        let mut out: String = s_chars.iter().take(TITLE_MAX_DISPLAY - 1).collect();
        out.push('…');
        Some(out)
    } else {
        Some(sentence.to_string())
    }
}

/// Pad-right or trim `s` to exactly `width` characters. Treats
/// the input as Unicode (chars, not bytes) so multi-byte
/// glyphs land in the right cell.
pub(super) fn pad_or_trim(s: &str, width: usize) -> String {
    let cs: Vec<char> = s.chars().collect();
    if cs.len() >= width {
        cs.iter().take(width).collect()
    } else {
        let mut out: String = cs.iter().collect();
        while out.chars().count() < width {
            out.push(' ');
        }
        out
    }
}

/// Truncate a string to at most `max` characters, appending an
/// ellipsis when shortened. `max == 0` returns an empty string.
pub(super) fn truncate_to_chars(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut out: String = chars.iter().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests_active_duration {
    use super::format_active_duration;

    #[test]
    fn under_a_minute_is_zero() {
        assert_eq!(format_active_duration(0), "0m");
        assert_eq!(format_active_duration(45), "0m");
    }

    #[test]
    fn minutes_only() {
        assert_eq!(format_active_duration(60), "1m");
        assert_eq!(format_active_duration(3540), "59m");
    }

    #[test]
    fn hours_with_minutes() {
        assert_eq!(format_active_duration(3600), "1h 00m");
        assert_eq!(format_active_duration(3660), "1h 01m");
        assert_eq!(format_active_duration(7325), "2h 02m");
    }
}
