//! `inkhaven event …` subcommands (1.2.7+).
//!
//! Phase 1 of the timeline feature exposes three operations:
//!
//!   * `event add` — create a new event paragraph under the
//!     book's auto-created Timeline chapter.
//!   * `event list` — chronological listing across the project
//!     (filterable by book / track).
//!   * `event show` — print event details + linked
//!     paragraphs for one slug-path.
//!
//! All three early-out with a clear error when
//! `timeline.enabled = false` in HJSON so users don't
//! accidentally seed events into a project that hasn't opted
//! in.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::cli::EventCommand;
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{EventData, Node, NodeKind};
use crate::store::{reconcile_event_orphan_tag, InsertPosition, Store};
use crate::timeline::{Calendar, Precision};

pub fn run(project: &Path, cmd: EventCommand) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    if !cfg.timeline.enabled {
        return Err(anyhow!(
            "`inkhaven event` requires `timeline.enabled: true` in inkhaven.hjson"
        ));
    }
    let store = Store::open(layout.clone(), &cfg)?;
    let calendar = Calendar::from_config(cfg.timeline.calendar.clone());

    match cmd {
        EventCommand::Add {
            title,
            start,
            end,
            precision,
            track,
            book_name,
        } => add(&cfg, &store, &calendar, &title, &start, end.as_deref(), precision.as_deref(), track.as_deref(), book_name.as_deref()),
        EventCommand::List { book_name, track } => {
            list(&store, &calendar, book_name.as_deref(), track.as_deref())
        }
        EventCommand::Show { path } => show(&store, &calendar, &path),
    }
}

fn add(
    cfg: &Config,
    store: &Store,
    calendar: &Calendar,
    title: &str,
    start: &str,
    end: Option<&str>,
    precision_override: Option<&str>,
    track: Option<&str>,
    book_name: Option<&str>,
) -> Result<()> {
    let (start_point, inferred_prec) = calendar
        .parse(start)
        .map_err(|e| anyhow!("--start: {e}"))?;
    let end_point = match end {
        Some(s) => Some(
            calendar
                .parse(s)
                .map_err(|e| anyhow!("--end: {e}"))?
                .0
                .ticks(),
        ),
        None => None,
    };
    let precision = match precision_override {
        Some(s) => Precision::from_str(s)
            .ok_or_else(|| anyhow!("--precision: unknown precision `{s}`"))?,
        None => inferred_prec,
    };
    if let Some(end_t) = end_point {
        if end_t < start_point.ticks() {
            return Err(anyhow!(
                "--end ({end_t}) is before --start ({}) — events can't run backwards",
                start_point.ticks(),
            ));
        }
    }

    let hierarchy = Hierarchy::load(store)?;
    let book = resolve_user_book(&hierarchy, book_name)?;
    let timeline_chapter_id = store.ensure_timeline_chapter(cfg, book.id)?;

    // Reload hierarchy so the freshly-created Timeline
    // chapter (if it didn't already exist) is visible.
    let hierarchy = Hierarchy::load(store)?;
    let timeline_chapter = hierarchy
        .get(timeline_chapter_id)
        .cloned()
        .ok_or_else(|| anyhow!("Timeline chapter went missing right after creation"))?;

    let mut node = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        title,
        Some(&timeline_chapter),
        None,
        InsertPosition::End,
    )?;
    node.event = Some(EventData {
        start_ticks: start_point.ticks(),
        end_ticks: end_point,
        precision,
        characters: Vec::new(),
        places: Vec::new(),
        track: track.map(str::to_owned),
    });
    reconcile_event_orphan_tag(&mut node);
    node.modified_at = chrono::Utc::now();
    store
        .raw()
        .update_metadata(node.id, node.to_json())
        .map_err(|e| anyhow!("stamp event metadata: {e}"))?;
    store.sync()?;

    let end_label = end_point
        .map(|t| {
            format!(
                " → {}",
                calendar.format(crate::timeline::TimelinePoint::from_ticks(t), precision)
            )
        })
        .unwrap_or_default();
    println!(
        "event `{title}` added under `{book}` at {start}{end_label} (precision {prec})",
        title = title,
        book = book.title,
        start = calendar.format(start_point, precision),
        end_label = end_label,
        prec = precision.as_str(),
    );
    Ok(())
}

fn list(
    store: &Store,
    calendar: &Calendar,
    book_filter: Option<&str>,
    track_filter: Option<&str>,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let book_filter_id = match book_filter {
        Some(name) => Some(resolve_user_book(&hierarchy, Some(name))?.id),
        None => None,
    };
    let mut rows: Vec<(&Node, &EventData)> = hierarchy
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .collect();
    if let Some(id) = book_filter_id {
        rows.retain(|(n, _)| {
            // Walk up the parent chain until we hit a Book.
            let mut cur = *n;
            loop {
                if cur.kind == NodeKind::Book {
                    return cur.id == id;
                }
                let Some(parent_id) = cur.parent_id else {
                    return false;
                };
                match hierarchy.get(parent_id) {
                    Some(p) => cur = p,
                    None => return false,
                }
            }
        });
    }
    if let Some(track) = track_filter {
        rows.retain(|(_, ev)| {
            ev.track
                .as_deref()
                .map(|t| t.eq_ignore_ascii_case(track))
                .unwrap_or(false)
        });
    }
    rows.sort_by_key(|(_, ev)| ev.start_ticks);

    if rows.is_empty() {
        eprintln!("(no events match)");
        return Ok(());
    }
    for (n, ev) in &rows {
        let start = calendar.format(
            crate::timeline::TimelinePoint::from_ticks(ev.start_ticks),
            ev.precision,
        );
        let glyph = if ev.end_ticks.is_some() {
            "─"
        } else if n.tags.iter().any(|t| t == "orphan") {
            "◌"
        } else {
            "●"
        };
        let track = ev.track.as_deref().unwrap_or("—");
        let mut path_parts: Vec<&str> =
            n.path.iter().map(String::as_str).collect();
        path_parts.push(n.slug.as_str());
        println!(
            "  {start:>14} {glyph}  {title:<40}  track={track}  path={path}",
            start = start,
            glyph = glyph,
            title = n.title,
            track = track,
            path = path_parts.join("/"),
        );
    }
    Ok(())
}

fn show(store: &Store, calendar: &Calendar, path: &str) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let needle = path.trim().trim_matches('/');
    let target = hierarchy.flatten().into_iter().find_map(|(n, _)| {
        let mut parts: Vec<&str> = n.path.iter().map(String::as_str).collect();
        parts.push(n.slug.as_str());
        let joined = parts.join("/");
        if joined.eq_ignore_ascii_case(needle) {
            Some(n.clone())
        } else {
            None
        }
    });
    let node = target.ok_or_else(|| anyhow!("no node at `{path}`"))?;
    let event = node.event.as_ref().ok_or_else(|| {
        anyhow!("`{path}` is not an event (no event metadata attached)")
    })?;
    let start_p = crate::timeline::TimelinePoint::from_ticks(event.start_ticks);
    println!("title:      {}", node.title);
    println!("slug:       {}", node.slug);
    println!("start:      {}", calendar.format(start_p, event.precision));
    if let Some(end_ticks) = event.end_ticks {
        let end_p = crate::timeline::TimelinePoint::from_ticks(end_ticks);
        println!("end:        {}", calendar.format(end_p, event.precision));
    } else {
        println!("end:        — (instant)");
    }
    println!("precision:  {}", event.precision.as_str());
    println!(
        "track:      {}",
        event.track.as_deref().unwrap_or("(default)")
    );
    println!("characters: {}", event.characters.len());
    println!("places:     {}", event.places.len());
    println!("paragraphs: {}", node.linked_paragraphs.len());
    if !node.tags.is_empty() {
        println!("tags:       {}", node.tags.join(", "));
    }
    Ok(())
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
        return Err(anyhow!(
            "no user books in this project — `inkhaven add book \"...\"` first"
        ));
    }
    match book_name {
        Some(name) => {
            let needle = name.trim().to_ascii_lowercase();
            user_books
                .into_iter()
                .find(|b| {
                    b.title.to_ascii_lowercase() == needle
                        || b.slug.to_ascii_lowercase() == needle
                })
                .ok_or_else(|| anyhow!("no user book matches `--book-name {name}`"))
        }
        None => {
            if user_books.len() > 1 {
                let names: Vec<String> =
                    user_books.iter().map(|b| format!("`{}`", b.title)).collect();
                Err(anyhow!(
                    "project has {} user books — pass --book-name <name>. Available: {}",
                    user_books.len(),
                    names.join(", ")
                ))
            } else {
                Ok(user_books[0])
            }
        }
    }
}
