//! Writing systems (LANG-1 P5). P5.1: the glyph suitability preflight. P5.2:
//! the pure-Rust font-source compiler (`font`) — glyph SVGs → a UFO via norad.
//! P5.3: in-process UFO → TrueType (`compile`) via write-fonts (no external
//! tool). P5.4: the `font` config block (`conlang::types::font`) binds glyph
//! artwork to phonemes + codepoints in the language book — `language
//! font-import-glyph` / `font-build --language` compile a script from its own
//! definition. P5.5: `draft` — AI text-to-SVG glyph drafting, extracted +
//! preflighted before it can be bound (`language glyph-draft`). P5.6: `compose`
//! — place component glyphs into a `types::spatial::SpatialTemplate`'s cells,
//! the engine for both binding times: bake one precomposed block glyph at
//! font-build (`compose_block`, `language font-compose`), or arrange components
//! at layout time in a Typst quadrat (`quadrat_typst`, `language spatial-typst`).

pub mod compile;
pub mod compose;
pub mod draft;
pub mod font;
pub mod preflight;
