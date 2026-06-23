//! WORLD-4 — the `inkhaven realworld` CLI surface (RFC §10.1).
//!
//! P0 ships the astronomy slice: scaffold a `world.hjson`, validate it, and
//! compile the astronomy layer to a human summary or JSON. Storage,
//! materialization into the World book, and the remaining layers land in later
//! increments; the command surface grows with them.

use std::path::Path;

use crate::cli::RealworldCommand;
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
    }
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
        "climate" | "hydrology" | "demographics" => {
            return Err(Error::Config(format!(
                "layer `{l}` is not implemented yet — astronomy + geology have landed (WORLD-4 P0/P1)"
            )));
        }
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
    use crate::world::compile::compile_geology;
    let def = load(project)?;
    let out = compile_geology(&def);

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
