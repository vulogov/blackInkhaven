//! Writing systems (LANG-1 P5). P5.1: the glyph suitability preflight. P5.2:
//! the pure-Rust font-source compiler (`font`) — glyph SVGs → a UFO via norad.
//! P5.3: in-process UFO → TrueType (`compile`) via write-fonts (no external
//! tool). The writing-system data model + glyph→phoneme binding land next.

pub mod compile;
pub mod font;
pub mod preflight;
