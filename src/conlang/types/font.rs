//! Font-generation config (LANG-1 P5.4).
//!
//! A language's writing system is described by a `{ font: { family, upm,
//! glyphs } }` HJSON block in the Phonology chapter. Each glyph binds a piece
//! of artwork (an SVG in the project glyph store) to an optional phoneme/
//! grapheme label and a Unicode codepoint, so the script is part of the
//! language definition — `language font-build --language <lang>` compiles it
//! straight from the book (via the P5.2/P5.3 pipeline) instead of a loose
//! directory of files.

use serde::{Deserialize, Deserializer};

/// The default design grid (units-per-em).
pub const DEFAULT_UPM: f64 = 1000.0;

#[derive(Debug, Clone, Deserialize)]
pub struct FontConfig {
    /// Font family name (falls back to the language name when unset).
    #[serde(default)]
    pub family: Option<String>,
    /// Design grid.
    #[serde(default = "default_upm")]
    pub upm: f64,
    /// Glyph bindings.
    #[serde(default)]
    pub glyphs: Vec<FontGlyph>,
    /// Custom spatial templates for composed blocks (P5.6). Built-in templates
    /// (`lr`/`tb`/`quad`/`stack3`) are always available; a config template of
    /// the same name overrides the built-in.
    #[serde(default)]
    pub templates: Vec<crate::conlang::types::spatial::SpatialTemplate>,
}

impl Default for FontConfig {
    fn default() -> Self {
        FontConfig { family: None, upm: DEFAULT_UPM, glyphs: Vec::new(), templates: Vec::new() }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct FontGlyph {
    /// Glyph name — also the artwork's filename stem in the glyph store
    /// (`<store>/<name>.svg`) and the UFO/`post` glyph name.
    pub name: String,
    /// Unicode codepoint this glyph renders for. Accepts a single character
    /// (`"a"`) or a hex code (`"U+E000"`, `"0xE000"`, `"E000"`).
    #[serde(default, deserialize_with = "de_codepoint")]
    pub codepoint: Option<char>,
    /// The phoneme (or romanization grapheme) this glyph stands for —
    /// documentation today, the hook for input methods later.
    #[serde(default)]
    pub phoneme: Option<String>,
}

fn default_upm() -> f64 {
    DEFAULT_UPM
}

impl FontConfig {
    /// Parse a `{ font: { … } }` block. `None` when the body is not a font
    /// block (so the loader skips unrelated Phonology paragraphs).
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Wrapper {
            font: Option<FontConfig>,
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Wrapper>(block) {
            Ok(Wrapper { font: Some(font) }) => Ok(Some(font)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("font HJSON parse failed: {e}")),
        }
    }

    /// Replace the binding with this `name`, or append it.
    pub fn upsert(&mut self, glyph: FontGlyph) {
        if let Some(slot) = self.glyphs.iter_mut().find(|g| g.name == glyph.name) {
            *slot = glyph;
        } else {
            self.glyphs.push(glyph);
        }
    }
}

/// Parse a codepoint from a single character or a hex code.
pub fn parse_codepoint(s: &str) -> Result<char, String> {
    let t = s.trim();
    if t.chars().count() == 1 {
        return Ok(t.chars().next().unwrap());
    }
    let hex = t
        .trim_start_matches("U+")
        .trim_start_matches("u+")
        .trim_start_matches("0x")
        .trim_start_matches("0X");
    u32::from_str_radix(hex, 16)
        .ok()
        .and_then(char::from_u32)
        .ok_or_else(|| format!("not a single character or hex codepoint: `{s}`"))
}

fn de_codepoint<'de, D>(d: D) -> Result<Option<char>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;
    let raw: Option<String> = Option::deserialize(d)?;
    match raw {
        None => Ok(None),
        Some(s) if s.trim().is_empty() => Ok(None),
        Some(s) => parse_codepoint(&s).map(Some).map_err(D::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_font_block() {
        let body = r#"{
            font: {
                family: "Eldar"
                upm: 1024
                glyphs: [
                    { name: "a", codepoint: "a", phoneme: "a" }
                    { name: "ka", codepoint: "U+E000", phoneme: "k" }
                ]
            }
        }"#;
        let cfg = FontConfig::from_hjson(body).unwrap().unwrap();
        assert_eq!(cfg.family.as_deref(), Some("Eldar"));
        assert_eq!(cfg.upm, 1024.0);
        assert_eq!(cfg.glyphs.len(), 2);
        assert_eq!(cfg.glyphs[0].codepoint, Some('a'));
        assert_eq!(cfg.glyphs[1].codepoint, Some('\u{E000}'));
        assert_eq!(cfg.glyphs[1].phoneme.as_deref(), Some("k"));
    }

    #[test]
    fn non_font_block_is_none() {
        let body = r#"{ diachronics: { proto: "x", rules: [] } }"#;
        assert!(FontConfig::from_hjson(body).unwrap().is_none());
    }

    #[test]
    fn upsert_replaces_by_name() {
        let mut cfg = FontConfig::default();
        cfg.upsert(FontGlyph { name: "a".into(), codepoint: Some('a'), phoneme: None });
        cfg.upsert(FontGlyph { name: "a".into(), codepoint: Some('a'), phoneme: Some("a".into()) });
        assert_eq!(cfg.glyphs.len(), 1);
        assert_eq!(cfg.glyphs[0].phoneme.as_deref(), Some("a"));
    }

    #[test]
    fn codepoint_forms() {
        assert_eq!(parse_codepoint("a").unwrap(), 'a');
        assert_eq!(parse_codepoint("U+0041").unwrap(), 'A');
        assert_eq!(parse_codepoint("0x41").unwrap(), 'A');
        assert_eq!(parse_codepoint("E000").unwrap(), '\u{E000}');
        assert!(parse_codepoint("nope").is_err());
    }
}
