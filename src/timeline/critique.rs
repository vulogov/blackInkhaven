//! Normalised payload builder for the Phase 3 AI critique
//! chords (y / Y / Ctrl+Y inside the swim-lane view).
//!
//! The model never sees raw calendar config or
//! `Node`-shaped objects — it sees a flat, human-readable
//! summary one event per line + their links. Keeps the prompt
//! token-economical and lets the model focus on inconsistency
//! detection rather than schema interpretation.

use uuid::Uuid;

use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::timeline::{Calendar, Precision, TimelinePoint};
use crate::tui::timeline_state::TimelineEvent;

/// Build the prompt body for a timeline health check.
///
/// `scope_crumb` is the human-readable scope path (e.g.
/// `"Aerin Saga ▸ Chapter 4"`). `track_filter` set means
/// only events on that track are included; `None` means
/// every track in the input.
pub fn build_health_payload(
    events: &[TimelineEvent],
    calendar: &Calendar,
    hierarchy: &Hierarchy,
    scope_crumb: &str,
    track_filter: Option<&str>,
    default_track: &str,
) -> String {
    let mut out = String::new();
    out.push_str("Timeline health check.\n\n");
    out.push_str(&format!("Scope:  {scope_crumb}\n"));
    out.push_str(&format!(
        "Track:  {}\n",
        track_filter.unwrap_or("(all tracks)")
    ));
    out.push_str(&format!("Calendar units (base→top): {}\n", calendar_units_summary(calendar)));
    let total_visible = events
        .iter()
        .filter(|e| matches_track(e, track_filter, default_track))
        .count();
    out.push_str(&format!("Events visible at this scope: {total_visible}\n"));
    out.push_str("\nEvents (chronological):\n");

    for ev in events {
        if !matches_track(ev, track_filter, default_track) {
            continue;
        }
        let start = calendar.format(
            TimelinePoint::from_ticks(ev.start_ticks),
            ev.precision,
        );
        let end_label = match ev.end_ticks {
            Some(t) => {
                let s = calendar.format(TimelinePoint::from_ticks(t), ev.precision);
                format!(" → {s}")
            }
            None => String::new(),
        };
        let track = ev.track.as_deref().unwrap_or(default_track);
        let orphan_tag = if ev.is_orphan { "  [ORPHAN]" } else { "" };
        out.push_str(&format!(
            "  • {start}{end_label}  · {title}  · track={track}  · precision={prec}{orphan}\n",
            start = start,
            end_label = end_label,
            title = ev.title,
            track = track,
            prec = ev.precision.as_str(),
            orphan = orphan_tag,
        ));
        // Linked paragraphs as slug-paths.
        let para_paths: Vec<String> = ev
            .linked_paragraphs
            .iter()
            .filter_map(|id| resolve_slug_path(hierarchy, *id))
            .collect();
        if !para_paths.is_empty() {
            out.push_str(&format!("      paragraphs: {}\n", para_paths.join(", ")));
        }
        // Characters + places resolved to titles.
        let char_names = resolve_titles(hierarchy, &ev.characters);
        if !char_names.is_empty() {
            out.push_str(&format!("      characters: {}\n", char_names.join(", ")));
        }
        let place_names = resolve_titles(hierarchy, &ev.places);
        if !place_names.is_empty() {
            out.push_str(&format!("      places:     {}\n", place_names.join(", ")));
        }
    }
    out.push('\n');
    out.push_str(
        "Audit checklist (think through each silently, surface anything that matters):\n\
         - Travel-time / co-location conflicts: a character at two events whose start-to-start gap is shorter than the world makes plausible.\n\
         - Paragraph mismatches: a manuscript paragraph referencing an event by name but the event's date contradicts the paragraph's setting.\n\
         - Fuzzy overlaps: two events with `season` / `month` precision whose fuzz windows overlap suspiciously.\n\
         - Orphan signals: an event tagged ORPHAN that looks like it should attach to a paragraph mentioned above.\n\
         - Pacing: long unexplained gaps or rushed sequences. Comment only on outliers.\n\
         \n\
         Return a tight list of concrete issues. For each: which event(s), what's wrong, one-line proposed fix. \
         If everything looks coherent, say so in one sentence — don't pad.\n",
    );
    out
}

fn matches_track(ev: &TimelineEvent, filter: Option<&str>, default_track: &str) -> bool {
    let Some(needle) = filter else { return true };
    let track = ev.track.as_deref().unwrap_or(default_track);
    track.eq_ignore_ascii_case(needle)
}

fn calendar_units_summary(c: &Calendar) -> String {
    c.unit_names().join(" → ")
}

fn resolve_slug_path(h: &Hierarchy, id: Uuid) -> Option<String> {
    let node = h.get(id)?;
    let mut parts = node.path.clone();
    parts.push(node.slug.clone());
    Some(parts.join("/"))
}

fn resolve_titles(h: &Hierarchy, ids: &[Uuid]) -> Vec<String> {
    ids.iter()
        .filter_map(|id| h.get(*id).map(|n| n.title.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::calendar::CalendarConfig;

    fn cal() -> Calendar {
        Calendar::from_config(CalendarConfig {
            preset: "sols".into(),
            ..Default::default()
        })
    }

    fn ev(title: &str, start: i64, end: Option<i64>, track: Option<&str>) -> TimelineEvent {
        TimelineEvent {
            id: Uuid::nil(),
            title: title.into(),
            start_ticks: start,
            end_ticks: end,
            precision: Precision::Day,
            track: track.map(str::to_owned),
            is_orphan: false,
            linked_paragraphs: Vec::new(),
            book_prefix: String::new(),
            characters: Vec::new(),
            places: Vec::new(),
        }
    }

    #[test]
    fn body_includes_event_title_and_start() {
        let h = Hierarchy::default();
        let events = vec![ev("Storm", 5, None, Some("main"))];
        let body = build_health_payload(
            &events,
            &cal(),
            &h,
            "Aerin Saga ▸ Chapter 4",
            None,
            "main",
        );
        assert!(body.contains("Sol 6"));
        assert!(body.contains("Storm"));
        assert!(body.contains("track=main"));
        assert!(body.contains("(all tracks)"));
        assert!(body.contains("Scope:  Aerin Saga ▸ Chapter 4"));
    }

    #[test]
    fn track_filter_drops_off_track_events() {
        let h = Hierarchy::default();
        let events = vec![
            ev("Main thing", 0, None, Some("main")),
            ev("Side thing", 1, None, Some("flashback")),
        ];
        let body = build_health_payload(&events, &cal(), &h, "scope", Some("main"), "main");
        assert!(body.contains("Main thing"));
        assert!(!body.contains("Side thing"));
        // Visible count reports the filtered tally.
        assert!(body.contains("Events visible at this scope: 1"));
    }

    #[test]
    fn duration_event_renders_arrow() {
        let h = Hierarchy::default();
        let events = vec![ev("Storm", 5, Some(8), Some("main"))];
        let body = build_health_payload(&events, &cal(), &h, "scope", None, "main");
        assert!(body.contains("Sol 6 → Sol 9"));
    }

    #[test]
    fn orphan_marker_surfaces() {
        let h = Hierarchy::default();
        let mut e = ev("Lost map", 5, None, Some("main"));
        e.is_orphan = true;
        let body = build_health_payload(&[e], &cal(), &h, "scope", None, "main");
        assert!(body.contains("[ORPHAN]"));
    }

    #[test]
    fn audit_checklist_appended() {
        let h = Hierarchy::default();
        let body =
            build_health_payload(&[], &cal(), &h, "scope", None, "main");
        // The instructional block is always present so the
        // model knows what to look for even when there are
        // zero events.
        assert!(body.contains("Audit checklist"));
        assert!(body.contains("Travel-time"));
    }
}
