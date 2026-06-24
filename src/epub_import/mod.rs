//! EPUB **import** (the inverse of `crate::epub` export).
//!
//! Reads a `.epub` and materialises it as an inkhaven Book → Chapters →
//! Paragraphs, mirroring `crate::scrivener::import`. Built on the
//! in-tree `zip` + `quick-xml` (no new deps). Untrusted input is parsed
//! defensively — malformed bytes error rather than panic.

pub mod import;
pub mod package;
pub mod xhtml;

pub use import::{import_epub, EpubImportOpts};
