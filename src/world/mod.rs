//! WORLD-4 — World Simulation (`inkhaven realworld` + the real-time
//! fact-checker). See `Documentation/PROPOSALS/WORLD-4_PLAN.md`.
//!
//! Branch A is a deterministic, layered, on-demand compiler that turns a
//! structured world definition (`world.hjson` + optional `world.bund`) into
//! populated system books. Five MVP layers run in dependency order —
//! astronomy → geology → climate → hydrology → demographics — each a pure
//! function of `(definition, seed)` (layer 5 has seeded-stochastic + AI parts).
//!
//! All five physical layers are built and tested (`compile/`), each materialises
//! into its World-book chapter (`materialize.rs`), and the surface is complete:
//! `realworld compile [--layer <name>|all]`, the plakat `map`, the `places`
//! bridge, `magic`, `coherence`, co-location, proposals, and the fast/slow
//! fact-checker. WORLD-7 (1.6.0) unifies a bare `compile` into a one-command
//! whole-world compile + materialise, surfaces every layer in the TUI, and
//! deepens the world→prose bridge.
//!
//! Authority discipline (the spine of the RFC): the author always wins. The
//! compiler *proposes*; nothing commits without acceptance. Astronomy is the one
//! layer with no proposals — its outputs are closed-form physics, treated as
//! fact and re-asserted every run unless the author hand-overrides.


// WORLD-7 — the simulation is now wired end-to-end (compile → materialise →
// surface), so the blanket `unused_imports` allow is retired. A handful of
// foundation items remain ahead of their consumers (the `WorldError`/`Result`
// pair, a few reserved fact-check / storage / utopia helpers), so `dead_code`
// stays scoped here.
#![allow(dead_code)]

pub mod calc_read;
pub mod commit;
pub mod compile;
pub mod critique;
pub mod fact_check;
pub mod fact_check_lang;
pub mod language_proposals;
pub mod fact_check_slow;
pub mod materialize;
pub mod plakat;
pub mod plausibility;
pub mod myth_proposals;
pub mod proposals;
pub mod ruler_proposals;
pub mod scene;
pub mod storage;
pub mod timeline_context;
pub mod travel;
pub mod types;
pub mod weather;
// WORLD-6 — utopian/dystopian coherence checker.
pub mod utopia;

use std::fmt;

/// The crate-local result type for world operations.
pub type Result<T> = std::result::Result<T, WorldError>;

/// Errors from parsing, validating, or compiling a world definition.
#[derive(Debug, Clone)]
pub enum WorldError {
    /// The `world.hjson` could not be parsed into a [`types::WorldDefinition`].
    Parse(String),
    /// The definition parsed but failed an internal-consistency check.
    Validate(String),
    /// A compilation layer failed.
    Compile(String),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Parse(m) => write!(f, "world definition parse error: {m}"),
            WorldError::Validate(m) => write!(f, "world definition invalid: {m}"),
            WorldError::Compile(m) => write!(f, "world compile error: {m}"),
        }
    }
}

impl std::error::Error for WorldError {}

/// The `world.hjson` scaffold `inkhaven realworld new` writes, and the base the
/// worldbuilder folds deltas onto when a project has no definition yet — so
/// every REQUIRED field (name, astronomy star/planet/orbit/calendar) is present.
pub fn starter_template(name: &str) -> String {
    // A JSON string literal is valid HJSON, so quotes / backslashes / newlines
    // in the name can no longer break the file the scaffold writes.
    let name = serde_json::to_string(name).unwrap_or_else(|_| "\"Untitled world\"".to_string());
    format!(
        r#"// A world definition for `inkhaven realworld`.
// Edit freely, then `inkhaven realworld compile --materialize` to compile and
// write the whole world (astronomy · geology · climate · hydrology · demographics)
// into the World book, or `compile --layer <name>` for one layer. Geology /
// climate / hydrology / demographics are generated from `seed` below; add an
// optional block for any of them to override the defaults, and `magic: {{ … }}`
// to declare an author rules ledger (`realworld magic`).
{{
    name: {name}
    seed: 0x1A2B3C
    primary_language: "en"

    astronomy: {{
        star: {{ class: "G2V", age_gyr: 4.6, luminosity_solar: 1.0 }}
        planet: {{
            mass_earth: 1.0
            radius_earth: 1.0
            axial_tilt_deg: 23.4
            day_length_hours: 24.0
            rotation_direction: "prograde"
        }}
        orbit: {{ semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }}
        moons: [
            {{ name: "Moon", mass_lunar: 1.0, period_days: 27.32 }}
        ]
        calendar: {{
            months: 12
            month_length_days: 30
            weekdays: 7
            new_year_aligns_to: "winter_solstice"
        }}
    }}
}}
"#
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn starter_template_survives_a_hostile_name() {
        for name in ["Thalor", "The \"Ninth\" Lantern", "back\\slash", "two\nlines", "Земля"] {
            let def = crate::world::types::WorldDefinition::from_hjson(&starter_template(name))
                .unwrap_or_else(|e| panic!("{name:?}: {e}"));
            assert_eq!(def.name, name);
        }
    }

    use super::starter_template;
}
