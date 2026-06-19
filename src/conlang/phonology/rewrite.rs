//! Generic ordered context-rewrite engine (LANG-1 P1.6).
//!
//! The shared core behind both allophony (rewrites over phonemes) and tone
//! sandhi (rewrites over tone labels): apply a list of [`AllophonyRule`]s in
//! order to a sequence of string items, each rule a single left-to-right
//! pass over the previous rule's output (feeding order). A context atom is a
//! class name (resolved against the supplied class map) or a literal item,
//! or `#` for a sequence edge. Pure and deterministic.

use std::collections::BTreeMap;

use crate::conlang::types::{AllophonyRule, PatternAtom};

type Classes = BTreeMap<String, Vec<String>>;

/// Apply every non-optional rule in order to `items`.
pub fn apply_ordered(items: &[String], rules: &[AllophonyRule], classes: &Classes) -> Vec<String> {
    let mut seq = items.to_vec();
    for rule in rules.iter().filter(|r| !r.optional) {
        seq = apply_rule(&seq, rule, classes);
    }
    seq
}

fn apply_rule(seq: &[String], rule: &AllophonyRule, classes: &Classes) -> Vec<String> {
    let focus_len = if rule.lhs.is_none() { 0 } else { 1 };
    let mut out: Vec<String> = Vec::with_capacity(seq.len());
    let mut i = 0usize;
    while i <= seq.len() {
        let focus_fits = i + focus_len <= seq.len();
        let matched = focus_fits
            && focus_matches(classes, &rule.lhs, seq, i)
            && left_matches(classes, &rule.left, &seq[..i])
            && right_matches(classes, &rule.right, &seq[i + focus_len..]);

        if matched {
            if let Some(r) = &rule.rhs {
                out.push(r.clone());
            }
            if focus_len == 0 {
                if i < seq.len() {
                    out.push(seq[i].clone());
                }
                i += 1;
            } else {
                i += focus_len;
            }
        } else {
            if i < seq.len() {
                out.push(seq[i].clone());
            }
            i += 1;
        }
    }
    out
}

fn focus_matches(classes: &Classes, lhs: &Option<PatternAtom>, seq: &[String], i: usize) -> bool {
    match lhs {
        None => true,
        Some(atom) => i < seq.len() && atom_matches(classes, atom, &seq[i]),
    }
}

fn left_matches(classes: &Classes, atoms: &[PatternAtom], left: &[String]) -> bool {
    let mut li = left.len();
    for atom in atoms.iter().rev() {
        match atom {
            PatternAtom::Boundary => return li == 0,
            _ => {
                if li == 0 {
                    return false;
                }
                li -= 1;
                if !atom_matches(classes, atom, &left[li]) {
                    return false;
                }
            }
        }
    }
    true
}

fn right_matches(classes: &Classes, atoms: &[PatternAtom], right: &[String]) -> bool {
    let mut ri = 0;
    for atom in atoms {
        match atom {
            PatternAtom::Boundary => return ri == right.len(),
            _ => {
                if ri >= right.len() || !atom_matches(classes, atom, &right[ri]) {
                    return false;
                }
                ri += 1;
            }
        }
    }
    true
}

/// A `Symbol` matches a class member when the symbol names a declared class,
/// otherwise it matches the literal item.
fn atom_matches(classes: &Classes, atom: &PatternAtom, seg: &str) -> bool {
    match atom {
        PatternAtom::Boundary => false,
        PatternAtom::Symbol(s) => match classes.get(s) {
            Some(members) => members.iter().any(|m| m == seg),
            None => s == seg,
        },
    }
}
