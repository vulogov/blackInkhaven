//! WORLD-4 compilation layers. P0 = astronomy (the others land in P1/P2).

pub mod astronomy_layer;
pub mod climate_layer;
pub mod geology_layer;

pub use astronomy_layer::compile_astronomy;
pub use climate_layer::compile_climate;
pub use geology_layer::compile_geology;
