//! `inkhaven event …` subcommands (1.2.6+).
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
    let cfg = Config::load_layered(&layout.config_path())?;
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
        EventCommand::Critique {
            track,
            book_name,
            legacy,
            migration_check,
            diff,
            no_elaborate,
            force,
        } => crate::cli::event_critique::run(
            &cfg,
            &store,
            &calendar,
            track.as_deref(),
            book_name.as_deref(),
            legacy,
            migration_check,
            diff,
            no_elaborate,
            force,
        ),
        EventCommand::LinkCharacter { path, name } => {
            link_entity(&store, &path, &name, false)
        }
        EventCommand::LinkPlace { path, name } => link_entity(&store, &path, &name, true),
    }
}

/// Resolve an event paragraph by its slug-path (as printed by `event show` /
/// `inkhaven list`). Errors if the path is unknown or not an event.
fn resolve_event_node(hierarchy: &Hierarchy, path: &str) -> Result<Node> {
    let needle = path.trim().trim_matches('/');
    let node = hierarchy
        .flatten()
        .into_iter()
        .find_map(|(n, _)| {
            let mut parts: Vec<&str> = n.path.iter().map(String::as_str).collect();
            parts.push(n.slug.as_str());
            if parts.join("/").eq_ignore_ascii_case(needle) {
                Some(n.clone())
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("no node at `{path}`"))?;
    if node.event.is_none() {
        return Err(anyhow!("`{path}` is not an event (no event metadata attached)"));
    }
    Ok(node)
}

/// Find an entry (case-insensitive title match) under a system book.
fn resolve_entity(
    hierarchy: &Hierarchy,
    system_tag: &str,
    label: &str,
    name: &str,
) -> Result<(uuid::Uuid, String)> {
    let root = hierarchy
        .iter()
        .find(|n| n.system_tag.as_deref() == Some(system_tag))
        .ok_or_else(|| anyhow!("this project has no {label} book"))?;
    let needle = name.trim();
    let mut stack: Vec<uuid::Uuid> =
        hierarchy.children_of(Some(root.id)).iter().map(|n| n.id).collect();
    while let Some(id) = stack.pop() {
        let Some(n) = hierarchy.get(id) else { continue };
        if matches!(n.kind, NodeKind::Paragraph) && n.title.trim().eq_ignore_ascii_case(needle) {
            return Ok((n.id, n.title.clone()));
        }
        stack.extend(hierarchy.children_of(Some(id)).iter().map(|n| n.id));
    }
    Err(anyhow!("no entry titled `{name}` under the {label} book"))
}

/// Attach an explicit Character/Place participant to an event. These explicit
/// lists are what KEN's presence grants read (the advisory co-location check
/// additionally derives participants from linked scenes).
fn link_entity(store: &Store, path: &str, name: &str, is_place: bool) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let mut node = resolve_event_node(&hierarchy, path)?;
    let (system_tag, label, kind) = if is_place {
        (crate::store::SYSTEM_TAG_PLACES, "Places", "place")
    } else {
        (crate::store::SYSTEM_TAG_CHARACTERS, "Characters", "character")
    };
    let (entity_id, entity_title) = resolve_entity(&hierarchy, system_tag, label, name)?;

    let already = node
        .event
        .as_ref()
        .map(|ev| {
            let list = if is_place { &ev.places } else { &ev.characters };
            list.contains(&entity_id)
        })
        .unwrap_or(false);
    if already {
        println!("`{entity_title}` is already linked to `{}`.", node.title);
        return Ok(());
    }
    if let Some(ev) = node.event.as_mut() {
        if is_place {
            ev.places.push(entity_id);
        } else {
            ev.characters.push(entity_id);
        }
    }
    reconcile_event_orphan_tag(&mut node);
    node.modified_at = chrono::Utc::now();
    store
        .raw()
        .update_metadata(node.id, node.to_json())
        .map_err(|e| anyhow!("stamp event metadata: {e}"))?;
    store.sync()?;
    println!("linked {kind} `{entity_title}` to event `{}`.", node.title);
    Ok(())
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
    let book = crate::cli::resolve_user_book(&hierarchy, book_name, "event")
        .map_err(|m| anyhow!(m))?;
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

    // 1.2.6+ Phase 4 — fire `hook.on_event_added` so Bund
    // scripts can react (timeline-aware indexing, automated
    // critique, etc.).
    crate::scripting::hooks::fire(
        "hook.on_event_added",
        vec![rust_dynamic::value::Value::from_string(
            node.id.to_string(),
        )],
    );

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
        Some(name) => Some(
            crate::cli::resolve_user_book(&hierarchy, Some(name), "event")
                .map_err(|m| anyhow!(m))?
                .id,
        ),
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

