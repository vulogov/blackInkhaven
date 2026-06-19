//! Grammar typological spec (LANG-1 P3.4).
//!
//! The language's answers to the typological questionnaire — a map of
//! WALS-aligned feature id → chosen value — stored as a `{ grammar: { … } }`
//! HJSON paragraph under the Grammar chapter. The catalog of features +
//! options lives in `conlang::grammar`; the AI grammar book (P6) reads these
//! tags.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GrammarSpec {
    #[serde(default)]
    pub grammar: BTreeMap<String, String>,
}

impl GrammarSpec {
    /// Parse a `{ grammar: { … } }` block. Returns `None` for an empty map so
    /// the loader can skip non-typology paragraphs in the Grammar chapter.
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Self>(block) {
            Ok(s) if !s.grammar.is_empty() => Ok(Some(s)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("grammar HJSON parse failed: {e}")),
        }
    }
}
