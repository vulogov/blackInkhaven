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

use crate::world::proposals::PlaceLink;
use crate::world::types::magic::{CheckContext, MagicLedger};

/// A project gazetteer over the world-linked Places: resolves a place name in
/// prose to its `PlaceLink` (climate zone / biome / modelled population), so the
/// climate and demographics checks can compare a claim against the place's
/// actual world data.
pub struct Gazetteer {
    places: Vec<PlaceLink>,
}

impl Gazetteer {
    pub fn new(places: Vec<PlaceLink>) -> Self {
        Self { places }
    }

    /// The world-linked places whose name appears (as a whole word) in `text`.
    pub fn mentioned_in(&self, text: &str) -> Vec<&PlaceLink> {
        let lower = text.to_lowercase();
        self.places.iter().filter(|p| contains_word(&lower, &p.name.to_lowercase())).collect()
    }
}

/// The world data the fact-checker needs beyond the magic ledger: the gazetteer
/// (place names → world data) and the astronomy facts (moon names). Bundled so
/// new world-data categories don't keep changing the check signature.
pub struct WorldContext {
    pub gazetteer: Gazetteer,
    /// The world's moon names (for the astronomy check).
    pub moons: Vec<String>,
}

impl WorldContext {
    pub fn new(gazetteer: Gazetteer, moons: Vec<String>) -> Self {
        Self { gazetteer, moons }
    }
}

/// Whole-word containment (so "Or" doesn't match inside "Orenarm").
fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let bytes = haystack.as_bytes();
    let mut from = 0;
    while let Some(pos) = haystack[from..].find(needle) {
        let start = from + pos;
        let end = start + needle.len();
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = start + 1;
    }
    false
}

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
/// roles in scope (for magic-ledger consultation); `gaz` resolves place names to
/// their world data (the climate + demographics checks need it; travel time
/// doesn't). Both may be empty / `None`.
pub fn check_paragraph(
    text: &str,
    ledger: &MagicLedger,
    roles: &[String],
    ctx: Option<&WorldContext>,
) -> Vec<Finding> {
    let mut findings = Vec::new();
    findings.extend(check_travel_time(text, ledger, roles));
    if let Some(c) = ctx {
        findings.extend(check_climate(text, &c.gazetteer, ledger));
        findings.extend(check_population(text, &c.gazetteer, ledger));
        findings.extend(check_astronomy(text, &c.moons, ledger));
    }
    findings
}

/// Astronomy check: a stated moon count that disagrees with the world's sky.
fn check_astronomy(text: &str, moons: &[String], ledger: &MagicLedger) -> Vec<Finding> {
    let world_count = moons.len();
    if world_count == 0 {
        return Vec::new();
    }
    let mut out = Vec::new();
    for sentence in split_sentences(text) {
        let Some(claimed) = find_moon_count(sentence) else {
            continue;
        };
        if claimed == world_count {
            continue;
        }
        let body = format!(
            "The prose implies {claimed} moon(s), but this world has {world_count} ({}).",
            moons.join(", ")
        );
        let ctx = CheckContext { category: "astronomy", ..Default::default() };
        let suppressed_by = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
        let severity = if suppressed_by.is_some() { "info" } else { "warning" };
        out.push(Finding { category: "astronomy".into(), severity: severity.into(), body, suppressed_by });
    }
    out
}

/// Extract a moon count from a sentence: "the three moons", "both moons", "two
/// moons hung overhead".
fn find_moon_count(s: &str) -> Option<usize> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r"(?i)\b(\d+|both|one|two|three|four|five|six|seven)\s+moons?\b",
        )
        .unwrap()
    });
    let caps = re.captures(s)?;
    let w = caps.get(1)?.as_str().to_ascii_lowercase();
    if w == "both" {
        return Some(2);
    }
    word_to_number(&w).map(|n| n as usize)
}

/// Climate check: weather described at a place that contradicts the place's
/// climate zone (snow in the tropics, jungle heat on the tundra).
fn check_climate(text: &str, gaz: &Gazetteer, ledger: &MagicLedger) -> Vec<Finding> {
    let mut out = Vec::new();
    for sentence in split_sentences(text) {
        let Some(weather) = detect_weather(sentence) else {
            continue;
        };
        for p in gaz.mentioned_in(sentence) {
            let Some(verdict) = climate_conflict(&p.climate_zone, weather) else {
                continue;
            };
            let body = format!(
                "{}: {} at {}, whose climate zone is {}.",
                verdict, weather_label(weather), p.name, p.climate_zone.replace('_', " ")
            );
            let ctx = CheckContext {
                category: "climate_anomaly",
                roles: &[],
                region: Some(&p.name),
                ..Default::default()
            };
            let suppressed_by = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
            let severity = if suppressed_by.is_some() { "info" } else { "warning" };
            out.push(Finding {
                category: "climate".into(),
                severity: severity.into(),
                body,
                suppressed_by,
            });
        }
    }
    out
}

/// Demographics check: a population stated for a place that diverges sharply
/// from the modelled population.
fn check_population(text: &str, gaz: &Gazetteer, ledger: &MagicLedger) -> Vec<Finding> {
    let mut out = Vec::new();
    for sentence in split_sentences(text) {
        let Some(claimed) = find_population(sentence) else {
            continue;
        };
        let places: Vec<_> = gaz.mentioned_in(sentence).into_iter().filter(|p| p.population > 0).collect();
        // Only compare when exactly one place is in the sentence (otherwise the
        // number's owner is ambiguous — better to stay quiet than false-positive).
        if places.len() != 1 {
            continue;
        }
        let p = places[0];
        let modeled = p.population as f32;
        let ratio = claimed / modeled;
        if ratio <= 3.0 && ratio >= 0.33 {
            continue; // close enough
        }
        let body = format!(
            "{} is described with ~{} people, but the world models ~{} for it.",
            p.name, fmt_pop(claimed as u64), fmt_pop(p.population)
        );
        let ctx = CheckContext { category: "demographics", region: Some(&p.name), ..Default::default() };
        let suppressed_by = ledger.find_suppressor(&ctx).map(|r| r.kind.clone());
        let severity = if suppressed_by.is_some() { "info" } else { "warning" };
        out.push(Finding {
            category: "demographics".into(),
            severity: severity.into(),
            body,
            suppressed_by,
        });
    }
    out
}

fn split_sentences(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c| c == '.' || c == '!' || c == '?' || c == '\n')
}

#[derive(Clone, Copy, PartialEq)]
enum Weather {
    Cold,
    Hot,
}

fn weather_label(w: Weather) -> &'static str {
    match w {
        Weather::Cold => "freezing weather",
        Weather::Hot => "tropical heat",
    }
}

fn detect_weather(s: &str) -> Option<Weather> {
    let l = s.to_lowercase();
    let cold = ["snow", "snowed", "snowing", "frost", "freezing", "blizzard", "frozen", "ice storm"];
    let hot = ["sweltering", "scorching", "tropical heat", "jungle heat", "blistering sun"];
    if cold.iter().any(|w| l.contains(w)) {
        Some(Weather::Cold)
    } else if hot.iter().any(|w| l.contains(w)) {
        Some(Weather::Hot)
    } else {
        None
    }
}

/// Whether `weather` contradicts a climate zone — returns the severity verb.
fn climate_conflict(zone: &str, weather: Weather) -> Option<&'static str> {
    let warm_zones = ["hot_desert", "savanna", "tropical_rainforest", "tropical_seasonal"];
    let cold_zones = ["tundra", "ice_cap", "taiga"];
    match weather {
        Weather::Cold if warm_zones.contains(&zone) => Some("Implausible"),
        Weather::Hot if cold_zones.contains(&zone) => Some("Implausible"),
        _ => None,
    }
}

/// Extract a population figure ("100,000", "100000", "2 thousand", "1 million").
fn find_population(s: &str) -> Option<f32> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?i)(\d[\d,]*(?:\.\d+)?)\s*(thousand|million)?").unwrap()
    });
    // Find the largest number in the sentence (the population claim).
    let mut best: Option<f32> = None;
    for caps in re.captures_iter(s) {
        let raw = caps.get(1)?.as_str().replace(',', "");
        let Ok(mut n) = raw.parse::<f32>() else { continue };
        match caps.get(2).map(|m| m.as_str().to_ascii_lowercase()) {
            Some(ref u) if u == "thousand" => n *= 1_000.0,
            Some(ref u) if u == "million" => n *= 1_000_000.0,
            _ => {}
        }
        // Only plausibly-population numbers (avoid catching years / small counts).
        if n >= 500.0 && best.map_or(true, |b| n > b) {
            best = Some(n);
        }
    }
    best
}

fn fmt_pop(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
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
            None,
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "travel_time");
        assert_eq!(f[0].severity, "contradiction");
        assert!(f[0].suppressed_by.is_none());
    }

    #[test]
    fn passes_a_plausible_pace() {
        // 120 km in 3 days = 40 km/day → plausible → no finding.
        let f = check_paragraph("They walked 120 km in three days.", &empty_ledger(), &[], None);
        assert!(f.is_empty(), "got {f:?}");
    }

    #[test]
    fn miles_are_converted() {
        // 300 miles ≈ 483 km in 2 days ≈ 241 km/day → contradiction.
        let f = check_paragraph("She flew 300 miles in two days.", &empty_ledger(), &[], None);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "contradiction");
    }

    #[test]
    fn magic_rule_suppresses_with_a_note() {
        let ledger: MagicLedger = serde_hjson::from_str(
            r#"{ enabled: true, rules: [ { kind: "messenger_birds", covers: ["travel_time"], applicable_to: { roles: ["any"] } } ] }"#,
        )
        .unwrap();
        let f = check_paragraph("The messenger rode 612 km in three days.", &ledger, &[], None);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].severity, "info"); // downgraded
        assert_eq!(f[0].suppressed_by.as_deref(), Some("messenger_birds"));
    }

    fn gaz() -> Gazetteer {
        Gazetteer::new(vec![
            PlaceLink {
                place_id: uuid::Uuid::nil(),
                name: "Velmaril".into(),
                biome: "tropical_seasonal".into(),
                climate_zone: "tropical_seasonal".into(),
                hydrology_basis: "river_mouth".into(),
                population: 40_000,
                x: 60,
                y: 69,
            },
            PlaceLink {
                place_id: uuid::Uuid::nil(),
                name: "Korthun".into(),
                biome: "tundra".into(),
                climate_zone: "tundra".into(),
                hydrology_basis: "confluence".into(),
                population: 8_000,
                x: 42,
                y: 12,
            },
        ])
    }

    #[test]
    fn flags_snow_in_the_tropics() {
        let g = WorldContext::new(gaz(), vec![]);
        let f = check_paragraph("A blizzard buried Velmaril overnight.", &empty_ledger(), &[], Some(&g));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "climate");
        assert_eq!(f[0].severity, "warning");
        // No false positive for cold weather at the tundra town.
        let f2 = check_paragraph("A blizzard buried Korthun overnight.", &empty_ledger(), &[], Some(&g));
        assert!(f2.is_empty(), "got {f2:?}");
    }

    #[test]
    fn flags_a_population_mismatch() {
        let g = WorldContext::new(gaz(), vec![]);
        // Velmaril models ~40k; prose claims 2 million → off by 50× → warning.
        let f = check_paragraph("Velmaril, a teeming city of 2 million souls.", &empty_ledger(), &[], Some(&g));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "demographics");
        // A close figure passes.
        let f2 = check_paragraph("Velmaril, a city of 45,000.", &empty_ledger(), &[], Some(&g));
        assert!(f2.is_empty(), "got {f2:?}");
    }

    #[test]
    fn gazetteer_matches_whole_words_only() {
        let g = gaz();
        // "Korthun" must not match inside another word.
        assert_eq!(g.mentioned_in("the Korthuns").len(), 0);
        assert_eq!(g.mentioned_in("near Korthun, north").len(), 1);
    }

    #[test]
    fn flags_wrong_moon_count() {
        let ctx = WorldContext::new(gaz(), vec!["Korthana".into(), "Eldra".into()]);
        // World has 2 moons; prose says three.
        let f = check_paragraph("All three moons hung over the bay.", &empty_ledger(), &[], Some(&ctx));
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].category, "astronomy");
        assert_eq!(f[0].severity, "warning");
        // "both moons" agrees with two → no finding.
        let f2 = check_paragraph("Both moons were full.", &empty_ledger(), &[], Some(&ctx));
        assert!(f2.is_empty(), "got {f2:?}");
    }
}
