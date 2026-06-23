//! Hydrology-layer outputs. Deterministic from geology (heightmap) + climate
//! (rainfall): D8 flow → rainfall-weighted accumulation → rivers + a
//! Strahler-style order → lakes (interior pits) → watersheds → settlement
//! priors. Per-cell grids are `#[serde(skip)]`-ped; the summary is what
//! materializes into the World book.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct HydrologyOutput {
    pub width: usize,
    pub height: usize,
    /// Number of distinct rivers (counted at their ocean mouths).
    pub river_count: usize,
    /// The largest rivers, ranked by flow.
    pub major_rivers: Vec<RiverSummary>,
    /// Interior depressions with no outflow.
    pub lake_count: usize,
    /// Number of distinct drainage basins reaching the sea or a lake.
    pub watershed_count: usize,
    /// Candidate settlement sites, ranked by score (for Layer 5 to consume).
    pub settlement_priors: Vec<SettlementPrior>,
    #[serde(skip)]
    pub flow_dir: Vec<i8>,
    #[serde(skip)]
    pub flow_accum: Vec<f32>,
    #[serde(skip)]
    pub is_river: Vec<bool>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct RiverSummary {
    /// Mouth cell coordinates.
    pub mouth_x: usize,
    pub mouth_y: usize,
    /// Strahler-style stream order at the mouth.
    pub order: u8,
    /// Accumulated flow at the mouth (rainfall-weighted cells).
    pub flow: f32,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SettlementPrior {
    pub x: usize,
    pub y: usize,
    /// "river_mouth" | "confluence" | "fertile_valley".
    pub kind: String,
    pub score: f32,
}
