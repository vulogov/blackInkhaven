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
    match mode.to_ascii_lowercase().as_str() {
        "ship" | "sail" | "sea" | "boat" => 120.0,
        "horse" | "ride" | "mounted" => 55.0,
        "cart" | "wagon" => 25.0,
        _ => 30.0, // foot / march
    }
}

/// Whether `mode` is a pace the model knows — so the caller can warn that an
/// unrecognized mode (e.g. `boat` misspelled) was assessed at the foot fallback.
pub fn mode_recognized(mode: &str) -> bool {
    matches!(
        mode.to_ascii_lowercase().as_str(),
        "ship" | "sail" | "sea" | "boat" | "horse" | "ride" | "mounted" | "cart" | "wagon"
            | "foot" | "walk" | "march" | "on foot"
    )
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

/// Kilometres per map cell along each axis `(east–west, north–south)`. The grid
/// is equirectangular: its width spans the full circumference (360°) while its
/// height spans pole-to-pole (180°), so a north–south cell is *shorter* than an
/// east–west one. Treating the grid as isotropic overstates N–S distance ~1.5×.
pub fn cell_km(radius_earth: f64, grid_width: usize, grid_height: usize) -> (f64, f64) {
    const EARTH_RADIUS_KM: f64 = 6371.0;
    let r = radius_earth.max(0.01) * EARTH_RADIUS_KM;
    let x = 2.0 * std::f64::consts::PI * r / (grid_width.max(1) as f64);
    let y = std::f64::consts::PI * r / (grid_height.max(1) as f64);
    (x, y)
}

/// Straight-line surface distance in km for a cell delta `(dx, dy)`, respecting
/// the grid's anisotropy (see [`cell_km`]).
pub fn distance_km(radius_earth: f64, grid_width: usize, grid_height: usize, dx: f64, dy: f64) -> f64 {
    let (xk, yk) = cell_km(radius_earth, grid_width, grid_height);
    ((dx * xk).powi(2) + (dy * yk).powi(2)).sqrt()
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
    fn cells_are_anisotropic_on_the_real_grid() {
        // The model grid is 160×120 equirectangular: an x-cell (2.25°) is longer
        // than a y-cell (1.5°) — the ratio is 2·H/W = 240/160 = 1.5.
        let (xk, yk) = cell_km(1.0, 160, 120);
        assert!(xk > yk);
        assert!((xk / yk - 1.5).abs() < 0.01, "ratio {}", xk / yk);
        // A bigger planet → more km per cell.
        let (xk2, _) = cell_km(2.0, 160, 120);
        assert!(xk2 > xk);
    }

    #[test]
    fn distance_respects_the_axis() {
        // A purely N–S delta is shorter than the same-magnitude E–W delta.
        let ns = distance_km(1.0, 160, 120, 0.0, 4.0);
        let ew = distance_km(1.0, 160, 120, 4.0, 0.0);
        assert!(ns < ew, "N–S {ns} should be < E–W {ew}");
    }

    #[test]
    fn unknown_mode_is_flagged() {
        assert!(mode_recognized("ship") && mode_recognized("boat") && mode_recognized("foot"));
        assert!(!mode_recognized("teleport"));
    }
}
