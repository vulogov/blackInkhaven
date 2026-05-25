//! Pure layout helpers for `Modal::TimelineView`. Keep these
//! free functions so the swim-lane shape is testable
//! independent of ratatui / theme / app state.

use uuid::Uuid;

use super::timeline_state::TimelineEvent;

/// One row in the swim lane = one track. Cells are
/// width-aligned; `None` is whitespace.
#[derive(Debug, Clone)]
pub(crate) struct TrackRow {
    pub label: String,
    pub cells: Vec<Option<TrackCell>>,
    /// True iff this is the synthetic orphan row.
    pub is_orphan_row: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TrackCell {
    pub glyph: char,
    pub event_id: Uuid,
    pub is_endpoint: bool,
    pub is_orphan: bool,
}

/// Lay out `events` into per-track rows for the given
/// horizontal viewport `[scroll_ticks, scroll_ticks + width *
/// ticks_per_cell)`. Orphans collected into one synthetic
/// trailing row when `show_orphans` is true.
///
/// Stable: row order is alphabetical by track label, with
/// `default_track_label` always first when present. Orphan
/// row always last.
pub(crate) fn layout_swim_lanes(
    events: &[TimelineEvent],
    scroll_ticks: i64,
    ticks_per_cell: f64,
    width: usize,
    default_track_label: &str,
    show_orphans: bool,
) -> Vec<TrackRow> {
    if width == 0 || ticks_per_cell <= 0.0 {
        return Vec::new();
    }

    // 1.2.7+ — when an event's `book_prefix` is non-empty
    // (set by `collect_book_events` in project-overlay mode),
    // prepend it to the track label as `<book-slug>/<track>`.
    // Otherwise the track key is the raw track name. Same
    // value used for both row creation and per-event row
    // lookup so the keys match.
    let track_key = |e: &TimelineEvent| -> String {
        let raw = e
            .track
            .clone()
            .unwrap_or_else(|| default_track_label.to_owned());
        if e.book_prefix.is_empty() {
            raw
        } else {
            format!("{}/{}", e.book_prefix, raw)
        }
    };

    // Collect unique non-orphan tracks.
    let mut tracks: Vec<String> = events
        .iter()
        .filter(|e| !e.is_orphan)
        .map(&track_key)
        .collect();
    tracks.sort();
    tracks.dedup();
    // Lift the default track to the top.
    if let Some(idx) = tracks.iter().position(|t| t == default_track_label) {
        tracks.swap(0, idx);
    }
    // For non-default first row, leave order as-is otherwise.

    let mut rows: Vec<TrackRow> = tracks
        .into_iter()
        .map(|label| TrackRow {
            label,
            cells: vec![None; width],
            is_orphan_row: false,
        })
        .collect();
    if show_orphans && events.iter().any(|e| e.is_orphan) {
        rows.push(TrackRow {
            label: "orphan".into(),
            cells: vec![None; width],
            is_orphan_row: true,
        });
    }

    for ev in events {
        let row_idx = if ev.is_orphan {
            // Orphans go to the dedicated row when it exists,
            // else they're dropped from the layout.
            rows.iter()
                .position(|r| r.is_orphan_row)
        } else {
            let want = track_key(ev);
            rows.iter().position(|r| !r.is_orphan_row && r.label == want)
        };
        let Some(row_idx) = row_idx else { continue };

        let start_col = tick_to_col(ev.start_ticks, scroll_ticks, ticks_per_cell);
        let end_col_excl = match ev.end_ticks {
            Some(end_t) => {
                let c = tick_to_col(end_t, scroll_ticks, ticks_per_cell);
                // Ensure end_col >= start_col + 1 so a bar
                // always paints at least one cell visibly.
                c.max(start_col + 1)
            }
            None => start_col + 1,
        };
        if end_col_excl <= 0 || start_col >= width as isize {
            continue;
        }
        let s = start_col.max(0) as usize;
        let e = (end_col_excl.min(width as isize)) as usize;
        for col in s..e {
            // Decide glyph.
            let glyph = if ev.end_ticks.is_none() {
                if ev.is_orphan { '◌' } else { '●' }
            } else {
                if col == s {
                    '├'
                } else if col == e - 1 {
                    '┤'
                } else {
                    '─'
                }
            };
            rows[row_idx].cells[col] = Some(TrackCell {
                glyph,
                event_id: ev.id,
                is_endpoint: col == s || col == e - 1,
                is_orphan: ev.is_orphan,
            });
        }
    }
    rows
}

/// Convert a tick value to a column index relative to the
/// viewport. Returns an `isize` because clipping math wants
/// negative values to mean "left of the viewport".
fn tick_to_col(ticks: i64, scroll_ticks: i64, ticks_per_cell: f64) -> isize {
    let delta = (ticks - scroll_ticks) as f64;
    (delta / ticks_per_cell).round() as isize
}

/// Inverse of `tick_to_col` — used by Enter / "create at
/// cursor" to translate the cursor column back to a tick
/// value.
pub(crate) fn col_to_tick(col: usize, scroll_ticks: i64, ticks_per_cell: f64) -> i64 {
    scroll_ticks + (col as f64 * ticks_per_cell).round() as i64
}

/// 1.2.7+ — compute the column indices where a vertical grid
/// stripe should land, given a per-day stride. Spacing is
/// expressed in ticks (= days when `base_unit = "day"`, the
/// default for all three calendar presets). `step_days == 0`
/// disables the grid (returns an empty vec). Columns that
/// don't fall inside `[0, width)` are skipped.
pub(crate) fn grid_columns(
    scroll_ticks: i64,
    ticks_per_cell: f64,
    width: usize,
    step_days: u32,
) -> Vec<usize> {
    if width == 0 || ticks_per_cell <= 0.0 || step_days == 0 {
        return Vec::new();
    }
    let step = step_days as i64;
    // First grid tick at or before scroll_ticks (aligned to
    // step boundaries).
    let aligned = scroll_ticks - scroll_ticks.rem_euclid(step);
    let span_ticks = (width as f64 * ticks_per_cell).ceil() as i64;
    let mut out = Vec::new();
    let mut t = aligned;
    while t <= scroll_ticks + span_ticks {
        let col = tick_to_col(t, scroll_ticks, ticks_per_cell);
        if col >= 0 && (col as usize) < width {
            out.push(col as usize);
        }
        t = t.saturating_add(step);
        // Sanity guard — at huge zoom-outs the loop is bounded
        // by span_ticks, but defensive cap keeps us safe.
        if out.len() > width {
            break;
        }
    }
    out
}

/// Compute the tick stamps that should carry a label on the
/// time axis. Chosen to be a roughly-even ~12-column cadence
/// without overlap. Returns `(column, tick)` pairs.
pub(crate) fn time_axis_labels(
    scroll_ticks: i64,
    ticks_per_cell: f64,
    width: usize,
) -> Vec<(usize, i64)> {
    if width == 0 || ticks_per_cell <= 0.0 {
        return Vec::new();
    }
    let target_spacing = 14usize.min(width);
    let n_labels = (width / target_spacing).max(1);
    let mut out = Vec::with_capacity(n_labels);
    for i in 0..=n_labels {
        let col = (i * width / n_labels).min(width.saturating_sub(1));
        let tick = col_to_tick(col, scroll_ticks, ticks_per_cell);
        out.push((col, tick));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Precision;

    fn ev(
        title: &str,
        track: Option<&str>,
        start: i64,
        end: Option<i64>,
    ) -> TimelineEvent {
        TimelineEvent {
            id: Uuid::nil(),
            title: title.into(),
            start_ticks: start,
            end_ticks: end,
            precision: Precision::Day,
            track: track.map(str::to_owned),
            is_orphan: false,
            linked_paragraphs: Vec::new(),
            characters: Vec::new(),
            places: Vec::new(),
            book_prefix: String::new(),
        }
    }

    #[test]
    fn instant_event_lands_one_cell() {
        let events = vec![ev("A", Some("main"), 5, None)];
        let rows = layout_swim_lanes(&events, 0, 1.0, 20, "main", true);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.label, "main");
        assert!(row.cells[5].is_some());
        assert_eq!(row.cells[5].as_ref().unwrap().glyph, '●');
        assert!(row.cells[6].is_none());
    }

    #[test]
    fn bar_event_spans_cells_with_endpoints() {
        let events = vec![ev("A", Some("main"), 5, Some(10))];
        let rows = layout_swim_lanes(&events, 0, 1.0, 20, "main", true);
        let row = &rows[0];
        assert_eq!(row.cells[5].as_ref().unwrap().glyph, '├');
        assert_eq!(row.cells[9].as_ref().unwrap().glyph, '┤');
        for c in &row.cells[6..9] {
            assert_eq!(c.as_ref().unwrap().glyph, '─');
        }
    }

    #[test]
    fn orphan_collected_into_synthetic_row() {
        let mut e = ev("A", None, 5, None);
        e.is_orphan = true;
        let rows = layout_swim_lanes(&[e], 0, 1.0, 10, "main", true);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].is_orphan_row);
        assert_eq!(rows[0].label, "orphan");
        assert_eq!(rows[0].cells[5].as_ref().unwrap().glyph, '◌');
    }

    #[test]
    fn orphan_hidden_when_show_orphans_false() {
        let mut e = ev("A", None, 5, None);
        e.is_orphan = true;
        let rows = layout_swim_lanes(&[e], 0, 1.0, 10, "main", false);
        assert!(rows.is_empty());
    }

    #[test]
    fn default_track_first_then_alpha() {
        let events = vec![
            ev("A", Some("zeta"), 0, None),
            ev("B", Some("main"), 1, None),
            ev("C", Some("alpha"), 2, None),
        ];
        let rows = layout_swim_lanes(&events, 0, 1.0, 10, "main", true);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].label, "main");
        assert_eq!(rows[1].label, "alpha");
        assert_eq!(rows[2].label, "zeta");
    }

    #[test]
    fn event_left_of_viewport_clipped() {
        // Bar [-5, 3). At zoom 1 from scroll 0, fills cells
        // 0..3 (cell 3 is exclusive).
        let events = vec![ev("A", Some("main"), -5, Some(3))];
        let rows = layout_swim_lanes(&events, 0, 1.0, 10, "main", true);
        let row = &rows[0];
        assert!(row.cells[0].is_some());
        assert!(row.cells[2].is_some());
        // [start, end) means cell 3 is the first one NOT
        // painted.
        assert!(row.cells[3].is_none());
    }

    #[test]
    fn zoom_changes_cells_per_event() {
        // Two events 10 days apart with zoom = 2 ticks/cell.
        let events = vec![
            ev("A", Some("main"), 0, None),
            ev("B", Some("main"), 10, None),
        ];
        let rows = layout_swim_lanes(&events, 0, 2.0, 20, "main", true);
        let row = &rows[0];
        // At 2 ticks/cell: A → col 0, B → col 5.
        assert!(row.cells[0].is_some());
        assert!(row.cells[5].is_some());
    }

    #[test]
    fn axis_labels_evenly_spaced() {
        let labels = time_axis_labels(0, 1.0, 60);
        // ~5 segments at width 60 / 14 spacing.
        assert!(labels.len() >= 4 && labels.len() <= 6);
        assert_eq!(labels[0].0, 0);
        assert!(labels.last().unwrap().0 >= 50);
    }

    #[test]
    fn col_to_tick_roundtrips_within_cell() {
        // tick → col → tick lands on the cell's starting tick.
        let tick = 42;
        let col = ((tick - 0) as f64 / 1.0).round() as usize;
        assert_eq!(col_to_tick(col, 0, 1.0), tick);
    }
}
