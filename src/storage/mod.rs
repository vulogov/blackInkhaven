//! Inkhaven's storage layer.
//!
//! Owns the on-disk shape of a project: `metadata.db` (DuckDB JSON
//! rows), `blobs.db` (DuckDB BLOBs), and `vectors/` (an HNSW index
//! via `vecstore`). Replaces what used to be a `bdslib::DocumentStorage`
//! dependency.
//!
//! Public surface mirrors the slice of bdslib that `src/store/mod.rs`
//! actually called — see `Documentation/RELEASE_NOTES/1.2.md` for the
//! migration write-up.

pub mod document;
pub mod embedding;
pub mod engine;
pub mod fingerprint;
pub mod vector;

pub use document::DocumentStorage;
pub use embedding::{EmbeddingEngine, Model};
