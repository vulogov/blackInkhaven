//! WORLD-4 value types: the parsed world definition and the per-layer outputs.
//!
//! P0 models the top-level definition (name / seed / primary language) and the
//! `astronomy` block in full; the other declaration blocks
//! (`geology` / `climate` / `technology` / `magic` / `compiler`) are accepted
//! and ignored for now — serde drops unknown fields, so a complete `world.hjson`
//! parses cleanly even though only astronomy is wired this phase.

pub mod astronomy;
pub mod world;

pub use astronomy::{
    AstronomyOutput, CalendarCheck, EclipsePotential, InsolationBand, MoonOutput, SeasonMarker,
    TideContribution, TideSummary,
};
pub use world::{
    AstronomyDef, Calendar, Moon, Orbit, Planet, SeedValue, Star, WorldDefinition,
};
