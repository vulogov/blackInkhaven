//! WORLD / WBLD-1 (WB-P3) — deterministic plausibility warnings + score.
//!
//! The per-layer `lint_*` functions return [`Warning`]s carrying a [`Severity`].
//! [`run_fast`] compiles the world's layers and runs every lint, deterministically
//! and without an LLM, so the worldbuilder can score a world 0–100 live:
//! `100 − Σ severity weight` (High 10 · Medium 5 · Low 2), clamped. Warnings a
//! `MagicRule` suppresses are already excluded before they reach the scorer (the
//! ledger runs inside the compile/lint path), so a valid exception raises the
//! score.
//!
//! `Warning` derefs to `&str` and displays as its text, so the existing CLI
//! callers that print or substring-match findings keep working unchanged.

use crate::world::types::{
    AstronomyOutput, ClimateOutput, DemographicsOutput, GeologyOutput, HydrologyOutput,
    WorldDefinition,
};

/// How much a warning costs the plausibility score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    /// Points deducted from 100 (RFC §D). Configurable weights are applied by the
    /// caller when it has a `worldbuilder.plausibility_weights` block.
    pub fn weight(self) -> i32 {
        match self {
            Severity::High => 10,
            Severity::Medium => 5,
            Severity::Low => 2,
        }
    }
}

/// One deterministic plausibility finding.
#[derive(Debug, Clone)]
pub struct Warning {
    pub severity: Severity,
    pub text: String,
}

impl Warning {
    pub fn high(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::High, text: text.into() }
    }
    pub fn medium(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::Medium, text: text.into() }
    }
    pub fn low(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::Low, text: text.into() }
    }
    /// Prefix the text with its originating layer (`"nations: …"`), keeping the
    /// severity — used when aggregating across layers in [`run_fast`].
    pub fn prefixed(self, layer: &str) -> Warning {
        Warning { severity: self.severity, text: format!("{layer}: {}", self.text) }
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::ops::Deref for Warning {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

/// Convert a bag of severity-tagged warnings into a 0–100 plausibility score.
pub fn compute_plausibility_score(warnings: &[Warning]) -> u8 {
    let mut score: i32 = 100;
    for w in warnings {
        score -= w.severity.weight();
    }
    score.clamp(0, 100) as u8
}

/// The five physical/quantitative compiler layers, produced deterministically by
/// [`compile_layers`]. The worldbuilder uses this both to score the world
/// ([`run_fast`]) and to summarise the *compiled* (not merely declared) world
/// state for the chat system prompt (WB-P5 `/compile`).
pub struct CompiledLayers {
    pub astronomy: AstronomyOutput,
    pub geology: GeologyOutput,
    pub climate: ClimateOutput,
    pub hydrology: HydrologyOutput,
    pub demographics: DemographicsOutput,
}

/// Run the pure compile chain (astronomy → geology → climate → hydrology →
/// demographics) with no LLM and no I/O beyond the definition. NB: geology uses
/// the pure `compile_geology` (not the project-DEM `geology_for`); DEM-aware
/// geology is a later refinement.
pub fn compile_layers(def: &WorldDefinition) -> CompiledLayers {
    use crate::world::compile::{
        astronomy_layer, climate_layer, demographics_layer, geology_layer, hydrology_layer,
    };
    let astronomy = astronomy_layer::compile_astronomy(&def.astronomy);
    let geology = geology_layer::compile_geology(def);
    let climate = climate_layer::compile_climate(def, &astronomy, &geology);
    let hydrology = hydrology_layer::compile_hydrology(&geology, &climate);
    let demographics = demographics_layer::compile_demographics(&climate, &hydrology);
    CompiledLayers { astronomy, geology, climate, hydrology, demographics }
}

/// A compact, deterministic prose summary of the *compiled* world state, suitable
/// for the worldbuilder chat system prompt. Unlike WB-P2's declaration summary,
/// this reports what the physics actually produced (year length, sea coverage,
/// mean climate, rivers, population), so the World Builder reasons over the
/// simulated world rather than the raw HJSON.
pub fn summarise_compiled(def: &WorldDefinition, layers: &CompiledLayers) -> String {
    let a = &layers.astronomy;
    let g = &layers.geology;
    let c = &layers.climate;
    let h = &layers.hydrology;
    let d = &layers.demographics;
    let mut s = String::new();
    s.push_str(&format!("World: {}\n", def.name));
    s.push_str(&format!(
        "Astronomy: {:.2} solar-mass star · year {:.0} planet-days · axial tilt {:.1}° · {} moon(s)\n",
        a.stellar_mass_solar,
        a.year_length_planet_days,
        a.axial_tilt_deg,
        a.moons.len(),
    ));
    s.push_str(&format!(
        "Geology: {}×{} · {} continent(s) · {:.0}% sea · {} mountain range(s) · boundaries {}c/{}d/{}t\n",
        g.width,
        g.height,
        g.continents,
        g.sea_coverage_pct,
        g.mountain_ranges.len(),
        g.boundaries.convergent,
        g.boundaries.divergent,
        g.boundaries.transform,
    ));
    s.push_str(&format!(
        "Climate: mean land {:.1}°C · {:.0}mm precip · {} biome zone(s)\n",
        c.mean_land_temp_c,
        c.mean_land_precip_mm,
        c.zones.len(),
    ));
    s.push_str(&format!(
        "Hydrology: {} river(s) ({} major) · {} lake(s) · {} watershed(s)\n",
        h.river_count,
        h.major_rivers.len(),
        h.lake_count,
        h.watershed_count,
    ));
    s.push_str(&format!(
        "Demographics: {} people · {:.0}% habitable · {} cities / {} towns / {} villages\n",
        d.total_population,
        d.habitable_fraction * 100.0,
        d.size_classes.cities,
        d.size_classes.towns,
        d.size_classes.villages,
    ));
    s
}

/// Compile every layer and run every deterministic lint, returning the aggregate
/// warnings (layer-prefixed). No LLM, no I/O beyond the pure compile chain —
/// suitable for the worldbuilder's live score.
pub fn run_fast(def: &WorldDefinition) -> Vec<Warning> {
    use crate::world::compile::{
        culture_layer, ecology_layer, history_layer, hydrology_layer, polities_layer,
    };

    let CompiledLayers { astronomy: _astro, geology: geo, climate, hydrology: _hydro, demographics: demo } =
        compile_layers(def);
    let seed = def.seed_u64();

    let mut out: Vec<Warning> = Vec::new();

    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = history_layer::compile_history(&demo, declared_hist, seed);
        out.extend(
            history_layer::lint_history(declared_hist, &hist)
                .into_iter()
                .map(|w| w.prefixed("history")),
        );
    }
    if !def.nations.is_empty() {
        out.extend(
            polities_layer::lint_polities(&def.nations, &demo)
                .into_iter()
                .map(|w| w.prefixed("nations")),
        );
    }
    if let Some(hy) = def.hydrology.as_ref() {
        if hy.rivers.iter().any(|r| r.from.is_some() && r.to.is_some()) {
            out.extend(
                hydrology_layer::lint_rivers(hy, &geo)
                    .into_iter()
                    .map(|w| w.prefixed("rivers")),
            );
        }
    }
    if !def.cultures.is_empty() {
        let pol = polities_layer::compile_polities(&demo, &def.nations, seed);
        let capital_biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                demo.settlements
                    .iter()
                    .find(|s| (s.x, s.y) == q.capital_pos)
                    .map(|s| s.biome.clone())
                    .unwrap_or_default()
            })
            .collect();
        out.extend(
            culture_layer::lint_culture(&def.cultures, &pol, &capital_biomes)
                .into_iter()
                .map(|w| w.prefixed("culture")),
        );
    }
    if let Some(eco) = def.ecology.as_ref().filter(|e| !e.regions.is_empty()) {
        out.extend(
            ecology_layer::lint_ecology(&eco.regions, &climate)
                .into_iter()
                .map(|w| w.prefixed("ecology")),
        );
    }
    if let Some(m) = def.magic.as_ref() {
        out.extend(m.lint().into_iter().map(|w| w.prefixed("magic")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_is_100_with_no_warnings() {
        assert_eq!(compute_plausibility_score(&[]), 100);
    }

    #[test]
    fn score_deducts_by_severity_weight_and_clamps() {
        let ws = vec![
            Warning::high("a"),   // -10
            Warning::medium("b"), // -5
            Warning::low("c"),    // -2
        ];
        assert_eq!(compute_plausibility_score(&ws), 100 - 10 - 5 - 2);
        // Clamp at 0.
        let many: Vec<Warning> = (0..20).map(|_| Warning::high("x")).collect();
        assert_eq!(compute_plausibility_score(&many), 0);
    }

    #[test]
    fn compile_layers_and_summary_are_deterministic_and_nonempty() {
        let body = r#"{
            name: "Terra"
            seed: 0x5151
            astronomy: {
                star: { luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0 }
                calendar: { months: 12, month_length_days: 30 }
            }
        }"#;
        let def = WorldDefinition::from_hjson(body).unwrap();
        let a = compile_layers(&def);
        let b = compile_layers(&def);
        // Same seed → identical compiled output.
        assert_eq!(a.geology.continents, b.geology.continents);
        assert_eq!(a.demographics.total_population, b.demographics.total_population);
        let summary = summarise_compiled(&def, &a);
        assert!(summary.contains("Astronomy:"));
        assert!(summary.contains("Geology:"));
        assert!(summary.contains("Demographics:"));
    }

    #[test]
    fn warning_derefs_to_str_and_displays_as_text() {
        let w = Warning::medium("river flows uphill");
        assert!(w.contains("uphill")); // via Deref<str>
        assert_eq!(format!("{w}"), "river flows uphill");
        assert_eq!(w.prefixed("rivers").text, "rivers: river flows uphill");
    }
}
