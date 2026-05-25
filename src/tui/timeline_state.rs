//! State + helpers for `Modal::TimelineView` (the F6 swim-lane
//! view). `TimelineEvent` is `pub` because the AI critique
//! payload builder in `crate::timeline::critique` consumes it
//! directly; the rest of the types are `pub(super)` since
//! nothing outside the TUI tree touches them. Extracted from
//! `tui::app` in the 1.2.7 refactor.

use uuid::Uuid;

/// 1.2.6+ — full state for `Modal::TimelineView`. Lives in
/// the modal only; not persisted across open/close.
#[derive(Debug, Clone)]
pub(crate) struct TimelineViewState {
    /// User book that anchors the visible events. Cross-book
    /// project mode (Ctrl+P) widens this conceptually but
    /// the field stays book-shaped for snapshot building.
    pub book_id: Uuid,
    /// Tree node the current view is scoped to. Events
    /// visible iff one of their `linked_paragraphs` (or the
    /// event itself, since events live in the Timeline
    /// chapter) sits in this subtree.
    pub scope_id: Uuid,
    /// Stack of previous scopes for "Esc back" in the
    /// descent picker. Phase-2 batch 3 wires this; Phase-2
    /// batch 1 just initialises empty.
    pub nav_history: Vec<Uuid>,
    /// All events in the current book, ticks-sorted. Rebuilt
    /// from the hierarchy whenever scope changes (cheap —
    /// books rarely hold thousands of events).
    pub events: Vec<TimelineEvent>,
    /// Track row name to highlight (cursor row). `None`
    /// means "first row". `Tab` cycles.
    pub track_highlight: Option<String>,
    /// Display scale — base units per cell. 1.0 means one
    /// base unit (day, hour, etc.) per terminal cell. +/-
    /// multiplies by 0.66 / 1.5; clamped to [0.05, 1000.0].
    pub ticks_per_cell: f64,
    /// Leftmost tick currently visible. ←/→ shifts this.
    pub scroll_ticks: i64,
    /// Cursor tick — where `n` would create an event.
    /// Initially anchored to the median visible event so the
    /// first frame isn't empty.
    pub cursor_ticks: i64,
    /// 1.2.7+ — the event the cursor is currently anchored
    /// to (None until the user steps with ↑/↓). When set, the
    /// render highlights every cell carrying this id, and
    /// `timeline_step_cursor` auto-pans the viewport so both
    /// `start_ticks` and `end_ticks` are visible.
    pub selected_event_id: Option<Uuid>,
    /// 1.2.7+ — tracks (by label) the user has collapsed.
    /// Collapsed tracks render as a single header line
    /// "▸ track-name · N events" instead of the full swim
    /// lane. Toggle with Space on the currently-highlighted
    /// track (Tab cycles the highlight).
    pub collapsed_tracks: std::collections::HashSet<String>,
    /// 1.2.7+ — the track whose events are currently shown
    /// as text sub-rows beneath the swim lane (tree-style
    /// expansion). At most one track is expanded at a time.
    /// `None` when navigation is at TRACK focus level; `Some`
    /// when the user has pressed Enter on a track and is now
    /// at EVENT focus level for that track.
    pub expanded_track: Option<String>,
    /// 1.2.7+ — navigation focus mode. `Track` (the default):
    /// Tab cycles tracks, Enter expands the focused track.
    /// `Event`: Tab cycles events of `expanded_track`, Enter
    /// opens the linked-paragraphs picker for the focused
    /// event. Esc / Backspace pops back to `Track`.
    pub focus_level: TimelineFocusLevel,
    /// Cross-book project overlay. Phase-2 batch 3.
    pub project_overlay: bool,
    /// 1.2.6+ — inline descent picker overlay. None when not
    /// open; `Some` when `d`/`D` is pressed and the user is
    /// choosing which child scope to enter.
    pub descent: Option<TimelineDescentState>,
}

/// 1.2.7+ — two-level navigation cursor for the timeline
/// view. Mirrors the tree pane's "Tab cycles siblings, Enter
/// descends into children" model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TimelineFocusLevel {
    /// Top-level. Tab cycles between tracks; Enter on a track
    /// expands that track's events as text sub-rows below the
    /// swim lane and drops focus into `Event`.
    Track,
    /// Inside an expanded track. Tab cycles events of that
    /// track in chronological order; Enter on an event opens
    /// the linked-paragraphs picker (same modal Ctrl+V L
    /// surfaces). Esc / Backspace pops back to `Track`.
    Event,
}

/// State for the inline descent picker shown over the swim
/// lanes when the user presses `d`.
#[derive(Debug, Clone)]
pub(crate) struct TimelineDescentState {
    pub choices: Vec<TimelineDescentChoice>,
    pub cursor: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineDescentChoice {
    pub id: Uuid,
    pub title: String,
    pub event_count: usize,
}

/// Snapshot of one event for the swim-lane view. Cached at
/// open / scope-change time so each render frame is a
/// straight columnar walk. Phase 3 widened this to carry
/// `characters` / `places` so the AI critique payload
/// builder doesn't need a second hierarchy walk.
#[derive(Debug, Clone)]
pub struct TimelineEvent {
    pub id: Uuid,
    pub title: String,
    pub start_ticks: i64,
    pub end_ticks: Option<i64>,
    pub precision: crate::timeline::Precision,
    pub track: Option<String>,
    pub is_orphan: bool,
    pub linked_paragraphs: Vec<Uuid>,
    pub characters: Vec<Uuid>,
    pub places: Vec<Uuid>,
    /// Optional book-slug prefix when the project overlay
    /// is on. Empty otherwise.
    pub book_prefix: String,
}

/// 1.2.6+ — pick a `(cursor_ticks, scroll_ticks, ticks_per_cell)`
/// triplet that makes the entire timeline span visible in the
/// current terminal. Used by `open_timeline_view` so a fresh open
/// shows the full range (`+`/`-` then drills in). Width is
/// sampled from `crossterm::terminal::size()` at call time;
/// caller is responsible for not calling this with an empty
/// event list (defaults are baked into `open_timeline_view`).
pub(crate) fn timeline_auto_fit(
    events: &[TimelineEvent],
) -> (i64, i64, f64) {
    let min_start = events
        .iter()
        .map(|e| e.start_ticks)
        .min()
        .unwrap_or(0);
    let max_end = events
        .iter()
        .map(|e| e.end_ticks.unwrap_or(e.start_ticks).max(e.start_ticks))
        .max()
        .unwrap_or(min_start);
    let span = (max_end - min_start).max(1);
    // Sample terminal width. The swim-lane modal eats ~2 cells of
    // border on each side + ~12 for the track-label gutter, so the
    // content area is roughly `terminal_width - 16`. Fall back to
    // 80 when crossterm can't tell us.
    let term_w = crossterm::terminal::size()
        .map(|(w, _)| w as usize)
        .unwrap_or(80);
    let content_w = term_w.saturating_sub(16).max(40);
    // 10% headroom on each side so events at the edges don't
    // touch the border.
    let target_w = (content_w as f64 * 0.8).max(20.0);
    let ticks_per_cell = ((span as f64) / target_w).max(1.0);
    let cursor_ticks = min_start + span / 2;
    // Scroll a little to the left of min_start so the first event
    // doesn't touch column 0.
    let pad = (content_w as f64 * 0.1 * ticks_per_cell).round() as i64;
    let scroll_ticks = min_start.saturating_sub(pad);
    (cursor_ticks, scroll_ticks, ticks_per_cell)
}

/// 1.2.6+ — jump cursor to the previous / next event by
/// chronological order (start_ticks). Used by the timeline view's
/// Up/Down arrows so the user can hop event-to-event without
/// hunting with horizontal scroll.
///
/// 1.2.7+ — returns the target event's uuid alongside its
/// start tick so the caller can stamp `selected_event_id` for
/// the highlight + auto-pan logic.
pub(super) fn timeline_step_event_cursor(
    events: &[TimelineEvent],
    cursor: i64,
    direction: i64,
) -> Option<(Uuid, i64)> {
    let mut by_start: Vec<(i64, Uuid)> = events
        .iter()
        .map(|e| (e.start_ticks, e.id))
        .collect();
    by_start.sort_by_key(|(t, _)| *t);
    if by_start.is_empty() {
        return None;
    }
    if direction > 0 {
        by_start.into_iter().find(|(t, _)| *t > cursor).map(|(t, id)| (id, t))
    } else {
        by_start
            .into_iter()
            .rev()
            .find(|(t, _)| *t < cursor)
            .map(|(t, id)| (id, t))
    }
}

/// Pick the next track in a cycle: `None` → tracks[0] →
/// tracks[1] → … → `None`. Stable / wrap-aware.
pub(crate) fn cycle_track(current: Option<&str>, tracks: &[String]) -> Option<String> {
    if tracks.is_empty() {
        return None;
    }
    match current {
        None => Some(tracks[0].clone()),
        Some(cur) => {
            let idx = tracks.iter().position(|t| t == cur);
            match idx {
                Some(i) if i + 1 < tracks.len() => Some(tracks[i + 1].clone()),
                _ => None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::cycle_track;

    #[test]
    fn cycle_through_tracks_then_back_to_none() {
        let tracks = vec!["flashback".to_string(), "main".to_string()];
        assert_eq!(cycle_track(None, &tracks).as_deref(), Some("flashback"));
        assert_eq!(
            cycle_track(Some("flashback"), &tracks).as_deref(),
            Some("main")
        );
        assert_eq!(cycle_track(Some("main"), &tracks), None);
    }

    #[test]
    fn cycle_empty_tracks_returns_none() {
        assert_eq!(cycle_track(None, &[]), None);
        assert_eq!(cycle_track(Some("anything"), &[]), None);
    }
}
