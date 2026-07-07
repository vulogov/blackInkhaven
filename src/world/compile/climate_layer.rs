//! WORLD-4 Layer 3 — Climate. A deterministic zonal model: astronomy gives the
//! latitudinal insolation profile (tilt-aware) and the star's flux sets the
//! global mean; geology gives the heightmap (elevation lapse + continentality +
//! orographic rain shadows). Output: per-cell temperature / precipitation /
//! biome, aggregated into climate zones, plus the prevailing-wind bands.
//!
//! Fiction-grade, not climate science (§4 non-goal): Hadley/Ferrel/Polar belts,
//! continental drying, windward/leeward rain — accurate enough for plausibility.

use std::collections::VecDeque;

use crate::world::types::astronomy::AstronomyOutput;
use crate::world::types::climate::*;
use crate::world::types::geology::GeologyOutput;
use crate::world::types::world::WorldDefinition;

/// Max land elevation assumed for the normalised heightmap (km) — sets the lapse
/// scale. Earthlike: high ranges ~5 km.
const MAX_LAND_KM: f32 = 5.0;
/// Environmental lapse rate (°C per km).
const LAPSE_C_PER_KM: f32 = 6.5;

pub fn compile_climate(
    def: &WorldDefinition,
    astro: &AstronomyOutput,
    geo: &GeologyOutput,
) -> ClimateOutput {
    let (w, h) = (geo.width, geo.height);

    // Global mean temperature from stellar flux (Earth-calibrated: flux=1 → 15°C).
    let flux = (def.astronomy.star.luminosity_solar
        / def.astronomy.orbit.semi_major_axis_au.powi(2)) as f32;
    let t_mean = -18.0 + 33.0 * flux.max(1e-4).powf(0.25);

    // Renormalise the astronomy annual-mean insolation to [0,1] (equator→1,
    // pole→0) so it becomes a clean latitude temperature factor.
    let (imin, imax) = astro.insolation_bands.iter().fold((f32::MAX, f32::MIN), |(lo, hi), b| {
        (lo.min(b.annual_mean as f32), hi.max(b.annual_mean as f32))
    });
    let ispan = (imax - imin).max(1e-6);

    // Distance (in cells) from each cell to the nearest ocean, for continentality.
    let dist_ocean = distance_to_ocean(geo, w, h);
    let max_dist = dist_ocean.iter().copied().fold(0u16, u16::max).max(1) as f32;

    let mut temperature_c = vec![0.0_f32; w * h];
    let mut precipitation_mm = vec![0.0_f32; w * h];
    let mut biome = vec![Biome::Ocean; w * h];

    for y in 0..h {
        let lat = 90.0 - (y as f32 + 0.5) / h as f32 * 180.0;
        let insol = interp_insolation(astro, lat);
        let lat_factor = ((insol - imin) / ispan).clamp(0.0, 1.0);
        // Equator (factor 1) → mean+11; pole (factor 0) → mean-40.
        let t_lat = t_mean + 11.0 - 51.0 * (1.0 - lat_factor);
        let zonal_p = zonal_precip(lat);
        // Prevailing wind for this band: easterlies in the tropics & poles,
        // westerlies in the mid-latitudes (flipped for a retrograde planet).
        let wind_west = matches!(lat.abs() as u32, 30..=60);
        let prograde = def.astronomy.planet.rotation_direction != "retrograde";
        let wind_dx: i32 = if wind_west == prograde { 1 } else { -1 };

        for x in 0..w {
            let i = y * w + x;
            let elev = geo.heightmap[i];
            if elev <= geo.sea_level {
                biome[i] = Biome::Ocean;
                temperature_c[i] = t_lat; // sea-surface ~ latitude temp
                precipitation_mm[i] = zonal_p;
                continue;
            }
            let elev_km = (elev - geo.sea_level) / (1.0 - geo.sea_level).max(1e-6) * MAX_LAND_KM;
            let t = t_lat - elev_km * LAPSE_C_PER_KM;

            // Continentality: interiors drier.
            let cont = 1.0 - 0.6 * (dist_ocean[i] as f32 / max_dist);
            // Orographic: rising into the wind → wetter; rain shadow → drier.
            let ux = (x as i32 - wind_dx).clamp(0, w as i32 - 1) as usize;
            let upwind = geo.heightmap[y * w + ux];
            let rise = elev - upwind; // >0 windward slope, <0 leeward
            let oro = (1.0 + rise * 6.0).clamp(0.4, 1.8);

            let p = (zonal_p * cont * oro).max(0.0);
            temperature_c[i] = t;
            precipitation_mm[i] = p;
            biome[i] = classify(t, p);
        }
    }

    let zones = aggregate_zones(&biome, &temperature_c, &precipitation_mm);
    let (mut tsum, mut psum, mut land) = (0.0_f64, 0.0_f64, 0u32);
    for i in 0..w * h {
        if biome[i] != Biome::Ocean {
            tsum += temperature_c[i] as f64;
            psum += precipitation_mm[i] as f64;
            land += 1;
        }
    }
    let land_f = land.max(1) as f64;

    ClimateOutput {
        width: w,
        height: h,
        mean_land_temp_c: (tsum / land_f) as f32,
        mean_land_precip_mm: (psum / land_f) as f32,
        zones,
        winds: wind_bands(def),
        biome,
        temperature_c,
        precipitation_mm,
    }
}

/// Linear-interpolate the astronomy annual-mean insolation at a latitude.
fn interp_insolation(astro: &AstronomyOutput, lat: f32) -> f32 {
    let bands = &astro.insolation_bands;
    if bands.is_empty() {
        return 0.5;
    }
    // Bands are sorted by lat_center ascending (-85..85). Astronomy works in
    // f64; cast to f32 at this boundary.
    let (mut prev_lc, mut prev_am) = (bands[0].lat_center_deg as f32, bands[0].annual_mean as f32);
    for b in bands {
        let (lc, am) = (b.lat_center_deg as f32, b.annual_mean as f32);
        if lc >= lat {
            if (lc - prev_lc).abs() < 1e-6 {
                return am;
            }
            let t = (lat - prev_lc) / (lc - prev_lc);
            return prev_am + t * (am - prev_am);
        }
        prev_lc = lc;
        prev_am = am;
    }
    prev_am
}

/// Zonal rainfall belt by latitude (mm/yr): equatorial convergence wet, the ~30°
/// subtropical highs dry, the ~55° storm track wet, the poles dry.
fn zonal_precip(lat: f32) -> f32 {
    let a = lat.abs();
    let g = |center: f32, sigma: f32, amp: f32| {
        let d = (a - center) / sigma;
        amp * (-0.5 * d * d).exp()
    };
    150.0 + g(0.0, 11.0, 2000.0) + g(55.0, 13.0, 900.0) - g(30.0, 10.0, 250.0)
}

fn classify(t: f32, p: f32) -> Biome {
    if t < -7.0 {
        Biome::IceCap
    } else if t < 2.0 {
        Biome::Tundra
    } else if p < 200.0 {
        if t > 18.0 { Biome::HotDesert } else { Biome::ColdDesert }
    } else if p < 500.0 {
        if t > 20.0 { Biome::Savanna } else { Biome::TemperateGrassland }
    } else if t > 23.0 && p > 2000.0 {
        Biome::TropicalRainforest
    } else if t > 20.0 {
        Biome::TropicalSeasonal
    } else if t > 8.0 {
        if p < 800.0 { Biome::Mediterranean } else { Biome::TemperateForest }
    } else {
        Biome::Taiga
    }
}

/// BFS distance (in cells) from every cell to the nearest ocean cell.
fn distance_to_ocean(geo: &GeologyOutput, w: usize, h: usize) -> Vec<u16> {
    let mut dist = vec![u16::MAX; w * h];
    let mut q: VecDeque<usize> = VecDeque::new();
    for i in 0..w * h {
        if geo.heightmap[i] <= geo.sea_level {
            dist[i] = 0;
            q.push_back(i);
        }
    }
    while let Some(i) = q.pop_front() {
        let (x, y) = (i % w, i / w);
        let d = dist[i];
        let step = |nx: usize, ny: usize, dist: &mut Vec<u16>, q: &mut VecDeque<usize>| {
            let ni = ny * w + nx;
            if dist[ni] == u16::MAX {
                dist[ni] = d.saturating_add(1);
                q.push_back(ni);
            }
        };
        if x > 0 {
            step(x - 1, y, &mut dist, &mut q);
        }
        if x + 1 < w {
            step(x + 1, y, &mut dist, &mut q);
        }
        if y > 0 {
            step(x, y - 1, &mut dist, &mut q);
        }
        if y + 1 < h {
            step(x, y + 1, &mut dist, &mut q);
        }
    }
    // Any all-land world: leave the (unreached) interior at a large finite value.
    for d in dist.iter_mut() {
        if *d == u16::MAX {
            *d = (w + h) as u16;
        }
    }
    dist
}

fn aggregate_zones(biome: &[Biome], temp: &[f32], precip: &[f32]) -> Vec<ClimateZone> {
    use std::collections::HashMap;
    let mut acc: HashMap<&'static str, (usize, f32, f32, f32, f32)> = HashMap::new();
    let mut land = 0usize;
    for i in 0..biome.len() {
        if biome[i] == Biome::Ocean {
            continue;
        }
        land += 1;
        let e = acc.entry(biome[i].as_str()).or_insert((0, f32::MAX, f32::MIN, f32::MAX, f32::MIN));
        e.0 += 1;
        e.1 = e.1.min(temp[i]);
        e.2 = e.2.max(temp[i]);
        e.3 = e.3.min(precip[i]);
        e.4 = e.4.max(precip[i]);
    }
    let land = land.max(1) as f32;
    let mut zones: Vec<ClimateZone> = acc
        .into_iter()
        .map(|(biome, (n, tmin, tmax, pmin, pmax))| ClimateZone {
            biome: biome.to_string(),
            area_pct: n as f32 / land * 100.0,
            temp_min_c: tmin,
            temp_max_c: tmax,
            precip_min_mm: pmin,
            precip_max_mm: pmax,
        })
        .collect();
    // Largest area first, with the biome name as a stable tiebreak — the source
    // is a HashMap, so without it two equal-area biomes would order randomly
    // across runs, breaking the pure-function-of-(world, seed) contract (and
    // propagating the nondeterminism into ecology, the map's regions, and the
    // materialized book).
    zones.sort_by(|a, b| {
        b.area_pct
            .partial_cmp(&a.area_pct)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.biome.cmp(&b.biome))
    });
    zones
}

fn wind_bands(def: &WorldDefinition) -> Vec<WindBand> {
    let prograde = def.astronomy.planet.rotation_direction != "retrograde";
    let east = if prograde { "easterly" } else { "westerly" };
    let west = if prograde { "westerly" } else { "easterly" };
    vec![
        WindBand { lat_min: 0.0, lat_max: 30.0, name: "trade winds".into(), direction: east.into() },
        WindBand { lat_min: 30.0, lat_max: 60.0, name: "westerlies".into(), direction: west.into() },
        WindBand { lat_min: 60.0, lat_max: 90.0, name: "polar easterlies".into(), direction: east.into() },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::{compile_astronomy, compile_geology};
    use crate::world::types::world::{GeneratedGeology, GeologyDef, WorldDefinition};

    fn earth_like() -> WorldDefinition {
        let body = r#"{
            name: "T"
            seed: 0x5151
            astronomy: {
                star: { luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0 }
                calendar: { months: 12, month_length_days: 30 }
            }
        }"#;
        let mut def = WorldDefinition::from_hjson(body).unwrap();
        def.geology = Some(GeologyDef {
            generated: Some(GeneratedGeology {
                plates: 7,
                continents: 4,
                mountain_orogeny: "active".into(),
                sea_level: 0.4,
                ..Default::default()
            }),
            dem: None,
        });
        def
    }

    fn compile(def: &WorldDefinition) -> ClimateOutput {
        let astro = compile_astronomy(&def.astronomy);
        let geo = compile_geology(def);
        compile_climate(def, &astro, &geo)
    }

    #[test]
    fn deterministic() {
        let def = earth_like();
        assert_eq!(compile(&def), compile(&def));
    }

    #[test]
    fn earth_like_produces_sane_varied_climate() {
        let c = compile(&earth_like());
        // A sane planetary land-temperature band (the exact value depends on
        // where the random continents land — Earth's own land mean is ~8°C).
        assert!(c.mean_land_temp_c > -25.0 && c.mean_land_temp_c < 35.0, "got {}", c.mean_land_temp_c);
        assert!(c.mean_land_precip_mm > 100.0, "got {}", c.mean_land_precip_mm);
        // A world spanning pole to equator yields a variety of biomes.
        assert!(c.zones.len() >= 3, "got zones {:?}", c.zones.iter().map(|z| z.biome.clone()).collect::<Vec<_>>());
        // Land area shares sum to ~100%.
        let total: f32 = c.zones.iter().map(|z| z.area_pct).sum();
        assert!((total - 100.0).abs() < 0.5, "land area sums to {total}");
    }

    #[test]
    fn poles_are_cold_equator_is_warm() {
        let def = earth_like();
        let astro = compile_astronomy(&def.astronomy);
        let geo = compile_geology(&def);
        let c = compile_climate(&def, &astro, &geo);
        let w = c.width;
        // Average temperature of the top row (near +90°) vs the middle (equator).
        let pole_row: f32 = (0..w).map(|x| c.temperature_c[x]).sum::<f32>() / w as f32;
        let eq_y = c.height / 2;
        let eq_row: f32 = (0..w).map(|x| c.temperature_c[eq_y * w + x]).sum::<f32>() / w as f32;
        assert!(eq_row > pole_row + 20.0, "equator {eq_row} vs pole {pole_row}");
    }

    #[test]
    fn brighter_star_warms_the_world() {
        let mut hot = earth_like();
        hot.astronomy.star.luminosity_solar = 1.6;
        let cold = compile(&earth_like());
        let warm = compile(&hot);
        assert!(warm.mean_land_temp_c > cold.mean_land_temp_c + 3.0);
    }

    #[test]
    fn retrograde_flips_trade_winds() {
        let mut def = earth_like();
        def.astronomy.planet.rotation_direction = "retrograde".into();
        let c = compile(&def);
        let trades = c.winds.iter().find(|w| w.name == "trade winds").unwrap();
        assert_eq!(trades.direction, "westerly");
    }
}
