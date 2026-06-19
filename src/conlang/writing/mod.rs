//! Writing systems (LANG-1 P5). P5.1: the glyph suitability preflight. P5.2:
//! the pure-Rust font-source compiler (`font`) — glyph SVGs → a UFO via norad.
//! P5.3: in-process UFO → TrueType (`compile`) via write-fonts (no external
//! tool). P5.4: the `font` config block (`conlang::types::font`) binds glyph
//! artwork to phonemes + codepoints in the language book — `language
//! font-import-glyph` / `font-build --language` compile a script from its own
//! definition.

pub mod compile;
pub mod font;
pub mod preflight;
