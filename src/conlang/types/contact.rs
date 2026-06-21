//! Language-contact types (LANG-2 P2) — how a language *nativises* a borrowed
//! word. A loanword is a **phonotactic repair**: the donor form is perceived
//! against the recipient's phoneme inventory, then any sequence the recipient's
//! phonotactics forbid is repaired. Declared as a `{ loan_phonology: { … } }`
//! block in the recipient's Phonology chapter.

use std::collections::BTreeMap;

use serde::Deserialize;

/// How a recipient language adapts borrowings.
#[derive(Debug, Clone, Deserialize)]
pub struct LoanPhonology {
    /// How illegal clusters are repaired: `epenthesis` (insert a vowel) or
    /// `deletion` (drop the offending consonant).
    #[serde(default = "default_repair")]
    pub repair: String,
    /// The vowel inserted by epenthesis. When empty, the recipient's first
    /// declared vowel is used.
    #[serde(default)]
    pub epenthetic_vowel: String,
    /// Per-sound substitutions applied during perception: a donor sound the
    /// recipient lacks → its nearest native equivalent (`θ` → `t`, `r` → `l`).
    #[serde(default)]
    pub substitutions: BTreeMap<String, String>,
}

fn default_repair() -> String {
    "epenthesis".to_string()
}

impl Default for LoanPhonology {
    fn default() -> Self {
        Self {
            repair: default_repair(),
            epenthetic_vowel: String::new(),
            substitutions: BTreeMap::new(),
        }
    }
}

#[derive(Deserialize)]
struct LoanWrap {
    #[serde(default)]
    loan_phonology: Option<LoanPhonology>,
}

impl LoanPhonology {
    /// Parse a `{ loan_phonology: { … } }` block. `None` when the paragraph has
    /// no such block (so the loader skips unrelated Phonology paragraphs).
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<LoanWrap>(block) {
            Ok(w) => Ok(w.loan_phonology),
            Err(e) => Err(format!("loan_phonology HJSON parse failed: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_loan_phonology() {
        let body = r#"{ loan_phonology: {
            repair: "epenthesis", epenthetic_vowel: "u",
            substitutions: { "θ": "t", "r": "l" }
        } }"#;
        let lp = LoanPhonology::from_hjson(body).unwrap().unwrap();
        assert_eq!(lp.repair, "epenthesis");
        assert_eq!(lp.epenthetic_vowel, "u");
        assert_eq!(lp.substitutions.get("θ").unwrap(), "t");
    }

    #[test]
    fn no_block_is_none() {
        assert!(LoanPhonology::from_hjson("{ phonemes: [] }").unwrap().is_none());
    }
}
