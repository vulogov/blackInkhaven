//! WORLD-10 — travel / distance continuity. Given a straight-line distance and a
//! claimed travel time + mode, report whether the journey is plausible on this
//! planet. Deterministic; the caller consults the magic ledger's `travel_time`
//! rules for sanctioned exceptions. A concrete continuity catch, cheap because
//! the map already carries coordinates and the astronomy carries the planet size.

#[derive(Debug, Clone, PartialEq)]
pub struct TravelAssessment {
    pub distance_km: f64,
    pub mode: String,
    /// Minimum days the straight-line distance needs at this mode's pace.
    pub needed_days: f64,
    pub claimed_days: f64,
    /// False when the claim is faster than physically reasonable.
    pub plausible: bool,
}

/// Sustained overland/sea pace in km/day (rough, pre-industrial).
pub fn speed_km_per_day(mode: &str) -> f64 {
    match mode {
        "ship" | "sail" | "sea" => 120.0,
        "horse" | "ride" | "mounted" => 55.0,
        "cart" | "wagon" => 25.0,
        _ => 30.0, // foot / march
    }
}

pub fn assess(distance_km: f64, claimed_days: f64, mode: &str) -> TravelAssessment {
    let needed = distance_km / speed_km_per_day(mode).max(1.0);
    // Real routes are longer than the straight line, so the red flag is a claim
    // *faster* than the straight-line minimum. Allow a small margin (85%).
    let plausible = claimed_days >= needed * 0.85;
    TravelAssessment {
        distance_km,
        mode: mode.to_string(),
        needed_days: needed,
        claimed_days,
        plausible,
    }
}

/// Kilometres per map cell, from the planet's radius and the grid width (the grid
/// is treated as spanning the planet's circumference).
pub fn km_per_cell(radius_earth: f64, grid_width: usize) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let circumference = 2.0 * std::f64::consts::PI * radius_earth.max(0.01) * EARTH_RADIUS_KM;
    circumference / (grid_width.max(1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_day_on_foot_across_a_hundred_km_is_implausible() {
        // 100 km on foot (30 km/day) needs ~3.3 days.
        let a = assess(100.0, 1.0, "foot");
        assert!(!a.plausible);
        assert!(a.needed_days > 3.0);
        // Four days is fine.
        assert!(assess(100.0, 4.0, "foot").plausible);
    }

    #[test]
    fn ships_are_faster_than_marching() {
        assert!(speed_km_per_day("ship") > speed_km_per_day("foot"));
        // A voyage plausible by ship can be impossible on foot in the same time.
        assert!(assess(300.0, 3.0, "ship").plausible);
        assert!(!assess(300.0, 3.0, "foot").plausible);
    }

    #[test]
    fn km_per_cell_scales_with_planet_and_grid() {
        // Earth-size planet, 360-cell grid → ~111 km/cell (a degree).
        let k = km_per_cell(1.0, 360);
        assert!((k - 111.2).abs() < 2.0, "got {k}");
        // A bigger planet → more km per cell at the same grid width.
        assert!(km_per_cell(2.0, 360) > k);
    }
}
