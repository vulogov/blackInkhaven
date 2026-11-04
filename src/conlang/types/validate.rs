//! Structural validation of a loaded phonology (LANG-BUGFIX-1, LB-1 / LB-2).
//!
//! Parsing is deliberately tolerant — an unknown constraint kind no longer
//! disables the language (LB-1) — so the checks that used to be hard parse
//! errors, plus the ones that were never made at all (LB-2: undeclared class
//! names, rule targets that can never match), live here as [`Finding`]s.
//! The CLI loader prints them as warnings; `language doctor` reports them.

use std::fmt;

use super::template::TemplateAtom;
use super::{AllophonyRule, PatternAtom, Phonology, PhonotacticConstraint};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Something is off but the block still works as the author probably
    /// intended (a bigram over a non-phoneme, a class member outside the
    /// inventory).
    Warning,
    /// Part of the block is silently ignored or can never apply (an unknown
    /// constraint kind, an undeclared class, a rule target that spans two
    /// segments).
    Error,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Warning => "warning",
            Severity::Error => "error",
        }
    }
}

/// One validation finding, located by the HJSON path it concerns
/// (`constraints[2]`, `templates.root[0] "C V N"`, `allophony[1] "ai > e"`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub severity: Severity,
    pub location: String,
    pub message: String,
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}: {}", self.severity.as_str(), self.location, self.message)
    }
}

/// What a rule / constraint symbol resolves to inside one symbol space.
enum Symbol {
    Class,
    Unit,
    /// Not a unit itself, but a concatenation of ≥ 2 declared units.
    Sequence(Vec<String>),
    Unknown,
}

/// A symbol space: the declared units (phoneme IPA, or tone labels) plus
/// the class names that may stand for a set of them.
struct Vocabulary<'a> {
    units: Vec<&'a str>,
    classes: Vec<&'a str>,
}

impl<'a> Vocabulary<'a> {
    fn classify(&self, s: &str) -> Symbol {
        if self.classes.contains(&s) {
            return Symbol::Class;
        }
        if self.units.contains(&s) {
            return Symbol::Unit;
        }
        match self.split(s) {
            Some(parts) if parts.len() >= 2 => Symbol::Sequence(parts),
            _ => Symbol::Unknown,
        }
    }

    /// Greedy longest-unit decomposition of `s`; `None` unless the whole
    /// string is covered.
    fn split(&self, s: &str) -> Option<Vec<String>> {
        let mut units: Vec<&str> = self.units.clone();
        units.sort_by_key(|u| std::cmp::Reverse(u.chars().count()));
        let mut out = Vec::new();
        let mut rest = s;
        'outer: while !rest.is_empty() {
            for u in &units {
                if !u.is_empty() && rest.starts_with(u) {
                    out.push((*u).to_string());
                    rest = &rest[u.len()..];
                    continue 'outer;
                }
            }
            return None;
        }
        Some(out)
    }
}

fn template_text(pattern: &[TemplateAtom]) -> String {
    pattern
        .iter()
        .map(|a| match a {
            TemplateAtom::Class(c) => c.clone(),
            TemplateAtom::OptionalClass(c) => format!("({c})"),
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn finding(severity: Severity, location: impl Into<String>, message: impl Into<String>) -> Finding {
    Finding { severity, location: location.into(), message: message.into() }
}

/// Explain an unresolvable symbol in a rule position (`target`, `replacement`,
/// `context`).
fn symbol_problem(vocab: &Vocabulary, what: &str, sym: &str, unit_noun: &str) -> Option<(Severity, String)> {
    match vocab.classify(sym) {
        Symbol::Class | Symbol::Unit => None,
        Symbol::Sequence(parts) => Some((
            if what == "replacement" { Severity::Warning } else { Severity::Error },
            if what == "replacement" {
                format!(
                    "{what} `{sym}` spans {} {unit_noun}s ({}) and is emitted as one segment — declare `{sym}` as a {unit_noun} if it is one",
                    parts.len(),
                    parts.join(" ")
                )
            } else {
                format!(
                    "{what} `{sym}` spans {} {unit_noun}s ({}) but a rule matches one segment — declare `{sym}` as a {unit_noun}, or write one rule per segment",
                    parts.len(),
                    parts.join(" ")
                )
            },
        )),
        Symbol::Unknown => {
            // A replacement may legitimately be a surface allophone that is not
            // in the (underlying) inventory; a target or context cannot match
            // anything that is not declared.
            if what == "replacement" {
                None
            } else {
                Some((Severity::Error, format!("{what} `{sym}` is neither a declared {unit_noun} nor a class")))
            }
        }
    }
}

fn check_rule(vocab: &Vocabulary, location: &str, rule: &AllophonyRule, unit_noun: &str, out: &mut Vec<Finding>) {
    if let Some(PatternAtom::Symbol(s)) = &rule.lhs {
        if let Some((sev, msg)) = symbol_problem(vocab, "target", s, unit_noun) {
            out.push(finding(sev, location, msg));
        }
    }
    if let Some(r) = &rule.rhs {
        if let Some((sev, msg)) = symbol_problem(vocab, "replacement", r, unit_noun) {
            out.push(finding(sev, location, msg));
        }
    }
    for atom in rule.left.iter().chain(rule.right.iter()) {
        if let PatternAtom::Symbol(s) = atom {
            if let Some((sev, msg)) = symbol_problem(vocab, "context", s, unit_noun) {
                out.push(finding(sev, location, msg));
            }
        }
    }
}

impl Phonology {
    /// Validate the block's internal references (LB-1 / LB-2). An empty
    /// result means every class, constraint, template and rule refers to
    /// something declared and can apply as written.
    pub fn validate(&self) -> Vec<Finding> {
        let mut out = Vec::new();
        let vocab = Vocabulary {
            units: self.phonemes.iter().map(|p| p.ipa.as_str()).collect(),
            classes: self.classes.keys().map(String::as_str).collect(),
        };

        // Duplicate phonemes shadow each other in every lookup.
        for (i, p) in self.phonemes.iter().enumerate() {
            if self.phonemes[..i].iter().any(|q| q.ipa == p.ipa) {
                out.push(finding(
                    Severity::Warning,
                    format!("phonemes[{i}]"),
                    format!("phoneme `{}` is declared more than once", p.ipa),
                ));
            }
        }

        for (name, members) in &self.classes {
            for m in members {
                if !vocab.units.contains(&m.as_str()) {
                    out.push(finding(
                        Severity::Warning,
                        format!("classes.{name}"),
                        format!("member `{m}` is not in the phoneme inventory"),
                    ));
                }
            }
        }

        for (i, c) in self.constraints.iter().enumerate() {
            let loc = format!("constraints[{i}]");
            match c {
                PhonotacticConstraint::Unknown(kind) => out.push(finding(
                    Severity::Error,
                    loc,
                    format!(
                        "unknown constraint kind `{kind}` — this constraint is ignored (known: max_cluster_size, no_geminate, forbid_bigram, forbid_in_onset, forbid_in_coda, sonority_sequencing)"
                    ),
                )),
                PhonotacticConstraint::ForbidInOnset(classes) | PhonotacticConstraint::ForbidInCoda(classes) => {
                    for cl in classes {
                        if !vocab.classes.contains(&cl.as_str()) {
                            out.push(finding(
                                Severity::Error,
                                loc.clone(),
                                format!("class `{cl}` is not declared in `classes` — the constraint never fires"),
                            ));
                        }
                    }
                }
                PhonotacticConstraint::ForbidBigram(a, b) => {
                    for s in [a, b] {
                        if !vocab.units.contains(&s.as_str()) {
                            out.push(finding(
                                Severity::Warning,
                                loc.clone(),
                                format!("`{s}` is not a phoneme in the inventory — the bigram can never occur"),
                            ));
                        }
                    }
                }
                PhonotacticConstraint::MaxClusterSize(_)
                | PhonotacticConstraint::NoGeminate
                | PhonotacticConstraint::SonoritySequencing => {}
            }
        }

        for (role, templates) in &self.templates {
            for (i, t) in templates.iter().enumerate() {
                let loc = format!("templates.{role}[{i}] \"{}\"", template_text(&t.pattern));
                for atom in &t.pattern {
                    let cl = atom.class_name();
                    if !vocab.classes.contains(&cl) {
                        out.push(finding(
                            Severity::Error,
                            loc.clone(),
                            format!("class `{cl}` is not declared in `classes` — the template can never be filled"),
                        ));
                    }
                }
            }
        }

        for (i, d) in self.diphthongs.iter().enumerate() {
            if crate::conlang::phonology::syllable::split_diphthong(self, d).is_none() {
                out.push(finding(
                    Severity::Warning,
                    format!("diphthongs[{i}] \"{d}\""),
                    "not a sequence of two or more declared vowels — it never forms a nucleus",
                ));
            }
        }

        for (i, r) in self.allophony.iter().enumerate() {
            check_rule(&vocab, &format!("allophony[{i}] \"{}\"", r.source), r, "phoneme", &mut out);
        }

        for (i, scheme) in self.romanizations.iter().enumerate() {
            for (j, rule) in scheme.contextual.iter().enumerate() {
                let loc = format!("romanizations[{i}].contextual[{j}] \"{}\"", rule.roman);
                if !vocab.units.contains(&rule.ipa.as_str()) {
                    out.push(finding(
                        Severity::Warning,
                        loc.clone(),
                        format!("`{}` is not a phoneme in the inventory", rule.ipa),
                    ));
                }
                for atom in rule.before.iter().chain(rule.after.iter()) {
                    if let PatternAtom::Symbol(s) = atom {
                        if let Some((sev, msg)) = symbol_problem(&vocab, "context", s, "phoneme") {
                            out.push(finding(sev, loc.clone(), msg));
                        }
                    }
                }
            }
        }

        if let Some(t) = &self.tone {
            let tones = Vocabulary {
                units: t.tones.iter().map(String::as_str).collect(),
                classes: t.classes.keys().map(String::as_str).collect(),
            };
            for (name, members) in &t.classes {
                for m in members {
                    if !tones.units.contains(&m.as_str()) {
                        out.push(finding(
                            Severity::Warning,
                            format!("tone.classes.{name}"),
                            format!("member `{m}` is not in `tone.tones`"),
                        ));
                    }
                }
            }
            if !t.tones.is_empty() {
                for (i, r) in t.sandhi.iter().enumerate() {
                    check_rule(&tones, &format!("tone.sandhi[{i}] \"{}\"", r.source), r, "tone", &mut out);
                }
            }
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn phon(body: &str) -> Phonology {
        Phonology::from_hjson(body).expect("parses").expect("non-empty")
    }

    const CLEAN: &str = r#"{
        phonemes: [
            { ipa: "p", kind: "consonant" }, { ipa: "t", kind: "consonant" }, { ipa: "k", kind: "consonant" },
            { ipa: "tʃ", kind: "consonant" }, { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }
        ],
        classes: { C: ["p", "t", "k"], V: ["a", "i"] },
        templates: { root: [ { pattern: "C V (C)" } ] },
        constraints: [ { kind: "no_geminate" }, { kind: "forbid_in_coda", classes: ["C"] }, { kind: "forbid_bigram", a: "t", b: "i" } ],
        allophony: [ { rule: "k > tʃ / _ i" }, { rule: "k > x / V _ V" } ],
        tone: { kind: "register", tones: ["H", "L"], classes: { T: ["H", "L"] }, sandhi: [ { rule: "L > H / H _" } ] }
    }"#;

    fn messages(p: &Phonology) -> Vec<String> {
        p.validate().iter().map(|f| f.to_string()).collect()
    }

    #[test]
    fn a_clean_block_has_no_findings() {
        assert_eq!(messages(&phon(CLEAN)), Vec::<String>::new());
    }

    #[test]
    fn an_unknown_constraint_kind_loads_and_is_reported() {
        // LB-1: the typo no longer locks out the language.
        let p = phon(r#"{ phonemes: [ { ipa: "a", kind: "vowel" } ], constraints: [ { kind: "bogus_kind_typo" }, { kind: "no_geminate" } ] }"#);
        assert_eq!(p.constraints.len(), 2);
        assert!(matches!(&p.constraints[0], PhonotacticConstraint::Unknown(k) if k == "bogus_kind_typo"));
        let f = p.validate();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Error);
        assert_eq!(f[0].location, "constraints[0]");
        assert!(f[0].message.contains("unknown constraint kind `bogus_kind_typo`"), "{}", f[0].message);
    }

    #[test]
    fn undeclared_classes_in_constraints_and_templates_are_errors() {
        let p = phon(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "a", kind: "vowel" } ],
            classes: { C: ["t"], V: ["a"] },
            templates: { root: [ { pattern: "C V N" } ] },
            constraints: [ { kind: "forbid_in_coda", classes: ["NoSuchClass"] } ]
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m[0].starts_with("error: constraints[0]: class `NoSuchClass` is not declared"), "{}", m[0]);
        assert!(m[1].starts_with("error: templates.root[0] \"C V N\": class `N` is not declared"), "{}", m[1]);
    }

    #[test]
    fn a_multi_segment_rule_target_is_an_error() {
        // LB-2: `ai > e` can never match — the rewriter matches one segment.
        let p = phon(r#"{
            phonemes: [ { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" }, { ipa: "e", kind: "vowel" } ],
            allophony: [ { rule: "ai > e / _" } ]
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].starts_with("error: allophony[0] \"ai > e / _\": target `ai` spans 2 phonemes (a i)"), "{}", m[0]);
    }

    #[test]
    fn unknown_context_symbols_are_errors_but_surface_replacements_are_fine() {
        let p = phon(r#"{
            phonemes: [ { ipa: "k", kind: "consonant" }, { ipa: "a", kind: "vowel" } ],
            classes: { V: ["a"] },
            allophony: [ { rule: "k > x / V _ V" }, { rule: "k > x / Q _" } ]
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].contains("context `Q` is neither a declared phoneme nor a class"), "{}", m[0]);
    }

    #[test]
    fn a_multi_segment_replacement_is_only_a_warning() {
        let p = phon(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "s", kind: "consonant" }, { ipa: "a", kind: "vowel" } ],
            allophony: [ { rule: "t > ts / _ a" } ]
        }"#);
        let f = p.validate();
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, Severity::Warning);
        assert!(f[0].message.contains("replacement `ts` spans 2 phonemes"), "{}", f[0].message);
    }

    #[test]
    fn class_members_and_bigrams_outside_the_inventory_warn() {
        let p = phon(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "a", kind: "vowel" } ],
            classes: { C: ["t", "d"] },
            constraints: [ { kind: "forbid_bigram", a: "t", b: "u" } ]
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m[0].starts_with("warning: classes.C: member `d` is not in the phoneme inventory"), "{}", m[0]);
        assert!(m[1].starts_with("warning: constraints[0]: `u` is not a phoneme"), "{}", m[1]);
    }

    #[test]
    fn tone_sandhi_is_checked_against_the_tone_labels() {
        let p = phon(r#"{
            phonemes: [ { ipa: "a", kind: "vowel" } ],
            tone: { kind: "register", tones: ["H", "L"], classes: { T: ["H", "M"] }, sandhi: [ { rule: "L > H / R _" } ] }
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m[0].starts_with("warning: tone.classes.T: member `M` is not in `tone.tones`"), "{}", m[0]);
        assert!(m[1].starts_with("error: tone.sandhi[0] \"L > H / R _\": context `R` is neither a declared tone nor a class"), "{}", m[1]);
    }

    #[test]
    fn invalid_diphthongs_warn() {
        let p = phon(r#"{
            phonemes: [ { ipa: "t", kind: "consonant" }, { ipa: "a", kind: "vowel" }, { ipa: "i", kind: "vowel" } ],
            diphthongs: ["ai", "at", "a"]
        }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m[0].starts_with("warning: diphthongs[1] \"at\": not a sequence of two or more declared vowels"), "{}", m[0]);
        assert!(m[1].starts_with("warning: diphthongs[2] \"a\": not a sequence"), "{}", m[1]);
    }

    #[test]
    fn duplicate_phonemes_warn() {
        let p = phon(r#"{ phonemes: [ { ipa: "a", kind: "vowel" }, { ipa: "a", kind: "vowel" } ] }"#);
        let m = messages(&p);
        assert_eq!(m.len(), 1, "{m:?}");
        assert!(m[0].starts_with("warning: phonemes[1]: phoneme `a` is declared more than once"), "{}", m[0]);
    }
}
