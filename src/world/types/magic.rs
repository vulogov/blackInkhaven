//! WORLD-4 — the magic ledger (RFC §8.2). The author's declared exceptions to
//! physics. The fact-checker consults it *after* generating a candidate warning:
//! if a rule covers the warning's category and applies to the actors/place, the
//! warning is suppressed with a note ("consistent with declared magic rule …").
//! That keeps a one-time author setup from producing a permanent stream of false
//! positives.
//!
//! `kind` is a controlled vocabulary in spirit (extended_lifespan,
//! weather_control, messenger_birds, …) but stored as a string so a project can
//! introduce its own without a code change. Kind-specific parameters
//! (`speed_kph_override`, `multiplier`, `frequency_per_year`, …) flatten into
//! `parameters`.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MagicLedger {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub rules: Vec<MagicRule>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MagicRule {
    pub kind: String,
    /// Which fact-check categories this rule may suppress, e.g. `["travel_time"]`.
    #[serde(default)]
    pub covers: Vec<String>,
    #[serde(default)]
    pub description: String,
    /// Who / where / when it applies.
    #[serde(default, rename = "applicable_to")]
    pub applicable_to: Applicability,
    /// Kind-specific parameters (everything not named above).
    #[serde(flatten)]
    pub parameters: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Applicability {
    #[serde(default)]
    pub roles: Option<Vec<String>>,
    #[serde(default)]
    pub regions: Option<Vec<String>>,
    #[serde(default)]
    pub seasons: Option<Vec<String>>,
}

/// The context a candidate warning is generated in, for ledger consultation.
#[derive(Debug, Clone, Default)]
pub struct CheckContext<'a> {
    pub category: &'a str,
    pub roles: &'a [String],
    pub region: Option<&'a str>,
    pub season: Option<&'a str>,
}

/// The built-in fact-check categories a rule's `covers` can target. Advisory:
/// a project may introduce its own, and a checker that emits a custom category
/// will still be suppressed by a matching rule.
pub const KNOWN_CATEGORIES: &[&str] = &[
    "astronomy",
    "climate",
    "climate_anomaly",
    "date_coherence",
    "demographics",
    "economy",
    "travel_time",
    "character_age",
];

impl MagicLedger {
    /// WORLD-7 (W7-P4) — lint the ledger for internal problems. Returns advisory
    /// strings; the ledger is never rejected (inform, never block). Catches the
    /// dead / malformed / redundant cases so a one-time setup doesn't silently
    /// fail to suppress.
    pub fn lint(&self) -> Vec<String> {
        let mut out = Vec::new();
        if self.enabled && self.rules.is_empty() {
            out.push("ledger is enabled but has no rules".to_string());
        }
        if !self.enabled && !self.rules.is_empty() {
            out.push(format!(
                "{} rule(s) declared but the ledger is disabled (enabled: false) — nothing suppresses",
                self.rules.len()
            ));
        }
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (i, r) in self.rules.iter().enumerate() {
            let label = if r.kind.trim().is_empty() {
                format!("rule #{}", i + 1)
            } else {
                format!("rule `{}`", r.kind)
            };
            if r.kind.trim().is_empty() {
                out.push(format!("{label}: empty `kind`"));
            }
            if r.covers.is_empty() {
                out.push(format!("{label}: covers no category — it can never suppress a warning"));
            }
            for c in &r.covers {
                if !c.eq_ignore_ascii_case("any") && !KNOWN_CATEGORIES.contains(&c.as_str()) {
                    out.push(format!(
                        "{label}: `{c}` is not a built-in fact-check category (check spelling; a custom one is fine)"
                    ));
                }
            }
            let mut covers_sorted = r.covers.clone();
            covers_sorted.sort();
            let sig = format!("{}|{}", r.kind.to_lowercase(), covers_sorted.join(","));
            if !r.kind.trim().is_empty() && !seen.insert(sig) {
                out.push(format!("{label}: duplicate of an earlier rule (same kind + covers)"));
            }
        }
        out
    }

    /// Find a rule that suppresses a candidate finding in the given context — the
    /// fact-checker's lazy consultation. `None` means "emit the warning".
    pub fn find_suppressor(&self, ctx: &CheckContext) -> Option<&MagicRule> {
        if !self.enabled {
            return None;
        }
        self.rules.iter().find(|r| r.covers.iter().any(|c| c == ctx.category) && r.applies(ctx))
    }
}

impl MagicRule {
    /// Does this rule apply in `ctx`? An unset facet ("any" / `None`) matches
    /// everything; a set facet must intersect the context.
    pub fn applies(&self, ctx: &CheckContext) -> bool {
        // Roles: also honour a bare `applies_to_role` parameter (RFC examples
        // use both spellings).
        let role_ok = match &self.applicable_to.roles {
            Some(rs) if !any(rs) => rs.iter().any(|r| ctx.roles.iter().any(|c| c == r)),
            _ => match self.parameters.get("applies_to_role").and_then(|v| v.as_str()) {
                Some(role) => ctx.roles.iter().any(|c| c == role),
                None => true,
            },
        };
        let region_ok = facet_matches(&self.applicable_to.regions, ctx.region);
        let season_ok = facet_matches(&self.applicable_to.seasons, ctx.season);
        role_ok && region_ok && season_ok
    }
}

/// `true` if the list is `None`, contains "any", or contains `value`.
fn facet_matches(list: &Option<Vec<String>>, value: Option<&str>) -> bool {
    match list {
        None => true,
        Some(items) if any(items) => true,
        Some(items) => match value {
            Some(v) => items.iter().any(|i| i == v),
            None => false,
        },
    }
}

fn any(items: &[String]) -> bool {
    items.iter().any(|i| i.eq_ignore_ascii_case("any"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ledger() -> MagicLedger {
        let body = r#"{
            enabled: true
            rules: [
                {
                    kind: "messenger_birds"
                    covers: ["travel_time"]
                    description: "Royal pelicans fly day and night"
                    speed_kph_override: 80
                    applicable_to: { roles: ["royal_messenger"], regions: ["any"] }
                }
                {
                    kind: "extended_lifespan"
                    covers: ["character_age"]
                    description: "Sun-priests live to 200"
                    applies_to_role: "sun_priest"
                    multiplier: 2.5
                }
            ]
        }"#;
        serde_hjson::from_str(body).unwrap()
    }

    #[test]
    fn suppresses_a_covered_finding_for_the_right_role() {
        let l = ledger();
        let roles = vec!["royal_messenger".to_string()];
        let ctx = CheckContext { category: "travel_time", roles: &roles, ..Default::default() };
        let r = l.find_suppressor(&ctx).expect("should suppress");
        assert_eq!(r.kind, "messenger_birds");
        // The kind-specific parameter survived the flatten.
        assert_eq!(r.parameters.get("speed_kph_override").and_then(|v| v.as_i64()), Some(80));
    }

    #[test]
    fn does_not_suppress_a_different_category_or_role() {
        let l = ledger();
        let messenger = vec!["royal_messenger".to_string()];
        // Right role, wrong category.
        let ctx = CheckContext { category: "climate_anomaly", roles: &messenger, ..Default::default() };
        assert!(l.find_suppressor(&ctx).is_none());
        // Right category, wrong role.
        let farmer = vec!["farmer".to_string()];
        let ctx = CheckContext { category: "travel_time", roles: &farmer, ..Default::default() };
        assert!(l.find_suppressor(&ctx).is_none());
    }

    #[test]
    fn applies_to_role_parameter_form_works() {
        let l = ledger();
        let priest = vec!["sun_priest".to_string()];
        let ctx = CheckContext { category: "character_age", roles: &priest, ..Default::default() };
        let r = l.find_suppressor(&ctx).expect("should suppress");
        assert_eq!(r.kind, "extended_lifespan");
    }

    #[test]
    fn disabled_ledger_suppresses_nothing() {
        let mut l = ledger();
        l.enabled = false;
        let roles = vec!["royal_messenger".to_string()];
        let ctx = CheckContext { category: "travel_time", roles: &roles, ..Default::default() };
        assert!(l.find_suppressor(&ctx).is_none());
    }

    #[test]
    fn lint_passes_a_clean_ledger() {
        assert!(ledger().lint().is_empty());
    }

    #[test]
    fn lint_flags_the_dead_and_malformed_cases() {
        let body = r#"{
            enabled: true
            rules: [
                { kind: "", covers: ["travel_time"] }
                { kind: "windwalk", covers: [] }
                { kind: "seer", covers: ["climat"] }
                { kind: "windwalk", covers: [] }
            ]
        }"#;
        let l: MagicLedger = serde_hjson::from_str(body).unwrap();
        let out = l.lint();
        assert!(out.iter().any(|s| s.contains("empty `kind`")));
        assert!(out.iter().any(|s| s.contains("covers no category")));
        assert!(out.iter().any(|s| s.contains("not a built-in fact-check category")));
        assert!(out.iter().any(|s| s.contains("duplicate")));
    }

    #[test]
    fn lint_flags_disabled_with_rules() {
        let mut l = ledger();
        l.enabled = false;
        assert!(l.lint().iter().any(|s| s.contains("disabled")));
    }
}
