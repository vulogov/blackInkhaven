//! `inkhaven export-timeline` (1.2.8+).
//!
//! Emit a calendar-formatted timeline for a user book to
//! a file.  Three formats:
//!
//!   * `typst` (default) — a text listing typst users can
//!     `#include` in a query letter / wiki page / pitch
//!     doc.  Compile through `typst compile <file>` to get
//!     PDF / SVG / PNG via typst's own pipeline.
//!   * `svg` — vector swim-lane render: track rows + a date
//!     axis at the top, instant events as circles, duration
//!     events as bars, orphan markers dashed.  Self-
//!     contained SVG; drop into HTML or open in a browser.
//!   * `png` — same SVG rasterised through `resvg` +
//!     `tiny-skia`.  Pixel-density follows the SVG's
//!     intrinsic size.
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
    match format {
        TimelineExportFormat::Typst => {
            let body = render_typst(
                &book_title,
                track_filter,
                &default_track,
                &rows,
                &calendar,
            );
            std::fs::write(output, body.as_bytes())
                .map_err(|e| anyhow!("write {}: {e}", output.display()))?;
        }
        TimelineExportFormat::Svg => {
            let svg = render_svg(
                &book_title,
                track_filter,
                &default_track,
                &rows,
                &calendar,
            );
            std::fs::write(output, svg.as_bytes())
                .map_err(|e| anyhow!("write {}: {e}", output.display()))?;
        }
        TimelineExportFormat::Png => {
            let svg = render_svg(
                &book_title,
                track_filter,
                &default_track,
                &rows,
                &calendar,
            );
            let png = svg_to_png_bytes(&svg)
                .map_err(|e| anyhow!("PNG rasterise: {e}"))?;
            std::fs::write(output, &png)
                .map_err(|e| anyhow!("write {}: {e}", output.display()))?;
        }
    }
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

// ── SVG renderer ────────────────────────────────────────────

/// Canvas defaults — picked so a typical 5-track book lands
/// near 1200×400 px, comfortable in a query letter or a wiki
/// page.  Scale with track count.
const SVG_CANVAS_W: u32 = 1200;
const SVG_LEFT_MARGIN: u32 = 140;
const SVG_RIGHT_PAD: u32 = 40;
const SVG_TITLE_H: u32 = 36;
const SVG_DATE_ROW_H: u32 = 28;
const SVG_TRACK_H: u32 = 44;
const SVG_BOTTOM_PAD: u32 = 24;

fn render_svg(
    book_title: &str,
    track_filter: Option<&str>,
    default_track: &str,
    rows: &[(&Node, &EventData)],
    calendar: &Calendar,
) -> String {
    // Compute the unique track list (preserving first-appearance
    // order across the time-sorted rows = roughly chronological
    // introduction).
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

    let canvas_h = SVG_TITLE_H
        + SVG_DATE_ROW_H
        + (tracks.len().max(1) as u32) * SVG_TRACK_H
        + SVG_BOTTOM_PAD;

    let mut out = String::with_capacity(2048 + rows.len() * 200);
    out.push_str(&format!(
        r##"<?xml version="1.0" encoding="UTF-8"?>
<svg xmlns="http://www.w3.org/2000/svg" version="1.1" width="{w}" height="{h}" viewBox="0 0 {w} {h}" font-family="Helvetica, Arial, sans-serif">
"##,
        w = SVG_CANVAS_W,
        h = canvas_h,
    ));
    // Background.
    out.push_str(&format!(
        r##"  <rect width="{}" height="{}" fill="#ffffff"/>
"##,
        SVG_CANVAS_W, canvas_h
    ));

    // Title.
    let title_text = match track_filter {
        Some(t) => format!("{} — timeline (track: {})", book_title, t),
        None => format!("{} — timeline", book_title),
    };
    out.push_str(&format!(
        r##"  <text x="{x}" y="{y}" font-size="18" font-weight="bold" fill="#222">{title}</text>
"##,
        x = 16,
        y = SVG_TITLE_H - 12,
        title = escape_svg(&title_text),
    ));

    if rows.is_empty() {
        out.push_str(&format!(
            r##"  <text x="{x}" y="{y}" font-size="14" fill="#888" font-style="italic">No events match</text>
"##,
            x = 16,
            y = SVG_TITLE_H + 24,
        ));
        out.push_str("</svg>\n");
        return out;
    }

    // Compute tick range.
    let min_tick = rows.iter().map(|(_, e)| e.start_ticks).min().unwrap_or(0);
    let max_tick = rows
        .iter()
        .map(|(_, e)| e.end_ticks.unwrap_or(e.start_ticks).max(e.start_ticks))
        .max()
        .unwrap_or(min_tick);
    let span = (max_tick - min_tick).max(1);
    let plot_w = SVG_CANVAS_W - SVG_LEFT_MARGIN - SVG_RIGHT_PAD;
    let x_of = |tick: i64| -> f64 {
        SVG_LEFT_MARGIN as f64
            + ((tick - min_tick) as f64) * (plot_w as f64) / (span as f64)
    };

    // Date axis: pick ~6 evenly-spaced ticks across the span,
    // format each via the calendar at Day precision.
    let date_y = SVG_TITLE_H + SVG_DATE_ROW_H - 10;
    let axis_y = SVG_TITLE_H + SVG_DATE_ROW_H;
    let n_labels: usize = 6;
    for i in 0..=n_labels {
        let tick = min_tick + (span * i as i64) / (n_labels as i64);
        let xp = x_of(tick);
        // Tick mark.
        out.push_str(&format!(
            r##"  <line x1="{x}" y1="{y1}" x2="{x}" y2="{y2}" stroke="#aaa" stroke-width="1"/>
"##,
            x = xp,
            y1 = axis_y - 4,
            y2 = axis_y + 4,
        ));
        let label = calendar.format(
            crate::timeline::TimelinePoint::from_ticks(tick),
            crate::timeline::Precision::Day,
        );
        out.push_str(&format!(
            r##"  <text x="{x}" y="{y}" font-size="11" fill="#444" text-anchor="middle">{label}</text>
"##,
            x = xp,
            y = date_y,
            label = escape_svg(&label),
        ));
    }
    // Horizontal axis line.
    out.push_str(&format!(
        r##"  <line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#bbb" stroke-width="1"/>
"##,
        x1 = SVG_LEFT_MARGIN,
        x2 = SVG_CANVAS_W - SVG_RIGHT_PAD,
        y = axis_y,
    ));

    // Tracks + events.
    for (row_i, track) in tracks.iter().enumerate() {
        let row_top = SVG_TITLE_H + SVG_DATE_ROW_H + (row_i as u32) * SVG_TRACK_H;
        let row_mid = row_top + SVG_TRACK_H / 2;
        // Row separator.
        if row_i > 0 {
            out.push_str(&format!(
                r##"  <line x1="0" y1="{y}" x2="{w}" y2="{y}" stroke="#eee" stroke-width="1"/>
"##,
                y = row_top,
                w = SVG_CANVAS_W,
            ));
        }
        // Track label (right-aligned to leave space for the
        // plot area).
        out.push_str(&format!(
            r##"  <text x="{x}" y="{y}" font-size="13" fill="#333" text-anchor="end" font-weight="bold">{label}</text>
"##,
            x = SVG_LEFT_MARGIN - 12,
            y = row_mid + 4,
            label = escape_svg(track),
        ));
        // Faint baseline through this row.
        out.push_str(&format!(
            r##"  <line x1="{x1}" y1="{y}" x2="{x2}" y2="{y}" stroke="#f0f0f0" stroke-width="1"/>
"##,
            x1 = SVG_LEFT_MARGIN,
            x2 = SVG_CANVAS_W - SVG_RIGHT_PAD,
            y = row_mid,
        ));
        // Events on this track.
        for (n, ev) in rows {
            let evt_track = ev
                .track
                .clone()
                .unwrap_or_else(|| default_track.to_string());
            if &evt_track != track {
                continue;
            }
            let xs = x_of(ev.start_ticks);
            let is_orphan = n.tags.iter().any(|t| t == "orphan");
            let primary = if is_orphan { "#888" } else { "#3a7fd5" };
            let dash = if is_orphan {
                r##" stroke-dasharray="3,3""##
            } else {
                ""
            };
            match ev.end_ticks {
                Some(end_t) if end_t > ev.start_ticks => {
                    let xe = x_of(end_t);
                    let bar_h = 12.0_f64;
                    out.push_str(&format!(
                        r##"  <rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{c}" fill-opacity="0.25" stroke="{c}"{dash}/>
"##,
                        x = xs,
                        y = row_mid as f64 - bar_h / 2.0,
                        w = (xe - xs).max(2.0),
                        h = bar_h,
                        c = primary,
                        dash = dash,
                    ));
                }
                _ => {
                    // Instant event: a circle on the baseline.
                    out.push_str(&format!(
                        r##"  <circle cx="{x}" cy="{y}" r="5" fill="{c}" stroke="#fff" stroke-width="1"/>
"##,
                        x = xs,
                        y = row_mid,
                        c = primary,
                    ));
                }
            }
            // Event title above the marker — truncated to keep
            // the layout readable when events cluster.
            let max_chars = 32;
            let label: String = if n.title.chars().count() > max_chars {
                let mut s: String = n.title.chars().take(max_chars - 1).collect();
                s.push('…');
                s
            } else {
                n.title.clone()
            };
            out.push_str(&format!(
                r##"  <text x="{x}" y="{y}" font-size="10" fill="#222">{label}</text>
"##,
                x = xs + 6.0,
                y = row_mid as f64 - 9.0,
                label = escape_svg(&label),
            ));
        }
    }

    out.push_str("</svg>\n");
    out
}

fn escape_svg(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// ── SVG → PNG ───────────────────────────────────────────────

/// Rasterise the given SVG to PNG bytes via `resvg` +
/// `tiny-skia`. Pixel-density follows the SVG's intrinsic
/// size; the swim-lane renderer above sets that to
/// `SVG_CANVAS_W × <computed height>`. Failures bubble up
/// as `String` for the CLI's `anyhow` wrap.
fn svg_to_png_bytes(svg: &str) -> Result<Vec<u8>, String> {
    use resvg::{tiny_skia, usvg};
    let opts = usvg::Options::default();
    let tree =
        usvg::Tree::from_str(svg, &opts).map_err(|e| format!("svg parse: {e}"))?;
    let int_size = tree.size().to_int_size();
    let (w, h) = (int_size.width(), int_size.height());
    if w == 0 || h == 0 {
        return Err("rendered SVG has zero size".into());
    }
    let mut pixmap = tiny_skia::Pixmap::new(w, h)
        .ok_or_else(|| format!("cannot allocate {w}×{h} pixmap"))?;
    pixmap.fill(tiny_skia::Color::WHITE);
    resvg::render(&tree, tiny_skia::Transform::default(), &mut pixmap.as_mut());
    pixmap.encode_png().map_err(|e| format!("encode PNG: {e}"))
}
