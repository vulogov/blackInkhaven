//! In-memory conlang types (LANG-1).
//!
//! These are reconstructed from the `Language` book's HJSON chapters — the
//! book stays the system of record (see `Documentation/PROPOSALS/LANG-1_PLAN.md`).
//! P1.1 introduces the phonological substrate: the [`Phonology`] aggregate
//! and its phoneme / template / constraint parts.

pub mod allophony;
pub mod constraint;
pub mod contact;
pub mod diachronic;
pub mod expression;
pub mod font;
pub mod grammar;
pub mod morphology;
pub mod phoneme;
pub mod romanization;
pub mod spatial;
pub mod stress;
pub mod template;
pub mod tone;
pub mod variety;

pub use allophony::{AllophonyRule, PatternAtom};
pub use constraint::PhonotacticConstraint;
pub use phoneme::{Phoneme, PhonemeKind};
pub use romanization::RomanizationScheme;
pub use stress::StressRule;
pub use template::{SyllableTemplate, TemplateRole};
pub use tone::ToneSystem;

use std::collections::BTreeMap;

use serde::Deserialize;

/// The phonological substrate of a language: the inventory, the named
/// classes templates draw from, the syllable templates per role, and the
/// phonotactic constraints a generated word must satisfy.
///
/// Deserialized from the typed HJSON block in the language's `Phonology`
/// chapter via [`Phonology::from_hjson`].
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Phonology {
    #[serde(default)]
    pub phonemes: Vec<Phoneme>,
    /// Named phoneme classes → the IPA strings they contain, e.g.
    /// `C: ["p", "t", "k"]`, `V: ["a", "e", "i"]`.
    #[serde(default)]
    pub classes: BTreeMap<String, Vec<String>>,
    /// Templates keyed by role (`root`, `prefix`, …).
    #[serde(default)]
    pub templates: BTreeMap<String, Vec<SyllableTemplate>>,
    #[serde(default)]
    pub constraints: Vec<PhonotacticConstraint>,
    /// Ordered allophony rules — underlying → surface rewrites (P1.3).
    #[serde(default)]
    pub allophony: Vec<AllophonyRule>,
    /// Primary-stress rule (P1.4). `None` = the language marks no stress.
    #[serde(default)]
    pub stress: Option<StressRule>,
    /// Named romanization schemes (P1.5). Empty = the per-phoneme `romanize`
    /// field is the only (implicit) scheme.
    #[serde(default)]
    pub romanizations: Vec<RomanizationScheme>,
    /// Name of the default scheme; falls back to the first when unset.
    #[serde(default)]
    pub default_romanization: Option<String>,
    /// Tone system (P1.6). `None` = the language is non-tonal.
    #[serde(default)]
    pub tone: Option<ToneSystem>,
    /// Upper bound on syllables per word. Parsed now; consumed by the
    /// multi-syllable compounder in a later P1 increment.
    #[serde(default = "default_max_syllables")]
    #[allow(dead_code)]
    pub max_word_syllables: usize,
}

fn default_max_syllables() -> usize {
    4
}

impl Phonology {
    /// Parse a `Phonology` from a `Phonology`-chapter paragraph body. Tries
    /// the whole body as pure HJSON first, then falls back to the first
    /// fenced ```` ```hjson ```` block (the same dual format the dictionary
    /// and meta parsers accept). An empty body yields `None`.
    pub fn from_hjson(body: &str) -> Result<Option<Self>, String> {
        if body.trim().is_empty() {
            return Ok(None);
        }
        // Pure-HJSON paragraphs (the 1.2.13 Phase D.1 format) have no fence;
        // legacy Typst-wrapped bodies do. Prefer the fenced block when one
        // exists, else parse the whole body — and propagate the real error
        // either way.
        let block = crate::language_entry::extract_hjson_block(body).unwrap_or(body);
        serde_hjson::from_str::<Self>(block)
            .map(Some)
            .map_err(|e| format!("phonology HJSON parse failed: {e}"))
    }

    pub fn phoneme(&self, ipa: &str) -> Option<&Phoneme> {
        self.phonemes.iter().find(|p| p.ipa == ipa)
    }

    pub fn kind_of(&self, ipa: &str) -> Option<PhonemeKind> {
        self.phoneme(ipa).map(|p| p.kind)
    }

    /// The IPA members of a class, or an empty slice when the class is
    /// undeclared.
    pub fn class_members(&self, name: &str) -> &[String] {
        self.classes.get(name).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The templates declared for a role, or an empty slice.
    pub fn templates_for(&self, role: TemplateRole) -> &[SyllableTemplate] {
        self.templates
            .get(role.as_str())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    /// Resolve a romanization scheme: a named one when `name` is given, else
    /// the configured default, else the first declared scheme. `None` when no
    /// scheme matches (callers fall back to the per-phoneme `romanize`).
    pub fn scheme(&self, name: Option<&str>) -> Option<&RomanizationScheme> {
        match name {
            Some(n) => self.romanizations.iter().find(|s| s.name.eq_ignore_ascii_case(n)),
            None => self
                .default_romanization
                .as_deref()
                .and_then(|d| self.romanizations.iter().find(|s| s.name.eq_ignore_ascii_case(d)))
                .or_else(|| self.romanizations.first()),
        }
    }

    /// Segment a written word into the inventory's phonemes (by IPA) using a
    /// greedy longest-grapheme match — so a multi-char romanization (`sh` →
    /// `ʃ`) is preferred over its single-char prefixes. A run that matches no
    /// phoneme is emitted one character at a time, so the function is total.
    /// This is the reverse of word rendering; full contextual deromanization
    /// (digraph disambiguation) arrives with romanization schemes in P1.4.
    pub fn segment(&self, word: &str) -> Vec<String> {
        let mut graphs: Vec<(&str, &str)> = self
            .phonemes
            .iter()
            .map(|p| (p.grapheme(), p.ipa.as_str()))
            .filter(|(g, _)| !g.is_empty())
            .collect();
        graphs.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));

        let mut out = Vec::new();
        let mut rest = word;
        'outer: while !rest.is_empty() {
            for (g, ipa) in &graphs {
                if rest.starts_with(g) {
                    out.push((*ipa).to_string());
                    rest = &rest[g.len()..];
                    continue 'outer;
                }
            }
            let ch = rest.chars().next().unwrap();
            out.push(ch.to_string());
            rest = &rest[ch.len_utf8()..];
        }
        out
    }
}
