//! Workflow-status helpers (the Napkin → Ready ladder) used
//! across the TUI: tree pips, editor header chip, status-bar
//! filter modal, F1 help renderer. Extracted from
//! `tui::app` in the 1.2.7 refactor.

use ratatui::style::{Modifier, Style};

/// Document-status workflow ring. `Ctrl+B R` advances through
/// this sequence; the ring wraps back to "None" after "Ready".
/// `None` is represented by both the absence of `status` on
/// the Node and the literal "None" string in the ring — the
/// helpers below collapse the two views.
pub(super) const STATUS_CYCLE: &[&str] = &[
    "None", "Napkin", "First", "Second", "Third", "Final", "Ready",
];

/// Map a digit chord (`1`..`7`) to its workflow-status string.
/// Once used inline by the meta dispatcher; the dispatcher now
/// resolves to `StatusFilter*` actions via the binding table,
/// so this stays only as a unit-test helper for the
/// digit → status mapping itself.
#[cfg(test)]
#[allow(dead_code)]
pub(super) fn digit_to_status(c: char) -> Option<&'static str> {
    match c {
        '1' => Some("Ready"),
        '2' => Some("Final"),
        '3' => Some("Third"),
        '4' => Some("Second"),
        '5' => Some("First"),
        '6' => Some("Napkin"),
        '7' => Some("None"),
        _ => None,
    }
}

/// Advance to the next status in the cycle. Wraps at "Ready".
pub(super) fn next_status(current: Option<&str>) -> &'static str {
    let cur = display_status(current);
    let idx = STATUS_CYCLE.iter().position(|s| *s == cur).unwrap_or(0);
    STATUS_CYCLE[(idx + 1) % STATUS_CYCLE.len()]
}

/// Step backward through the cycle. Wraps at "None".
pub(super) fn prev_status(current: Option<&str>) -> &'static str {
    let cur = display_status(current);
    let idx = STATUS_CYCLE.iter().position(|s| *s == cur).unwrap_or(0);
    STATUS_CYCLE[(idx + STATUS_CYCLE.len() - 1) % STATUS_CYCLE.len()]
}

/// Collapse `None` / empty / whitespace status into the
/// canonical `"None"` label that the rest of the code +
/// the user-facing pickers expect.
pub(super) fn display_status(current: Option<&str>) -> &str {
    match current {
        None => "None",
        Some(s) if s.trim().is_empty() => "None",
        Some(s) => s,
    }
}

/// Compact one-character badge for the tree-pane row. The
/// colour (from `status_style`) carries the meaning; the
/// letter just gives the row a visual anchor so the user
/// knows that column means status.
pub(super) fn status_letter(label: &str) -> &'static str {
    match label {
        "Napkin" => "n",
        "First" => "1",
        "Second" => "2",
        "Third" => "3",
        "Final" => "F",
        "Ready" => "R",
        _ => " ",
    }
}

/// Colour the editor header uses for each status — picks
/// from the existing theme palette so users with custom
/// themes keep their preferred hues.
pub(super) fn status_style(label: &str, theme: &super::theme::Theme) -> Style {
    let base = match label {
        "None" => return Style::default().add_modifier(Modifier::DIM),
        "Napkin" => theme.grammar_change_fg, // red — "rough"
        "First" => theme.ai_scope_fg,        // peach
        "Second" => theme.characters_fg,     // amber
        "Third" => theme.places_fg,          // cyan
        "Final" => theme.border_saved,       // green
        "Ready" => theme.border_saved,       // green + bold
        _ => return Style::default(),
    };
    let mut style = Style::default().fg(base).add_modifier(Modifier::BOLD);
    if label == "Ready" {
        style = style.add_modifier(Modifier::REVERSED);
    }
    style
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digit_to_status_mapping() {
        assert_eq!(digit_to_status('1'), Some("Ready"));
        assert_eq!(digit_to_status('2'), Some("Final"));
        assert_eq!(digit_to_status('3'), Some("Third"));
        assert_eq!(digit_to_status('4'), Some("Second"));
        assert_eq!(digit_to_status('5'), Some("First"));
        assert_eq!(digit_to_status('6'), Some("Napkin"));
        assert_eq!(digit_to_status('7'), Some("None"));
        // 0, 8, 9 and letters don't map.
        assert_eq!(digit_to_status('0'), None);
        assert_eq!(digit_to_status('8'), None);
        assert_eq!(digit_to_status('a'), None);
    }

    #[test]
    fn status_letter_returns_one_char_or_space() {
        assert_eq!(status_letter("Napkin"), "n");
        assert_eq!(status_letter("First"), "1");
        assert_eq!(status_letter("Second"), "2");
        assert_eq!(status_letter("Third"), "3");
        assert_eq!(status_letter("Final"), "F");
        assert_eq!(status_letter("Ready"), "R");
        assert_eq!(status_letter("None"), " ");
        assert_eq!(status_letter("Unknown"), " ");
    }

    #[test]
    fn next_status_walks_the_ring() {
        assert_eq!(next_status(None), "Napkin");
        assert_eq!(next_status(Some("Napkin")), "First");
        assert_eq!(next_status(Some("First")), "Second");
        assert_eq!(next_status(Some("Second")), "Third");
        assert_eq!(next_status(Some("Third")), "Final");
        assert_eq!(next_status(Some("Final")), "Ready");
        // Wrap.
        assert_eq!(next_status(Some("Ready")), "None");
        // Empty string = same as None.
        assert_eq!(next_status(Some("")), "Napkin");
    }

    #[test]
    fn prev_status_walks_backwards_and_wraps() {
        assert_eq!(prev_status(Some("Napkin")), "None");
        assert_eq!(prev_status(Some("Ready")), "Final");
        assert_eq!(prev_status(Some("Final")), "Third");
        assert_eq!(prev_status(None), "Ready"); // wrap from None backwards
    }

    #[test]
    fn next_status_unknown_value_treated_as_none() {
        assert_eq!(next_status(Some("WeirdCustom")), "Napkin");
    }

    #[test]
    fn display_status_collapses_none_variants() {
        assert_eq!(display_status(None), "None");
        assert_eq!(display_status(Some("")), "None");
        assert_eq!(display_status(Some("   ")), "None");
        assert_eq!(display_status(Some("Napkin")), "Napkin");
    }
}
