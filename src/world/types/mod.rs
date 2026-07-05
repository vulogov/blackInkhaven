//! WORLD-4 value types: the parsed world definition and the per-layer outputs.
//!
//! The top-level definition (name / seed / primary language) plus the per-layer
//! declaration blocks (`astronomy`, and optional `geology` / `climate` / `magic`
//! overrides) drive the five compile layers, each of which produces a populated
//! `*Output` here. Layers without an explicit block are generated from the seed.

// A deliberate flat re-export API (`crate::world::types::*`): some names are
// consumed via their submodule path or by field access rather than through the
// flat alias, so the binary-crate `unused_imports` lint mis-fires on them.
#![allow(unused_imports)]

pub mod astronomy;
pub mod climate;
pub mod demographics;
pub mod geology;
pub mod hydrology;
pub mod magic;
pub mod world;

pub use astronomy::{
    AstronomyOutput, CalendarCheck, EclipsePotential, InsolationBand, MoonOutput, SeasonMarker,
    TideContribution, TideSummary,
};
pub use climate::{Biome, ClimateOutput, ClimateZone, WindBand};
pub use demographics::{DemographicsOutput, Settlement, SizeClassSummary};
pub use geology::{
    BoundarySummary, ElevationStats, GeologyOutput, MineralHint, MountainRange, Plate,
};
pub use hydrology::{HydrologyOutput, RiverSummary, SettlementPrior};
pub use magic::{Applicability, CheckContext, MagicLedger, MagicRule};
pub use world::{
    AstronomyDef, Calendar, CultureDef, DemGeology, EcologyDef, EcologyRegionDef, GeneratedGeology,
    GeologyDef, HistEventDef, HistoryDef, HydrologyDef, Moon, NamedWater, NationDef, NationRelation,
    Orbit, Planet, SeedValue, Star, WorldDefinition,
};
