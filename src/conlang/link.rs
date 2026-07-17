//! LING-1 L-P5 (Wave 3) — argument linking (RRG `/link` + `/theme`).
//!
//! Given a verb's valence and the noun phrases in a clause, work out the
//! argument structure: each argument's *thematic role* (agent, patient, theme,
//! recipient), its Role-and-Reference-Grammar *macrorole* (actor vs undergoer),
//! and its *grammatical relation* (subject, object, indirect object). This is the
//! syntax–semantics interface made explicit — the default linking a transitive
//! clause draws, and where an argument count doesn't match the verb's valence.
//!
//! Deterministic and theory-default: it applies RRG's standard actor–undergoer
//! assignment (the highest-ranking argument is the actor, the lowest the
//! undergoer) rather than trying to infer lexical aspect.

use serde::Serialize;

/// One linked argument.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LinkedArg {
    pub arg: String,
    pub theta_role: &'static str,
    pub macrorole: &'static str,
    pub relation: &'static str,
}

/// The linking report for one clause.
#[derive(Debug, Clone, Default, Serialize, PartialEq)]
pub struct LinkReport {
    pub verb: String,
    pub valence: String,
    pub args: Vec<LinkedArg>,
    /// Arity or valence problems (e.g. two arguments for an intransitive verb).
    pub issues: Vec<String>,
}

/// The number of core arguments a valence expects.
fn expected_arity(valence: &str) -> Option<usize> {
    match valence.to_lowercase().as_str() {
        "impersonal" => Some(0),
        "intransitive" => Some(1),
        "transitive" => Some(2),
        "ditransitive" => Some(3),
        _ => None,
    }
}

/// Link `args` under `verb`'s `valence`. Unknown valence falls back to inferring
/// from the argument count.
pub fn link(verb: &str, valence: &str, args: &[String]) -> LinkReport {
    let mut r = LinkReport { verb: verb.to_string(), valence: valence.to_string(), ..Default::default() };

    let expected = expected_arity(valence);
    // With an unrecognised valence, infer one from the argument count so the tool
    // still gives an answer.
    let effective_arity = expected.unwrap_or(args.len());
    if expected.is_none() {
        r.issues.push(format!(
            "unknown valence `{valence}` — inferring {effective_arity} core argument(s) from the input"
        ));
    } else if let Some(exp) = expected {
        if exp != args.len() {
            r.issues.push(format!(
                "{valence} expects {exp} argument(s) but {} were given",
                args.len()
            ));
        }
    }

    // The standard RRG default linking, by the effective arity.
    let template: &[(&'static str, &'static str, &'static str)] = match effective_arity {
        0 => &[],
        1 => &[("agent", "actor", "subject")],
        2 => &[("agent", "actor", "subject"), ("patient", "undergoer", "object")],
        _ => &[
            ("agent", "actor", "subject"),
            ("theme", "undergoer", "object"),
            ("recipient", "non-macrorole", "indirect-object"),
        ],
    };

    for (i, arg) in args.iter().enumerate() {
        let (theta, macro_, rel) = template.get(i).copied().unwrap_or(("oblique", "non-macrorole", "adjunct"));
        r.args.push(LinkedArg { arg: arg.clone(), theta_role: theta, macrorole: macro_, relation: rel });
    }

    r
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_transitive_clause_gets_actor_and_undergoer() {
        let r = link("see", "transitive", &["she".into(), "bird".into()]);
        assert!(r.issues.is_empty());
        assert_eq!(r.args[0].macrorole, "actor");
        assert_eq!(r.args[0].relation, "subject");
        assert_eq!(r.args[1].theta_role, "patient");
        assert_eq!(r.args[1].macrorole, "undergoer");
    }

    #[test]
    fn a_ditransitive_clause_links_a_recipient() {
        let r = link("give", "ditransitive", &["she".into(), "book".into(), "him".into()]);
        assert!(r.issues.is_empty());
        assert_eq!(r.args[2].theta_role, "recipient");
        assert_eq!(r.args[2].relation, "indirect-object");
        assert_eq!(r.args[2].macrorole, "non-macrorole");
    }

    #[test]
    fn an_arity_mismatch_is_flagged_but_still_linked() {
        // Two arguments for an intransitive verb.
        let r = link("sleep", "intransitive", &["she".into(), "bed".into()]);
        assert_eq!(r.issues.len(), 1);
        assert_eq!(r.args[0].macrorole, "actor");
        // The extra argument links as an adjunct, not a core role.
        assert_eq!(r.args[1].relation, "adjunct");
    }

    #[test]
    fn an_unknown_valence_infers_from_arity() {
        let r = link("frob", "quadrivalent", &["a".into(), "b".into()]);
        assert!(r.issues.iter().any(|i| i.contains("inferring")));
        assert_eq!(r.args.len(), 2);
        assert_eq!(r.args[1].macrorole, "undergoer");
    }
}
