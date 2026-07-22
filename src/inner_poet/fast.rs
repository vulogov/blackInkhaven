//! Inner Poet fast track (PO-P5) — a deterministic metre + rhyme scan of a
//! stanza against its declared `poem:` form. Pure, offline, zero AI: it
//! observes and reports, never rewrites a line to fit the metre.
//!
//! Composes the four POEM engines: the `poem:` form (PO-P1) declares the target;
//! the syllabifier (PO-P2) and metre scanner (PO-P3) check each line; the rhyme
//! engine (PO-P4) checks the declared rhyme scheme.

use crate::poetry::form::PoemForm;
use crate::poetry::metre::{self, Foot};
use crate::poetry::rhyme::{self, RhymeQuality};
use crate::prose::ProseLanguage;

/// A finding's weight. The Inner Poet, unlike most companions, also *praises* a
/// metrically clean line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Praise,
    Note,
    Concern,
}

/// One fast-track finding on a stanza.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub severity: Severity,
    /// `Metre` or `Rhyme`.
    pub kind: &'static str,
    /// 1-based line within the stanza (0 = stanza-level).
    pub line: usize,
    pub message: String,
}

/// Scan a stanza's lines against its declared form.
pub fn scan_stanza(text: &str, form: &PoemForm) -> Vec<Finding> {
    let lang = ProseLanguage::from_label(&form.language);
    let lines: Vec<String> =
        text.lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
    let mut findings = Vec::new();
    scan_metre(&lines, form, &lang, &mut findings);
    scan_rhyme(&lines, form, &lang, &mut findings);
    findings
}

fn scan_metre(lines: &[String], form: &PoemForm, lang: &ProseLanguage, out: &mut Vec<Finding>) {
    let tradition =
        if form.metre_tradition.is_empty() { "accentual_syllabic" } else { form.metre_tradition.as_str() };
    if tradition != "accentual_syllabic" {
        return; // syllabic / quantitative / accentual / free: no per-line count findings here
    }
    let Some(foot) = Foot::parse(&form.metre) else { return };
    if form.feet == 0 {
        return;
    }
    let feet = form.feet as usize;

    for (i, line) in lines.iter().enumerate() {
        let ln = i + 1;
        let beats = metre::line_to_beats(line, lang.clone());
        if beats.is_empty() {
            continue;
        }
        let scan = metre::scan_line(&beats, foot, feet);
        let (n, exp) = (scan.syllables, scan.expected_syllables);

        if n > exp + 1 {
            out.push(Finding {
                severity: Severity::Concern,
                kind: "Metre",
                line: ln,
                message: format!(
                    "Line {ln} has {n} syllables; declared {} allows {exp} (±1 for a feminine ending). Long by {}.",
                    form.metre,
                    n - exp - 1
                ),
            });
        } else if n + 1 < exp {
            out.push(Finding {
                severity: Severity::Note,
                kind: "Metre",
                line: ln,
                message: format!("Line {ln} scans short: {n} of {exp} syllables."),
            });
        }

        if scan.conformance < 0.7 {
            out.push(Finding {
                severity: Severity::Note,
                kind: "Metre",
                line: ln,
                message: format!(
                    "Line {ln} departs from the declared {} (fit {:.0}%).",
                    form.metre,
                    scan.conformance * 100.0
                ),
            });
        } else if scan.conformance >= 0.95 && (n == exp || scan.feminine_ending) {
            out.push(Finding {
                severity: Severity::Praise,
                kind: "Metre",
                line: ln,
                message: format!("Line {ln} scans cleanly as {} {}.", form.metre, length_word(feet)),
            });
        }
    }
}

fn scan_rhyme(lines: &[String], form: &PoemForm, lang: &ProseLanguage, out: &mut Vec<Finding>) {
    let scheme = form.rhyme_scheme.trim();
    if scheme.is_empty() || scheme == "-" {
        return;
    }
    let labels: Vec<char> = scheme.chars().filter(|c| !c.is_whitespace()).collect();
    let end_words: Vec<String> = lines.iter().map(|l| last_word(l)).collect();

    use std::collections::BTreeMap;
    let mut groups: BTreeMap<char, Vec<usize>> = BTreeMap::new();
    for (i, &lab) in labels.iter().enumerate() {
        if lab == '-' || i >= lines.len() {
            continue;
        }
        groups.entry(lab).or_default().push(i);
    }

    for (lab, idxs) in &groups {
        for w in idxs.windows(2) {
            let (a, b) = (w[0], w[1]);
            if end_words[a].is_empty() || end_words[b].is_empty() {
                continue;
            }
            let r = rhyme::analyse_rhyme(&end_words[a], &end_words[b], lang.clone());
            let pair = format!("{}↔{} ({lab}–{lab})", a + 1, b + 1);
            match r.quality {
                RhymeQuality::Perfect => {}
                RhymeQuality::Near => out.push(Finding {
                    severity: Severity::Note,
                    kind: "Rhyme",
                    line: b + 1,
                    message: format!(
                        "Lines {pair}: “{}” / “{}” — {}. Intended?",
                        end_words[a],
                        end_words[b],
                        r.note.as_deref().unwrap_or("near-rhyme")
                    ),
                }),
                RhymeQuality::Eye => out.push(Finding {
                    severity: Severity::Concern,
                    kind: "Rhyme",
                    line: b + 1,
                    message: format!("Lines {pair}: “{}” / “{}” — eye-rhyme only.", end_words[a], end_words[b]),
                }),
                RhymeQuality::None => out.push(Finding {
                    severity: Severity::Concern,
                    kind: "Rhyme",
                    line: b + 1,
                    message: format!("Lines {pair}: “{}” / “{}” — do not rhyme.", end_words[a], end_words[b]),
                }),
            }
        }
    }
}

/// The last word of a line, stripped of surrounding punctuation.
fn last_word(line: &str) -> String {
    line.split_whitespace()
        .next_back()
        .map(|w| w.trim_matches(|c: char| !c.is_alphabetic() && c != '\u{301}').to_string())
        .unwrap_or_default()
}

fn length_word(feet: usize) -> &'static str {
    match feet {
        1 => "monometer",
        2 => "dimeter",
        3 => "trimeter",
        4 => "tetrameter",
        5 => "pentameter",
        6 => "hexameter",
        _ => "verse",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poetry::form::PoemForm;

    fn iambic_pentameter() -> PoemForm {
        PoemForm {
            form: "blank_verse".into(),
            metre: "iambic".into(),
            feet: 5,
            metre_tradition: "accentual_syllabic".into(),
            rhyme_scheme: "-".into(),
            language: "en".into(),
            ..Default::default()
        }
    }

    #[test]
    fn flags_an_over_long_line() {
        // A 14-syllable line against declared iambic pentameter (10, +1 tolerance).
        let form = iambic_pentameter();
        let text = "seven eight nine ten eleven twelve thirteen fourteen so on and on and on more";
        let f = scan_stanza(text, &form);
        assert!(f.iter().any(|x| x.kind == "Metre" && x.severity == Severity::Concern));
    }

    #[test]
    fn rhyme_scheme_flags_a_non_rhyme() {
        // AABB where the B pair doesn't rhyme.
        let form = PoemForm {
            metre_tradition: "free".into(),
            rhyme_scheme: "AABB".into(),
            language: "en".into(),
            ..Default::default()
        };
        let text = "the night is light\nwith stars so bright\nthe cat sat down\nbeside the tree";
        let f = scan_stanza(text, &form);
        // A pair (light/bright) rhymes → no finding; B pair (down/tree) does not.
        assert!(f.iter().any(|x| x.kind == "Rhyme" && x.severity == Severity::Concern));
        assert!(!f.iter().any(|x| x.kind == "Rhyme" && x.message.contains("night")));
    }

    #[test]
    fn perfect_rhymes_are_silent() {
        let form = PoemForm {
            metre_tradition: "free".into(),
            rhyme_scheme: "AA".into(),
            language: "ru".into(),
            ..Default::default()
        };
        let text = "тёмный дом\nвысокий том";
        let f = scan_stanza(text, &form);
        assert!(!f.iter().any(|x| x.kind == "Rhyme"));
    }

    #[test]
    fn free_verse_and_no_scheme_are_quiet() {
        let form = PoemForm {
            metre_tradition: "free".into(),
            rhyme_scheme: String::new(),
            language: "en".into(),
            ..Default::default()
        };
        assert!(scan_stanza("whatever the wind\ncarries away", &form).is_empty());
    }
}
