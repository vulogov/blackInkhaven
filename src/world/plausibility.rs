//! WORLD / WBLD-1 (WB-P3) — deterministic plausibility warnings + score.
//!
//! The per-layer `lint_*` functions return [`Warning`]s carrying a [`Severity`].
//! [`run_fast`] compiles the world's layers and runs every lint, deterministically
//! and without an LLM, so the worldbuilder can score a world 0–100 live:
//! `100 − Σ severity weight` (High 10 · Medium 5 · Low 2), clamped. Warnings a
//! `MagicRule` suppresses are already excluded before they reach the scorer (the
//! ledger runs inside the compile/lint path), so a valid exception raises the
//! score.
//!
//! `Warning` derefs to `&str` and displays as its text, so the existing CLI
//! callers that print or substring-match findings keep working unchanged.

use crate::world::types::WorldDefinition;

/// How much a warning costs the plausibility score.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    High,
    Medium,
    Low,
}

impl Severity {
    /// Points deducted from 100 (RFC §D). Configurable weights are applied by the
    /// caller when it has a `worldbuilder.plausibility_weights` block.
    pub fn weight(self) -> i32 {
        match self {
            Severity::High => 10,
            Severity::Medium => 5,
            Severity::Low => 2,
        }
    }
}

/// One deterministic plausibility finding.
#[derive(Debug, Clone)]
pub struct Warning {
    pub severity: Severity,
    pub text: String,
}

impl Warning {
    pub fn high(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::High, text: text.into() }
    }
    pub fn medium(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::Medium, text: text.into() }
    }
    pub fn low(text: impl Into<String>) -> Warning {
        Warning { severity: Severity::Low, text: text.into() }
    }
    /// Prefix the text with its originating layer (`"nations: …"`), keeping the
    /// severity — used when aggregating across layers in [`run_fast`].
    pub fn prefixed(self, layer: &str) -> Warning {
        Warning { severity: self.severity, text: format!("{layer}: {}", self.text) }
    }
}

impl std::fmt::Display for Warning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.text)
    }
}

impl std::ops::Deref for Warning {
    type Target = str;
    fn deref(&self) -> &str {
        &self.text
    }
}

/// Convert a bag of severity-tagged warnings into a 0–100 plausibility score.
pub fn compute_plausibility_score(warnings: &[Warning]) -> u8 {
    let mut score: i32 = 100;
    for w in warnings {
        score -= w.severity.weight();
    }
    score.clamp(0, 100) as u8
}

/// Compile every layer and run every deterministic lint, returning the aggregate
/// warnings (layer-prefixed). No LLM, no I/O beyond the pure compile chain —
/// suitable for the worldbuilder's live score. NB: geology uses the pure
/// `compile_geology` (not the project-DEM `geology_for`); DEM-aware geology is a
/// WB-P5 refinement.
pub fn run_fast(def: &WorldDefinition) -> Vec<Warning> {
    use crate::world::compile::{
        astronomy_layer, climate_layer, culture_layer, demographics_layer, ecology_layer,
        geology_layer, history_layer, hydrology_layer, polities_layer,
    };

    let astro = astronomy_layer::compile_astronomy(&def.astronomy);
    let geo = geology_layer::compile_geology(def);
    let climate = climate_layer::compile_climate(def, &astro, &geo);
    let hydro = hydrology_layer::compile_hydrology(&geo, &climate);
    let demo = demographics_layer::compile_demographics(&climate, &hydro);
    let seed = def.seed_u64();

    let mut out: Vec<Warning> = Vec::new();

    let declared_hist = def.history.as_ref().map(|h| h.events.as_slice()).unwrap_or(&[]);
    if !declared_hist.is_empty() {
        let hist = history_layer::compile_history(&demo, declared_hist, seed);
        out.extend(
            history_layer::lint_history(declared_hist, &hist)
                .into_iter()
                .map(|w| w.prefixed("history")),
        );
    }
    if !def.nations.is_empty() {
        out.extend(
            polities_layer::lint_polities(&def.nations, &demo)
                .into_iter()
                .map(|w| w.prefixed("nations")),
        );
    }
    if let Some(hy) = def.hydrology.as_ref() {
        if hy.rivers.iter().any(|r| r.from.is_some() && r.to.is_some()) {
            out.extend(
                hydrology_layer::lint_rivers(hy, &geo)
                    .into_iter()
                    .map(|w| w.prefixed("rivers")),
            );
        }
    }
    if !def.cultures.is_empty() {
        let pol = polities_layer::compile_polities(&demo, &def.nations, seed);
        let capital_biomes: Vec<String> = pol
            .polities
            .iter()
            .map(|q| {
                demo.settlements
                    .iter()
                    .find(|s| (s.x, s.y) == q.capital_pos)
                    .map(|s| s.biome.clone())
                    .unwrap_or_default()
            })
            .collect();
        out.extend(
            culture_layer::lint_culture(&def.cultures, &pol, &capital_biomes)
                .into_iter()
                .map(|w| w.prefixed("culture")),
        );
    }
    if let Some(eco) = def.ecology.as_ref().filter(|e| !e.regions.is_empty()) {
        out.extend(
            ecology_layer::lint_ecology(&eco.regions, &climate)
                .into_iter()
                .map(|w| w.prefixed("ecology")),
        );
    }
    if let Some(m) = def.magic.as_ref() {
        out.extend(m.lint().into_iter().map(|w| w.prefixed("magic")));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_is_100_with_no_warnings() {
        assert_eq!(compute_plausibility_score(&[]), 100);
    }

    #[test]
    fn score_deducts_by_severity_weight_and_clamps() {
        let ws = vec![
            Warning::high("a"),   // -10
            Warning::medium("b"), // -5
            Warning::low("c"),    // -2
        ];
        assert_eq!(compute_plausibility_score(&ws), 100 - 10 - 5 - 2);
        // Clamp at 0.
        let many: Vec<Warning> = (0..20).map(|_| Warning::high("x")).collect();
        assert_eq!(compute_plausibility_score(&many), 0);
    }

    #[test]
    fn warning_derefs_to_str_and_displays_as_text() {
        let w = Warning::medium("river flows uphill");
        assert!(w.contains("uphill")); // via Deref<str>
        assert_eq!(format!("{w}"), "river flows uphill");
        assert_eq!(w.prefixed("rivers").text, "rivers: river flows uphill");
    }
}
