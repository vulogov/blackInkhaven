//! WORLD-4 Branch B — the fact-checker (P4, fast track). Reads author prose for
//! assertions about the world and verifies them against the simulation + the
//! magic ledger. This is the deterministic *fast* track: per-language pattern
//! matching, no LLM, sub-millisecond per paragraph. The first category is
//! travel time (the most-fired error in fiction); more land alongside it.
//!
//! Findings emit to the Output pane (PANE-1) as `fact_check_warning` messages,
//! so the author sees them without the checker blocking the manuscript. Before
//! emitting, each candidate is run past the magic ledger — a declared exception
//! to physics suppresses the warning with a note (lazy consultation, §8.21).

use crate::world::types::magic::{CheckContext, MagicLedger};

/// A fact-check finding. `body` is the human message; `suppressed_by` is set
/// when a magic rule covered it (the warning is informational, not a problem).
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    pub category: String,
    /// "info" | "warning" | "contradiction".
    pub severity: String,
    pub body: String,
    /// The magic rule kind that suppressed this finding, if any.
    pub suppressed_by: Option<String>,
}

/// Run the fast fact-check over a paragraph of prose. `roles` are any actor
/// roles in scope (for magic-ledger consultation); empty is fine.
pub fn check_paragraph(text: &str, ledger: &MagicLedger, roles: &[String]) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(check_travel_time(text, ledger, roles));
    findings
}

/// Travel-time check: a sentence that states both a distance and a duration
/// implies a pace; flag paces that exceed pre-industrial overland travel.
fn check_travel_time(text: &str, ledger: &MagicLedger, roles: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    for sentence in text.split(|c| c == '.' || c == '!' || c == '?' || c == '\n') {
        let (Some(km), Some(days)) = (find_distance_km(sentence), find_duration_days(sentence))
        else {
            continue;
        };
        if days <= 0.0 || km <= 0.0 {
            continue;
        }
        let pace = km / days;
        // Baseline pre-industrial overland pace: ~25–40 km/day on foot, 50–80
        // mounted. Use a generous mounted median; flag clear outliers.
        let baseline = 65.0_f32;
        let ratio = pace / baseline;
        let (severity, note) = if ratio > 2.5 {
            ("contradiction", "far exceeds")
        } else if ratio > 1.5 {
            ("warning", "exceeds")
        } else {
            continue; // plausible
        };
        let body = format!(
            "Travel of {km:.0} km in {days:.0} day(s) = {pace:.0} km/day, which {note} \
             pre-industrial overland travel (typically 25–80 km/day)."
        );
        // Lazy magic consultation.
        let ctx = CheckContext { category: "travel_time", roles, ..Default::default() };
        let suppressed_by = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
        let severity = if suppressed_by.is_some() { "info" } else { severity };
        out.push(Finding {
            category: "travel_time".into(),
            severity: severity.into(),
            body,
            suppressed_by,
        });
    }
    out
}

/// Emit a finding to the Output pane as a `fact_check_warning` (a no-op outside
/// the TUI). Suppressed findings carry the rule note in their metadata.
pub fn emit_finding(f: &Finding) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity};
    let severity = match f.severity.as_str() {
        "contradiction" => Severity::Contradiction,
        "warning" => Severity::Warning,
        _ => Severity::Info,
    };
    let text = match &f.suppressed_by {
        Some(rule) => format!("{} (consistent with magic rule `{rule}`)", f.body),
        None => f.body.clone(),
    };
    let msg = Message::new(
        kinds::FACT_CHECK_WARNING,
        severity,
        Lifetime::UntilActedOn,
        serde_json::json!({
            "text": text,
            "category": f.category,
            "track": "fast",
            "suppressed_by": f.suppressed_by,
        }),
    );
    crate::pane::output::emit(&msg);
}

/// Extract a distance from a sentence, normalised to km. Recognises km, miles,
/// and leagues.
fn find_distance_km(s: &str) -> Option<f32> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)(\d+(?:[.,]\d+)?)\s*(km|kilometres?|kilometers?|mi|miles?|leagues?)")
            .unwrap()
    });
    let caps = re.captures(s)?;
    let n: f32 = caps.get(1)?.as_str().replace(',', ".").parse().ok()?;
    let unit = caps.get(2)?.as_str().to_ascii_lowercase();
    Some(match unit.as_str() {
        u if u.starts_with("mi") => n * 1.609,
        u if u.starts_with("league") => n * 4.828, // ~3 miles
        _ => n,
    })
}

/// Extract a duration from a sentence, normalised to days. Digits or spelled-out
/// one..twelve; days or weeks.
fn find_duration_days(s: &str) -> Option<f32> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\b(\d+|one|two|three|four|five|six|seven|eight|nine|ten|eleven|twelve)\s+(day|days|week|weeks)\b",
        )
        .unwrap()
    });
    let caps = re.captures(s)?;
    let n = word_to_number(caps.get(1)?.as_str())?;
    let unit = caps.get(2)?.as_str().to_ascii_lowercase();
    Some(if unit.starts_with("week") { n * 7.0 } else { n })
}

fn word_to_number(w: &str) -> Option<f32> {
    if let Ok(n) = w.parse::<f32>() {
        return Some(n);
    }
    Some(match w.to_ascii_lowercase().as_str() {
        "one" => 1.0,
        "two" => 2.0,
        "three" => 3.0,
        "four" => 4.0,
        "five" => 5.0,
        "six" => 6.0,
        "seven" => 7.0,
        "eight" => 8.0,
        "nine" => 9.0,
        "ten" => 10.0,
        "eleven" => 11.0,
        "twelve" => 12.0,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_ledger() -> MagicLedger {
        MagicLedger::default()
    }

    #[test]
    fn flags_an_impossible_pace() {
        // 612 km in 3 days = 204 km/day → far exceeds → contradiction.
        let f = check_paragraph(
            "The messenger rode 612 km in three days to reach the capital.",
            &empty_ledger(),
            &[],
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "travel_time");
        assert_eq!(f[0].severity, "contradiction");
        assert!(f[0].suppressed_by.is_none());
    }

    #[test]
    fn passes_a_plausible_pace() {
        // 120 km in 3 days = 40 km/day → plausible → no finding.
        let f = check_paragraph("They walked 120 km in three days.", &empty_ledger(), &[]);
        assert!(f.is_empty(), "got {f:?}");
    }

    #[test]
    fn miles_are_converted() {
        // 300 miles ≈ 483 km in 2 days ≈ 241 km/day → contradiction.
        let f = check_paragraph("She flew 300 miles in two days.", &empty_ledger(), &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "contradiction");
    }

    #[test]
    fn magic_rule_suppresses_with_a_note() {
        let ledger: MagicLedger = serde_hjson::from_str(
            r#"{ enabled: true, rules: [ { kind: "messenger_birds", covers: ["travel_time"], applicable_to: { roles: ["any"] } } ] }"#,
        )
        .unwrap();
        let f = check_paragraph("The messenger rode 612 km in three days.", &ledger, &[]);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "info"); // downgraded
        assert_eq!(f[0].suppressed_by.as_deref(), Some("messenger_birds"));
    }
}
