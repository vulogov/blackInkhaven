//! Idioms + metaphors (LANG-1 P3.5).
//!
//! Language-level expression resources: multi-word **idioms** (a phrase with a
//! literal word-by-word gloss and a separate idiomatic meaning) and declared
//! conceptual **metaphors** (a source→target domain mapping). Stored as a
//! `{ idioms: [...], metaphors: [...] }` HJSON paragraph in the Grammar
//! chapter; the AI translation / text generation consults them to stay
//! idiomatic rather than literal-from-English.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Idiom {
    /// The phrase as a whole.
    pub form: String,
    /// Word-by-word literal gloss.
    #[serde(default)]
    pub literal: String,
    /// What it actually means.
    #[serde(default)]
    pub meaning: String,
    #[serde(default)]
    pub register: Vec<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Metaphor {
    /// Source domain (e.g. `JOURNEY`).
    pub source: String,
    /// Target domain (e.g. `LIFE`).
    pub target: String,
    #[serde(default)]
    pub examples: Vec<String>,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Expressions {
    #[serde(default)]
    pub idioms: Vec<Idiom>,
    #[serde(default)]
    pub metaphors: Vec<Metaphor>,
}

impl Expressions {
    /// Parse the `{ idioms: [...], metaphors: [...] }` block; `None` when
    /// empty (so the loader skips unrelated Grammar paragraphs).
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Self>(block) {
            Ok(e) if !e.idioms.is_empty() || !e.metaphors.is_empty() => Ok(Some(e)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("expressions HJSON parse failed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_idioms_and_metaphors_block() {
        let body = r#"{
            idioms: [ { form: "kala men", literal: "cold heart", meaning: "unforgiving", register: ["formal"] } ]
            metaphors: [ { source: "JOURNEY", target: "LIFE", examples: ["kala men"] } ]
        }"#;
        let e = Expressions::from_hjson(body).unwrap().unwrap();
        assert_eq!(e.idioms.len(), 1);
        assert_eq!(e.idioms[0].meaning, "unforgiving");
        assert_eq!(e.idioms[0].register, vec!["formal"]);
        assert_eq!(e.metaphors[0].source, "JOURNEY");
        // an unrelated block (no idioms/metaphors) → None.
        assert!(Expressions::from_hjson(r#"{ grammar: { word_order: "sov" } }"#).unwrap().is_none());
    }
}
