//! Tone-sandhi evaluation (LANG-1 P1.6).
//!
//! Apply a tone system's ordered sandhi rules to a sequence of tone labels,
//! reusing the generic ordered-rewrite engine shared with allophony. Pure
//! and deterministic.

use crate::conlang::phonology::rewrite;
use crate::conlang::types::tone::ToneSystem;

/// Rewrite a tone sequence by the system's sandhi rules.
pub fn apply_sandhi(system: &ToneSystem, tones: &[String]) -> Vec<String> {
    rewrite::apply_ordered(tones, &system.sandhi, &system.classes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::tone::ToneKind;

    fn system(sandhi: &[&str]) -> ToneSystem {
        let rules = sandhi
            .iter()
            .map(|r| {
                serde_hjson::from_str(&format!("{{ rule: \"{r}\" }}")).unwrap()
            })
            .collect();
        ToneSystem {
            kind: ToneKind::Contour,
            tones: vec!["1".into(), "2".into(), "3".into(), "4".into()],
            classes: Default::default(),
            sandhi: rules,
        }
    }

    fn seq(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn mandarin_third_tone_sandhi() {
        // A third tone before another third tone becomes a second tone.
        let s = system(&["3 > 2 / _ 3"]);
        assert_eq!(apply_sandhi(&s, &seq(&["3", "3"])), seq(&["2", "3"]));
        assert_eq!(apply_sandhi(&s, &seq(&["3", "1"])), seq(&["3", "1"])); // unchanged
        // 3-3-3 → only the non-final low tones raise (single L→R pass).
        assert_eq!(apply_sandhi(&s, &seq(&["3", "3", "3"])), seq(&["2", "2", "3"]));
    }

    #[test]
    fn tone_classes_in_context() {
        // A H tone lowers to L after any non-high tone (class NonHigh).
        let mut s = system(&["H > L / NonHigh _"]);
        s.tones = vec!["H".into(), "L".into(), "R".into()];
        s.classes = [("NonHigh".to_string(), vec!["L".to_string(), "R".to_string()])]
            .into_iter()
            .collect();
        assert_eq!(apply_sandhi(&s, &seq(&["L", "H"])), seq(&["L", "L"]));
        assert_eq!(apply_sandhi(&s, &seq(&["H", "H"])), seq(&["H", "H"])); // H not in NonHigh
    }

    #[test]
    fn no_sandhi_is_identity() {
        let s = system(&[]);
        assert_eq!(apply_sandhi(&s, &seq(&["1", "2", "3"])), seq(&["1", "2", "3"]));
    }
}
