//! Astronomy-layer outputs — closed-form planetary physics, deterministic from
//! the definition alone (the seed is unused; nothing here is stochastic).

use serde::{Deserialize, Serialize};

/// The complete astronomy-layer output for a world.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct AstronomyOutput {
    /// Stellar mass in solar units (declared, or derived from luminosity).
    pub stellar_mass_solar: f64,
    /// Orbital period in Earth-days (Kepler's third law).
    pub orbital_period_days_earth: f64,
    /// The year length expressed in the planet's own days.
    pub year_length_planet_days: f64,
    /// The author-declared year length, if any.
    pub declared_year_length_days: Option<f64>,
    /// Percent divergence of the declared value from the computed one
    /// (`(declared - computed) / computed * 100`), if a value was declared.
    pub year_length_divergence_pct: Option<f64>,
    pub axial_tilt_deg: f64,
    /// The four season markers (equinoxes + solstices) as fractions of the year
    /// and as a planet-day-of-year.
    pub seasons: Vec<SeasonMarker>,
    /// Relative daily insolation per 10° latitude band, at each season and as an
    /// annual mean. Normalised so equator-at-equinox = 1.0.
    pub insolation_bands: Vec<InsolationBand>,
    pub moons: Vec<MoonOutput>,
    /// Per-moon eclipse-alignment estimate (an upper bound — see the note).
    pub eclipses: Vec<EclipsePotential>,
    pub tide: TideSummary,
    pub calendar_check: CalendarCheck,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SeasonMarker {
    /// e.g. "vernal_equinox", "summer_solstice".
    pub name: String,
    /// Fraction of the orbit `[0,1)` at which it occurs.
    pub year_fraction: f64,
    /// The planet-day-of-year (0-based) at which it occurs.
    pub planet_day_of_year: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct InsolationBand {
    /// The band's centre latitude in degrees (negative = southern hemisphere).
    pub lat_center_deg: f64,
    pub summer: f64,
    pub equinox: f64,
    pub winter: f64,
    pub annual_mean: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct MoonOutput {
    pub name: String,
    pub mass_lunar: f64,
    /// Sidereal period (Earth-days), echoed from the definition.
    pub sidereal_period_days_earth: f64,
    /// Synodic period (new-moon to new-moon), accounting for the planet's orbit.
    pub synodic_period_days_earth: f64,
    /// The synodic period in planet-days.
    pub synodic_period_planet_days: f64,
    /// Lunations per orbital year.
    pub lunar_months_per_year: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct EclipsePotential {
    pub moon: String,
    /// Potential solar-eclipse alignments (new moons) per year. Upper bound.
    pub potential_solar_alignments_per_year: f64,
    /// Potential lunar-eclipse alignments (full moons) per year. Upper bound.
    pub potential_lunar_alignments_per_year: f64,
    /// Honest caveat: true eclipse frequency needs the orbital inclination /
    /// node geometry, which the definition does not declare.
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TideContribution {
    pub moon: String,
    /// Share of the total lunar tidal forcing, as a percentage.
    pub relative_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TideSummary {
    pub contributions: Vec<TideContribution>,
    /// The moon with the largest tidal forcing, if any moons exist.
    pub dominant_moon: Option<String>,
    /// Solar tidal forcing relative to the strongest moon (Earth ≈ 0.46).
    pub solar_relative_to_dominant: f64,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct CalendarCheck {
    /// `months × month_length_days`.
    pub declared_days: f64,
    /// The computed year length in planet-days.
    pub computed_days: f64,
    /// `declared_days - computed_days` (positive = calendar runs long).
    pub diff_days: f64,
    /// Whether the declared calendar is within ±1 day of the computed year.
    pub consistent: bool,
}
