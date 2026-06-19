//! Romanization engine (LANG-1 P1.5).
//!
//! Forward: an IPA phoneme sequence → written text, using a scheme's
//! mappings (falling back to the per-phoneme `romanize`, then the IPA).
//! Reverse: written text → IPA, by greedy longest-grapheme match, with
//! single-segment contextual rules resolving any grapheme that maps to more
//! than one phoneme. `deromanize(romanize(seq))` round-trips for an
//! unambiguous scheme. Pure and deterministic.

use crate::conlang::types::{PatternAtom, Phonology, RomanizationScheme};

/// IPA sequence → text under `scheme`.
pub fn romanize(scheme: &RomanizationScheme, phon: &Phonology, seq: &[String]) -> String {
    seq.iter().map(|ipa| roman_for(scheme, phon, ipa)).collect()
}

fn roman_for(scheme: &RomanizationScheme, phon: &Phonology, ipa: &str) -> String {
    scheme
        .mappings
        .iter()
        .find(|m| m.ipa == ipa)
        .map(|m| m.roman.clone())
        .or_else(|| phon.phoneme(ipa).and_then(|p| p.romanize.clone()))
        .unwrap_or_else(|| ipa.to_string())
}

/// `(grapheme, ipa)` decode candidates, longest grapheme first: the scheme's
/// mappings take priority, then any inventory phoneme the scheme didn't cover
/// (via its `romanize`/IPA grapheme) so a partial scheme still decodes.
fn decode_table<'a>(scheme: &'a RomanizationScheme, phon: &'a Phonology) -> Vec<(String, String)> {
    let mut table: Vec<(String, String)> = scheme
        .mappings
        .iter()
        .map(|m| (m.roman.clone(), m.ipa.clone()))
        .collect();
    for p in &phon.phonemes {
        let g = p.grapheme().to_string();
        if !table.iter().any(|(_, ipa)| *ipa == p.ipa) {
            table.push((g, p.ipa.clone()));
        }
    }
    table.retain(|(g, _)| !g.is_empty());
    table.sort_by(|a, b| b.0.chars().count().cmp(&a.0.chars().count()));
    table
}

/// Text → IPA sequence under `scheme`.
pub fn deromanize(scheme: &RomanizationScheme, phon: &Phonology, text: &str) -> Vec<String> {
    let table = decode_table(scheme, phon);
    let mut out: Vec<String> = Vec::new();
    let mut rest = text;

    'outer: while !rest.is_empty() {
        // Longest grapheme that prefixes the remaining text.
        let Some((g, _)) = table.iter().find(|(g, _)| rest.starts_with(g.as_str())) else {
            // No grapheme matches — consume one char verbatim.
            let ch = rest.chars().next().unwrap();
            out.push(ch.to_string());
            rest = &rest[ch.len_utf8()..];
            continue;
        };
        let g = g.clone();
        let after_text = &rest[g.len()..];

        // All IPA candidates for this exact grapheme.
        let candidates: Vec<&str> =
            table.iter().filter(|(gr, _)| *gr == g).map(|(_, ipa)| ipa.as_str()).collect();

        let chosen = if candidates.len() == 1 {
            candidates[0].to_string()
        } else {
            resolve_contextual(scheme, phon, &table, &g, &out, after_text)
                .unwrap_or_else(|| candidates[0].to_string())
        };
        out.push(chosen);
        rest = after_text;
        if rest.is_empty() {
            break 'outer;
        }
    }
    out
}

/// Pick the IPA for an ambiguous grapheme `g` from the scheme's contextual
/// rules: a rule fires when its `after` matches the last decoded phoneme and
/// its `before` matches the next phoneme (peek-decoded without context).
fn resolve_contextual(
    scheme: &RomanizationScheme,
    phon: &Phonology,
    table: &[(String, String)],
    g: &str,
    decoded: &[String],
    after_text: &str,
) -> Option<String> {
    let next_phoneme = peek_phoneme(table, after_text);
    for rule in scheme.contextual.iter().filter(|r| r.roman == g) {
        let after_ok = match &rule.after {
            None => true,
            Some(PatternAtom::Boundary) => decoded.is_empty(),
            Some(a) => decoded.last().is_some_and(|p| atom_matches(phon, a, p)),
        };
        let before_ok = match &rule.before {
            None => true,
            Some(PatternAtom::Boundary) => next_phoneme.is_none(),
            Some(b) => next_phoneme.as_deref().is_some_and(|p| atom_matches(phon, b, p)),
        };
        if after_ok && before_ok {
            return Some(rule.ipa.clone());
        }
    }
    None
}

/// Greedy decode of just the next grapheme to its first IPA candidate — used
/// only to test a `before` context, so context resolution stays single-pass.
fn peek_phoneme(table: &[(String, String)], text: &str) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    table
        .iter()
        .find(|(g, _)| text.starts_with(g.as_str()))
        .map(|(_, ipa)| ipa.clone())
}

fn atom_matches(phon: &Phonology, atom: &PatternAtom, seg: &str) -> bool {
    match atom {
        PatternAtom::Boundary => false,
        PatternAtom::Symbol(s) => {
            if phon.classes.contains_key(s) {
                phon.class_members(s).iter().any(|m| m == seg)
            } else {
                s == seg
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conlang::types::{Phoneme, PhonemeKind};

    fn ph(ipa: &str, rom: &str, kind: PhonemeKind) -> Phoneme {
        Phoneme { ipa: ipa.into(), romanize: Some(rom.into()), kind, sonority: None }
    }

    fn lang() -> Phonology {
        let mut p = Phonology {
            phonemes: vec![
                ph("k", "k", PhonemeKind::Consonant),
                ph("s", "s", PhonemeKind::Consonant),
                ph("ʃ", "sh", PhonemeKind::Consonant),
                ph("a", "a", PhonemeKind::Vowel),
                ph("i", "i", PhonemeKind::Vowel),
            ],
            ..Default::default()
        };
        p.classes =
            [("FrontV".to_string(), vec!["i".to_string()])].into_iter().collect();
        p
    }

    fn scheme_json(s: &str) -> RomanizationScheme {
        serde_hjson::from_str(s).unwrap()
    }

    fn seq(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn forward_uses_scheme_then_falls_back() {
        let p = lang();
        let s = scheme_json(r#"{ name: "x", mappings: [ { ipa: "ʃ", roman: "x" } ] }"#);
        // ʃ → "x" via scheme; k/a fall back to per-phoneme romanize.
        assert_eq!(romanize(&s, &p, &seq(&["ʃ", "a", "k", "a"])), "xaka");
    }

    #[test]
    fn reverse_prefers_longest_grapheme() {
        let p = lang();
        let s = scheme_json(r#"{ name: "default", mappings: [] }"#);
        // "sha" → ʃ (sh) + a, not s + h + a.
        assert_eq!(deromanize(&s, &p, "sha"), seq(&["ʃ", "a"]));
    }

    #[test]
    fn roundtrip_unambiguous() {
        let p = lang();
        let s = scheme_json(r#"{ name: "default", mappings: [] }"#);
        let orig = seq(&["k", "a", "ʃ", "i"]);
        assert_eq!(deromanize(&s, &p, &romanize(&s, &p, &orig)), orig);
    }

    #[test]
    fn contextual_disambiguation_c_before_front_vowel() {
        let p = lang();
        // Both /k/ and /s/ romanize as "c"; "c" is /s/ before a front vowel,
        // else /k/.
        let s = scheme_json(
            r#"{ name: "latinish",
                 mappings: [ { ipa: "k", roman: "c" }, { ipa: "s", roman: "c" } ],
                 contextual: [ { roman: "c", ipa: "s", before: "FrontV" } ] }"#,
        );
        assert_eq!(deromanize(&s, &p, "ci"), seq(&["s", "i"])); // before /i/ → /s/
        assert_eq!(deromanize(&s, &p, "ca"), seq(&["k", "a"])); // else → /k/ (first candidate)
    }
}
