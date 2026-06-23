//! Demographics-layer outputs. Deterministic from climate (biome → carrying
//! capacity) + hydrology (settlement priors → where people cluster) + geology.
//! Layer 5 is where the deterministic substrate meets author choice: the
//! settlement *list* is computed here, but turning settlements into named Place
//! records flows through the proposal queue (the author accepts each).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct DemographicsOutput {
    /// Total modelled population over the habitable surface.
    pub total_population: u64,
    /// Fraction of land that is habitable (capacity > 0).
    pub habitable_fraction: f32,
    /// Settlements, ranked largest-first (a Rank-Size hierarchy).
    pub settlements: Vec<Settlement>,
    pub size_classes: SizeClassSummary,
    /// Plausible social role types for this world (heuristic; AI-elaborated
    /// prose + per-settlement detail arrive with the proposal queue).
    pub role_archetypes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Settlement {
    pub x: usize,
    pub y: usize,
    pub population: u64,
    /// "city" | "town" | "village".
    pub class: String,
    /// The hydrology prior it sits on: "river_mouth" | "confluence" | "fertile_valley".
    pub basis: String,
    pub biome: String,
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
pub struct SizeClassSummary {
    pub cities: usize,
    pub towns: usize,
    pub villages: usize,
}
