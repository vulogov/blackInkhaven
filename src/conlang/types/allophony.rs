//! Allophony rules (LANG-1 P1.3).
//!
//! Synchronic surface-form derivation: stored phonemes are *underlying*;
//! ordered allophony rules rewrite them into the *surface* form shown in the
//! editor and the grammar book. Rules use the familiar SPE string notation
//!
//!   `k > tʃ / _ i`        palatalize /k/ to [tʃ] before /i/
//!   `d > t / _ #`         final devoicing
//!   `∅ > ə / C _ C`       epenthesis between two consonants
//!   `V > ∅ / _ #`         final-vowel deletion
//!
//! `>` (or `→`) separates target from replacement; `/` introduces the
//! context; `_` marks the target slot; `#` is a word boundary; `∅` / `0` is
//! the empty string (insertion when on the left, deletion when on the right).
//! A context token is a class name when one is declared, else a literal
//! phoneme — resolved at evaluation time against the inventory.
//!
//! Target and replacement are *sequences* (LANG-IMPROVE-1 LI-4): tokens are
//! split on whitespace exactly like a context, so `a i > e / _ #`
//! monophthongises a diphthong and `t > t s / _ i` emits two segments. A
//! single joined token (`ai > e`, `t > ts`) that is neither a declared unit
//! nor a class is split against the inventory when the rule is resolved
//! ([`AllophonyRule::resolve`], run by `Phonology::normalize` and by the
//! language model for every other rule set), so both spellings are the same
//! rule.

use serde::Deserialize;

/// One element of a context (or the target): a symbol (class-or-phoneme,
/// resolved against the inventory at eval time) or a word boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternAtom {
    Symbol(String),
    Boundary,
}

/// A single ordered rewrite rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllophonyRule {
    pub name: String,
    /// Target segments, matched in order. Empty = ∅ (the rule inserts).
    pub lhs: Vec<PatternAtom>,
    /// Replacement segments, emitted in order. Empty = ∅ (the rule deletes).
    pub rhs: Vec<String>,
    /// Left context (segments that must immediately precede the target).
    pub left: Vec<PatternAtom>,
    /// Right context (segments that must immediately follow the target).
    pub right: Vec<PatternAtom>,
    /// Optional rules describe a variant pronunciation; the canonical
    /// surface derivation skips them.
    pub optional: bool,
    /// The original SPE rule string (`k > tʃ / _ i`), for display.
    pub source: String,
}

impl PatternAtom {
    /// NFC-normalise a symbol atom (LB-3); a boundary is unchanged.
    pub fn normalize(&mut self) {
        if let PatternAtom::Symbol(s) = self {
            super::norm::nfc_in_place(s);
        }
    }
}

impl AllophonyRule {
    /// NFC-normalise the target, replacement and both contexts (LB-3).
    pub fn normalize(&mut self) {
        self.lhs.iter_mut().for_each(PatternAtom::normalize);
        self.rhs.iter_mut().for_each(super::norm::nfc_in_place);
        self.left.iter_mut().for_each(PatternAtom::normalize);
        self.right.iter_mut().for_each(PatternAtom::normalize);
    }

    /// Resolve joined tokens against a symbol space (LI-4): a target,
    /// replacement or context symbol that is neither a declared unit nor a
    /// class, but is a concatenation of declared units, is split into those
    /// units (greedy longest match, as [`split_units`]). `ai > e` with `a`
    /// and `i` in the inventory becomes `a i > e`; a declared `ai` or a class
    /// named `ai` is left alone. Idempotent.
    pub fn resolve(&mut self, units: &[&str], classes: &[&str]) {
        let split = |s: &str| -> Option<Vec<String>> {
            if classes.contains(&s) || units.contains(&s) {
                return None;
            }
            split_units(s, units).filter(|parts| parts.len() >= 2)
        };
        let resolve_atoms = |atoms: &mut Vec<PatternAtom>| {
            let mut out = Vec::with_capacity(atoms.len());
            for atom in atoms.drain(..) {
                match atom {
                    PatternAtom::Symbol(s) => match split(&s) {
                        Some(parts) => out.extend(parts.into_iter().map(PatternAtom::Symbol)),
                        None => out.push(PatternAtom::Symbol(s)),
                    },
                    b => out.push(b),
                }
            }
            *atoms = out;
        };
        resolve_atoms(&mut self.lhs);
        resolve_atoms(&mut self.left);
        resolve_atoms(&mut self.right);
        let mut rhs = Vec::with_capacity(self.rhs.len());
        for s in self.rhs.drain(..) {
            match split(&s) {
                Some(parts) => rhs.extend(parts),
                None => rhs.push(s),
            }
        }
        self.rhs = rhs;
    }

    /// The raw target and replacement tokens of [`AllophonyRule::source`],
    /// before any inventory split — what the author actually wrote. `None`
    /// when the source does not parse (a rule built by hand in code).
    pub fn source_tokens(&self) -> Option<(Vec<String>, Vec<String>)> {
        let (lhs, mut rhs, _, _) = parse_rule(&self.source).ok()?;
        let lhs = lhs
            .into_iter()
            .filter_map(|a| match a {
                PatternAtom::Symbol(mut s) => {
                    super::norm::nfc_in_place(&mut s);
                    Some(s)
                }
                PatternAtom::Boundary => None,
            })
            .collect();
        rhs.iter_mut().for_each(super::norm::nfc_in_place);
        Some((lhs, rhs))
    }
}

/// Greedy longest-unit decomposition of `s` over `units`; `None` unless the
/// whole string is covered. Shared by rule resolution and validation.
pub fn split_units(s: &str, units: &[&str]) -> Option<Vec<String>> {
    let mut sorted: Vec<&str> = units.iter().copied().filter(|u| !u.is_empty()).collect();
    sorted.sort_by_key(|u| std::cmp::Reverse(u.chars().count()));
    let mut out = Vec::new();
    let mut rest = s;
    'outer: while !rest.is_empty() {
        for u in &sorted {
            if rest.starts_with(u) {
                out.push((*u).to_string());
                rest = &rest[u.len()..];
                continue 'outer;
            }
        }
        return None;
    }
    Some(out)
}

/// A second full decomposition of `s` over `units` that differs from `not`,
/// when one exists — the evidence that a greedy split was ambiguous. Bounded
/// depth-first search; `None` when the decomposition is unique.
pub fn alternative_split(s: &str, units: &[&str], not: &[String]) -> Option<Vec<String>> {
    fn go<'a>(rest: &str, units: &[&'a str], path: &mut Vec<String>, not: &[String], budget: &mut usize) -> Option<Vec<String>> {
        if *budget == 0 {
            return None;
        }
        *budget -= 1;
        if rest.is_empty() {
            return (path.as_slice() != not).then(|| path.clone());
        }
        for u in units {
            if !u.is_empty() && rest.starts_with(u) {
                path.push((*u).to_string());
                if let Some(found) = go(&rest[u.len()..], units, path, not, budget) {
                    return Some(found);
                }
                path.pop();
            }
        }
        None
    }
    let mut budget = 4096usize;
    go(s, units, &mut Vec::new(), not, &mut budget)
}

/// Wire form: `{ name: "palatalization", rule: "k > tʃ / _ i", optional: false }`.
#[derive(Deserialize)]
struct RawRule {
    #[serde(default)]
    name: String,
    rule: String,
    #[serde(default)]
    optional: bool,
}

impl TryFrom<RawRule> for AllophonyRule {
    type Error = String;

    fn try_from(r: RawRule) -> Result<Self, Self::Error> {
        let (lhs, rhs, left, right) = parse_rule(&r.rule)?;
        Ok(AllophonyRule {
            name: if r.name.trim().is_empty() {
                r.rule.trim().to_string()
            } else {
                r.name
            },
            lhs,
            rhs,
            left,
            right,
            optional: r.optional,
            source: r.rule.trim().to_string(),
        })
    }
}

impl<'de> Deserialize<'de> for AllophonyRule {
    fn deserialize<D>(d: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        AllophonyRule::try_from(RawRule::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// Strip SPE brackets / slashes and whitespace from a token, and normalize
/// the empty-string markers (`∅`, `0`) to "".
fn clean(tok: &str) -> String {
    let t = tok.trim().trim_matches(|c| c == '/' || c == '[' || c == ']').trim();
    if t == "∅" || t == "0" {
        String::new()
    } else {
        t.to_string()
    }
}

fn parse_atom(tok: &str) -> Option<PatternAtom> {
    let t = clean(tok);
    if t.is_empty() {
        None
    } else if t == "#" {
        Some(PatternAtom::Boundary)
    } else {
        Some(PatternAtom::Symbol(t))
    }
}

fn parse_context(part: &str) -> Vec<PatternAtom> {
    part.split_whitespace().filter_map(parse_atom).collect()
}

#[allow(clippy::type_complexity)]
fn parse_rule(
    s: &str,
) -> Result<(Vec<PatternAtom>, Vec<String>, Vec<PatternAtom>, Vec<PatternAtom>), String> {
    // `/` is overloaded — it delimits `/k/` phonemes *and* introduces the
    // context. The real separator is the last `/` before the `_` target
    // marker; a rule with no `_` has no context.
    let (change, context) = match s.find('_') {
        Some(upos) => match s[..upos].rfind('/') {
            Some(sep) => (&s[..sep], Some(&s[sep + 1..])),
            None => return Err(format!("allophony rule `{s}` has a `_` context but no `/`")),
        },
        None => (s, None),
    };

    let arrow = change.find('>').or_else(|| change.find('→'));
    let arrow = arrow.ok_or_else(|| format!("allophony rule `{s}` has no `>` / `→`"))?;
    let (lhs_str, rhs_str) = change.split_at(arrow);
    // skip the arrow char(s)
    let rhs_str = rhs_str.trim_start_matches(['>', '→']);

    // Target and replacement are whitespace-separated sequences, like a
    // context: `a i > e`, `t > t s`, `∅ > ə`. A lone `∅` / `0` is the empty
    // sequence.
    let lhs = parse_context(lhs_str);
    let rhs: Vec<String> = rhs_str.split_whitespace().map(clean).filter(|t| !t.is_empty()).collect();
    if lhs.is_empty() && rhs.is_empty() {
        return Err(format!("allophony rule `{s}` is ∅ > ∅ (no-op)"));
    }
    if lhs.contains(&PatternAtom::Boundary) {
        return Err(format!("allophony rule `{s}` targets a boundary"));
    }
    if rhs.iter().any(|t| t == "#") {
        return Err(format!("allophony rule `{s}` emits a boundary"));
    }

    let (left, right) = match context {
        Some(ctx) => {
            let (l, r) = ctx
                .split_once('_')
                .ok_or_else(|| format!("allophony rule `{s}` context has no `_`"))?;
            (parse_context(l), parse_context(r))
        }
        None => (Vec::new(), Vec::new()),
    };

    Ok((lhs, rhs, left, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parsed(rule: &str) -> AllophonyRule {
        AllophonyRule::try_from(RawRule { name: String::new(), rule: rule.into(), optional: false })
            .expect("parses")
    }

    fn sym(s: &str) -> PatternAtom {
        PatternAtom::Symbol(s.into())
    }

    fn strs(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn parses_substitution_with_right_context() {
        let r = parsed("k > tʃ / _ i");
        assert_eq!(r.lhs, vec![sym("k")]);
        assert_eq!(r.rhs, strs(&["tʃ"]));
        assert!(r.left.is_empty());
        assert_eq!(r.right, vec![sym("i")]);
    }

    #[test]
    fn parses_final_devoicing_with_boundary() {
        let r = parsed("d > t / _ #");
        assert_eq!(r.rhs, strs(&["t"]));
        assert_eq!(r.right, vec![PatternAtom::Boundary]);
    }

    #[test]
    fn parses_epenthesis_and_deletion() {
        let ins = parsed("∅ > ə / C _ C");
        assert!(ins.lhs.is_empty());
        assert_eq!(ins.rhs, strs(&["ə"]));
        assert_eq!(ins.left, vec![sym("C")]);

        let del = parsed("V > 0 / _ #");
        assert_eq!(del.lhs, vec![sym("V")]);
        assert!(del.rhs.is_empty());
    }

    #[test]
    fn strips_spe_brackets() {
        let r = parsed("/k/ > [x] / V _ V");
        assert_eq!(r.lhs, vec![sym("k")]);
        assert_eq!(r.rhs, strs(&["x"]));
        assert_eq!(r.left, vec![sym("V")]);
        assert_eq!(r.right, vec![sym("V")]);
    }

    #[test]
    fn whitespace_splits_target_and_replacement_into_sequences() {
        // LI-4: a diphthong target and a two-segment replacement.
        let mono = parsed("a i > e / _ #");
        assert_eq!(mono.lhs, vec![sym("a"), sym("i")]);
        assert_eq!(mono.rhs, strs(&["e"]));
        assert_eq!(mono.right, vec![PatternAtom::Boundary]);

        let affr = parsed("t > t s / _ i");
        assert_eq!(affr.lhs, vec![sym("t")]);
        assert_eq!(affr.rhs, strs(&["t", "s"]));

        let ins = parsed("∅ > a i / C _ #");
        assert!(ins.lhs.is_empty());
        assert_eq!(ins.rhs, strs(&["a", "i"]));
    }

    #[test]
    fn a_boundary_in_the_target_or_replacement_is_rejected() {
        let bad = |rule: &str| {
            AllophonyRule::try_from(RawRule { name: String::new(), rule: rule.into(), optional: false })
                .expect_err("rejected")
        };
        assert!(bad("a # > e").contains("targets a boundary"));
        assert!(bad("a > # / _").contains("emits a boundary"));
        assert!(bad("0 > ∅").contains("no-op"));
    }

    #[test]
    fn resolve_splits_joined_tokens_against_the_inventory() {
        let units = ["a", "i", "e", "t", "s", "tʃ"];
        let classes = ["V"];

        let mut mono = parsed("ai > e / _ #");
        mono.resolve(&units, &classes);
        assert_eq!(mono.lhs, vec![sym("a"), sym("i")]);
        assert_eq!(mono, {
            let mut spaced = parsed("a i > e / _ #");
            spaced.source = "ai > e / _ #".into();
            spaced.name = "ai > e / _ #".into();
            spaced
        });

        let mut affr = parsed("t > ts / _ ai");
        affr.resolve(&units, &classes);
        assert_eq!(affr.rhs, strs(&["t", "s"]));
        assert_eq!(affr.right, vec![sym("a"), sym("i")]);

        // A declared unit, a class, and an unknown surface replacement stay whole.
        let mut keep = parsed("tʃ > x / V _");
        keep.resolve(&units, &classes);
        assert_eq!(keep.lhs, vec![sym("tʃ")]);
        assert_eq!(keep.rhs, strs(&["x"]));
        assert_eq!(keep.left, vec![sym("V")]);

        // Idempotent.
        let again = mono.clone();
        mono.resolve(&units, &classes);
        assert_eq!(mono, again);
    }

    #[test]
    fn split_units_is_greedy_and_alternative_split_finds_ambiguity() {
        let units = ["ab", "a", "bc", "c"];
        let greedy = split_units("abc", &units).unwrap();
        assert_eq!(greedy, strs(&["ab", "c"]));
        assert_eq!(alternative_split("abc", &units, &greedy), Some(strs(&["a", "bc"])));
        assert_eq!(split_units("abx", &units), None);

        let unique = ["a", "i"];
        let g = split_units("ai", &unique).unwrap();
        assert_eq!(alternative_split("ai", &unique, &g), None);
    }

    #[test]
    fn source_tokens_report_what_the_author_wrote() {
        let mut r = parsed("ai > ts / _ #");
        r.resolve(&["a", "i", "t", "s"], &[]);
        assert_eq!(r.lhs.len(), 2);
        assert_eq!(r.source_tokens(), Some((strs(&["ai"]), strs(&["ts"]))));
    }
}
