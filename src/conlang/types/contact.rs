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

/// A language's contact profile (LANG-2 P3) — its membership in a *linguistic
/// area* (Sprachbund): the neighbouring languages it is in contact with, and the
/// typological features that have converged across the area. Declared as a
/// `{ contact: { … } }` block in the Grammar chapter.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Contact {
    /// The name of the contact area / region.
    #[serde(default)]
    pub region: String,
    /// Neighbouring languages this one is in contact with.
    #[serde(default)]
    pub with: Vec<String>,
    /// Areal (Sprachbund) features that have spread across the area — typology
    /// feature id → the converged value.
    #[serde(default)]
    pub areal_features: BTreeMap<String, String>,
}

impl Contact {
    /// `true` when the block carries no real information (so the loader skips it).
    fn is_empty(&self) -> bool {
        self.region.trim().is_empty() && self.with.is_empty() && self.areal_features.is_empty()
    }

    /// Parse a `{ contact: { … } }` block. `None` when there is no contact info.
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        match serde_hjson::from_str::<ContactWrap>(block) {
            Ok(w) => Ok(w.contact.filter(|c| !c.is_empty())),
            Err(e) => Err(format!("contact HJSON parse failed: {e}")),
        }
    }
}

#[derive(Deserialize)]
struct ContactWrap {
    #[serde(default)]
    contact: Option<Contact>,
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

    #[test]
    fn parses_contact() {
        let body = r#"{ contact: {
            region: "the Inner Sea", with: ["Sindar", "Khuz"],
            areal_features: { word_order: "sov", alignment: "ergative_absolutive" }
        } }"#;
        let c = Contact::from_hjson(body).unwrap().unwrap();
        assert_eq!(c.region, "the Inner Sea");
        assert_eq!(c.with, vec!["Sindar".to_string(), "Khuz".to_string()]);
        assert_eq!(c.areal_features.get("word_order").unwrap(), "sov");
        assert!(Contact::from_hjson("{ contact: {} }").unwrap().is_none());
    }
}
