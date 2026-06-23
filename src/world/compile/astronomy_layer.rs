//! WORLD-4 Layer 1 — Astronomy. Closed-form planetary physics: Kepler's third
//! law for the year, daily-insolation by latitude band, lunar synodic periods,
//! tidal forcing. Deterministic from the definition alone (no seed, no RNG).
//!
//! Astronomy is *fact, not opinion* — the compiler treats the math as truth and
//! re-asserts it on every run unless the author hand-overrides. So this layer
//! produces no proposals; it produces the canonical numbers.

use std::f64::consts::PI;

use crate::world::types::astronomy::*;
use crate::world::types::world::AstronomyDef;

/// Earth's sidereal year, in days — the unit-bridge for Kepler's third law.
const EARTH_SIDEREAL_YEAR_DAYS: f64 = 365.25636;
/// Earth's Moon sidereal period squared (27.32²) — the tidal-forcing reference.
const MOON_REF_PERIOD_SQ: f64 = 746.38;
/// Earth's solar tide as a fraction of its lunar tide.
const SOLAR_TIDE_EARTH_RATIO: f64 = 0.46;

/// Compile the astronomy layer for a world's `astronomy` block.
pub fn compile_astronomy(def: &AstronomyDef) -> AstronomyOutput {
    // Stellar mass: declared, else from the main-sequence mass–luminosity
    // relation L ∝ M^3.5  ⇒  M = L^(1/3.5). (Sun: L=1 ⇒ M=1.)
    let stellar_mass_solar = def
        .star
        .mass_solar
        .unwrap_or_else(|| def.star.luminosity_solar.max(1e-6).powf(1.0 / 3.5));

    // Kepler's third law: P_years = sqrt(a³ / M_star).
    let a = def.orbit.semi_major_axis_au;
    let period_years = (a.powi(3) / stellar_mass_solar.max(1e-6)).sqrt();
    let orbital_period_days_earth = period_years * EARTH_SIDEREAL_YEAR_DAYS;
    let year_length_planet_days =
        orbital_period_days_earth * 24.0 / def.planet.day_length_hours.max(1e-6);

    let declared_year_length_days = def.orbit.year_length_days;
    let year_length_divergence_pct = declared_year_length_days
        .map(|d| (d - year_length_planet_days) / year_length_planet_days * 100.0);

    let tilt = def.planet.axial_tilt_deg;

    // ── Seasons ──────────────────────────────────────────────────────────
    // Markers in orbital order from the vernal equinox; the calendar's new-year
    // anchor decides which sits at year-fraction 0.
    let base = [
        ("vernal_equinox", 0.0_f64),
        ("summer_solstice", 0.25),
        ("autumnal_equinox", 0.5),
        ("winter_solstice", 0.75),
    ];
    let offset = season_anchor_fraction(def.calendar.new_year_aligns_to.as_deref());
    let seasons = base
        .iter()
        .map(|(name, frac)| {
            let year_fraction = (frac - offset).rem_euclid(1.0);
            SeasonMarker {
                name: (*name).to_string(),
                year_fraction,
                planet_day_of_year: year_fraction * year_length_planet_days,
            }
        })
        .collect();

    // ── Insolation by 10° latitude band ─────────────────────────────────
    let base_insol = daily_insolation(0.0, 0.0); // equator at equinox = the unit
    let insolation_bands = (0..18)
        .map(|i| {
            let lat = -85.0 + (i as f64) * 10.0;
            let at_plus = daily_insolation(lat, tilt) / base_insol;
            let at_minus = daily_insolation(lat, -tilt) / base_insol;
            let equinox = daily_insolation(lat, 0.0) / base_insol;
            let summer = at_plus.max(at_minus);
            let winter = at_plus.min(at_minus);
            InsolationBand {
                lat_center_deg: lat,
                summer,
                equinox,
                winter,
                // Trapezoidal mean over the year (solstice, equinox, solstice).
                annual_mean: (at_plus + 2.0 * equinox + at_minus) / 4.0,
            }
        })
        .collect();

    // ── Moons: synodic periods + lunations ──────────────────────────────
    let moons: Vec<MoonOutput> = def
        .moons
        .iter()
        .map(|m| {
            let p_s = m.period_days.max(1e-6);
            // Synodic period: 1/T_syn = 1/T_moon - 1/T_orbit (prograde moon).
            let inv = 1.0 / p_s - 1.0 / orbital_period_days_earth;
            let synodic_earth = if inv > 1e-9 { 1.0 / inv } else { p_s };
            MoonOutput {
                name: m.name.clone(),
                mass_lunar: m.mass_lunar,
                sidereal_period_days_earth: p_s,
                synodic_period_days_earth: synodic_earth,
                synodic_period_planet_days: synodic_earth * 24.0
                    / def.planet.day_length_hours.max(1e-6),
                lunar_months_per_year: orbital_period_days_earth / synodic_earth,
            }
        })
        .collect();

    let eclipses = moons
        .iter()
        .map(|m| EclipsePotential {
            moon: m.name.clone(),
            potential_solar_alignments_per_year: m.lunar_months_per_year,
            potential_lunar_alignments_per_year: m.lunar_months_per_year,
            note: "upper bound; actual eclipse frequency depends on the orbital \
                   inclination / node geometry, which the definition does not declare"
                .to_string(),
        })
        .collect();

    // ── Tides ────────────────────────────────────────────────────────────
    // Tidal forcing ∝ mass / distance³. For a moon, distance ∝ (M_planet·P²)^⅓
    // (Kepler about the planet), so force ∝ mass·P_ref² / (M_planet·P²), in units
    // of Earth's-Moon tide. The sun's forcing is Earth-calibrated (0.46×lunar).
    let m_planet = def.planet.mass_earth.max(1e-6);
    let indices: Vec<(String, f64)> = def
        .moons
        .iter()
        .map(|m| {
            let idx = m.mass_lunar * MOON_REF_PERIOD_SQ / (m_planet * m.period_days.max(1e-6).powi(2));
            (m.name.clone(), idx)
        })
        .collect();
    let total: f64 = indices.iter().map(|(_, v)| v).sum();
    let contributions = indices
        .iter()
        .map(|(name, v)| TideContribution {
            moon: name.clone(),
            relative_pct: if total > 0.0 { v / total * 100.0 } else { 0.0 },
        })
        .collect();
    let dominant = indices
        .iter()
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .cloned();
    let solar_index = SOLAR_TIDE_EARTH_RATIO * stellar_mass_solar / a.max(1e-6).powi(3);
    let solar_relative_to_dominant = match &dominant {
        Some((_, idx)) if *idx > 0.0 => solar_index / idx,
        _ => 0.0,
    };
    let tide = TideSummary {
        contributions,
        dominant_moon: dominant.map(|(n, _)| n),
        solar_relative_to_dominant,
    };

    // ── Calendar consistency ─────────────────────────────────────────────
    let declared_days =
        (def.calendar.months as f64) * (def.calendar.month_length_days as f64);
    let diff_days = declared_days - year_length_planet_days;
    let calendar_check = CalendarCheck {
        declared_days,
        computed_days: year_length_planet_days,
        diff_days,
        consistent: diff_days.abs() <= 1.0,
    };

    AstronomyOutput {
        stellar_mass_solar,
        orbital_period_days_earth,
        year_length_planet_days,
        declared_year_length_days,
        year_length_divergence_pct,
        axial_tilt_deg: tilt,
        seasons,
        insolation_bands,
        moons,
        eclipses,
        tide,
        calendar_check,
    }
}

/// The base year-fraction of the season the calendar anchors its new year to.
fn season_anchor_fraction(anchor: Option<&str>) -> f64 {
    match anchor.map(|s| s.trim().to_ascii_lowercase()).as_deref() {
        Some("summer_solstice") => 0.25,
        Some("autumnal_equinox") | Some("autumn_equinox") | Some("fall_equinox") => 0.5,
        Some("winter_solstice") => 0.75,
        // vernal/spring equinox, unknown, or unset → vernal at fraction 0.
        _ => 0.0,
    }
}

/// Relative daily-mean insolation at a latitude for a given solar declination.
/// The standard insolation integral over the daylit hour-angle (distance and
/// eccentricity omitted — a zonal model for fiction, not climate science).
fn daily_insolation(lat_deg: f64, decl_deg: f64) -> f64 {
    let phi = lat_deg.to_radians();
    let dec = decl_deg.to_radians();
    // Sunset hour angle H0 = acos(-tan φ tan δ), clamped for polar day/night.
    let arg = -(phi.tan()) * dec.tan();
    let h0 = if arg <= -1.0 {
        PI // polar day
    } else if arg >= 1.0 {
        0.0 // polar night
    } else {
        arg.acos()
    };
    (h0 * phi.sin() * dec.sin() + phi.cos() * dec.cos() * h0.sin()).max(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::types::world::{AstronomyDef, Calendar, Moon, Orbit, Planet, Star};

    fn earth_like() -> AstronomyDef {
        AstronomyDef {
            star: Star {
                class: "G2V".into(),
                age_gyr: 4.6,
                luminosity_solar: 1.0,
                mass_solar: None,
            },
            planet: Planet {
                mass_earth: 1.0,
                radius_earth: 1.0,
                axial_tilt_deg: 23.44,
                day_length_hours: 24.0,
                rotation_direction: "prograde".into(),
            },
            orbit: Orbit { semi_major_axis_au: 1.0, eccentricity: 0.0167, year_length_days: None },
            moons: vec![Moon {
                name: "Luna".into(),
                mass_lunar: 1.0,
                period_days: 27.32,
                eccentricity: 0.0549,
            }],
            calendar: Calendar {
                months: 12,
                month_length_days: 30,
                weekdays: 7,
                month_names: vec![],
                day_names: vec![],
                new_year_aligns_to: None,
            },
        }
    }

    fn velmaron() -> AstronomyDef {
        AstronomyDef {
            star: Star {
                class: "G2V".into(),
                age_gyr: 4.6,
                luminosity_solar: 1.0,
                mass_solar: None,
            },
            planet: Planet {
                mass_earth: 1.04,
                radius_earth: 1.02,
                axial_tilt_deg: 23.5,
                day_length_hours: 26.2,
                rotation_direction: "prograde".into(),
            },
            orbit: Orbit {
                semi_major_axis_au: 0.98,
                eccentricity: 0.017,
                year_length_days: Some(348.0),
            },
            moons: vec![
                Moon { name: "Korthana".into(), mass_lunar: 1.1, period_days: 28.3, eccentricity: 0.05 },
                Moon { name: "Eldra".into(), mass_lunar: 0.4, period_days: 14.7, eccentricity: 0.02 },
            ],
            calendar: Calendar {
                months: 12,
                month_length_days: 29,
                weekdays: 7,
                month_names: vec![],
                day_names: vec![],
                new_year_aligns_to: Some("winter_solstice".into()),
            },
        }
    }

    #[test]
    fn earth_year_is_about_365_days() {
        let out = compile_astronomy(&earth_like());
        assert!((out.stellar_mass_solar - 1.0).abs() < 1e-6);
        assert!(
            (out.year_length_planet_days - 365.25).abs() < 0.5,
            "got {}",
            out.year_length_planet_days
        );
        // Earth's solar tide is ~0.46× its lunar tide.
        assert!(
            (out.tide.solar_relative_to_dominant - 0.46).abs() < 0.02,
            "got {}",
            out.tide.solar_relative_to_dominant
        );
        assert_eq!(out.tide.dominant_moon.as_deref(), Some("Luna"));
    }

    #[test]
    fn equator_gets_more_annual_sun_than_poles() {
        let out = compile_astronomy(&earth_like());
        let equator = out
            .insolation_bands
            .iter()
            .min_by(|a, b| a.lat_center_deg.abs().partial_cmp(&b.lat_center_deg.abs()).unwrap())
            .unwrap();
        let pole = out
            .insolation_bands
            .iter()
            .max_by(|a, b| a.lat_center_deg.abs().partial_cmp(&b.lat_center_deg.abs()).unwrap())
            .unwrap();
        assert!(
            equator.annual_mean > pole.annual_mean,
            "equator {} should beat pole {}",
            equator.annual_mean,
            pole.annual_mean
        );
        // Equator-at-equinox is the normalisation unit.
        let eq0 = out.insolation_bands.iter().find(|b| b.lat_center_deg == -5.0).unwrap();
        assert!(eq0.equinox > 0.9);
    }

    #[test]
    fn velmaron_flags_calendar_divergence() {
        let out = compile_astronomy(&velmaron());
        // a=0.98, M≈1 ⇒ ~354 Earth-days ⇒ /26.2h ⇒ ~324.6 planet-days.
        assert!(
            (out.year_length_planet_days - 324.6).abs() < 1.0,
            "got {}",
            out.year_length_planet_days
        );
        // Declared 348 vs computed ~324.6 ⇒ the calendar runs ~23 days long.
        assert!(!out.calendar_check.consistent);
        assert!(out.calendar_check.diff_days > 20.0 && out.calendar_check.diff_days < 26.0);
        // Declared year diverges from the physical one by a few percent.
        let div = out.year_length_divergence_pct.unwrap();
        assert!(div > 5.0 && div < 9.0, "got {div}");
        assert_eq!(out.moons.len(), 2);
        // Eldra dominates the tides despite being lighter: at half Korthana's
        // period it orbits far closer, and tidal forcing ∝ 1/distance³.
        assert_eq!(out.tide.dominant_moon.as_deref(), Some("Eldra"));
    }

    #[test]
    fn winter_solstice_anchor_shifts_year_zero() {
        let out = compile_astronomy(&velmaron());
        let winter = out.seasons.iter().find(|s| s.name == "winter_solstice").unwrap();
        // The calendar anchors new year to the winter solstice → fraction 0.
        assert!(winter.year_fraction.abs() < 1e-9);
        let vernal = out.seasons.iter().find(|s| s.name == "vernal_equinox").unwrap();
        assert!((vernal.year_fraction - 0.25).abs() < 1e-9);
    }

    #[test]
    fn compilation_is_deterministic() {
        let def = velmaron();
        assert_eq!(compile_astronomy(&def), compile_astronomy(&def));
    }

    #[test]
    fn parses_world_hjson_and_compiles() {
        use crate::world::types::world::{SeedValue, WorldDefinition};
        // The CLI path: parse a world.hjson body, then compile astronomy from
        // it. Exercises the `0x…` hex seed (HJSON renders unquoted hex as a
        // string) and serde dropping the not-yet-modelled blocks.
        let body = r#"
        {
            name: "Velmaron"
            seed: 0x4A3B7C
            primary_language: "en"
            astronomy: {
                star: { class: "G2V", luminosity_solar: 1.0 }
                planet: { mass_earth: 1.04, radius_earth: 1.02, axial_tilt_deg: 23.5, day_length_hours: 26.2 }
                orbit: { semi_major_axis_au: 0.98, eccentricity: 0.017, year_length_days: 348 }
                moons: [
                    { name: "Korthana", mass_lunar: 1.1, period_days: 28.3 }
                    { name: "Eldra", mass_lunar: 0.4, period_days: 14.7 }
                ]
                calendar: { months: 12, month_length_days: 29, weekdays: 7, new_year_aligns_to: "winter_solstice" }
            }
            geology: { generated: { plates: 7, continents: 4 } }
            magic: { enabled: true, rules: [] }
        }
        "#;
        let def = WorldDefinition::from_hjson(body).expect("parse world.hjson");
        // Hex seed resolves; the unmodelled geology/magic blocks are ignored.
        assert_eq!(def.seed_u64(), 0x4A3B7C);
        assert!(matches!(def.seed, SeedValue::Str(_)));
        assert_eq!(def.name, "Velmaron");
        let out = compile_astronomy(&def.astronomy);
        assert_eq!(out.moons.len(), 2);
        assert!((out.year_length_planet_days - 324.6).abs() < 1.0);
    }
}
