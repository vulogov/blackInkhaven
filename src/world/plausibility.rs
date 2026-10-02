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
/// demographics) with no LLM and no I/O beyond the definition. Geology is always
/// GENERATED here; a surface that knows the project root should call
/// [`compile_layers_at`] so a declared heightmap is honoured.
pub fn compile_layers(def: &WorldDefinition) -> CompiledLayers {
    compile_layers_at(def, None)
}

/// WORLD-KEEP-1 (WK-P3) — [`compile_layers`], DEM-aware: with a project root, a
/// declared `geology.dem` is read (as the CLI does) instead of compiling the
/// world's procedural twin. A heightmap that cannot be read falls back to the
/// generated terrain; [`run_fast_at`] reports that as a warning.
pub fn compile_layers_at(def: &WorldDefinition, root: Option<&std::path::Path>) -> CompiledLayers {
    use crate::world::compile::{
        astronomy_layer, climate_layer, demographics_layer, geology_layer, hydrology_layer,
    };
    let astronomy = astronomy_layer::compile_astronomy(&def.astronomy);
    let geology = match root {
        Some(r) => crate::world::compile::compile_geology_at_or_generated(def, r),
        None => geology_layer::compile_geology(def),
    };
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
    if let Some(s) = def.geology.as_ref().and_then(|g| g.dem.as_ref()).and_then(|d| d.scale_km_per_pixel) {
        if !s.is_finite() || s <= 0.0 {
            w.push(Warning::medium(format!(
                "geology: dem.scale_km_per_pixel {s} must be a positive number — ignored (the image is read as the whole planet)"
            )));
        }
    }
    if let Some(d) = def.geology.as_ref().and_then(|g| g.dem.as_ref()) {
        if let Some(lat) = d.center_lat {
            if !lat.is_finite() || lat.abs() > 90.0 {
                w.push(Warning::medium(format!(
                    "geology: dem.center_lat {lat} must be between -90 and 90 — clamped"
                )));
            }
            if d.scale_km_per_pixel.is_none() {
                w.push(Warning::low(
                    "geology: dem.center_lat is set but the heightmap declares no scale_km_per_pixel — without a scale the image is the whole planet and center_lat is ignored",
                ));
            }
        }
        if let Some(lon) = d.center_lon {
            if !lon.is_finite() || lon.abs() > 180.0 {
                w.push(Warning::medium(format!(
                    "geology: dem.center_lon {lon} must be between -180 and 180 — clamped"
                )));
            }
        }
    }
    // Declared positions that fall off the map are clamped to its edge by the
    // renderer — a typo then draws a marker on the border, silently.
    if let Some(g) = def.geography.as_ref() {
        use crate::world::compile::geology_layer::{GRID_H, GRID_W};
        for l in &g.landmarks {
            if l.x.is_some_and(|x| x >= GRID_W) || l.y.is_some_and(|y| y >= GRID_H) {
                w.push(Warning::medium(format!(
                    "geography: landmark `{}` at cell ({}, {}) is off the {GRID_W}×{GRID_H} map — drawn clamped to the edge",
                    l.name, l.x.unwrap_or(0), l.y.unwrap_or(0)
                )));
            }
            if l.lat.is_some_and(|v| !v.is_finite() || v.abs() > 90.0)
                || l.lon.is_some_and(|v| !v.is_finite() || v.abs() > 180.0)
            {
                w.push(Warning::medium(format!(
                    "geography: landmark `{}` has lat/lon outside ±90 / ±180 — drawn clamped to the edge",
                    l.name
                )));
            }
        }
        for r in &g.regions {
            if r.x.is_some_and(|x| x >= GRID_W) || r.y.is_some_and(|y| y >= GRID_H) {
                w.push(Warning::medium(format!(
                    "geography: region `{}` at cell ({}, {}) is off the {GRID_W}×{GRID_H} map",
                    r.name, r.x.unwrap_or(0), r.y.unwrap_or(0)
                )));
            }
        }
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

/// WORLD-KEEP-1 — the lints about the MAP as compiled: a declared heightmap that
/// could not be used, a regional map whose climate still spans pole to pole, a
/// map larger than its planet, and a landmark sitting on a realm capital's
/// cell. Shared by the worldbuilder's score and `realworld validate`, so the
/// two cannot say different things about the same world.
pub fn lint_map(
    def: &WorldDefinition,
    root: Option<&std::path::Path>,
    layers: &CompiledLayers,
) -> Vec<Warning> {
    use crate::world::compile::polities_layer;
    let (geo, demo) = (&layers.geology, &layers.demographics);
    let mut out: Vec<Warning> = Vec::new();
    let dem = def.geology.as_ref().and_then(|g| g.dem.as_ref());
    // The layers were compiled DEM-aware; if a heightmap is declared and they
    // are not DEM-sourced, it could not be read (no second decode to find out).
    if let (Some(_), Some(dem)) = (root, dem) {
        if geo.source != "dem" {
            out.push(Warning::high(format!(
                "geology: the declared heightmap `{}` could not be used — showing generated terrain instead (`realworld validate` names the error)",
                dem.path
            )));
        }
    }
    let pole_to_pole = std::f64::consts::PI * 6371.0 * def.astronomy.planet.radius_earth.max(0.01);
    if let Some((_, yk)) = geo.cell_km {
        let tall_km = yk * geo.height as f64;
        // Resolved once the map says where it is: `dem.center_lat` gives it the
        // latitude band its height really covers.
        if tall_km < 0.5 * pole_to_pole && !geo.latmap.is_regional() {
            out.push(Warning::low(format!(
                "geology: the heightmap's declared scale makes the map {:.0} km tall — a region — but its climate still spans pole to pole across it ({:.0} km on this planet); add `dem.center_lat` to weather it as a region at that latitude, or omit `scale_km_per_pixel` for a whole-world map",
                tall_km, pole_to_pole
            )));
        }
    } else if let (Some(scale), true) = (dem.and_then(|d| d.scale_km_per_pixel), geo.source == "dem") {
        // A declared scale that was NOT applied: the map is planet-sized (fine)
        // or larger than the planet (a typo worth a word).
        if scale.is_finite() && scale > 0.0 {
            if let Some((_, ih)) = dem
                .and_then(|d| root.map(|r| r.join(&d.path)))
                .and_then(|p| image::image_dimensions(p).ok())
            {
                let tall_km = scale as f64 * ih as f64;
                if tall_km > 1.5 * pole_to_pole {
                    out.push(Warning::medium(format!(
                        "geology: dem.scale_km_per_pixel {scale} makes the map {:.0} km tall — larger than the planet ({:.0} km pole to pole); read as the whole planet (a typo?)",
                        tall_km, pole_to_pole
                    )));
                }
            }
        }
    }
    // A landmark on a realm capital's cell is dropped by the map (the capital
    // claims the cell first), along with every road declared to it.
    if let Some(g) = def.geography.as_ref() {
        let placed: Vec<(&str, (usize, usize))> = g
            .landmarks
            .iter()
            .filter_map(|l| l.grid_on(geo.width, geo.height, &geo.latmap).map(|c| (l.name.as_str(), c)))
            .collect();
        if !placed.is_empty() {
            let pol = polities_layer::compile_polities(demo, &def.nations, def.seed_u64());
            for (name, cell) in placed {
                if let Some(p) = pol.polities.iter().find(|p| p.capital_pos == cell) {
                    out.push(Warning::low(format!(
                        "geography: landmark `{name}` sits on the capital cell of `{}` — the map draws the capital and drops the landmark (and any road to it); move it a cell",
                        p.name
                    )));
                }
            }
        }
    }
    out
}

/// Compile every layer and run every deterministic lint, returning the aggregate
/// warnings (layer-prefixed). No LLM, no I/O beyond the pure compile chain —
/// suitable for the worldbuilder's live score.
pub fn run_fast(def: &WorldDefinition) -> Vec<Warning> {
    run_fast_at(def, None)
}

/// WORLD-KEEP-1 (WK-P3) — [`run_fast`] over the DEM-aware compile, so the score
/// judges the world the CLI compiles. A declared heightmap that cannot be read
/// is a high-severity warning (the lints below then ran on generated terrain).
pub fn run_fast_at(def: &WorldDefinition, root: Option<&std::path::Path>) -> Vec<Warning> {
    let layers = compile_layers_at(def, root);
    run_fast_with(def, root, &layers)
}

/// WORLD-KEEP-1 (WK-P5) — the lints over layers the caller has ALREADY compiled,
/// so a surface that needs both the score and the layers (the worldbuilder: ★
/// score + map + summary) compiles the world once per change instead of once per
/// consumer. With a declared heightmap that is one image decode, not three.
pub fn run_fast_with(
    def: &WorldDefinition,
    root: Option<&std::path::Path>,
    layers: &CompiledLayers,
) -> Vec<Warning> {
    use crate::world::compile::{
        culture_layer, ecology_layer, history_layer, hydrology_layer, polities_layer,
    };
    let (geo, climate, demo) = (&layers.geology, &layers.climate, &layers.demographics);
    let seed = def.seed_u64();

    let mut out: Vec<Warning> = lint_definition(def);
    out.extend(lint_map(def, root, layers));

    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = history_layer::compile_history(demo, declared_hist, &def.nations, seed);
        out.extend(
            history_layer::lint_history(declared_hist, &hist)
                .into_iter()
                .map(|w| w.prefixed("history")),
        );
    }
    if !def.nations.is_empty() {
        out.extend(
            polities_layer::lint_polities(&def.nations, demo)
                .into_iter()
                .map(|w| w.prefixed("nations")),
        );
    }
    if let Some(hy) = def.hydrology.as_ref() {
        if hy.rivers.iter().any(|r| r.from.is_some() && r.to.is_some()) {
            out.extend(
                hydrology_layer::lint_rivers(hy, geo)
                    .into_iter()
                    .map(|w| w.prefixed("rivers")),
            );
        }
    }
    if !def.cultures.is_empty() {
        let pol = polities_layer::compile_polities(demo, &def.nations, seed);
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
            ecology_layer::lint_ecology(&eco.regions, climate)
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
    fn the_dem_aware_compile_reads_the_heightmap_and_reports_a_missing_one() {
        let dir = tempfile::tempdir().unwrap();
        let mut def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("D")).unwrap();
        def.geology = Some(crate::world::types::GeologyDef {
            generated: None,
            dem: Some(serde_json::from_value(serde_json::json!({ "path": "h.png" })).unwrap()),
        });
        // Missing file: falls back to generated terrain, and says so.
        let w = run_fast_at(&def, Some(dir.path()));
        assert!(w.iter().any(|x| x.text.contains("could not be used")), "missing DEM is reported");
        assert_ne!(compile_layers_at(&def, Some(dir.path())).geology.source, "dem");
        // A real heightmap: the compile is DEM-sourced, and the warning is gone.
        let img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(64, 48, |x, y| {
            image::Luma([((x * 1000 + y * 300) % 65535) as u16])
        });
        img.save(dir.path().join("h.png")).unwrap();
        let with = compile_layers_at(&def, Some(dir.path()));
        let without = compile_layers(&def);
        assert_ne!(with.geology.source, without.geology.source, "the DEM is honoured only with a root");
        assert!(!run_fast_at(&def, Some(dir.path())).iter().any(|x| x.text.contains("could not be used")));
    }

    #[test]
    fn one_ruler_population_follows_the_planet_and_a_declared_map_scale() {
        let def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("S")).unwrap();
        let earth = compile_layers(&def);
        // An Earth-like world keeps a bronze-age population (tens of millions).
        let p = earth.demographics.total_population;
        assert!((20_000_000..80_000_000).contains(&p), "Earth-like population {p}");
        // Per-row areas exist, widest at the equator, pinched at the poles.
        let areas = &earth.climate.cell_area_km2;
        assert_eq!(areas.len(), earth.climate.height);
        assert!(areas[areas.len() / 2] > areas[0] * 10.0);
        // The same ruler travel uses: an equatorial cell is cell_km x × y.
        let (xk, yk) = crate::world::travel::cell_km(1.0, earth.geology.width, earth.geology.height);
        assert!((areas[areas.len() / 2] / (xk * yk) - 1.0).abs() < 0.01);

        // Twice the radius → four times the ground → about four times the people
        // (the old fixed cell ignored the planet's size entirely).
        let mut big = def.clone();
        big.astronomy.planet.radius_earth = 2.0;
        let ratio = compile_layers(&big).demographics.total_population as f64 / p as f64;
        assert!((3.0..5.5).contains(&ratio), "radius 2 → ~4× population, got {ratio:.2}×");

        // A heightmap WITH a scale is a region; without one it is the planet.
        let dir = tempfile::tempdir().unwrap();
        let img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(320, 240, |x, y| {
            image::Luma([(((x * 37 + y * 91) % 997) * 60) as u16])
        });
        img.save(dir.path().join("h.png")).unwrap();
        let dem = |scale: Option<f32>| {
            let mut d = def.clone();
            let mut v = serde_json::json!({ "path": "h.png" });
            if let Some(s) = scale {
                v["scale_km_per_pixel"] = serde_json::json!(s);
            }
            d.geology = Some(crate::world::types::GeologyDef { generated: None, dem: Some(serde_json::from_value(v).unwrap()) });
            d
        };
        let globe = compile_layers_at(&dem(None), Some(dir.path()));
        assert_eq!(globe.geology.cell_km, None, "no scale → the whole planet");
        let region_def = dem(Some(5.0));
        let region = compile_layers_at(&region_def, Some(dir.path()));
        // 320 px × 5 km over 160 cells = 10 km per cell, both ways.
        assert_eq!(region.geology.cell_km, Some((10.0, 10.0)));
        let d_region = crate::world::travel::distance_km_on(1.0, &region.geology, 30.0, 40.0);
        assert!((d_region - 500.0).abs() < 1e-6, "30×40 cells at 10 km = 500 km, got {d_region}");
        assert!(crate::world::travel::distance_km_on(1.0, &globe.geology, 30.0, 40.0) > 5.0 * d_region);
        assert!(region.demographics.total_population < globe.demographics.total_population / 10);
        // A scale that makes the image planet-sized IS the planet (cells narrow
        // toward the poles); one larger than the planet is a typo worth a word.
        let whole = compile_layers_at(&dem(Some(100.0)), Some(dir.path())); // 24 000 km tall
        assert_eq!(whole.geology.cell_km, None, "a planet-sized scaled map is the planet");
        assert_eq!(whole.demographics.total_population, globe.demographics.total_population);
        let huge = dem(Some(5000.0));
        assert!(run_fast_at(&huge, Some(dir.path())).iter().any(|w| w.text.contains("larger than the planet")));
        let mut bad = dem(Some(0.0));
        assert!(lint_definition(&bad).iter().any(|w| w.text.contains("scale_km_per_pixel")));
        bad = dem(None);
        assert!(!lint_definition(&bad).iter().any(|w| w.text.contains("scale_km_per_pixel")));
        // …and the region/pole-to-pole mismatch is told.
        assert!(run_fast_at(&region_def, Some(dir.path())).iter().any(|w| w.text.contains("a region")));
        assert!(!run_fast_at(&dem(None), Some(dir.path())).iter().any(|w| w.text.contains("a region")));
    }

    #[test]
    fn a_regional_heightmap_with_a_centre_latitude_is_weathered_as_that_region() {
        let def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("R")).unwrap();
        let dir = tempfile::tempdir().unwrap();
        let img = image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_fn(320, 240, |x, y| {
            image::Luma([(20_000 + ((x * 37 + y * 91) % 997) * 40) as u16])
        });
        img.save(dir.path().join("h.png")).unwrap();
        let dem = |extra: serde_json::Value| {
            let mut d = def.clone();
            let mut v = serde_json::json!({ "path": "h.png", "scale_km_per_pixel": 5.0, "sea_level_pixel_value": 1 });
            for (k, val) in extra.as_object().unwrap() {
                v[k] = val.clone();
            }
            d.geology = Some(crate::world::types::GeologyDef { generated: None, dem: Some(serde_json::from_value(v).unwrap()) });
            d
        };
        // Without center_lat: measured as a region, weathered as a globe, and told so.
        let plain_def = dem(serde_json::json!({}));
        let plain = compile_layers_at(&plain_def, Some(dir.path()));
        assert!(!plain.geology.latmap.is_regional());
        assert!(run_fast_at(&plain_def, Some(dir.path())).iter().any(|w| w.text.contains("center_lat")));

        // With center_lat 10°N: a 1200 km band in the tropics — warm everywhere,
        // no ice at the top edge. With 80°N: the same land is cold.
        let tropic_def = dem(serde_json::json!({ "center_lat": 10.0 }));
        let tropic = compile_layers_at(&tropic_def, Some(dir.path()));
        assert!(tropic.geology.latmap.is_regional());
        assert!((tropic.geology.latmap.north - tropic.geology.latmap.south - 10.79).abs() < 0.1);
        let polar = compile_layers_at(&dem(serde_json::json!({ "center_lat": 80.0 })), Some(dir.path()));
        assert!(
            tropic.climate.mean_land_temp_c > polar.climate.mean_land_temp_c + 15.0,
            "tropic {} vs polar {}",
            tropic.climate.mean_land_temp_c,
            polar.climate.mean_land_temp_c
        );
        // The band is narrow: top and bottom rows of the tropical map differ by a
        // few degrees, not the pole-to-equator range the globe mapping gives.
        let w = tropic.climate.width;
        let row_mean = |c: &crate::world::types::ClimateOutput, y: usize| {
            c.temperature_c[y * w..(y + 1) * w].iter().sum::<f32>() / w as f32
        };
        let h = tropic.climate.height;
        assert!((row_mean(&tropic.climate, 0) - row_mean(&tropic.climate, h - 1)).abs() < 15.0);
        assert!((row_mean(&plain.climate, 0) - row_mean(&plain.climate, h / 2)).abs() > 20.0);
        // The warning is resolved by the key it names.
        assert!(!run_fast_at(&tropic_def, Some(dir.path())).iter().any(|w| w.text.contains("a region")));

        // A landmark given in degrees lands on the row weathered at that latitude.
        let lm: crate::world::types::world::GeoLandmark =
            serde_json::from_value(serde_json::json!({ "name": "Port", "lat": 12.0, "lon": 0.0 })).unwrap();
        let (_, row) = lm.grid_on(w, h, &tropic.geology.latmap).unwrap();
        assert!((tropic.geology.latmap.row_lat(row, h) - 12.0).abs() < 0.1);
        let (_, globe_row) = lm.grid(w, h).unwrap();
        assert_ne!(row, globe_row, "on the globe 12°N is a different row");

        // Sanity lints.
        let mut odd = def.clone();
        odd.geology = Some(crate::world::types::GeologyDef {
            generated: None,
            dem: Some(serde_json::from_value(serde_json::json!({ "path": "h.png", "center_lat": 120.0 })).unwrap()),
        });
        let text = lint_definition(&odd).iter().map(|w| w.text.clone()).collect::<Vec<_>>().join("\n");
        assert!(text.contains("between -90 and 90") && text.contains("center_lat is ignored"), "{text}");
    }

    #[test]
    fn the_calendar_dates_a_day_with_its_month_and_weekday() {
        let mut def = crate::world::types::WorldDefinition::from_hjson(&crate::world::starter_template("C")).unwrap();
        let cal = &mut def.astronomy.calendar; // 12 × 30, 7-day week
        assert_eq!(cal.month_day(0.0), Some((1, 1)));
        assert_eq!(cal.month_day(31.0), Some((2, 2)));
        assert_eq!(cal.month_day(360.0), None, "past the last month");
        assert_eq!(cal.weekday(8.0), Some((2, None)));
        assert_eq!(cal.date_label(31.0), "day 2 of month 2 · day 4 of the 7-day week");
        assert!(cal.date_label(362.0).starts_with("intercalary day 3"));
        cal.month_names = (1..=12).map(|i| format!("M{i}")).collect();
        cal.day_names = ["Oneday", "Twoday", "Threeday", "Fourday", "Fiveday", "Sixday", "Restday"].map(String::from).to_vec();
        assert_eq!(cal.date_label(31.0), "day 2 of M2 · Fourday");
        // Non-finite and absurd days never panic or underflow.
        assert_eq!(cal.date_label(f64::NAN), "");
        assert_eq!(cal.date_label(f64::NEG_INFINITY), "");
        assert!(cal.date_label(-5.0).starts_with("day 1 of M1"));
        let _ = cal.date_label(1e300);
        // No week declared → no weekday invented.
        cal.weekdays = 0;
        assert_eq!(cal.weekday(5.0), None);
        assert_eq!(cal.date_label(31.0), "day 2 of M2");
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
        // Positions off the map are named, not silently clamped.
        def.geography = Some(serde_json::from_value(serde_json::json!({
            "landmarks": [ { "name": "Edge", "x": 9999, "y": 2 }, { "name": "Polar", "lat": 123.0, "lon": 0.0 } ],
            "regions": [ { "name": "Nowhere", "x": 1, "y": 9999 } ]
        })).unwrap());
        let text = lint_definition(&def).iter().map(|x| x.text.clone()).collect::<Vec<_>>().join("\n");
        for needle in ["landmark `Edge`", "landmark `Polar`", "region `Nowhere`"] {
            assert!(text.contains(needle), "missing lint for {needle}:\n{text}");
        }
        def.geography = None;
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
