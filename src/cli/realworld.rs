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
        RealworldCommand::Compile { layer, json } => compile(project, layer.as_deref(), json),
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

fn compile(project: &Path, layer: Option<&str>, json: bool) -> Result<()> {
    // P0: only the astronomy layer exists. A `--layer <other>` names a real but
    // not-yet-implemented layer; say so rather than silently doing astronomy.
    if let Some(l) = layer {
        let known = ["astronomy", "geology", "climate", "hydrology", "demographics"];
        if !known.contains(&l) {
            return Err(Error::Config(format!(
                "unknown layer `{l}` (one of: {})",
                known.join(", ")
            )));
        }
        if l != "astronomy" {
            return Err(Error::Config(format!(
                "layer `{l}` is not implemented yet — only `astronomy` has landed (WORLD-4 P0)"
            )));
        }
    }

    let def = load(project)?;
    let out = compile_astronomy(&def.astronomy);

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
    Ok(())
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
