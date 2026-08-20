//! POEM-6 (PO-P7) — form completion and missing-component detection.
//!
//! Given a poem's lines and its declared `poem:` form, report how much of the
//! form is written (a completion ratio) and any structural components the form
//! requires but the text is missing or breaking: a villanelle's refrain
//! schedule, a pantoum's line repetition, a ghazal's signature (maqta), a
//! sonnet's line count. Deterministic and offline — it measures, never rewrites.

use crate::poetry::form::PoemForm;

/// The completion status of a poem against its declared form.
#[derive(Debug, Clone, PartialEq)]
pub struct FormStatus {
    pub lines_written: usize,
    /// Expected line total, when the form fixes one (`None` = open form).
    pub expected_lines: Option<usize>,
    /// Whether the line count meets or exceeds the expected total.
    pub complete: bool,
    /// Structural problems: a broken refrain, a missing signature, and so on.
    pub issues: Vec<String>,
}

/// POEM-TUI (PO-P14) — the compact outline/tree chip for a completion status:
/// `("8/14", false)` while drafting, `("14/14", true)` when a bounded form is
/// complete, or the bare line count for an open form. The bool drives the chip
/// colour at the call site (green + ✓ when complete).
pub fn completion_chip(st: &FormStatus) -> (String, bool) {
    let text = match st.expected_lines {
        Some(exp) => format!("{}/{}", st.lines_written, exp),
        None => format!("{}", st.lines_written),
    };
    (text, st.complete)
}

/// Check a poem's full text against its declared form.
pub fn check_form(text: &str, form: &PoemForm) -> FormStatus {
    let lines: Vec<String> =
        text.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect();
    let written = lines.len();
    let expected = expected_lines(form);
    // A bounded form is complete when it has *exactly* its line count — an
    // over-length poem is not "complete ✓" (the form checker flags it as too
    // long, and the Outline chip must not show a green tick over `16/14`).
    let complete = expected.map(|e| written == e).unwrap_or(false);

    let mut issues = Vec::new();
    match form.form.as_str() {
        "villanelle" => check_villanelle(&lines, &mut issues),
        "pantoum" => check_pantoum(&lines, &mut issues),
        "ghazal" => check_ghazal(&lines, form, &mut issues),
        f if f.contains("sonnet") => check_sonnet(&lines, &mut issues),
        _ => {}
    }

    FormStatus { lines_written: written, expected_lines: expected, complete, issues }
}

/// The fixed line total a form requires, if any.
fn expected_lines(form: &PoemForm) -> Option<usize> {
    let n = match form.form.as_str() {
        "sonnet" | "petrarchan_sonnet" | "shakespearean_sonnet" => 14,
        "haiku" | "senryuu" => 3,
        "tanka" => 5,
        "limerick" => 5,
        "villanelle" => 19,
        _ => {
            return if form.stanzas > 0 && form.lines_per_stanza > 0 {
                Some(form.stanzas.saturating_mul(form.lines_per_stanza) as usize)
            } else {
                None
            };
        }
    };
    Some(n)
}

/// The villanelle's two refrains (lines 1 and 3) recur on a schedule: line 1 at
/// lines 6, 12, 18; line 3 at lines 9, 15, 19.
fn check_villanelle(lines: &[String], issues: &mut Vec<String>) {
    if lines.len() < 6 {
        return;
    }
    let a1 = lines[0].clone();
    let a2 = lines.get(2).cloned().unwrap_or_default();
    let refrain = |slots: &[usize], refr: &str, which: &str, issues: &mut Vec<String>| {
        if refr.is_empty() {
            return;
        }
        for &p in slots {
            if let Some(line) = lines.get(p) {
                if !line.eq_ignore_ascii_case(refr) {
                    issues.push(format!(
                        "Villanelle: line {} should repeat the {which} refrain “{refr}”.",
                        p + 1
                    ));
                }
            }
        }
    };
    // 0-based indices of lines 6/12/18 and 9/15/19.
    refrain(&[5, 11, 17], &a1, "first", issues);
    refrain(&[8, 14, 18], &a2, "third", issues);
}

/// Pantoum: lines 2 and 4 of each quatrain recur as lines 1 and 3 of the next.
fn check_pantoum(lines: &[String], issues: &mut Vec<String>) {
    let stanzas = lines.len() / 4;
    for s in 0..stanzas.saturating_sub(1) {
        let (a, b) = (s * 4, (s + 1) * 4);
        if lines[a + 1] != lines[b] {
            issues.push(format!("Pantoum: line {} should repeat line {}.", b + 1, a + 2));
        }
        if lines[a + 3] != lines[b + 2] {
            issues.push(format!("Pantoum: line {} should repeat line {}.", b + 3, a + 4));
        }
    }
}

/// Ghazal: the closing couplet (maqta) traditionally names the poet — a
/// signature word declared as `poem.signature_word`.
fn check_ghazal(lines: &[String], form: &PoemForm, issues: &mut Vec<String>) {
    let Some(sig) = form.signature_word.as_deref().filter(|s| !s.is_empty()) else {
        return;
    };
    if let Some(last) = lines.last() {
        if !last.to_lowercase().contains(&sig.to_lowercase()) {
            issues.push(format!("Ghazal: the closing maqta should include the signature “{sig}”."));
        }
    }
}

/// A sonnet is fourteen lines; flag an over-length one (under-length is just
/// incomplete, reported by the ratio).
fn check_sonnet(lines: &[String], issues: &mut Vec<String>) {
    if lines.len() > 14 {
        issues.push(format!("Sonnet: {} lines — a sonnet is fourteen.", lines.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::poetry::form::PoemForm;

    fn form(name: &str) -> PoemForm {
        PoemForm { form: name.into(), ..Default::default() }
    }

    #[test]
    fn completion_ratio_for_a_partial_sonnet() {
        let text = "one\ntwo\nthree\nfour\nfive\nsix\nseven";
        let st = check_form(text, &form("shakespearean_sonnet"));
        assert_eq!(st.lines_written, 7);
        assert_eq!(st.expected_lines, Some(14));
        assert!(!st.complete);
    }

    #[test]
    fn completion_chip_formats_bounded_open_and_complete() {
        // PO-P14: the outline/tree chip text + complete flag.
        let partial = check_form("one\ntwo\nthree", &form("shakespearean_sonnet"));
        assert_eq!(completion_chip(&partial), ("3/14".to_string(), false));

        let full_text = (1..=14).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let full = check_form(&full_text, &form("shakespearean_sonnet"));
        assert_eq!(completion_chip(&full), ("14/14".to_string(), true));

        // 1.8.23: an over-length bounded form is NOT complete (no green ✓).
        let over_text = (1..=16).map(|i| i.to_string()).collect::<Vec<_>>().join("\n");
        let over = check_form(&over_text, &form("shakespearean_sonnet"));
        assert_eq!(completion_chip(&over), ("16/14".to_string(), false));

        // An open form (free verse) reports a bare count, never "complete".
        let open = check_form("a\nb\nc\nd", &form("free_verse"));
        let (text, done) = completion_chip(&open);
        assert_eq!(text, "4");
        assert!(!done);
    }

    #[test]
    fn villanelle_flags_a_broken_refrain() {
        // Line 6 (index 5) should repeat line 1 but doesn't.
        let mut lines: Vec<String> = (1..=6).map(|i| format!("line {i}")).collect();
        lines[0] = "the refrain returns".into();
        let text = lines.join("\n");
        let mut issues = Vec::new();
        check_villanelle(&text.lines().map(str::to_string).collect::<Vec<_>>(), &mut issues);
        assert!(issues.iter().any(|i| i.contains("line 6") && i.contains("refrain")));
    }

    #[test]
    fn villanelle_clean_refrain_is_silent() {
        let mut lines: Vec<String> = (1..=6).map(|i| format!("line {i}")).collect();
        lines[0] = "the refrain".into();
        lines[5] = "the refrain".into(); // line 6 matches line 1
        let mut issues = Vec::new();
        check_villanelle(&lines, &mut issues);
        assert!(issues.iter().all(|i| !i.contains("first refrain")));
    }

    #[test]
    fn pantoum_flags_a_missing_carry_forward() {
        // Stanza 2 line 1 (index 4) should equal stanza 1 line 2 (index 1).
        let lines: Vec<String> = vec![
            "a", "carried two", "b", "carried four", "different", "x", "y", "z",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let mut issues = Vec::new();
        check_pantoum(&lines, &mut issues);
        assert!(issues.iter().any(|i| i.contains("line 5")));
    }

    #[test]
    fn ghazal_flags_a_missing_signature() {
        let mut f = form("ghazal");
        f.signature_word = Some("Ghalib".into());
        let st = check_form("first couplet here\nsecond couplet ends", &f);
        assert!(st.issues.iter().any(|i| i.contains("Ghalib")));
        // With the signature present, no issue.
        let ok = check_form("first couplet\nand Ghalib signs off", &f);
        assert!(ok.issues.is_empty());
    }

    #[test]
    fn open_form_has_no_expected_total() {
        let st = check_form("a\nb\nc", &form("free_verse"));
        assert_eq!(st.expected_lines, None);
        assert!(!st.complete);
    }
}
