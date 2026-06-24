//! EPUB **import** (the inverse of `crate::epub` export).
//!
//! Reads a `.epub` and materialises it as an inkhaven Book → Chapters →
//! Paragraphs, mirroring `crate::scrivener::import`. Built on the
//! in-tree `zip` + `quick-xml` (no new deps). Untrusted input is parsed
//! defensively — malformed bytes error rather than panic.
//!
//! NOTE: `allow(dead_code)` is temporary while the feature is built up
//! phase by phase — the package/xhtml parsers land before the CLI
//! orchestrator that consumes them; the allow is removed once wired.
#![allow(dead_code)]

pub mod package;
pub mod xhtml;
