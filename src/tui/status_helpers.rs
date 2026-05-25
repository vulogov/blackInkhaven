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
