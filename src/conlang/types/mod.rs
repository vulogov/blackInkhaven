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
pub mod norm;
pub mod phoneme;
pub mod romanization;
pub mod spatial;
pub mod stress;
pub mod template;
pub mod tone;
pub mod validate;
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
    /// Vowel sequences that form one syllable nucleus (LB-8), each written as
    /// the IPA of its vowels in order (`"ai"`, `"au"`). Any other adjacent
    /// vowels are hiatus — separate syllables. A diphthong declared as a
    /// phoneme of its own (`{ ipa: "ai", kind: "vowel" }`) needs no entry.
    #[serde(default)]
    pub diphthongs: Vec<String>,
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
            .map(|mut p| {
                p.normalize();
                Some(p)
            })
            .map_err(|e| format!("phonology HJSON parse failed: {e}"))
    }

    /// NFC-normalise every string the phonology compares against user input:
    /// phoneme IPA and graphemes, class members, bigram constraints, allophony
    /// rule atoms, romanization mappings and tone labels / sandhi (LB-3). Run
    /// by [`Phonology::from_hjson`]; a phonology built by hand in code can call
    /// it directly.
    pub fn normalize(&mut self) {
        for p in &mut self.phonemes {
            norm::nfc_in_place(&mut p.ipa);
            if let Some(r) = p.romanize.as_mut() {
                norm::nfc_in_place(r);
            }
        }
        for members in self.classes.values_mut() {
            members.iter_mut().for_each(norm::nfc_in_place);
        }
        for c in &mut self.constraints {
            if let PhonotacticConstraint::ForbidBigram(a, b) = c {
                norm::nfc_in_place(a);
                norm::nfc_in_place(b);
            }
        }
        self.allophony.iter_mut().for_each(AllophonyRule::normalize);
        let (units, classes) = self.rule_vocabulary();
        let (units, classes): (Vec<&str>, Vec<&str>) =
            (units.iter().map(String::as_str).collect(), classes.iter().map(String::as_str).collect());
        self.allophony.iter_mut().for_each(|r| r.resolve(&units, &classes));
        self.diphthongs.iter_mut().for_each(norm::nfc_in_place);
        for scheme in &mut self.romanizations {
            for m in &mut scheme.mappings {
                norm::nfc_in_place(&mut m.ipa);
                norm::nfc_in_place(&mut m.roman);
            }
            for r in &mut scheme.contextual {
                norm::nfc_in_place(&mut r.ipa);
                norm::nfc_in_place(&mut r.roman);
                r.before.iter_mut().for_each(PatternAtom::normalize);
                r.after.iter_mut().for_each(PatternAtom::normalize);
            }
        }
        if let Some(t) = self.tone.as_mut() {
            t.tones.iter_mut().for_each(norm::nfc_in_place);
            for members in t.classes.values_mut() {
                members.iter_mut().for_each(norm::nfc_in_place);
            }
            t.sandhi.iter_mut().for_each(AllophonyRule::normalize);
            let tones: Vec<&str> = t.tones.iter().map(String::as_str).collect();
            let tclasses: Vec<&str> = t.classes.keys().map(String::as_str).collect();
            t.sandhi.iter_mut().for_each(|r| r.resolve(&tones, &tclasses));
        }
    }

    /// The symbol space rules over phonemes resolve against: phoneme IPA and
    /// class names (owned copies, so a caller may hold `&mut self`).
    fn rule_vocabulary(&self) -> (Vec<String>, Vec<String>) {
        (
            self.phonemes.iter().map(|p| p.ipa.clone()).collect(),
            self.classes.keys().cloned().collect(),
        )
    }

    /// NFC-normalise and resolve a rule set declared *outside* the phonology
    /// block but applied over its inventory — diachronic sound changes, a
    /// variety's `sound_changes`, a morpheme's ablaut `rules` (LI-4). The
    /// phonology's own allophony and sandhi are resolved by [`Self::normalize`].
    pub fn resolve_rules(&self, rules: &mut [AllophonyRule]) {
        let (units, classes) = self.rule_vocabulary();
        let (units, classes): (Vec<&str>, Vec<&str>) =
            (units.iter().map(String::as_str).collect(), classes.iter().map(String::as_str).collect());
        for r in rules {
            r.normalize();
            r.resolve(&units, &classes);
        }
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
    ///
    /// The word and the graphemes are compared after NFC + Unicode
    /// case-folding (LB-3 / LB-9): `Pata`, `pata`, and a decomposed `pãta`
    /// all segment identically, and callers need not lowercase first. An
    /// unmatched run is emitted in its folded form.
    pub fn segment(&self, word: &str) -> Vec<String> {
        let mut graphs: Vec<(String, &str)> = self
            .phonemes
            .iter()
            .map(|p| (norm::fold(p.grapheme()), p.ipa.as_str()))
            .filter(|(g, _)| !g.is_empty())
            .collect();
        graphs.sort_by_key(|(g, _)| std::cmp::Reverse(g.chars().count()));

        let word = norm::fold(word);
        let mut out = Vec::new();
        let mut rest = word.as_str();
        'outer: while !rest.is_empty() {
            for (g, ipa) in &graphs {
                if rest.starts_with(g.as_str()) {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn phon(body: &str) -> Phonology {
        Phonology::from_hjson(body).unwrap().unwrap()
    }

    const BASIC: &str = r#"{
        phonemes: [
            { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" },
            { ipa: "ʃ", romanize: "sh", kind: "consonant" },
            { ipa: "a", kind: "vowel" }, { ipa: "ã", kind: "vowel" }
        ],
        classes: { V: ["a", "ã"] },
        allophony: [ { rule: "t > ʃ / _ ã" } ]
    }"#;

    #[test]
    fn segment_is_case_insensitive() {
        let p = phon(BASIC);
        assert_eq!(p.segment("Pata"), p.segment("pata"));
        assert_eq!(p.segment("SHATA"), vec!["ʃ", "a", "t", "a"]);
    }

    #[test]
    fn segment_composes_input_before_matching() {
        let p = phon(BASIC);
        let nfc = p.segment("p\u{00E3}ta");
        let nfd = p.segment("pa\u{0303}ta");
        assert_eq!(nfc, vec!["p", "\u{00E3}", "t", "a"]);
        assert_eq!(nfd, nfc);
    }

    #[test]
    fn from_hjson_composes_the_inventory_and_rules() {
        // The inventory, class and rule are authored in decomposed form; the
        // loaded phonology is NFC throughout, so a precomposed input matches.
        let p = phon(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "ã", kind: "vowel", romanize: "ã" } ],
            classes: { V: ["ã"] },
            constraints: [ { kind: "forbid_bigram", a: "ã", b: "ã" } ],
            allophony: [ { rule: "t > d / _ ã" } ],
            tone: { kind: "register", tones: ["ã"], sandhi: [ { rule: "ã > a / _ #" } ] }
        }"#);
        assert_eq!(p.phonemes[1].ipa, "\u{00E3}");
        assert_eq!(p.phonemes[1].romanize.as_deref(), Some("\u{00E3}"));
        assert_eq!(p.classes["V"], vec!["\u{00E3}"]);
        assert!(matches!(&p.constraints[0], PhonotacticConstraint::ForbidBigram(a, b) if a == "\u{00E3}" && b == "\u{00E3}"));
        assert_eq!(p.allophony[0].right, vec![PatternAtom::Symbol("\u{00E3}".into())]);
        let t = p.tone.as_ref().unwrap();
        assert_eq!(t.tones, vec!["\u{00E3}"]);
        assert_eq!(t.sandhi[0].lhs, vec![PatternAtom::Symbol("\u{00E3}".into())]);
        assert_eq!(p.segment("t\u{00E3}"), vec!["t", "\u{00E3}"]);
        assert!(p.phoneme("\u{00E3}").is_some());
    }

    #[test]
    fn from_hjson_splits_joined_rule_tokens_against_the_inventory() {
        // LI-4: `ai > e` and `a i > e` are the same rule; a declared `tʃ` is one
        // unit; a class name is never split; tone sandhi resolves against tones.
        let p = phon(r#"{
            phonemes: [
                { ipa: "t", kind: "consonant" }, { ipa: "s", kind: "consonant" }, { ipa: "tʃ", kind: "consonant" },
                { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "e", kind: "vowel" }
            ],
            classes: { V: ["a", "i", "e"], ia: ["a"] },
            allophony: [
                { rule: "ai > e / _ #" }, { rule: "a i > e / _ #" }, { rule: "t > ts / _ i" },
                { rule: "tʃ > s / _ #" }, { rule: "V > ∅ / _ ia" }, { rule: "t > x / _ #" }
            ],
            tone: { kind: "contour", tones: ["1", "2", "3"], sandhi: [ { rule: "33 > 23 / _ #" } ] }
        }"#);
        let sym = |s: &str| PatternAtom::Symbol(s.into());
        assert_eq!(p.allophony[0].lhs, vec![sym("a"), sym("i")]);
        assert_eq!(p.allophony[0].lhs, p.allophony[1].lhs);
        assert_eq!(p.allophony[0].rhs, p.allophony[1].rhs);
        assert_eq!(p.allophony[2].rhs, vec!["t".to_string(), "s".to_string()]);
        assert_eq!(p.allophony[3].lhs, vec![sym("tʃ")]);
        // `ia` is a declared class (and also a cover `i a`): the class wins.
        assert_eq!(p.allophony[4].right, vec![sym("ia")]);
        // An unknown surface replacement is left as the author wrote it.
        assert_eq!(p.allophony[5].rhs, vec!["x".to_string()]);
        let t = p.tone.as_ref().unwrap();
        assert_eq!(t.sandhi[0].lhs, vec![sym("3"), sym("3")]);
        assert_eq!(t.sandhi[0].rhs, vec!["2".to_string(), "3".to_string()]);
    }

    #[test]
    fn resolve_rules_covers_rule_sets_declared_outside_the_block() {
        let p = phon(BASIC);
        let mut rules = vec![serde_hjson::from_str::<AllophonyRule>("{ rule: \"pa > t / _ #\" }").unwrap()];
        p.resolve_rules(&mut rules);
        assert_eq!(rules[0].lhs, vec![PatternAtom::Symbol("p".into()), PatternAtom::Symbol("a".into())]);
    }

    #[test]
    fn unmatched_runs_are_emitted_folded_one_char_at_a_time() {
        let p = phon(BASIC);
        assert_eq!(p.segment("Xpa"), vec!["x", "p", "a"]);
    }
}
