//! Input method — typing a constructed script (LANG-1 P5.6c).
//!
//! A constructed script is bound to Unicode codepoints in the `font` block,
//! each glyph carrying the phoneme (or romanization key) it stands for. Typing
//! the script, then, is a transliteration: greedily match the longest glyph
//! key at each position of the romanized input and emit that glyph's codepoint.
//! Digraph keys (`th`, `ka`) win over their prefixes because keys are tried
//! longest-first. Pure + deterministic; this is the engine a live editor input
//! mode would drive.

use crate::conlang::types::font::FontConfig;

#[derive(Debug, Default)]
pub struct ScriptOut {
    /// The transliterated text — a string of glyph codepoints (renders in the
    /// generated font) with unmatched characters passed through verbatim.
    pub script: String,
    /// Number of input runs that mapped to a glyph.
    pub mapped: usize,
    /// Input characters that matched no glyph key (passed through).
    pub unmatched: Vec<char>,
}

/// Transliterate romanized/phonemic `text` into the script's codepoints using
/// the `font` block's glyph→phoneme→codepoint bindings.
pub fn to_script(font: &FontConfig, text: &str) -> ScriptOut {
    // (key, codepoint) pairs, longest key first for greedy matching.
    let mut keys: Vec<(Vec<char>, char)> = font
        .glyphs
        .iter()
        .filter_map(|g| {
            let key = g.phoneme.as_deref()?;
            let cp = g.codepoint?;
            (!key.is_empty()).then(|| (key.chars().collect(), cp))
        })
        .collect();
    keys.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    let chars: Vec<char> = text.chars().collect();
    let mut out = ScriptOut::default();
    let mut i = 0;
    while i < chars.len() {
        let matched = keys
            .iter()
            .find(|(k, _)| i + k.len() <= chars.len() && chars[i..i + k.len()] == k[..]);
        match matched {
            Some((k, cp)) => {
                out.script.push(*cp);
                out.mapped += 1;
                i += k.len();
            }
            None => {
                let c = chars[i];
                out.script.push(c);
                if !c.is_whitespace() {
                    out.unmatched.push(c);
                }
                i += 1;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::font::FontGlyph;

    fn font(glyphs: &[(&str, &str, char)]) -> FontConfig {
        FontConfig {
            glyphs: glyphs
                .iter()
                .map(|(name, ph, cp)| FontGlyph {
                    name: name.to_string(),
                    codepoint: Some(*cp),
                    phoneme: Some(ph.to_string()),
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn greedy_longest_key_wins() {
        // keys: k, a, ta — "kata" → k, a, ta (the digraph beats t+a).
        let f = font(&[("k", "k", '\u{E000}'), ("a", "a", '\u{E001}'), ("ta", "ta", '\u{E002}')]);
        let out = to_script(&f, "kata");
        assert_eq!(out.script, "\u{E000}\u{E001}\u{E002}");
        assert_eq!(out.mapped, 3);
        assert!(out.unmatched.is_empty());
    }

    #[test]
    fn single_phoneme_keys() {
        let f = font(&[("k", "k", '\u{E000}'), ("a", "a", '\u{E001}'), ("t", "t", '\u{E003}')]);
        let out = to_script(&f, "kata");
        assert_eq!(out.script, "\u{E000}\u{E001}\u{E003}\u{E001}");
        assert_eq!(out.mapped, 4);
    }

    #[test]
    fn unmatched_passes_through() {
        let f = font(&[("k", "k", '\u{E000}'), ("a", "a", '\u{E001}')]);
        let out = to_script(&f, "k x a");
        // 'x' is unmatched; spaces pass through but aren't flagged.
        assert_eq!(out.unmatched, vec!['x']);
        assert!(out.script.starts_with('\u{E000}'));
        assert!(out.script.contains(' '));
    }

    #[test]
    fn empty_font_passes_everything_through() {
        let out = to_script(&FontConfig::default(), "abc");
        assert_eq!(out.script, "abc");
        assert_eq!(out.mapped, 0);
        assert_eq!(out.unmatched, vec!['a', 'b', 'c']);
    }
}
