//! WORLD-4 Layer 5 — Demographics priors. Deterministic where it can be:
//! biome carrying-capacity sets the total population, the hydrology settlement
//! priors say *where* people cluster, and a Rank-Size (Zipf) hierarchy assigns
//! sizes. Names + per-settlement role prose are AI work that flows through the
//! proposal queue (Layer 5 is the first layer that proposes rather than asserts).

use crate::world::types::climate::{Biome, ClimateOutput};
use crate::world::types::demographics::*;
use crate::world::types::hydrology::HydrologyOutput;

/// Area of one model cell, in km² (the grid is treated as a ~4000×3000 km
/// regional map: 25 km/cell). A coarse default; populations are order-of-
/// magnitude estimates, not census figures.
const CELL_KM2: f64 = 625.0;

/// Bronze-age agricultural carrying capacity by biome (people / km²).
fn capacity(b: Biome) -> f64 {
    match b {
        Biome::Mediterranean => 20.0,
        Biome::TemperateForest => 15.0,
        Biome::TemperateGrassland => 10.0,
        Biome::TropicalSeasonal => 9.0,
        Biome::Savanna => 7.0,
        Biome::TropicalRainforest => 5.0,
        Biome::Taiga => 2.0,
        Biome::Tundra => 1.0,
        Biome::ColdDesert | Biome::HotDesert => 0.5,
        Biome::IceCap | Biome::Ocean => 0.0,
    }
}

pub fn compile_demographics(climate: &ClimateOutput, hydro: &HydrologyOutput) -> DemographicsOutput {
    let n = climate.biome.len();

    // Total population from per-cell carrying capacity. River cells are far more
    // productive (irrigated valleys) — give them a fertility multiplier.
    let mut total_pop = 0.0_f64;
    let mut land = 0usize;
    let mut habitable = 0usize;
    for i in 0..n {
        let b = climate.biome[i];
        if b == Biome::Ocean {
            continue;
        }
        land += 1;
        let mut cap = capacity(b);
        if cap > 0.0 {
            habitable += 1;
        }
        if hydro.is_river[i] {
            cap *= 2.5; // river valleys carry far more people
        }
        total_pop += cap * CELL_KM2;
    }
    let total_population = total_pop.round() as u64;
    let habitable_fraction = if land > 0 { habitable as f32 / land as f32 } else { 0.0 };

    // Settlements: the hydrology priors (already ranked by score) become a
    // Rank-Size hierarchy. Pre-industrial populations are overwhelmingly rural,
    // so even the primate city is a small slice of the total (~0.3%); rank r
    // scales as ~1/r^0.9 (a gentle Zipf). This spreads the top sites across
    // city / town / village classes rather than making them all megacities.
    let primate = (total_pop * 0.003).max(0.0);
    let w = climate.width;
    let mut settlements: Vec<Settlement> = hydro
        .settlement_priors
        .iter()
        .enumerate()
        .map(|(r, p)| {
            let pop = (primate / ((r + 1) as f64).powf(0.9)).round() as u64;
            let class = if pop >= 10_000 {
                "city"
            } else if pop >= 2_000 {
                "town"
            } else {
                "village"
            };
            let biome = climate.biome[p.y * w + p.x].as_str().to_string();
            Settlement {
                x: p.x,
                y: p.y,
                population: pop,
                class: class.to_string(),
                basis: p.kind.clone(),
                biome,
            }
        })
        .collect();
    settlements.sort_by(|a, b| b.population.cmp(&a.population));

    let mut size_classes = SizeClassSummary::default();
    for s in &settlements {
        match s.class.as_str() {
            "city" => size_classes.cities += 1,
            "town" => size_classes.towns += 1,
            _ => size_classes.villages += 1,
        }
    }

    DemographicsOutput {
        total_population,
        habitable_fraction,
        settlements,
        size_classes,
        role_archetypes: role_archetypes(climate),
    }
}

/// Heuristic role types from the dominant biomes + whether the world is coastal
/// (it always is in this model — settlements sit on rivers reaching the sea).
fn role_archetypes(climate: &ClimateOutput) -> Vec<String> {
    let mut roles = vec![
        "farmer".to_string(),
        "fisher".to_string(),
        "merchant".to_string(),
        "priest".to_string(),
        "smith".to_string(),
        "warrior".to_string(),
    ];
    let has = |name: &str| climate.zones.iter().any(|z| z.biome == name);
    if has("savanna") || has("temperate_grassland") {
        roles.push("herder".to_string());
    }
    if has("temperate_forest") || has("taiga") {
        roles.push("woodcutter".to_string());
    }
    if has("mediterranean") {
        roles.push("vintner".to_string());
    }
    roles
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::{compile_astronomy, compile_climate, compile_geology, compile_hydrology};
    use crate::world::types::world::{GeneratedGeology, GeologyDef, WorldDefinition};

    fn compile(seed: u64) -> DemographicsOutput {
        let body = format!(
            r#"{{
                name: "T"
                seed: {seed}
                astronomy: {{
                    star: {{ luminosity_solar: 1.0 }}
                    planet: {{ mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }}
                    orbit: {{ semi_major_axis_au: 1.0 }}
                    calendar: {{ months: 12, month_length_days: 30 }}
                }}
            }}"#
        );
        let mut def = WorldDefinition::from_hjson(&body).unwrap();
        def.geology = Some(GeologyDef {
            generated: Some(GeneratedGeology {
                plates: 7,
                continents: 4,
                mountain_orogeny: "active".into(),
                sea_level: 0.4,
            }),
            dem: None,
        });
        let astro = compile_astronomy(&def.astronomy);
        let geo = compile_geology(&def);
        let climate = compile_climate(&def, &astro, &geo);
        let hydro = compile_hydrology(&geo, &climate);
        compile_demographics(&climate, &hydro)
    }

    #[test]
    fn deterministic() {
        assert_eq!(compile(0x9090), compile(0x9090));
    }

    #[test]
    fn produces_a_rank_size_hierarchy() {
        let d = compile(0x9090);
        assert!(d.total_population > 0, "no population");
        assert!(!d.settlements.is_empty(), "no settlements");
        // Ranked largest-first.
        let pops: Vec<u64> = d.settlements.iter().map(|s| s.population).collect();
        assert!(pops.windows(2).all(|w| w[0] >= w[1]), "not rank-ordered");
        // The primate city out-populates the smallest settlement (Zipf).
        assert!(pops.first().unwrap() >= pops.last().unwrap());
        // Class counts add up to the settlement count.
        let total = d.size_classes.cities + d.size_classes.towns + d.size_classes.villages;
        assert_eq!(total, d.settlements.len());
        // A baseline of social roles is always present.
        assert!(d.role_archetypes.contains(&"merchant".to_string()));
    }

    #[test]
    fn habitable_fraction_is_a_fraction() {
        let d = compile(0x9090);
        assert!(d.habitable_fraction >= 0.0 && d.habitable_fraction <= 1.0);
    }
}
