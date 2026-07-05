//! WORLD-4 — materialize compiled layer outputs into the **World** system book.
//!
//! The compiler is the owner of the structured leaves it writes here: astronomy
//! is closed-form physics ("fact, not opinion"), so re-compiling overwrites its
//! paragraphs rather than queuing proposals. (Layers that *are* opinion —
//! geology names, demographics — will route through the proposal queue in later
//! phases.) Materialization is idempotent: a paragraph is created once, then its
//! content is updated in place on subsequent compiles, so the World book never
//! accumulates duplicates.

use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;
use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_WORLD};
use crate::world::types::{
    AstronomyOutput, ClimateOutput, DemographicsOutput, GeologyOutput, HydrologyOutput, MagicLedger,
};

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

/// WORLD-8 (W8-P2) — materialize the history chronology into `World / History`
/// (the epochs + the founding chronology). The persistent world-book record; the
/// story Timeline is populated separately via the adoptable `event add` block
/// (`realworld history`), keeping the author in control of their timeline.
pub fn materialize_history(
    store: &Store,
    cfg: &Config,
    out: &crate::world::compile::history_layer::HistoryOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "History")?;

    let epochs = serde_json::json!({
        "span_years": out.span_years,
        "epochs": out.epochs.iter().map(|e| serde_json::json!({
            "name": e.name, "start_year": e.start_year, "end_year": e.end_year, "note": e.note,
        })).collect::<Vec<_>>(),
    });
    let foundings = serde_json::json!({
        "foundings": out.foundings.iter().map(|f| serde_json::json!({
            "year": f.year, "label": f.label, "class": f.class, "population": f.population,
        })).collect::<Vec<_>>(),
    });
    let events = serde_json::json!({
        "events": out.events.iter().map(|e| serde_json::json!({
            "year": e.year, "kind": e.kind, "description": e.description,
        })).collect::<Vec<_>>(),
    });

    let mut report = MaterializeReport { chapter: "History".into(), ..Default::default() };
    for (title, payload) in [("Epochs", epochs), ("Events", events), ("Foundings", foundings)] {
        let body = serde_json::to_string_pretty(&payload)
            .map_err(|e| Error::Store(format!("serializing {title}: {e}")))?;
        match ensure_paragraph(store, cfg, &chapter, title, &body)? {
            Outcome::Created => report.created.push(title.to_string()),
            Outcome::Updated => report.updated.push(title.to_string()),
        }
    }
    Ok(report)
}

/// Materialize a geology output into `World / Geology / *` (continents & plates,
/// mountains & ranges, mineral distribution) and write the heightmap as a
/// grayscale PNG under `assets/world/heightmap.png` — the heightmap travels as
/// an asset, not as a wall of JSON (the summary paragraph points at it).
pub fn materialize_geology(
    store: &Store,
    cfg: &Config,
    out: &GeologyOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Geology")?;

    let png = write_heightmap_png(store.project_root(), out)?;
    let png_rel = png
        .strip_prefix(store.project_root())
        .unwrap_or(&png)
        .display()
        .to_string();

    let continents = serde_json::json!({
        "source": out.source,
        "width": out.width,
        "height": out.height,
        "sea_level": out.sea_level,
        "plates": out.plates,
        "boundaries": out.boundaries,
        "continents": out.continents,
        "sea_coverage_pct": out.sea_coverage_pct,
        "elevation": out.elevation,
        "heightmap_asset": png_rel,
    });
    let mountains = serde_json::json!({ "mountain_ranges": out.mountain_ranges });
    let minerals = serde_json::json!({ "minerals": out.minerals });

    let mut report = MaterializeReport { chapter: "Geology".into(), ..Default::default() };
    for (title, payload) in [
        ("Continents and plates", continents),
        ("Mountains and ranges", mountains),
        ("Mineral distribution", minerals),
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

/// Materialize a climate output into `World / Climate / *`: the biome zones +
/// land means, and the prevailing-wind bands.
pub fn materialize_climate(
    store: &Store,
    cfg: &Config,
    out: &ClimateOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Climate")?;

    let zones = serde_json::json!({
        "width": out.width,
        "height": out.height,
        "mean_land_temp_c": out.mean_land_temp_c,
        "mean_land_precip_mm": out.mean_land_precip_mm,
        "zones": out.zones,
    });
    let winds = serde_json::json!({ "winds": out.winds });

    let mut report = MaterializeReport { chapter: "Climate".into(), ..Default::default() };
    for (title, payload) in [("Climate zones", zones), ("Prevailing winds", winds)] {
        let body = serde_json::to_string_pretty(&payload)
            .map_err(|e| Error::Store(format!("serializing {title}: {e}")))?;
        match ensure_paragraph(store, cfg, &chapter, title, &body)? {
            Outcome::Created => report.created.push(title.to_string()),
            Outcome::Updated => report.updated.push(title.to_string()),
        }
    }
    Ok(report)
}

/// Materialize a hydrology output into `World / Hydrology / *`: the river
/// systems, and the watersheds/lakes + settlement priors.
pub fn materialize_hydrology(
    store: &Store,
    cfg: &Config,
    out: &HydrologyOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Hydrology")?;

    let rivers = serde_json::json!({
        "width": out.width,
        "height": out.height,
        "river_count": out.river_count,
        "major_rivers": out.major_rivers,
    });
    let basins = serde_json::json!({
        "watershed_count": out.watershed_count,
        "lake_count": out.lake_count,
        "settlement_priors": out.settlement_priors,
    });

    let mut report = MaterializeReport { chapter: "Hydrology".into(), ..Default::default() };
    for (title, payload) in
        [("River systems", rivers), ("Watersheds and settlement priors", basins)]
    {
        let body = serde_json::to_string_pretty(&payload)
            .map_err(|e| Error::Store(format!("serializing {title}: {e}")))?;
        match ensure_paragraph(store, cfg, &chapter, title, &body)? {
            Outcome::Created => report.created.push(title.to_string()),
            Outcome::Updated => report.updated.push(title.to_string()),
        }
    }
    Ok(report)
}

/// Materialize a demographics output into `World / Demographics / *`: the
/// settlement overview + role archetypes, and the population distribution (the
/// settlement list). Turning settlements into named Place records flows through
/// the proposal queue (P2.2), not direct materialization.
pub fn materialize_demographics(
    store: &Store,
    cfg: &Config,
    out: &DemographicsOutput,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Demographics")?;

    let overview = serde_json::json!({
        "total_population": out.total_population,
        "habitable_fraction": out.habitable_fraction,
        "size_classes": out.size_classes,
        "role_archetypes": out.role_archetypes,
    });
    let distribution = serde_json::json!({ "settlements": out.settlements });

    let mut report = MaterializeReport { chapter: "Demographics".into(), ..Default::default() };
    for (title, payload) in
        [("Settlement overview", overview), ("Population distribution", distribution)]
    {
        let body = serde_json::to_string_pretty(&payload)
            .map_err(|e| Error::Store(format!("serializing {title}: {e}")))?;
        match ensure_paragraph(store, cfg, &chapter, title, &body)? {
            Outcome::Created => report.created.push(title.to_string()),
            Outcome::Updated => report.updated.push(title.to_string()),
        }
    }
    Ok(report)
}

/// Materialize the magic ledger into `World / Magic Ledger / Rules` so the
/// declared exceptions to physics live in the book alongside the world.
pub fn materialize_magic(
    store: &Store,
    cfg: &Config,
    ledger: &MagicLedger,
) -> Result<MaterializeReport> {
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Magic Ledger")?;
    let body = serde_json::to_string_pretty(&serde_json::json!({
        "enabled": ledger.enabled,
        "rules": ledger.rules,
    }))
    .map_err(|e| Error::Store(format!("serializing magic ledger: {e}")))?;
    let mut report = MaterializeReport { chapter: "Magic Ledger".into(), ..Default::default() };
    match ensure_paragraph(store, cfg, &chapter, "Rules", &body)? {
        Outcome::Created => report.created.push("Rules".into()),
        Outcome::Updated => report.updated.push("Rules".into()),
    }
    Ok(report)
}

/// Materialize the author-declared **Setting** — geography, hydrology, economy,
/// and any expanded-geology notes — into a `Setting` chapter of the World book.
/// A no-op (empty report) when the definition declares none of these blocks, so
/// it's safe to call unconditionally after a compile.
pub fn materialize_setting(
    store: &Store,
    cfg: &Config,
    def: &crate::world::types::WorldDefinition,
) -> Result<MaterializeReport> {
    let mut report = MaterializeReport { chapter: "Setting".into(), ..Default::default() };
    let geo_notes = def
        .geology
        .as_ref()
        .and_then(|g| g.generated.as_ref())
        .filter(|g| !g.volcanism.is_empty() || !g.mineral_richness.is_empty() || !g.notable_minerals.is_empty());
    if def.geography.is_none() && def.hydrology.is_none() && def.economy.is_none() && geo_notes.is_none() {
        return Ok(report);
    }
    let world = world_book(store)?;
    let chapter = ensure_chapter(store, cfg, &world, "Setting")?;

    let mut sections: Vec<(&str, String)> = Vec::new();

    if let Some(g) = def.geography.as_ref() {
        let mut s = String::new();
        if !g.regions.is_empty() {
            s.push_str("= Regions\n\n");
            for r in &g.regions {
                let bits = [r.climate.as_str(), r.biome.as_str()].iter().filter(|x| !x.is_empty()).cloned().collect::<Vec<_>>().join(", ");
                s.push_str(&format!("- *{}*{}{}\n", r.name, if bits.is_empty() { String::new() } else { format!(" — {bits}") }, if r.description.is_empty() { String::new() } else { format!(". {}", r.description) }));
            }
            s.push('\n');
        }
        if !g.landmarks.is_empty() {
            s.push_str("= Landmarks\n\n");
            for l in &g.landmarks {
                let pop = if l.population > 0 { format!(", pop {}", l.population) } else { String::new() };
                s.push_str(&format!("- *{}* ({}{}{}){}\n", l.name, l.kind, if l.climate_zone.is_empty() { String::new() } else { format!(", {}", l.climate_zone) }, pop, if l.description.is_empty() { String::new() } else { format!(" — {}", l.description) }));
            }
        }
        sections.push(("Geography", s));
    }

    if let Some(h) = def.hydrology.as_ref() {
        let mut s = String::new();
        if !h.rainfall.is_empty() {
            s.push_str(&format!("Rainfall: {}.\n\n", h.rainfall));
        }
        for (label, waters) in [("Rivers", &h.rivers), ("Lakes", &h.lakes), ("Seas", &h.seas)] {
            if !waters.is_empty() {
                s.push_str(&format!("= {label}\n\n"));
                for w in waters {
                    s.push_str(&format!("- *{}*{}\n", w.name, if w.description.is_empty() { String::new() } else { format!(" — {}", w.description) }));
                }
                s.push('\n');
            }
        }
        sections.push(("Hydrology", s));
    }

    if let Some(e) = def.economy.as_ref() {
        let mut s = String::new();
        if !e.tech_level.is_empty() {
            s.push_str(&format!("Technology level: {}.\n", e.tech_level));
        }
        if !e.currency.is_empty() {
            s.push_str(&format!("Currency: {}.\n", e.currency));
        }
        if !e.trade_goods.is_empty() {
            s.push_str(&format!("Trade goods: {}.\n", e.trade_goods.join(", ")));
        }
        if !e.resources.is_empty() {
            s.push_str(&format!("Resources: {}.\n", e.resources.join(", ")));
        }
        sections.push(("Economy", s));
    }

    if let Some(g) = geo_notes {
        let mut s = String::new();
        if !g.volcanism.is_empty() {
            s.push_str(&format!("Volcanism: {}.\n", g.volcanism));
        }
        if !g.mineral_richness.is_empty() {
            s.push_str(&format!("Mineral wealth: {}.\n", g.mineral_richness));
        }
        if !g.notable_minerals.is_empty() {
            s.push_str(&format!("Notable minerals: {}.\n", g.notable_minerals.join(", ")));
        }
        sections.push(("Geology Notes", s));
    }

    for (title, body) in sections {
        match ensure_paragraph(store, cfg, &chapter, title, body.trim_end())? {
            Outcome::Created => report.created.push(title.into()),
            Outcome::Updated => report.updated.push(title.into()),
        }
    }
    Ok(report)
}

/// Render the normalised heightmap as an 8-bit grayscale PNG asset.
fn write_heightmap_png(root: &Path, out: &GeologyOutput) -> Result<PathBuf> {
    let dir = root.join("assets").join("world");
    std::fs::create_dir_all(&dir)
        .map_err(|e| Error::Store(format!("creating {}: {e}", dir.display())))?;
    let path = dir.join("heightmap.png");
    let mut img = image::GrayImage::new(out.width as u32, out.height as u32);
    for y in 0..out.height {
        for x in 0..out.width {
            let v = (out.heightmap.get(y * out.width + x).copied().unwrap_or(0.0).clamp(0.0, 1.0) * 255.0)
                .round() as u8;
            img.put_pixel(x as u32, y as u32, image::Luma([v]));
        }
    }
    img.save(&path).map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
    Ok(path)
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
