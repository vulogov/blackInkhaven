//! Language varieties (LANG-2 P1) — a *variety* is a delta on the base language:
//! a dialect, register, sociolect, or idiolect. The headline insight of LANG-2
//! is that a dialect's pronunciation differences are just an ordered set of
//! sound changes — the **same `AllophonyRule` engine** diachronics uses — only
//! applied *synchronously* (a living variety) instead of *diachronically* (a
//! daughter language). A variety may also override individual words.
//!
//! Stored as a `{ varieties: [ … ] }` block in the Grammar chapter.

use std::collections::BTreeMap;

use serde::Deserialize;

use super::AllophonyRule;

/// One variety of a language — a delta applied on top of the base forms.
#[derive(Debug, Clone, Deserialize)]
pub struct Variety {
    /// Short identifier used on the CLI (`lowland`, `formal`, `court`).
    pub id: String,
    /// What kind of variety this is.
    #[serde(default = "default_kind")]
    pub kind: String, // dialect | register | sociolect | idiolect
    /// The axis the variety varies along.
    #[serde(default)]
    pub axis: String, // region | class | age | situation | formality
    /// Social standing, free-form (`high` / `low` / `neutral`).
    #[serde(default)]
    pub prestige: Option<String>,
    /// Ordered sound changes that distinguish this variety from the base — the
    /// same SPE notation as allophony / diachronics (`t > d / V _ V`).
    #[serde(default)]
    pub sound_changes: Vec<AllophonyRule>,
    /// Per-concept lexical overrides: working-language gloss → the variety's own
    /// word for it (a *suppletive* form the sound changes don't derive).
    #[serde(default)]
    pub lexicon: BTreeMap<String, String>,
    /// Free-form note.
    #[serde(default)]
    pub note: Option<String>,
}

fn default_kind() -> String {
    "dialect".to_string()
}

impl Variety {
    /// A one-line human characterisation for listings.
    pub fn summary(&self) -> String {
        let mut bits = vec![self.kind.clone()];
        if !self.axis.is_empty() {
            bits.push(self.axis.clone());
        }
        if let Some(p) = &self.prestige {
            bits.push(format!("{p} prestige"));
        }
        bits.push(format!("{} sound change(s)", self.sound_changes.len()));
        if !self.lexicon.is_empty() {
            bits.push(format!("{} word override(s)", self.lexicon.len()));
        }
        bits.join(", ")
    }
}

/// The `{ varieties: [ … ] }` block.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Varieties {
    #[serde(default)]
    pub varieties: Vec<Variety>,
}

impl Varieties {
    /// Parse a `{ varieties: [ … ] }` block (whole body, or a fenced ```hjson
    /// block). Returns `None` when there are no varieties, so the chapter loader
    /// can skip unrelated Grammar paragraphs.
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Self>(block) {
            Ok(v) if !v.varieties.is_empty() => Ok(Some(v)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("varieties HJSON parse failed: {e}")),
        }
    }

    /// Find a variety by id (case-insensitive).
    pub fn get(&self, id: &str) -> Option<&Variety> {
        self.varieties.iter().find(|v| v.id.eq_ignore_ascii_case(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_dialect_with_sound_changes_and_overrides() {
        let body = r#"{ varieties: [
            { id: "lowland", kind: "dialect", axis: "region", prestige: "low",
              sound_changes: [ { rule: "t > d / V _ V" } ],
              lexicon: { "water": "moru" } }
        ] }"#;
        let v = Varieties::from_hjson(body).unwrap().unwrap();
        assert_eq!(v.varieties.len(), 1);
        let d = v.get("LOWLAND").unwrap(); // case-insensitive
        assert_eq!(d.kind, "dialect");
        assert_eq!(d.sound_changes.len(), 1);
        assert_eq!(d.lexicon.get("water").unwrap(), "moru");
        assert!(d.summary().contains("region"));
    }

    #[test]
    fn empty_block_is_none() {
        assert!(Varieties::from_hjson("{ varieties: [] }").unwrap().is_none());
        assert!(Varieties::from_hjson("").unwrap().is_none());
    }
}
