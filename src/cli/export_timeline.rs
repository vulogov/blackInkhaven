//! `inkhaven export-timeline` (1.2.8+).
//!
//! Emit a calendar-formatted timeline listing for a user
//! book to a file.  The output format is opt-in:
//!
//!   * `typst` (default + only format in 1.2.8) — a text
//!     listing typst users can `#include` in a query
//!     letter / wiki page / pitch doc.  Compile through
//!     `typst compile <file>` to get PDF / SVG / PNG via
//!     typst's own pipeline.  No swim-lane geometry — for
//!     that wait for 1.2.9's SVG + PNG formats.
//!
//! Errors out cleanly when `timeline.enabled = false` so
//! seeded-but-not-opted-in projects don't get a confusing
//! empty export.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::cli::TimelineExportFormat;
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{EventData, Node, NodeKind};
use crate::timeline::{Calendar, TimelinePoint};

pub fn run(
    project: &Path,
    book_name: Option<&str>,
    format: TimelineExportFormat,
    output: &Path,
    track_filter: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    if !cfg.timeline.enabled {
        return Err(anyhow!(
            "`inkhaven export-timeline` requires `timeline.enabled: true` in inkhaven.hjson"
        ));
    }
    let store = Store::open(layout.clone(), &cfg)?;
    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());
    let hierarchy = Hierarchy::load(&store)?;
    let book = resolve_user_book(&hierarchy, book_name)?;
    let book_id = book.id;
    let book_title = book.title.clone();

    let mut rows: Vec<(&Node, &EventData)> = hierarchy
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .filter(|(n, _)| {
            let mut cur = *n;
            loop {
                if cur.kind == NodeKind::Book {
                    return cur.id == book_id;
                }
                let Some(pid) = cur.parent_id else { return false };
                match hierarchy.get(pid) {
                    Some(p) => cur = p,
                    None => return false,
                }
            }
        })
        .collect();
    if let Some(track) = track_filter {
        rows.retain(|(_, ev)| {
            ev.track
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case(track))
                .unwrap_or(false)
        });
    }
    rows.sort_by_key(|(_, ev)| ev.start_ticks);

    let default_track = cfg.timeline.default_track.clone();
    let body = match format {
        TimelineExportFormat::Typst => render_typst(
            &book_title,
            track_filter,
            &default_track,
            &rows,
            &calendar,
        ),
    };

    std::fs::write(output, body.as_bytes())
        .map_err(|e| anyhow!("write {}: {e}", output.display()))?;
    eprintln!(
        "exported {} event{} from `{}` → {}",
        rows.len(),
        if rows.len() == 1 { "" } else { "s" },
        book_title,
        output.display(),
    );
    Ok(())
}

fn render_typst(
    book_title: &str,
    track_filter: Option<&str>,
    default_track: &str,
    rows: &[(&Node, &EventData)],
    calendar: &Calendar,
) -> String {
    let mut out = String::new();
    out.push_str(&format!("// Inkhaven 1.2.8+ — timeline export\n"));
    out.push_str(&format!("// book: {book_title}\n"));
    if let Some(t) = track_filter {
        out.push_str(&format!("// track filter: {t}\n"));
    }
    out.push_str(&format!("// events: {}\n\n", rows.len()));

    out.push_str(&format!("= {book_title} — timeline\n\n"));

    if rows.is_empty() {
        out.push_str("_No events match._\n");
        return out;
    }

    // Group by track so each section is a chronologically-
    // ordered listing of its events.  Group iteration order
    // matches the rows' first appearance — which is start-
    // tick order across the whole project.
    let mut tracks: Vec<String> = Vec::new();
    for (_, ev) in rows {
        let t = ev
            .track
            .clone()
            .unwrap_or_else(|| default_track.to_string());
        if !tracks.contains(&t) {
            tracks.push(t);
        }
    }

    for track in &tracks {
        out.push_str(&format!("== {track}\n\n"));
        for (n, ev) in rows {
            let evt_track = ev
                .track
                .clone()
                .unwrap_or_else(|| default_track.to_string());
            if evt_track != *track {
                continue;
            }
            let start = calendar.format(
                TimelinePoint::from_ticks(ev.start_ticks),
                ev.precision,
            );
            let end_label = match ev.end_ticks {
                Some(t) => {
                    let s = calendar
                        .format(TimelinePoint::from_ticks(t), ev.precision);
                    format!(" — {s}")
                }
                None => String::new(),
            };
            let orphan = if n.tags.iter().any(|t| t == "orphan") {
                "  _[orphan]_"
            } else {
                ""
            };
            // Title-only event line, then optional metadata
            // bullets — kept structural so a future template
            // can restyle without touching the data.
            out.push_str(&format!(
                "- *{start}{end_label}* — {title}{orphan}\n",
                title = n.title,
            ));
            if !n.linked_paragraphs.is_empty() {
                out.push_str(&format!(
                    "  // {} linked paragraph(s)\n",
                    n.linked_paragraphs.len()
                ));
            }
        }
        out.push('\n');
    }

    out
}

fn resolve_user_book<'a>(
    hierarchy: &'a Hierarchy,
    book_name: Option<&str>,
) -> Result<&'a Node> {
    let user_books: Vec<&Node> = hierarchy
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();
    if user_books.is_empty() {
        return Err(anyhow!("project has no user books"));
    }
    if let Some(name) = book_name {
        let needle = name.trim().to_ascii_lowercase();
        let hit = user_books.iter().find(|b| {
            b.title.eq_ignore_ascii_case(&needle) || b.slug == needle
        });
        return hit.copied().ok_or_else(|| {
            anyhow!(
                "no user book named `{}` (have: {})",
                name,
                user_books
                    .iter()
                    .map(|b| b.title.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        });
    }
    if user_books.len() == 1 {
        return Ok(user_books[0]);
    }
    Err(anyhow!(
        "project has {} user books — pass --book-name to disambiguate (have: {})",
        user_books.len(),
        user_books
            .iter()
            .map(|b| b.title.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    ))
}
