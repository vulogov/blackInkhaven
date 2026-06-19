//! Writing systems (LANG-1 P5). P5.1: the glyph suitability preflight. P5.2:
//! the pure-Rust font-source compiler (`font`) — glyph SVGs → a UFO via norad.
//! In-process UFO → TTF/OTF (fontc) and the writing-system data model land in
//! P5.3.

pub mod font;
pub mod preflight;
