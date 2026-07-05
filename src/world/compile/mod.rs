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

pub use astronomy_layer::compile_astronomy;
pub use climate_layer::compile_climate;
pub use culture_layer::compile_culture;
pub use demographics_layer::compile_demographics;
pub use ecology_layer::compile_ecology;
pub use geology_layer::{compile_geology, compile_geology_dem};
pub use history_layer::compile_history;
pub use hydrology_layer::compile_hydrology;
pub use polities_layer::compile_polities;
