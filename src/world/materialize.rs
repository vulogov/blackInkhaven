//! WORLD-4 — materialize compiled layer outputs into the **World** system book.
//!
//! The compiler is the owner of the structured leaves it writes here: astronomy
//! is closed-form physics ("fact, not opinion"), so re-compiling overwrites its
//! paragraphs rather than queuing proposals. (Layers that *are* opinion —
//! geology names, demographics — will route through the proposal queue in later
//! phases.) Materialization is idempotent: a paragraph is created once, then its
//! content is updated in place on subsequent compiles, so the World book never
//! accumulates duplicates.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_WORLD};
use crate::world::types::AstronomyOutput;

/// What a materialize pass did, for the CLI/TUI to report.
#[derive(Debug, Default, Clone)]
pub struct MaterializeReport {
    pub chapter: String,
    pub created: Vec<String>,
    pub updated: Vec<String>,
}

/// Materialize an astronomy output into `World / Astronomy / *` as three
/// structured (JSON) paragraphs: system overview, calendar, celestial events.
pub fn materialize_astronomy(
    store: &Store,
    cfg: &Config,
    out: &AstronomyOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Astronomy")?;

    let overview = serde_json::json!({
        "stellar_mass_solar": out.stellar_mass_solar,
        "orbital_period_days_earth": out.orbital_period_days_earth,
        "year_length_planet_days": out.year_length_planet_days,
        "declared_year_length_days": out.declared_year_length_days,
        "year_length_divergence_pct": out.year_length_divergence_pct,
        "axial_tilt_deg": out.axial_tilt_deg,
        "insolation_bands": out.insolation_bands,
    });
    let calendar = serde_json::json!({
        "seasons": out.seasons,
        "calendar_check": out.calendar_check,
    });
    let celestial = serde_json::json!({
        "moons": out.moons,
        "eclipses": out.eclipses,
        "tide": out.tide,
    });

    let mut report = MaterializeReport { chapter: "Astronomy".into(), ..Default::default() };
    for (title, payload) in [
        ("System overview", overview),
        ("Calendar", calendar),
        ("Celestial events", celestial),
    ] {
        let body = serde_json::to_string_pretty(&payload)
            .map_err(|e| Error::Store(format!("serializing {title}: {e}")))?;
        match ensure_paragraph(store, cfg, &chapter, title, &body)? {
            Outcome::Created => report.created.push(title.to_string()),
            Outcome::Updated => report.updated.push(title.to_string()),
        }
    }
    Ok(report)
}

/// Locate the World system book (seeded by `ensure_system_books` on open).
fn world_book(store: &Store) -> Result<Node> {
    Hierarchy::load(store)?
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_WORLD))
        .cloned()
        .ok_or_else(|| {
            Error::Store("World system book missing — re-open the project to seed it".into())
        })
}

/// Find or create a chapter by title under a book.
fn ensure_chapter(store: &Store, cfg: &Config, book: &Node, title: &str) -> Result<Node> {
    let h = Hierarchy::load(store)?;
    if let Some(c) = h
        .children_of(Some(book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(title))
        .cloned()
    {
        return Ok(c);
    }
    let h = Hierarchy::load(store)?;
    store.create_node(cfg, &h, NodeKind::Chapter, title, Some(book), None, InsertPosition::End)
}

enum Outcome {
    Created,
    Updated,
}

/// Find or create a paragraph by title under a chapter, setting its content.
fn ensure_paragraph(
    store: &Store,
    cfg: &Config,
    chapter: &Node,
    title: &str,
    body: &str,
) -> Result<Outcome> {
    let h = Hierarchy::load(store)?;
    let existing = h
        .children_of(Some(chapter.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case(title))
        .cloned();
    let (mut node, outcome) = match existing {
        Some(p) => (p, Outcome::Updated),
        None => {
            let h = Hierarchy::load(store)?;
            let p = store.create_node(
                cfg,
                &h,
                NodeKind::Paragraph,
                title,
                Some(chapter),
                None,
                InsertPosition::End,
            )?;
            (p, Outcome::Created)
        }
    };
    // A structured-data leaf (RFC §7.4): flag it HJSON and write the body to the
    // file (the on-disk source of truth), then sync DB + embeddings — exactly
    // how `cli::language::create_chapter_paragraph` seeds a language block.
    node.content_type = Some("hjson".to_string());
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("writing {title}: {e}")))?;
    }
    store.update_paragraph_content(&mut node, body.as_bytes())?;
    Ok(outcome)
}
