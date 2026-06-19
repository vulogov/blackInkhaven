//! Cross-book conlang links (LANG-1 P2.6).
//!
//! Worldbuilding integration: a Place's primary/secondary language and a
//! Character's per-language proficiency. The Places / Characters system books
//! are *prose*, not structured HJSON, so the links live in a
//! `.inkhaven/conlang-links.json` **sidecar** (the advisory-sidecar pattern,
//! atomic writes via `io_atomic`), keyed by node name. Nothing in those books
//! is modified. Read by `language speakers` and (later) the AI dialog
//! generator, which adjusts a character's fluency to their declared level.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Declared fluency of a character in a language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Native,
    Fluent,
    Conversational,
    Broken,
    ReadingOnly,
}

impl Level {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "native" => Some(Self::Native),
            "fluent" => Some(Self::Fluent),
            "conversational" | "conversant" => Some(Self::Conversational),
            "broken" | "basic" => Some(Self::Broken),
            "reading_only" | "reading" => Some(Self::ReadingOnly),
            _ => None,
        }
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Fluent => "fluent",
            Self::Conversational => "conversational",
            Self::Broken => "broken",
            Self::ReadingOnly => "reading_only",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Proficiency {
    pub language: String,
    pub level: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PlaceLink {
    #[serde(default)]
    pub primary: Option<String>,
    #[serde(default)]
    pub secondary: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct CharacterLink {
    #[serde(default)]
    pub languages: Vec<Proficiency>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConlangLinks {
    #[serde(default)]
    pub places: BTreeMap<String, PlaceLink>,
    #[serde(default)]
    pub characters: BTreeMap<String, CharacterLink>,
}

impl ConlangLinks {
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("conlang-links.json")
    }

    pub fn load(project_root: &Path) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }

    /// Set a place's primary language (replacing any prior primary).
    pub fn set_place_primary(&mut self, place: &str, language: &str) {
        self.places.entry(place.to_string()).or_default().primary = Some(language.to_string());
    }

    /// Add a secondary language to a place (idempotent, case-insensitive).
    pub fn add_place_secondary(&mut self, place: &str, language: &str) {
        let link = self.places.entry(place.to_string()).or_default();
        if !link.secondary.iter().any(|l| l.eq_ignore_ascii_case(language)) {
            link.secondary.push(language.to_string());
        }
    }

    /// Set (or update) a character's proficiency in a language.
    pub fn set_character_proficiency(&mut self, character: &str, language: &str, level: Level) {
        let link = self.characters.entry(character.to_string()).or_default();
        if let Some(p) = link.languages.iter_mut().find(|p| p.language.eq_ignore_ascii_case(language)) {
            p.level = level.as_str().to_string();
        } else {
            link.languages.push(Proficiency {
                language: language.to_string(),
                level: level.as_str().to_string(),
            });
        }
    }

    /// Places that speak `language` (primary or secondary) and characters
    /// with any declared proficiency in it.
    pub fn speakers_of(&self, language: &str) -> (Vec<String>, Vec<(String, String)>) {
        let mut places: Vec<String> = self
            .places
            .iter()
            .filter(|(_, l)| {
                l.primary.as_deref().is_some_and(|p| p.eq_ignore_ascii_case(language))
                    || l.secondary.iter().any(|s| s.eq_ignore_ascii_case(language))
            })
            .map(|(name, _)| name.clone())
            .collect();
        places.sort();
        let mut chars: Vec<(String, String)> = self
            .characters
            .iter()
            .filter_map(|(name, l)| {
                l.languages
                    .iter()
                    .find(|p| p.language.eq_ignore_ascii_case(language))
                    .map(|p| (name.clone(), p.level.clone()))
            })
            .collect();
        chars.sort();
        (places, chars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn level_parse_is_lenient() {
        assert_eq!(Level::parse("Native"), Some(Level::Native));
        assert_eq!(Level::parse("reading-only"), Some(Level::ReadingOnly));
        assert_eq!(Level::parse("basic"), Some(Level::Broken));
        assert_eq!(Level::parse("nonsense"), None);
    }

    #[test]
    fn place_and_character_links_round_trip_in_speakers() {
        let mut links = ConlangLinks::default();
        links.set_place_primary("Tirion", "Quenya");
        links.add_place_secondary("Tirion", "Sindarin");
        links.set_place_primary("Menegroth", "Sindarin");
        links.set_character_proficiency("Erendil", "Quenya", Level::Native);
        links.set_character_proficiency("Erendil", "Sindarin", Level::Fluent);

        let (qya_places, qya_chars) = links.speakers_of("quenya");
        assert_eq!(qya_places, vec!["Tirion"]);
        assert_eq!(qya_chars, vec![("Erendil".to_string(), "native".to_string())]);

        let (sjn_places, sjn_chars) = links.speakers_of("Sindarin");
        assert_eq!(sjn_places, vec!["Menegroth", "Tirion"]); // primary + secondary, sorted
        assert_eq!(sjn_chars, vec![("Erendil".to_string(), "fluent".to_string())]);
    }

    #[test]
    fn updating_proficiency_replaces_not_duplicates() {
        let mut links = ConlangLinks::default();
        links.set_character_proficiency("Mara", "Drow", Level::Broken);
        links.set_character_proficiency("Mara", "Drow", Level::Fluent);
        let link = &links.characters["Mara"];
        assert_eq!(link.languages.len(), 1);
        assert_eq!(link.languages[0].level, "fluent");
    }
}
