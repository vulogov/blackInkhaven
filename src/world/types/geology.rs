//! Geology-layer outputs. Deterministic from `(definition, seed)`: a procedural
//! plate model + heightmap, summarised into continents / mountains / minerals.
//!
//! The full heightmap grid is kept in-memory (for the climate / hydrology layers
//! and PNG export) but `#[serde(skip)]`-ped so the materialized World-book
//! paragraph stays a compact summary — the heightmap travels as a PNG asset, not
//! as a wall of JSON.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct GeologyOutput {
    /// Whether the model was generated from the seed or imported from a DEM.
    pub source: String,
    pub width: usize,
    pub height: usize,
    /// The land/sea threshold on the normalised heightmap (0..1).
    pub sea_level: f32,
    pub plates: Vec<Plate>,
    pub boundaries: BoundarySummary,
    /// Number of distinct landmasses above a minimum size.
    pub continents: usize,
    /// Fraction of the surface below sea level, as a percentage.
    pub sea_coverage_pct: f32,
    pub mountain_ranges: Vec<MountainRange>,
    pub minerals: Vec<MineralHint>,
    pub elevation: ElevationStats,
    /// Row-major normalised heightmap (`width * height`, 0..1). Not serialised.
    #[serde(skip)]
    pub heightmap: Vec<f32>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Plate {
    pub id: usize,
    pub seed_x: f32,
    pub seed_y: f32,
    /// Plate-motion unit vector.
    pub motion_x: f32,
    pub motion_y: f32,
    /// Continental plates ride high; oceanic plates sit low.
    pub continental: bool,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct BoundarySummary {
    pub convergent: usize,
    pub divergent: usize,
    pub transform: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MountainRange {
    /// The two plates whose convergence raised the range.
    pub plate_a: usize,
    pub plate_b: usize,
    /// Peak elevation on the normalised scale (0..1).
    pub peak_elevation: f32,
    /// Approximate number of high cells in the range.
    pub cell_count: usize,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MineralHint {
    /// e.g. "copper", "coal", "gold".
    pub mineral: String,
    /// Where it concentrates, e.g. "convergent volcanic arcs".
    pub context: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ElevationStats {
    pub min: f32,
    pub max: f32,
    pub mean: f32,
    /// Fraction of cells above sea level.
    pub land_fraction: f32,
}
