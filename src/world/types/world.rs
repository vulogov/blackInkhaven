//! The parsed `world.hjson` definition (the astronomy block in full for P0).

use serde::{Deserialize, Serialize};

/// The top-level world definition. Only the fields P0 needs are modelled;
/// `geology` / `technology` / `magic` / `compiler` parse-and-ignore for now
/// (serde drops unknown fields), so a full definition loads without error.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WorldDefinition {
    pub name: String,
    /// Accepts a decimal integer or a `0x…` hex value (HJSON renders the latter
    /// as a quoteless string); resolve via [`SeedValue::resolve`].
    #[serde(default)]
    pub seed: SeedValue,
    #[serde(default = "default_language")]
    pub primary_language: String,
    pub astronomy: AstronomyDef,
    #[serde(default)]
    pub geology: Option<GeologyDef>,
    /// Author-declared geography: named regions + landmarks. Landmarks feed the
    /// gazetteer (so the fact-checker resolves them) and materialize.
    #[serde(default)]
    pub geography: Option<GeographyDef>,
    /// Author-declared hydrology: named rivers / lakes / seas + a rainfall note.
    /// Descriptive (the procedural hydrology layer still runs); materializes.
    #[serde(default)]
    pub hydrology: Option<HydrologyDef>,
    /// Author-declared economy: tech level, currency, trade goods, resources.
    /// `resources` augment the fact-checker's known minerals.
    #[serde(default)]
    pub economy: Option<EconomyDef>,
    #[serde(default)]
    pub magic: Option<super::magic::MagicLedger>,
    /// WORLD-11 (W11-P1) — author-declared history: events merged into the
    /// generated chronology, pinned to an epoch (declared or inferred), and
    /// adoptable onto the story Timeline.
    #[serde(default)]
    pub history: Option<HistoryDef>,
    /// WORLD-11 (W11-P2) — author-declared nations. Each pins a named realm to a
    /// capital cell; the remaining settlements cluster into generated realms.
    #[serde(default)]
    pub nations: Vec<NationDef>,
    /// WORLD-11 (W11-P4) — author-pinned cultures, matched to a nation by name.
    #[serde(default)]
    pub cultures: Vec<CultureDef>,
    /// WORLD-11 (W11-P4) — author-pinned ecology, matched to a biome.
    #[serde(default)]
    pub ecology: Option<EcologyDef>,
}

/// WORLD-11 — an author-pinned culture, overriding the generated one for its
/// nation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CultureDef {
    /// The nation whose culture this pins (matched by name).
    pub nation: String,
    #[serde(default)]
    pub ethos: String,
    #[serde(default)]
    pub belief: String,
    /// A conlang typology profile, e.g. "SOV · agglutinative · tonal".
    #[serde(default)]
    pub language: String,
}

/// WORLD-11 — the declared `ecology:` block.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EcologyDef {
    #[serde(default)]
    pub regions: Vec<EcologyRegionDef>,
}

/// An author-pinned ecology for one biome, overriding the generated one.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EcologyRegionDef {
    pub biome: String,
    #[serde(default)]
    pub flora: Vec<String>,
    #[serde(default)]
    pub fauna: Vec<String>,
    #[serde(default)]
    pub keystone: String,
}

/// WORLD-11 — a declared nation.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NationDef {
    pub name: String,
    /// The capital's map cell `[x, y]`; the nearest settlement becomes its seat.
    pub capital: [usize; 2],
    /// Declared relations with other named nations (override the seeded ones).
    #[serde(default)]
    pub relations: Vec<NationRelation>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NationRelation {
    pub with: String,
    /// "allied" | "rival" | "neutral".
    pub stance: String,
}

/// WORLD-11 — the declared `history:` block.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HistoryDef {
    #[serde(default)]
    pub events: Vec<HistEventDef>,
}

/// One author-declared historical event.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HistEventDef {
    /// Years relative to the present (0); negative is the past.
    pub year: i64,
    pub title: String,
    /// Optional epoch name; when omitted, inferred from the year.
    #[serde(default)]
    pub epoch: Option<String>,
    /// Optional accepted-Place names this event happened at (for Timeline links).
    #[serde(default)]
    pub places: Option<Vec<String>>,
    #[serde(default)]
    pub description: String,
}

impl WorldDefinition {
    /// Minerals the author declared beyond the procedural geology — economy
    /// `resources` plus geology `notable_minerals` — lowercased. Fed to the
    /// economy fact-check so trading a declared resource isn't flagged.
    pub fn declared_minerals(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        if let Some(g) = self.geology.as_ref().and_then(|g| g.generated.as_ref()) {
            out.extend(g.notable_minerals.iter().map(|m| m.to_lowercase()));
        }
        if let Some(e) = self.economy.as_ref() {
            out.extend(e.resources.iter().map(|m| m.to_lowercase()));
        }
        out.sort();
        out.dedup();
        out
    }
}

fn default_language() -> String {
    "en".to_string()
}

impl WorldDefinition {
    /// Parse a `world.hjson` body. Mirrors `conlang::*::from_hjson` — HJSON in,
    /// a typed value out, a human-readable error string on failure.
    pub fn from_hjson(body: &str) -> std::result::Result<Self, String> {
        serde_hjson::from_str(body).map_err(|e| e.to_string())
    }

    /// The seed as a concrete `u64`.
    pub fn seed_u64(&self) -> u64 {
        self.seed.resolve()
    }
}

/// A world seed: an integer in the HJSON, or a `0x…` hex string (HJSON renders
/// unquoted hex as a string since it isn't a valid JSON number).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum SeedValue {
    Int(i64),
    Str(String),
}

impl Default for SeedValue {
    fn default() -> Self {
        SeedValue::Int(0)
    }
}

impl SeedValue {
    /// Resolve to a `u64`. Hex (`0x…`) and decimal strings are both accepted;
    /// an unparseable string resolves to 0 (a valid, if unhelpful, seed).
    pub fn resolve(&self) -> u64 {
        match self {
            SeedValue::Int(n) => *n as u64,
            SeedValue::Str(s) => {
                let t = s.trim();
                if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                    u64::from_str_radix(hex, 16).unwrap_or(0)
                } else {
                    t.parse::<u64>().unwrap_or(0)
                }
            }
        }
    }
}

/// The `astronomy` declaration block.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AstronomyDef {
    pub star: Star,
    pub planet: Planet,
    pub orbit: Orbit,
    #[serde(default)]
    pub moons: Vec<Moon>,
    pub calendar: Calendar,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Star {
    #[serde(default)]
    pub class: String,
    #[serde(default)]
    pub age_gyr: f64,
    /// Bolometric luminosity in solar units. Stellar mass is derived from this
    /// via the main-sequence mass–luminosity relation when not given directly.
    pub luminosity_solar: f64,
    /// Optional explicit stellar mass (solar units); overrides the derivation.
    #[serde(default)]
    pub mass_solar: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Planet {
    pub mass_earth: f64,
    pub radius_earth: f64,
    pub axial_tilt_deg: f64,
    pub day_length_hours: f64,
    #[serde(default = "default_rotation")]
    pub rotation_direction: String,
}

fn default_rotation() -> String {
    "prograde".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Orbit {
    pub semi_major_axis_au: f64,
    #[serde(default)]
    pub eccentricity: f64,
    /// The author-declared year length in planet-days. The compiler computes the
    /// physical value from Kepler's third law and flags any divergence; this
    /// declared value is advisory (the author may want a stylised calendar).
    #[serde(default)]
    pub year_length_days: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Moon {
    pub name: String,
    #[serde(default)]
    pub mass_lunar: f64,
    /// Sidereal orbital period around the planet, in Earth-days.
    pub period_days: f64,
    #[serde(default)]
    pub eccentricity: f64,
}

/// The `geology` declaration block: either `generated` (procedural, from the
/// seed) or `dem` (import an external heightmap). If both are present, `dem`
/// wins; if neither, a default generated geology is used.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GeologyDef {
    #[serde(default)]
    pub generated: Option<GeneratedGeology>,
    #[serde(default)]
    pub dem: Option<DemGeology>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeneratedGeology {
    #[serde(default = "default_plates")]
    pub plates: u32,
    #[serde(default = "default_continents")]
    pub continents: u32,
    /// `active` | `quiet` | `ancient` — drives mountain elevation.
    #[serde(default = "default_orogeny")]
    pub mountain_orogeny: String,
    /// 0.0 (no oceans) … 1.0 (drowned world); the land/sea threshold.
    #[serde(default = "default_sea_level")]
    pub sea_level: f32,
    /// `quiet` | `moderate` | `active` — descriptive volcanism note (materializes).
    #[serde(default)]
    pub volcanism: String,
    /// `sparse` | `normal` | `rich` — descriptive mineral-wealth note.
    #[serde(default)]
    pub mineral_richness: String,
    /// Minerals the author asserts the land yields, beyond the procedural hints —
    /// fed to the economy fact-check via [`WorldDefinition::declared_minerals`].
    #[serde(default)]
    pub notable_minerals: Vec<String>,
}

impl Default for GeneratedGeology {
    fn default() -> Self {
        GeneratedGeology {
            plates: default_plates(),
            continents: default_continents(),
            mountain_orogeny: default_orogeny(),
            sea_level: default_sea_level(),
            volcanism: String::new(),
            mineral_richness: String::new(),
            notable_minerals: Vec::new(),
        }
    }
}

/// The `geography` declaration block — author-named regions + landmarks.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GeographyDef {
    #[serde(default)]
    pub regions: Vec<GeoRegion>,
    #[serde(default)]
    pub landmarks: Vec<GeoLandmark>,
}

/// A named region the author asserts (climate/biome hints for the World book).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoRegion {
    pub name: String,
    #[serde(default)]
    pub biome: String,
    #[serde(default)]
    pub climate: String,
    #[serde(default)]
    pub description: String,
    /// Optional anchor cell (0-based grid `x`/`y`, row 0 = north). MAPED-P4 —
    /// the worldbuilder's map editor places a region at a point and shows it
    /// there; absent for the descriptive-only regions of earlier definitions.
    #[serde(default)]
    pub x: Option<usize>,
    #[serde(default)]
    pub y: Option<usize>,
}

/// A named landmark (city, port, mountain, …). Cities/ports with a `climate_zone`
/// become gazetteer entries the fact-checker can resolve by name. WORLD-14: a
/// landmark given a position (`lat`/`lon` degrees, or raw grid `x`/`y`) is also
/// drawn on the plakat map.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoLandmark {
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub climate_zone: String,
    #[serde(default)]
    pub population: u64,
    #[serde(default)]
    pub description: String,
    /// Optional position — geographic degrees (preferred) …
    #[serde(default)]
    pub lat: Option<f64>,
    #[serde(default)]
    pub lon: Option<f64>,
    /// … or raw grid cells (0-based; row 0 = north).
    #[serde(default)]
    pub x: Option<usize>,
    #[serde(default)]
    pub y: Option<usize>,
}

impl GeoLandmark {
    /// The landmark's grid cell on a `w × h` map: raw `x`/`y` if given, else the
    /// cell-centre conversion of `lat`/`lon` (matching the climate convention),
    /// else `None` when the landmark carries no position.
    pub fn grid(&self, w: usize, h: usize) -> Option<(usize, usize)> {
        if let (Some(x), Some(y)) = (self.x, self.y) {
            return Some((x.min(w.saturating_sub(1)), y.min(h.saturating_sub(1))));
        }
        if let (Some(lat), Some(lon)) = (self.lat, self.lon) {
            if w == 0 || h == 0 {
                return None;
            }
            let gy = ((90.0 - lat.clamp(-90.0, 90.0)) / 180.0 * h as f64 - 0.5)
                .round()
                .clamp(0.0, (h - 1) as f64) as usize;
            let gx = ((lon.clamp(-180.0, 180.0) + 180.0) / 360.0 * w as f64 - 0.5)
                .round()
                .clamp(0.0, (w - 1) as f64) as usize;
            return Some((gx, gy));
        }
        None
    }
}

/// The `hydrology` declaration block — author-named waters + a rainfall note.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct HydrologyDef {
    /// `arid` | `temperate` | `wet` — descriptive.
    #[serde(default)]
    pub rainfall: String,
    #[serde(default)]
    pub rivers: Vec<NamedWater>,
    #[serde(default)]
    pub lakes: Vec<NamedWater>,
    #[serde(default)]
    pub seas: Vec<NamedWater>,
}

/// A named body of water.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamedWater {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// WORLD-11 (W11-P3) — optional declared source cell `[x, y]` (rivers).
    #[serde(default)]
    pub from: Option<[usize; 2]>,
    /// WORLD-11 (W11-P3) — optional declared mouth cell `[x, y]` (rivers).
    #[serde(default)]
    pub to: Option<[usize; 2]>,
}

/// The `economy` declaration block.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct EconomyDef {
    /// e.g. `bronze` | `iron` | `medieval` | `industrial`.
    #[serde(default)]
    pub tech_level: String,
    #[serde(default)]
    pub currency: String,
    #[serde(default)]
    pub trade_goods: Vec<String>,
    /// Resources the economy is built on — augment the fact-checker's minerals.
    #[serde(default)]
    pub resources: Vec<String>,
}

fn default_plates() -> u32 {
    7
}
fn default_continents() -> u32 {
    4
}
fn default_orogeny() -> String {
    "active".to_string()
}
fn default_sea_level() -> f32 {
    0.4
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DemGeology {
    pub path: String,
    #[serde(default = "default_dem_scale")]
    pub scale_km_per_pixel: f32,
    /// Pixel values at or below this are treated as sea.
    #[serde(default)]
    pub sea_level_pixel_value: Option<u16>,
}

fn default_dem_scale() -> f32 {
    5.0
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Calendar {
    pub months: u32,
    pub month_length_days: u32,
    #[serde(default)]
    pub weekdays: u32,
    #[serde(default)]
    pub month_names: Vec<String>,
    #[serde(default)]
    pub day_names: Vec<String>,
    #[serde(default)]
    pub new_year_aligns_to: Option<String>,
}

#[cfg(test)]
mod region_anchor_tests {
    use super::*;

    #[test]
    fn georegion_anchor_is_optional_and_round_trips() {
        // MAPED-P4 — a legacy descriptive region (no anchor) still parses …
        let legacy: GeoRegion =
            serde_hjson::from_str(r#"{ name: "The Reach", biome: "steppe" }"#).unwrap();
        assert_eq!(legacy.x, None);
        assert_eq!(legacy.y, None);
        // … and an anchored one round-trips its cell.
        let anchored: GeoRegion =
            serde_hjson::from_str(r#"{ name: "Harbor Coast", biome: "temperate_forest", x: 40, y: 22 }"#)
                .unwrap();
        assert_eq!((anchored.x, anchored.y), (Some(40), Some(22)));
        let json = serde_json::to_value(&anchored).unwrap();
        assert_eq!(json["x"], 40);
        assert_eq!(json["y"], 22);
    }
}
