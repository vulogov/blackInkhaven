//! Diachronics (LANG-1 P4.1).
//!
//! A language can derive from a proto-language by an ordered chain of
//! **sound changes** — which are, notationally and mechanically, the same as
//! allophony rules (`p > f / _ #`, `k > h / V _ V`), so the engine reuses the
//! generic ordered-rewrite (`phonology::rewrite`). The diachronics block lives
//! in the Phonology chapter as `{ diachronics: { proto, rules } }`.

use serde::Deserialize;

use super::AllophonyRule;

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Diachronics {
    /// Name of the parent language this derives from (the proto).
    #[serde(default)]
    pub proto: Option<String>,
    /// Ordered sound-change chain proto → this language.
    #[serde(default)]
    pub rules: Vec<AllophonyRule>,
}

impl Diachronics {
    /// Parse a `{ diachronics: { proto, rules } }` block. `None` when there
    /// are no rules (so the loader skips unrelated Phonology paragraphs).
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        #[derive(Deserialize)]
        struct Wrapper {
            #[serde(default)]
            diachronics: Diachronics,
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Wrapper>(block) {
            Ok(w) if !w.diachronics.rules.is_empty() => Ok(Some(w.diachronics)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("diachronics HJSON parse failed: {e}")),
        }
    }
}
