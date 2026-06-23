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
}

impl Default for GeneratedGeology {
    fn default() -> Self {
        GeneratedGeology {
            plates: default_plates(),
            continents: default_continents(),
            mountain_orogeny: default_orogeny(),
            sea_level: default_sea_level(),
        }
    }
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
