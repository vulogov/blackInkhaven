//! WORLD-10 — "weather / season at a date". Given a scene's day-of-year and a
//! place's latitude, compute the local season and a relative-insolation
//! descriptor from the compiled astronomy (season markers + per-band insolation).
//! Deterministic, zero-cost — so a scene's descriptions stay consistent with the
//! planet the author built.

use std::f64::consts::PI;

use crate::world::types::AstronomyOutput;

#[derive(Debug, Clone, PartialEq)]
pub struct WeatherAtDate {
    /// Local season (hemisphere-corrected): spring / summer / autumn / winter.
    pub season: String,
    /// Relative daily insolation at this latitude+day (0 = local mid-winter,
    /// 1 = local mid-summer, interpolated from the band's summer/winter values).
    pub insolation: f64,
    /// A short qualitative descriptor ("high summer", "deep winter", …).
    pub descriptor: String,
    /// The insolation band's centre latitude actually used.
    pub lat_band_deg: f64,
}

/// The season an interval starting at `marker` opens (northern-hemisphere naming;
/// flipped for the southern hemisphere by the caller).
fn season_after(marker: &str) -> &'static str {
    let m = marker.to_ascii_lowercase();
    if m.contains("vernal") || m.contains("spring") {
        "spring"
    } else if m.contains("summer") {
        "summer"
    } else if m.contains("autumn") || m.contains("fall") {
        "autumn"
    } else {
        "winter"
    }
}

fn flip_hemisphere(season: &str) -> &'static str {
    match season {
        "spring" => "autumn",
        "summer" => "winter",
        "autumn" => "spring",
        _ => "summer", // winter
    }
}

pub fn weather_at(astro: &AstronomyOutput, day_of_year: f64, lat_deg: f64) -> WeatherAtDate {
    let year = astro.year_length_planet_days.max(1.0);
    let day = day_of_year.rem_euclid(year);
    let southern = lat_deg < 0.0;

    // Season interval: the marker with the greatest planet-day-of-year <= day
    // (wrapping to the last marker when the day precedes them all).
    let mut markers: Vec<_> = astro.seasons.clone();
    markers.sort_by(|a, b| a.planet_day_of_year.partial_cmp(&b.planet_day_of_year).unwrap_or(std::cmp::Ordering::Equal));
    let opener = markers
        .iter()
        .rev()
        .find(|m| m.planet_day_of_year <= day)
        .or_else(|| markers.last());
    let mut season = opener.map(|m| season_after(&m.name)).unwrap_or("summer").to_string();
    if southern {
        season = flip_hemisphere(&season).to_string();
    }

    // Insolation: interpolate between the nearest band's winter (trough) and
    // summer (peak) using a cosine that peaks at the local summer solstice.
    let band = astro
        .insolation_bands
        .iter()
        .min_by(|a, b| {
            (a.lat_center_deg - lat_deg)
                .abs()
                .partial_cmp(&(b.lat_center_deg - lat_deg).abs())
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    let summer_day = astro
        .seasons
        .iter()
        .find(|s| s.name.to_ascii_lowercase().contains("summer"))
        .map(|s| s.planet_day_of_year)
        .unwrap_or(year * 0.5);
    // 1 at (northern) summer solstice, 0 half a year later.
    let north_frac = (1.0 + (2.0 * PI * (day - summer_day) / year).cos()) / 2.0;
    let local_frac = if southern { 1.0 - north_frac } else { north_frac };
    let (insolation, lat_band_deg) = match band {
        Some(b) => (b.winter + (b.summer - b.winter) * local_frac, b.lat_center_deg),
        None => (local_frac, lat_deg),
    };

    let descriptor = match local_frac {
        f if f >= 0.85 => "high summer",
        f if f >= 0.6 => "warm, late spring / early summer",
        f if f >= 0.4 => "mild, near the equinox",
        f if f >= 0.15 => "cool, late autumn / early winter",
        _ => "deep winter",
    }
    .to_string();

    WeatherAtDate { season, insolation, descriptor, lat_band_deg }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::compile_astronomy;
    use crate::world::types::WorldDefinition;

    fn earth() -> AstronomyOutput {
        // The starter template is Earth-like (23.4° tilt, 365-day year).
        let def = WorldDefinition::from_hjson(
            r#"{ name: "E", astronomy: {
                star: { class: "G2V", luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0, eccentricity: 0.017, year_length_days: 365 }
                moons: []
                calendar: { months: 12, month_length_days: 30 }
            } }"#,
        )
        .unwrap();
        compile_astronomy(&def.astronomy)
    }

    #[test]
    fn summer_solstice_is_warmest_in_the_north() {
        let a = earth();
        let solstice = a
            .seasons
            .iter()
            .find(|s| s.name.contains("summer"))
            .unwrap()
            .planet_day_of_year;
        let w = weather_at(&a, solstice, 45.0);
        assert_eq!(w.season, "summer");
        assert!(w.descriptor.contains("summer"));
        // Half a year later is winter at the same latitude.
        let winter = weather_at(&a, solstice + a.year_length_planet_days / 2.0, 45.0);
        assert_eq!(winter.season, "winter");
        assert!(winter.insolation < w.insolation);
    }

    #[test]
    fn hemispheres_are_opposite() {
        let a = earth();
        let solstice = a.seasons.iter().find(|s| s.name.contains("summer")).unwrap().planet_day_of_year;
        let north = weather_at(&a, solstice, 45.0);
        let south = weather_at(&a, solstice, -45.0);
        assert_eq!(north.season, "summer");
        assert_eq!(south.season, "winter");
    }
}
