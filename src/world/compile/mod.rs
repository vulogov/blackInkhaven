//! WORLD-4 compilation layers. P0 = astronomy (the others land in P1/P2).

pub mod astronomy_layer;
pub mod climate_layer;
pub mod culture_layer;
pub mod demographics_layer;
pub mod ecology_layer;
pub mod geology_layer;
pub mod history_layer;
pub mod hydrology_layer;
pub mod polities_layer;
pub mod trade_layer;

pub use astronomy_layer::compile_astronomy;
pub use climate_layer::compile_climate;
pub use culture_layer::compile_culture;
pub use demographics_layer::compile_demographics;
pub use ecology_layer::compile_ecology;
pub use geology_layer::{compile_geology, compile_geology_dem};

/// Compile the geology layer the way the CLI does: from the declared DEM when
/// `geology.dem` is present (its `path` resolved against `project_root`), else
/// generated from the seed. Every surface that shows the world should use this,
/// so a DEM world never reads as its procedural twin.
pub fn compile_geology_at(
    def: &crate::world::types::WorldDefinition,
    project_root: &std::path::Path,
) -> Result<crate::world::types::GeologyOutput, String> {
    match def.geology.as_ref().and_then(|g| g.dem.as_ref()) {
        Some(dem) => compile_geology_dem(def, &project_root.join(&dem.path)),
        None => Ok(compile_geology(def)),
    }
}

/// [`compile_geology_at`] for display surfaces: a DEM that fails to load falls
/// back to the generated terrain (the compile path reports the error itself).
pub fn compile_geology_at_or_generated(
    def: &crate::world::types::WorldDefinition,
    project_root: &std::path::Path,
) -> crate::world::types::GeologyOutput {
    compile_geology_at(def, project_root).unwrap_or_else(|_| compile_geology(def))
}
pub use history_layer::compile_history;
pub use hydrology_layer::compile_hydrology;
pub use polities_layer::compile_polities;
pub use trade_layer::compile_trade;
