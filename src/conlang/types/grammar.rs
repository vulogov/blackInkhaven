//! Grammar typological spec (LANG-1 P3.4 · 1.7 Wave-2 typed blocks).
//!
//! The language's answers to the typological questionnaire — a map of
//! WALS-aligned feature id → chosen value — stored as a `{ grammar: { … } }`
//! HJSON paragraph under the Grammar chapter. The catalog of features +
//! options lives in `conlang::grammar`; the AI grammar book (P6) reads these
//! tags.
//!
//! 1.7 adds optional *typed grammar blocks* alongside the flat feature map — the
//! schema the later syntax engine reads. This slice ships two: `ug_parameters`
//! (principles-and-parameters settings) and `verb_classes` (verb valence). Both
//! default to empty, so existing `{ grammar: … }` blocks are unchanged.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One verb class: a name and its valence (argument structure).
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
pub struct VerbClass {
    pub name: String,
    /// `intransitive` | `transitive` | `ditransitive` | `impersonal` (validated
    /// against `conlang::grammar::VERB_VALENCES`).
    #[serde(default)]
    pub valence: String,
    /// Optional free-text note (an example verb, a subclass label).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct GrammarSpec {
    #[serde(default)]
    pub grammar: BTreeMap<String, String>,
    /// Principles-and-parameters settings (`head_final`, `pro_drop`, …), each a
    /// parameter id → value. Validated against `conlang::grammar::UG_PARAMETERS`.
    #[serde(default)]
    pub ug_parameters: BTreeMap<String, String>,
    /// The language's verb classes by valence.
    #[serde(default)]
    pub verb_classes: Vec<VerbClass>,
}

impl GrammarSpec {
    /// Whether the spec carries any typology content at all.
    pub fn has_content(&self) -> bool {
        !self.grammar.is_empty() || !self.ug_parameters.is_empty() || !self.verb_classes.is_empty()
    }

    /// Parse a `{ grammar: { … } }` block (optionally with `ug_parameters` and
    /// `verb_classes`). Returns `None` for an empty spec so the loader can skip
    /// non-typology paragraphs in the Grammar chapter.
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<Self>(block) {
            Ok(s) if s.has_content() => Ok(Some(s)),
            Ok(_) => Ok(None),
            Err(e) => Err(format!("grammar HJSON parse failed: {e}")),
        }
    }
}
