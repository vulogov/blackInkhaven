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

/// Lint the DEFINITION itself — values that parse but are physically or
/// structurally impossible, and enum-like strings the compilers would silently
/// default. The compilers clamp these to something finite, so nothing panics;
/// but a `0`-month calendar, a star with no light, or `stance: "hostile"` (not a
/// stance) should be told to the author rather than quietly absorbed. Pure.
pub fn lint_definition(def: &WorldDefinition) -> Vec<Warning> {
    let mut w: Vec<Warning> = Vec::new();
    let a = &def.astronomy;
    let bad = |v: f64| !v.is_finite() || v <= 0.0;
    if bad(a.star.luminosity_solar) {
        w.push(Warning::high(format!("astronomy: star.luminosity_solar {} — the star must shine (Sun = 1.0)", a.star.luminosity_solar)));
    }
    if let Some(m) = a.star.mass_solar {
        if bad(m) {
            w.push(Warning::medium(format!("astronomy: star.mass_solar {m} must be positive")));
        }
    }
    if bad(a.planet.mass_earth) {
        w.push(Warning::high(format!("astronomy: planet.mass_earth {} must be positive (Earth = 1.0)", a.planet.mass_earth)));
    }
    if bad(a.planet.radius_earth) {
        w.push(Warning::high(format!("astronomy: planet.radius_earth {} must be positive (Earth = 1.0)", a.planet.radius_earth)));
    }
    if bad(a.planet.day_length_hours) {
        w.push(Warning::high(format!("astronomy: planet.day_length_hours {} — a day must have length", a.planet.day_length_hours)));
    }
    if !a.planet.axial_tilt_deg.is_finite() || !(0.0..=180.0).contains(&a.planet.axial_tilt_deg) {
        w.push(Warning::medium(format!("astronomy: planet.axial_tilt_deg {} is outside 0..180 (Earth 23.4; >90 means retrograde spin)", a.planet.axial_tilt_deg)));
    }
    let rot = a.planet.rotation_direction.trim().to_ascii_lowercase();
    if !matches!(rot.as_str(), "" | "prograde" | "retrograde") {
        w.push(Warning::medium(format!("astronomy: planet.rotation_direction {:?} is not `prograde` / `retrograde` — read as prograde", a.planet.rotation_direction)));
    }
    if bad(a.orbit.semi_major_axis_au) {
        w.push(Warning::high(format!("astronomy: orbit.semi_major_axis_au {} — the planet must orbit at some distance (Earth = 1.0)", a.orbit.semi_major_axis_au)));
    }
    if !a.orbit.eccentricity.is_finite() || !(0.0..1.0).contains(&a.orbit.eccentricity) {
        w.push(Warning::high(format!("astronomy: orbit.eccentricity {} must be in 0..1 (1 or more is not a closed orbit)", a.orbit.eccentricity)));
    }
    if let Some(y) = a.orbit.year_length_days {
        if bad(y) {
            w.push(Warning::medium(format!("astronomy: orbit.year_length_days {y} must be positive")));
        }
    }
    for m in &a.moons {
        if bad(m.period_days) {
            w.push(Warning::high(format!("astronomy: moon `{}` period_days {} must be positive", m.name, m.period_days)));
        }
        if !m.mass_lunar.is_finite() || m.mass_lunar < 0.0 {
            w.push(Warning::medium(format!("astronomy: moon `{}` mass_lunar {} must not be negative", m.name, m.mass_lunar)));
        }
        if !m.eccentricity.is_finite() || !(0.0..1.0).contains(&m.eccentricity) {
            w.push(Warning::medium(format!("astronomy: moon `{}` eccentricity {} must be in 0..1", m.name, m.eccentricity)));
        }
    }
    let c = &a.calendar;
    if c.months == 0 {
        w.push(Warning::high("astronomy: calendar.months is 0 — a year needs at least one month"));
    }
    if c.month_length_days == 0 {
        w.push(Warning::high("astronomy: calendar.month_length_days is 0 — a month needs at least one day"));
    }
    if !c.month_names.is_empty() && c.month_names.len() != c.months as usize {
        w.push(Warning::medium(format!("astronomy: calendar.month_names lists {} name(s) for {} month(s)", c.month_names.len(), c.months)));
    }
    if c.weekdays > 0 && !c.day_names.is_empty() && c.day_names.len() != c.weekdays as usize {
        w.push(Warning::low(format!("astronomy: calendar.day_names lists {} name(s) for {} weekday(s)", c.day_names.len(), c.weekdays)));
    }
    if let Some(anchor) = c.new_year_aligns_to.as_deref() {
        let k = anchor.trim().to_ascii_lowercase();
        if !matches!(k.as_str(), "vernal_equinox" | "spring_equinox" | "summer_solstice" | "autumnal_equinox" | "autumn_equinox" | "fall_equinox" | "winter_solstice") {
            w.push(Warning::medium(format!("astronomy: calendar.new_year_aligns_to {anchor:?} is not a solstice/equinox name — read as the vernal equinox")));
        }
    }
    if let Some(g) = def.geology.as_ref().and_then(|g| g.generated.as_ref()) {
        if g.plates < 2 {
            w.push(Warning::medium(format!("geology: generated.plates {} — at least 2 plates are needed (read as 2)", g.plates)));
        }
        if g.continents == 0 {
            w.push(Warning::medium("geology: generated.continents is 0 — read as 1"));
        }
        if g.continents > g.plates.max(2) {
            w.push(Warning::low(format!("geology: generated.continents {} exceeds generated.plates {} — continents ride on plates", g.continents, g.plates)));
        }
        if !g.sea_level.is_finite() || !(0.0..=1.0).contains(&g.sea_level) {
            w.push(Warning::high(format!("geology: generated.sea_level {} must be in 0..1 (Earth ≈ 0.6) — clamped, which drowns or drains the world", g.sea_level)));
        }
        let o = g.mountain_orogeny.trim().to_ascii_lowercase();
        if !matches!(o.as_str(), "" | "active" | "quiet" | "ancient") {
            w.push(Warning::medium(format!("geology: generated.mountain_orogeny {:?} is not active / quiet / ancient — read as active", g.mountain_orogeny)));
        }
    }
    if let Some(e) = def.seed.parse_error() {
        w.push(Warning::medium(e));
    }
    let names: Vec<String> = def.nations.iter().map(|n| n.name.trim().to_lowercase()).collect();
    for n in &def.nations {
        for r in &n.relations {
            let st = r.stance.trim().to_ascii_lowercase();
            if !matches!(st.as_str(), "allied" | "rival" | "neutral") {
                w.push(Warning::medium(format!("nations: `{}` → `{}` stance {:?} is not allied / rival / neutral — trade and history treat it as neutral", n.name, r.with, r.stance)));
            }
            if !names.contains(&r.with.trim().to_lowercase()) {
                w.push(Warning::medium(format!("nations: `{}` declares a relation with `{}`, which is not a declared nation", n.name, r.with)));
            }
        }
    }
    w
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

    let mut out: Vec<Warning> = lint_definition(def);

    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = history_layer::compile_history(&demo, declared_hist, &def.nations, seed);
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
    fn definition_lints_catch_impossible_values_and_bad_enums() {
        let mut def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("L")).unwrap();
        assert!(lint_definition(&def).is_empty(), "the starter is clean");
        def.astronomy.calendar.months = 0;
        def.astronomy.star.luminosity_solar = 0.0;
        def.astronomy.orbit.semi_major_axis_au = 0.0;
        def.astronomy.orbit.eccentricity = 1.5;
        def.astronomy.planet.rotation_direction = "Retrograde-ish".into();
        def.geology = Some(crate::world::types::GeologyDef {
            generated: Some(crate::world::types::GeneratedGeology {
                sea_level: 5.0,
                mountain_orogeny: "volcanic".into(),
                ..Default::default()
            }),
            dem: None,
        });
        def.seed = crate::world::types::SeedValue::Str("banana".into());
        def.nations = vec![crate::world::types::NationDef {
            name: "A".into(),
            capital: None,
            relations: vec![crate::world::types::NationRelation { with: "Nobody".into(), stance: "hostile".into() }],
        }];
        let w = lint_definition(&def);
        let text = w.iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n");
        for needle in ["calendar.months is 0", "luminosity_solar", "semi_major_axis_au", "eccentricity", "rotation_direction", "sea_level", "mountain_orogeny", "seed", "stance", "not a declared nation"] {
            assert!(text.contains(needle), "missing lint for {needle}:\n{text}");
        }
        // Case only is fine for the enum-like strings.
        def.astronomy.planet.rotation_direction = "Retrograde".into();
        assert!(!lint_definition(&def).iter().any(|x| x.text.contains("rotation_direction")));
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
