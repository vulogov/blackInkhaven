//! Climate-layer outputs. Deterministic from astronomy (insolation by latitude)
//! + geology (heightmap): a zonal model → per-cell temperature / precipitation /
//! biome, aggregated into climate zones, plus the prevailing-wind bands.

use serde::{Deserialize, Serialize};

/// Köppen-style biome classes (a fiction-grade subset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum Biome {
    IceCap,
    Tundra,
    Taiga,
    TemperateForest,
    TemperateGrassland,
    Mediterranean,
    ColdDesert,
    HotDesert,
    Savanna,
    TropicalSeasonal,
    TropicalRainforest,
    Ocean,
}

impl Biome {
    pub fn as_str(&self) -> &'static str {
        match self {
            Biome::IceCap => "ice_cap",
            Biome::Tundra => "tundra",
            Biome::Taiga => "taiga",
            Biome::TemperateForest => "temperate_forest",
            Biome::TemperateGrassland => "temperate_grassland",
            Biome::Mediterranean => "mediterranean",
            Biome::ColdDesert => "cold_desert",
            Biome::HotDesert => "hot_desert",
            Biome::Savanna => "savanna",
            Biome::TropicalSeasonal => "tropical_seasonal",
            Biome::TropicalRainforest => "tropical_rainforest",
            Biome::Ocean => "ocean",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClimateOutput {
    pub width: usize,
    pub height: usize,
    /// Area-weighted mean surface temperature (°C) over land.
    pub mean_land_temp_c: f32,
    /// Area-weighted mean precipitation (mm/yr) over land.
    pub mean_land_precip_mm: f32,
    /// Land biomes, aggregated and ranked by area.
    pub zones: Vec<ClimateZone>,
    pub winds: Vec<WindBand>,
    /// Per-cell biome (`width * height`, row-major). Not serialised.
    #[serde(skip)]
    pub biome: Vec<Biome>,
    /// Per-cell temperature (°C). Not serialised.
    #[serde(skip)]
    pub temperature_c: Vec<f32>,
    /// Per-cell precipitation (mm/yr). Not serialised.
    #[serde(skip)]
    pub precipitation_mm: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ClimateZone {
    pub biome: String,
    /// Share of land area, as a percentage.
    pub area_pct: f32,
    pub temp_min_c: f32,
    pub temp_max_c: f32,
    pub precip_min_mm: f32,
    pub precip_max_mm: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WindBand {
    pub lat_min: f32,
    pub lat_max: f32,
    /// e.g. "trade winds", "westerlies", "polar easterlies".
    pub name: String,
    /// "easterly" | "westerly".
    pub direction: String,
}
