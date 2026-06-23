//! WORLD-4 — the `inkhaven realworld` CLI surface (RFC §10.1).
//!
//! P0 ships the astronomy slice: scaffold a `world.hjson`, validate it, and
//! compile the astronomy layer to a human summary or JSON. Storage,
//! materialization into the World book, and the remaining layers land in later
//! increments; the command surface grows with them.

use std::path::Path;

use crate::cli::{ProposalsCommand, RealworldCommand};
use crate::error::{Error, Result};
use crate::world::compile::compile_astronomy;
use crate::world::types::WorldDefinition;

/// The default world-definition filename at the project root.
const WORLD_FILE: &str = "world.hjson";

pub fn run(project: &Path, cmd: RealworldCommand) -> Result<()> {
    match cmd {
        RealworldCommand::New { name, force } => new(project, &name, force),
        RealworldCommand::Validate => validate(project),
        RealworldCommand::Show { json } => show(project, json),
        RealworldCommand::Compile { layer, json, materialize } => {
            compile(project, layer.as_deref(), json, materialize)
        }
        RealworldCommand::Propose => propose(project),
        RealworldCommand::Proposals { cmd } => proposals(project, cmd),
        RealworldCommand::Places => places(project),
        RealworldCommand::Magic { materialize } => magic(project, materialize),
    }
}

/// Fact-check prose against the world (fast track). `--text` checks a literal
/// string; `--paragraph` reads a paragraph's content from the store.
pub fn fact_check(project: &Path, text: Option<String>, paragraph: Option<String>) -> Result<()> {
    use crate::world::fact_check::check_paragraph;
    // The magic ledger (if any) is consulted; a missing world.hjson is fine.
    let ledger = load(project).ok().and_then(|d| d.magic).unwrap_or_default();

    let prose = match (text, paragraph) {
        (Some(t), _) => t,
        (None, Some(pid)) => {
            use crate::config::Config;
            use crate::project::ProjectLayout;
            use crate::store::Store;
            let id = uuid::Uuid::parse_str(&pid)
                .map_err(|e| Error::Config(format!("bad paragraph id `{pid}`: {e}")))?;
            let layout = ProjectLayout::new(project);
            layout.require_initialized()?;
            let cfg = Config::load_layered(&layout.config_path())?;
            let store = Store::open(layout, &cfg)?;
            let bytes = store
                .get_content(id)
                .map_err(|e| Error::Store(format!("reading paragraph: {e}")))?
                .ok_or_else(|| Error::Config(format!("paragraph `{pid}` not found")))?;
            String::from_utf8_lossy(&bytes).into_owned()
        }
        (None, None) => {
            return Err(Error::Config("give --text \"…\" or --paragraph <id>".into()));
        }
    };

    let findings = check_paragraph(&prose, &ledger, &[]);
    if findings.is_empty() {
        println!("✓ no issues found");
        return Ok(());
    }
    for f in &findings {
        let icon = match f.severity.as_str() {
            "contradiction" => "⊗",
            "warning" => "⚠",
            _ => "●",
        };
        let note = f.suppressed_by.as_deref().map(|r| format!(" (ok — magic rule `{r}`)")).unwrap_or_default();
        println!("{icon} [{}] {}{note}", f.category, f.body);
    }
    println!("\n{} finding(s).", findings.len());
    Ok(())
}

/// Show (and optionally materialize) the magic ledger.
fn magic(project: &Path, materialize: bool) -> Result<()> {
    let def = load(project)?;
    let ledger = def.magic.clone().unwrap_or_default();
    if ledger.rules.is_empty() {
        println!("(no magic rules — add a `magic:` block to world.hjson)");
    } else {
        println!("magic ledger · {} ({})", def.name, if ledger.enabled { "enabled" } else { "DISABLED" });
        for r in &ledger.rules {
            println!("  {} · covers [{}]", r.kind, r.covers.join(", "));
            if !r.description.is_empty() {
                println!("      {}", r.description);
            }
            if let Some(roles) = &r.applicable_to.roles {
                println!("      roles: {}", roles.join(", "));
            }
        }
    }
    if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        let r = crate::world::materialize::materialize_magic(&store, &cfg, &ledger)?;
        println!("  → World/{}: {} created, {} updated", r.chapter, r.created.len(), r.updated.len());
    }
    Ok(())
}

/// List the Place ↔ World cross-references.
fn places(project: &Path) -> Result<()> {
    use crate::world::storage::WorldStore;
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let links = store.list_place_links().map_err(|e| Error::Store(format!("listing: {e}")))?;
    if links.is_empty() {
        println!("(no world-linked places yet — accept some proposals)");
        return Ok(());
    }
    for l in &links {
        println!(
            "  {:<16} ({:>3},{:<3}) · {:<18} · {:<14} · pop {}",
            l.name, l.x, l.y, l.climate_zone, l.hydrology_basis, l.population
        );
    }
    println!("\n{} world-linked place(s).", links.len());
    Ok(())
}

/// Run the compiler through demographics and seed the proposal queue.
fn propose(project: &Path) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    use crate::world::proposals::place_proposals;
    use crate::world::storage::WorldStore;

    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let demo = compile_demographics(&climate, &hydro);

    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    let resolved = store
        .resolved_signatures()
        .map_err(|e| Error::Store(format!("reading proposals: {e}")))?;
    store.clear_pending().map_err(|e| Error::Store(format!("clearing proposals: {e}")))?;

    let proposals = place_proposals(&demo, def.seed_u64());
    let (mut added, mut skipped) = (0usize, 0usize);
    for p in &proposals {
        if resolved.contains(&p.signature) {
            skipped += 1; // already accepted or rejected — don't re-propose
            continue;
        }
        store.insert(p).map_err(|e| Error::Store(format!("inserting proposal: {e}")))?;
        added += 1;
    }
    println!(
        "proposed {added} Place(s) into the queue ({skipped} already resolved, skipped)"
    );
    println!("review with `inkhaven realworld proposals list`");
    Ok(())
}

fn proposals(project: &Path, cmd: ProposalsCommand) -> Result<()> {
    use crate::world::storage::WorldStore;
    let store = WorldStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("opening world store: {e}")))?;
    match cmd {
        ProposalsCommand::List { status } => {
            let list = store
                .list(status.as_deref())
                .map_err(|e| Error::Store(format!("listing proposals: {e}")))?;
            if list.is_empty() {
                println!("(no proposals{})", status.map(|s| format!(" with status {s}")).unwrap_or_default());
                return Ok(());
            }
            for p in &list {
                println!("{} [{}] {} — {}", &p.id.to_string()[..8], p.status, p.name, p.rationale);
            }
            println!("\n{} proposal(s). Accept with `realworld proposals accept <id>`.", list.len());
            Ok(())
        }
        ProposalsCommand::Accept { id } => {
            let uuid = parse_id(&store, &id)?;
            accept_one(project, &store, uuid)?;
            Ok(())
        }
        ProposalsCommand::Reject { id } => {
            let uuid = parse_id(&store, &id)?;
            store.set_status(uuid, "rejected").map_err(|e| Error::Store(format!("reject: {e}")))?;
            println!("rejected {id}");
            Ok(())
        }
        ProposalsCommand::AcceptAll => {
            let pending = store
                .list(Some("pending"))
                .map_err(|e| Error::Store(format!("listing: {e}")))?;
            let mut n = 0;
            for p in &pending {
                accept_one(project, &store, p.id)?;
                n += 1;
            }
            println!("accepted {n} proposal(s)");
            Ok(())
        }
        ProposalsCommand::Clear => {
            store.clear_pending().map_err(|e| Error::Store(format!("clear: {e}")))?;
            println!("cleared pending proposals");
            Ok(())
        }
    }
}

/// Resolve a possibly-abbreviated proposal id (first 8 chars) to a full UUID.
fn parse_id(store: &crate::world::storage::WorldStore, id: &str) -> Result<uuid::Uuid> {
    if let Ok(u) = uuid::Uuid::parse_str(id) {
        return Ok(u);
    }
    let list = store.list(None).map_err(|e| Error::Store(format!("listing: {e}")))?;
    list.iter()
        .find(|p| p.id.to_string().starts_with(id))
        .map(|p| p.id)
        .ok_or_else(|| Error::Config(format!("no proposal matching id `{id}`")))
}

/// Accept a proposal: create the Place and mark it accepted.
fn accept_one(
    project: &Path,
    store: &crate::world::storage::WorldStore,
    id: uuid::Uuid,
) -> Result<()> {
    let p = store
        .get(id)
        .map_err(|e| Error::Store(format!("get proposal: {e}")))?
        .ok_or_else(|| Error::Config(format!("no proposal `{id}`")))?;
    if p.status == "accepted" {
        println!("{} already accepted", p.name);
        return Ok(());
    }
    let place_id = create_place(project, &p)?;
    store
        .insert_place_link(&crate::world::proposals::PlaceLink::from_proposal(place_id, &p))
        .map_err(|e| Error::Store(format!("place link: {e}")))?;
    store.set_status(id, "accepted").map_err(|e| Error::Store(format!("accept: {e}")))?;
    println!("accepted {} → Places", p.name);
    Ok(())
}

/// Commit an accepted Place proposal as a prose paragraph under the Places book.
/// Returns the created Place node id (for the world cross-reference link).
fn create_place(project: &Path, p: &crate::world::proposals::PlaceProposal) -> Result<uuid::Uuid> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::hierarchy::Hierarchy;
    use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_PLACES};

    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    let places = Hierarchy::load(&store)?
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_PLACES))
        .cloned()
        .ok_or_else(|| Error::Store("Places system book missing".into()))?;

    let pop = p.payload.get("population").and_then(|v| v.as_u64()).unwrap_or(0);
    let class = p.payload.get("class").and_then(|v| v.as_str()).unwrap_or("settlement");
    let basis = p.payload.get("basis").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ");
    let biome = p.payload.get("biome").and_then(|v| v.as_str()).unwrap_or("").replace('_', " ");
    let prose = format!(
        "{} is a {} of roughly {} people, set at a {} in a {} zone.\n\n// world-compiler proposal {}\n",
        p.name, class, pop, basis, biome, p.signature
    );

    let h = Hierarchy::load(&store)?;
    let mut node = store
        .create_node(&cfg, &h, NodeKind::Paragraph, &p.name, Some(&places), None, InsertPosition::End)
        .map_err(|e| Error::Store(format!("creating Place: {e}")))?;
    if let Some(rel) = &node.file {
        std::fs::write(store.project_root().join(rel), prose.as_bytes())
            .map_err(|e| Error::Store(format!("writing Place: {e}")))?;
    }
    store
        .update_paragraph_content(&mut node, prose.as_bytes())
        .map_err(|e| Error::Store(format!("saving Place: {e}")))?;
    Ok(node.id)
}

/// Load + parse the project's `world.hjson`.
fn load(project: &Path) -> Result<WorldDefinition> {
    let path = project.join(WORLD_FILE);
    let raw = std::fs::read_to_string(&path).map_err(|e| {
        Error::Config(format!(
            "reading {}: {e} — run `inkhaven realworld new <name>` to scaffold one",
            path.display()
        ))
    })?;
    WorldDefinition::from_hjson(&raw)
        .map_err(|e| Error::Config(format!("{}: {e}", path.display())))
}

/// Compile the geology layer, using the external DEM if the definition declares
/// one (its `path` is resolved relative to the project root) — else generating
/// it from the seed.
fn geology_for(
    project: &Path,
    def: &WorldDefinition,
) -> Result<crate::world::types::GeologyOutput> {
    use crate::world::compile::{compile_geology, compile_geology_dem};
    if let Some(dem) = def.geology.as_ref().and_then(|g| g.dem.as_ref()) {
        let path = project.join(&dem.path);
        compile_geology_dem(def, &path).map_err(Error::Config)
    } else {
        Ok(compile_geology(def))
    }
}

fn new(project: &Path, name: &str, force: bool) -> Result<()> {
    let path = project.join(WORLD_FILE);
    if path.exists() && !force {
        return Err(Error::Config(format!(
            "{} already exists — pass --force to overwrite",
            path.display()
        )));
    }
    let body = starter_template(name);
    crate::io_atomic::write(&path, body.as_bytes())
        .map_err(|e| Error::Store(format!("writing {}: {e}", path.display())))?;
    println!("scaffolded {} for world `{name}`", path.display());
    println!("edit it, then `inkhaven realworld compile`");
    Ok(())
}

fn validate(project: &Path) -> Result<()> {
    let def = load(project)?;
    println!(
        "ok — world `{}`, seed {:#x}, primary language `{}`",
        def.name,
        def.seed_u64(),
        def.primary_language
    );
    println!("  astronomy: {} moon(s), {}-month calendar", def.astronomy.moons.len(), def.astronomy.calendar.months);
    Ok(())
}

fn show(project: &Path, json: bool) -> Result<()> {
    let def = load(project)?;
    if json {
        let v = serde_json::to_string_pretty(&def)
            .map_err(|e| Error::Store(format!("serializing definition: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("world: {}", def.name);
    println!("  seed:             {:#x}", def.seed_u64());
    println!("  primary_language: {}", def.primary_language);
    println!("  star:             {} (L={} L☉)", def.astronomy.star.class, def.astronomy.star.luminosity_solar);
    println!(
        "  planet:           {:.2} M⊕, tilt {:.1}°, day {:.1}h",
        def.astronomy.planet.mass_earth, def.astronomy.planet.axial_tilt_deg, def.astronomy.planet.day_length_hours
    );
    println!("  moons:            {}", def.astronomy.moons.iter().map(|m| m.name.as_str()).collect::<Vec<_>>().join(", "));
    Ok(())
}

fn compile(project: &Path, layer: Option<&str>, json: bool, materialize: bool) -> Result<()> {
    let l = layer.unwrap_or("astronomy");
    let known = ["astronomy", "geology", "climate", "hydrology", "demographics"];
    if !known.contains(&l) {
        return Err(Error::Config(format!("unknown layer `{l}` (one of: {})", known.join(", "))));
    }
    match l {
        "geology" => return compile_geology_cli(project, json, materialize),
        "climate" => return compile_climate_cli(project, json, materialize),
        "hydrology" => return compile_hydrology_cli(project, json, materialize),
        "demographics" => return compile_demographics_cli(project, json, materialize),
        _ => {} // astronomy — falls through to the body below.
    }

    let def = load(project)?;
    let out = compile_astronomy(&def.astronomy);

    // Materialize first (a side effect that runs in both JSON and human modes),
    // so `--json --materialize` both writes the book and prints the output.
    let mat_report = if materialize {
        Some(materialize_to_store(project, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing astronomy: {e}")))?;
        println!("{v}");
        return Ok(());
    }

    println!("astronomy · {}", def.name);
    println!(
        "  year:     {:.1} planet-days  ({:.1} Earth-days, {:.3} M☉ star)",
        out.year_length_planet_days, out.orbital_period_days_earth, out.stellar_mass_solar
    );
    if let (Some(d), Some(div)) = (out.declared_year_length_days, out.year_length_divergence_pct) {
        let flag = if div.abs() > 1.0 { "  ⚠" } else { "" };
        println!("  declared: {d:.0} planet-days  ({div:+.1}% vs computed){flag}");
    }
    println!("  tilt:     {:.1}°", out.axial_tilt_deg);
    print!("  seasons:  ");
    let mut s = out.seasons.clone();
    s.sort_by(|a, b| a.year_fraction.partial_cmp(&b.year_fraction).unwrap());
    println!(
        "{}",
        s.iter()
            .map(|m| format!("{} d{:.0}", m.name.replace('_', " "), m.planet_day_of_year))
            .collect::<Vec<_>>()
            .join(" · ")
    );
    for m in &out.moons {
        println!(
            "  moon {}:  synodic {:.1} planet-days, {:.1} lunations/yr",
            m.name, m.synodic_period_planet_days, m.lunar_months_per_year
        );
    }
    if let Some(dom) = &out.tide.dominant_moon {
        println!(
            "  tides:    {} dominant; sun {:.2}× the dominant moon",
            dom, out.tide.solar_relative_to_dominant
        );
    }
    let c = &out.calendar_check;
    println!(
        "  calendar: {:.0} declared vs {:.1} computed days  ({})",
        c.declared_days,
        c.computed_days,
        if c.consistent { "consistent" } else { "off by >1 day ⚠" }
    );
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the generated geology layer. (Materialization into the World
/// book + heightmap PNG export lands in the next WORLD-4 increment.)
fn compile_geology_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    let def = load(project)?;
    let out = geology_for(project, &def)?;

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_geology(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing geology: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("geology · {} ({} source, {}×{} grid)", def.name, out.source, out.width, out.height);
    println!(
        "  plates:     {} ({} continental) · boundaries {}▲ {}▽ {}↔",
        out.plates.len(),
        out.plates.iter().filter(|p| p.continental).count(),
        out.boundaries.convergent,
        out.boundaries.divergent,
        out.boundaries.transform
    );
    println!(
        "  land:       {} continent(s) · {:.0}% ocean · land fraction {:.2}",
        out.continents, out.sea_coverage_pct, out.elevation.land_fraction
    );
    println!(
        "  elevation:  min {:.2} · mean {:.2} · max {:.2}",
        out.elevation.min, out.elevation.mean, out.elevation.max
    );
    println!("  mountains:  {} range(s)", out.mountain_ranges.len());
    for r in out.mountain_ranges.iter().take(4) {
        println!("    plates {}–{} · peak {:.2} · {} cells", r.plate_a, r.plate_b, r.peak_elevation, r.cell_count);
    }
    println!(
        "  minerals:   {}",
        out.minerals.iter().map(|m| m.mineral.as_str()).collect::<Vec<_>>().join(", ")
    );
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated; heightmap → assets/world/heightmap.png",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the climate layer (astronomy + geology → zonal climate).
fn compile_climate_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let out = compile_climate(&def, &astro, &geo);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_climate(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing climate: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("climate · {} ({}×{} grid)", def.name, out.width, out.height);
    println!(
        "  land mean: {:.1}°C · {:.0} mm/yr precipitation",
        out.mean_land_temp_c, out.mean_land_precip_mm
    );
    println!("  winds:     {}", out.winds.iter().map(|w| format!("{} ({})", w.name, w.direction)).collect::<Vec<_>>().join(" · "));
    println!("  biomes ({}):", out.zones.len());
    for z in out.zones.iter().take(8) {
        println!(
            "    {:<20} {:>4.0}% · {:>5.0}…{:<4.0}°C · {:>5.0}…{:.0} mm",
            z.biome, z.area_pct, z.temp_min_c, z.temp_max_c, z.precip_min_mm, z.precip_max_mm
        );
    }
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the hydrology layer (geology + climate → D8 flow).
fn compile_hydrology_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_hydrology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let out = compile_hydrology(&geo, &climate);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_hydrology(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing hydrology: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("hydrology · {} ({}×{} grid)", def.name, out.width, out.height);
    println!(
        "  rivers:     {} ({} major) · {} lake(s) · {} watershed(s)",
        out.river_count, out.major_rivers.len(), out.lake_count, out.watershed_count
    );
    for r in out.major_rivers.iter().take(4) {
        println!("    mouth ({:>3},{:>3}) · order {} · flow {:.0}", r.mouth_x, r.mouth_y, r.order, r.flow);
    }
    println!("  settlement priors ({}):", out.settlement_priors.len());
    for p in out.settlement_priors.iter().take(6) {
        println!("    {:<14} ({:>3},{:>3}) · score {:.0}", p.kind, p.x, p.y, p.score);
    }
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Compile + print the demographics layer (climate + hydrology → settlements).
fn compile_demographics_cli(project: &Path, json: bool, materialize: bool) -> Result<()> {
    use crate::world::compile::{compile_astronomy, compile_climate, compile_demographics, compile_hydrology};
    let def = load(project)?;
    let astro = compile_astronomy(&def.astronomy);
    let geo = geology_for(project, &def)?;
    let climate = compile_climate(&def, &astro, &geo);
    let hydro = compile_hydrology(&geo, &climate);
    let out = compile_demographics(&climate, &hydro);

    let mat_report = if materialize {
        use crate::config::Config;
        use crate::project::ProjectLayout;
        use crate::store::Store;
        let layout = ProjectLayout::new(project);
        layout.require_initialized()?;
        let cfg = Config::load_layered(&layout.config_path())?;
        let store = Store::open(layout, &cfg)?;
        Some(crate::world::materialize::materialize_demographics(&store, &cfg, &out)?)
    } else {
        None
    };

    if json {
        let v = serde_json::to_string_pretty(&out)
            .map_err(|e| Error::Store(format!("serializing demographics: {e}")))?;
        println!("{v}");
        return Ok(());
    }
    println!("demographics · {} ({}×{} grid)", def.name, climate.width, climate.height);
    println!(
        "  population: {} · {:.0}% of land habitable",
        fmt_pop(out.total_population),
        out.habitable_fraction * 100.0
    );
    println!(
        "  settlements: {} ({} cities, {} towns, {} villages)",
        out.settlements.len(),
        out.size_classes.cities,
        out.size_classes.towns,
        out.size_classes.villages
    );
    for s in out.settlements.iter().take(6) {
        println!(
            "    {:<8} ({:>3},{:>3}) · pop {:>7} · {} · {}",
            s.class, s.x, s.y, fmt_pop(s.population), s.basis, s.biome
        );
    }
    println!("  roles: {}", out.role_archetypes.join(", "));
    if let Some(r) = &mat_report {
        println!(
            "  → World/{}: {} paragraph(s) created, {} updated",
            r.chapter,
            r.created.len(),
            r.updated.len()
        );
    }
    Ok(())
}

/// Human-readable population (12,450 / 1.2M).
fn fmt_pop(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Open the project store and materialize the astronomy output into the World
/// system book. Requires an initialized project (the World book is seeded on
/// open).
fn materialize_to_store(
    project: &Path,
    out: &crate::world::types::AstronomyOutput,
) -> Result<crate::world::materialize::MaterializeReport> {
    use crate::config::Config;
    use crate::project::ProjectLayout;
    use crate::store::Store;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    crate::world::materialize::materialize_astronomy(&store, &cfg, out)
}

/// A minimal, valid starter `world.hjson` (Earth-like, one moon) — enough to
/// `compile` immediately; the author edits from here.
fn starter_template(name: &str) -> String {
    format!(
        r#"// A world definition for `inkhaven realworld`.
// Edit freely, then `inkhaven realworld compile`. Only the astronomy block is
// wired today (WORLD-4 P0); geology / climate / hydrology / demographics / magic
// land in later phases and are accepted-and-ignored for now.
{{
    name: "{name}"
    seed: 0x1A2B3C
    primary_language: "en"

    astronomy: {{
        star: {{ class: "G2V", age_gyr: 4.6, luminosity_solar: 1.0 }}
        planet: {{
            mass_earth: 1.0
            radius_earth: 1.0
            axial_tilt_deg: 23.4
            day_length_hours: 24.0
            rotation_direction: "prograde"
        }}
        orbit: {{ semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }}
        moons: [
            {{ name: "Moon", mass_lunar: 1.0, period_days: 27.32 }}
        ]
        calendar: {{
            months: 12
            month_length_days: 30
            weekdays: 7
            new_year_aligns_to: "winter_solstice"
        }}
    }}
}}
"#
    )
}
