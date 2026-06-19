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
    /// Target. `None` = ∅ (the rule inserts).
    pub lhs: Option<PatternAtom>,
    /// Replacement IPA. `None` = ∅ (the rule deletes).
    pub rhs: Option<String>,
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
) -> Result<(Option<PatternAtom>, Option<String>, Vec<PatternAtom>, Vec<PatternAtom>), String> {
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

    let lhs = parse_atom(lhs_str);
    let rhs = {
        let c = clean(rhs_str);
        if c.is_empty() {
            None
        } else {
            Some(c)
        }
    };
    if lhs.is_none() && rhs.is_none() {
        return Err(format!("allophony rule `{s}` is ∅ > ∅ (no-op)"));
    }
    if matches!(lhs, Some(PatternAtom::Boundary)) {
        return Err(format!("allophony rule `{s}` targets a boundary"));
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

    #[test]
    fn parses_substitution_with_right_context() {
        let r = parsed("k > tʃ / _ i");
        assert_eq!(r.lhs, Some(PatternAtom::Symbol("k".into())));
        assert_eq!(r.rhs, Some("tʃ".into()));
        assert!(r.left.is_empty());
        assert_eq!(r.right, vec![PatternAtom::Symbol("i".into())]);
    }

    #[test]
    fn parses_final_devoicing_with_boundary() {
        let r = parsed("d > t / _ #");
        assert_eq!(r.rhs, Some("t".into()));
        assert_eq!(r.right, vec![PatternAtom::Boundary]);
    }

    #[test]
    fn parses_epenthesis_and_deletion() {
        let ins = parsed("∅ > ə / C _ C");
        assert_eq!(ins.lhs, None);
        assert_eq!(ins.rhs, Some("ə".into()));
        assert_eq!(ins.left, vec![PatternAtom::Symbol("C".into())]);

        let del = parsed("V > 0 / _ #");
        assert_eq!(del.lhs, Some(PatternAtom::Symbol("V".into())));
        assert_eq!(del.rhs, None);
    }

    #[test]
    fn strips_spe_brackets() {
        let r = parsed("/k/ > [x] / V _ V");
        assert_eq!(r.lhs, Some(PatternAtom::Symbol("k".into())));
        assert_eq!(r.rhs, Some("x".into()));
        assert_eq!(r.left, vec![PatternAtom::Symbol("V".into())]);
        assert_eq!(r.right, vec![PatternAtom::Symbol("V".into())]);
    }
}
