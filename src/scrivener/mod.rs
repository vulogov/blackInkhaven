//! Scrivener (.scriv) import (1.2.4+).
//!
//! Walks a Scrivener project package, parses its binder XML,
//! converts every document's RTF body to Typst markup, and
//! materialises the hierarchy as inkhaven nodes. Single-binary —
//! no shell-out to pandoc / textutil / Scrivener itself. The
//! exposed entry point is `import_scrivener_project`; the CLI
//! at `src/cli/import_scrivener.rs` calls it.
//!
//! ## Layering
//!
//! * `binder` — parses `<name>.scrivx` into a typed
//!   `BinderItem` tree (UUID, kind, title, children).
//! * `rtf` — converts a single document's `.rtf` bytes to a
//!   string of Typst markup. Uses the `rtf-parser-tt` crate
//!   (MIT, smart-quote-aware fork of rtf-parser).
//! * `mapping` — pure-data rules mapping Scrivener
//!   `BinderItem.kind` to inkhaven `NodeKind`.
//! * `import` — orchestrates the three above against a live
//!   `Store`. Produces an `ImportReport`.

pub mod binder;
pub mod import;
pub mod mapping;
pub mod rtf;

pub use import::{import_scrivener_project, ImportOpts};
