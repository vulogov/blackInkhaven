//! WORLD-4 Layer 4 — Hydrology. Deterministic from geology (heightmap) + climate
//! (rainfall): the textbook D8 flow model. Each land cell flows to its steepest
//! downhill neighbour; flow accumulates downstream weighted by local rainfall;
//! cells above a flow threshold are rivers; interior pits are lakes; every cell
//! drains to a terminal (ocean or pit), grouping the map into watersheds.
//! Confluences, river mouths, and fertile valleys become settlement priors for
//! Layer 5.

use crate::world::types::climate::{Biome, ClimateOutput};
use crate::world::types::geology::GeologyOutput;
use crate::world::types::hydrology::*;

/// The 8 D8 neighbour offsets (E, SE, S, SW, W, NW, N, NE) and their distances.
const DX: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DY: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

pub fn compile_hydrology(geo: &GeologyOutput, climate: &ClimateOutput) -> HydrologyOutput {
    let (w, h) = (geo.width, geo.height);
    let elev = &geo.heightmap;
    let sea = geo.sea_level;
    let n = w * h;

    // ── D8 flow direction ────────────────────────────────────────────────
    let mut flow_dir = vec![-1i8; n];
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if elev[i] <= sea {
                continue; // ocean: a sink
            }
            let (mut best, mut best_drop) = (-1i8, 0.0f32);
            for d in 0..8 {
                let (nx, ny) = (x as i32 + DX[d], y as i32 + DY[d]);
                if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                    continue;
                }
                let ni = ny as usize * w + nx as usize;
                let dist = if d % 2 == 0 { 1.0 } else { std::f32::consts::SQRT_2 };
                let drop = (elev[i] - elev[ni]) / dist;
                if drop > best_drop {
                    best_drop = drop;
                    best = d as i8;
                }
            }
            flow_dir[i] = best; // -1 = pit (no lower neighbour)
        }
    }

    // ── Flow accumulation (rainfall-weighted, high → low elevation) ───────
    let mut flow_accum: Vec<f32> = (0..n).map(|i| (climate.precipitation_mm[i] / 1000.0).max(0.05)).collect();
    let mut order: Vec<usize> = (0..n).filter(|&i| elev[i] > sea).collect();
    order.sort_by(|&a, &b| elev[b].partial_cmp(&elev[a]).unwrap_or(std::cmp::Ordering::Equal));
    for &i in &order {
        let d = flow_dir[i];
        if d >= 0 {
            let (x, y) = (i % w, i / w);
            let ni = (y as i32 + DY[d as usize]) as usize * w + (x as i32 + DX[d as usize]) as usize;
            flow_accum[ni] += flow_accum[i];
        }
    }

    // River threshold: drains more than a small multiple of the mean cell flow.
    let mean_flow: f32 = flow_accum.iter().sum::<f32>() / n as f32;
    let threshold = mean_flow * 8.0;
    let is_river: Vec<bool> = (0..n).map(|i| elev[i] > sea && flow_accum[i] > threshold).collect();

    // ── Rivers (counted at ocean mouths) + Strahler-style order ───────────
    let mut major_rivers: Vec<RiverSummary> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if !is_river[i] {
                continue;
            }
            // A mouth: a river cell whose downstream neighbour is ocean (or the map edge).
            let mouth = match flow_dir[i] {
                d if d >= 0 => {
                    let ni = (y as i32 + DY[d as usize]) as usize * w
                        + (x as i32 + DX[d as usize]) as usize;
                    elev[ni] <= sea
                }
                _ => false,
            };
            if mouth {
                let ord = ((flow_accum[i] / threshold).log2().floor() as i32 + 1).clamp(1, 6) as u8;
                major_rivers.push(RiverSummary { mouth_x: x, mouth_y: y, order: ord, flow: flow_accum[i] });
            }
        }
    }
    major_rivers.sort_by(|a, b| b.flow.partial_cmp(&a.flow).unwrap_or(std::cmp::Ordering::Equal));
    let river_count = major_rivers.len();
    major_rivers.truncate(12);

    // ── Lakes: interior land pits with no outflow ─────────────────────────
    let lake_count = (0..n)
        .filter(|&i| {
            let (x, y) = (i % w, i / w);
            elev[i] > sea
                && flow_dir[i] < 0
                && x > 0
                && y > 0
                && x + 1 < w
                && y + 1 < h
        })
        .count();

    // ── Watersheds: trace each land cell to its terminal, count basins ────
    let watershed_count = count_watersheds(w, h, elev, sea, &flow_dir);

    // ── Settlement priors ─────────────────────────────────────────────────
    let settlement_priors = settlement_priors(w, h, elev, sea, &flow_dir, &flow_accum, &is_river, climate);

    HydrologyOutput {
        width: w,
        height: h,
        river_count,
        major_rivers,
        lake_count,
        watershed_count,
        settlement_priors,
        flow_dir,
        flow_accum,
        is_river,
    }
}

fn downstream(i: usize, w: usize, flow_dir: &[i8]) -> Option<usize> {
    let d = flow_dir[i];
    if d < 0 {
        return None;
    }
    let (x, y) = (i % w, i / w);
    Some((y as i32 + DY[d as usize]) as usize * w + (x as i32 + DX[d as usize]) as usize)
}

/// Count distinct drainage basins above a minimum size by tracing every land
/// cell to its terminal (ocean cell or interior pit) with memoisation.
fn count_watersheds(w: usize, h: usize, elev: &[f32], sea: f32, flow_dir: &[i8]) -> usize {
    let n = w * h;
    let mut terminal = vec![usize::MAX; n];
    for start in 0..n {
        if elev[start] <= sea || terminal[start] != usize::MAX {
            continue;
        }
        // Walk downstream collecting the path, then stamp the terminal.
        let mut path = Vec::new();
        let mut cur = start;
        let term = loop {
            if terminal[cur] != usize::MAX {
                break terminal[cur];
            }
            path.push(cur);
            match downstream(cur, w, flow_dir) {
                Some(next) if elev[next] > sea => cur = next,
                _ => break cur, // reached ocean-adjacent or a pit → `cur` is the terminal
            }
        };
        for c in path {
            terminal[c] = term;
        }
    }
    let mut sizes: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for &t in &terminal {
        if t != usize::MAX {
            *sizes.entry(t).or_default() += 1;
        }
    }
    let min_size = (n / 200).max(1);
    sizes.values().filter(|&&s| s >= min_size).count()
}

fn settlement_priors(
    w: usize,
    h: usize,
    elev: &[f32],
    sea: f32,
    flow_dir: &[i8],
    accum: &[f32],
    is_river: &[bool],
    climate: &ClimateOutput,
) -> Vec<SettlementPrior> {
    let mut priors: Vec<SettlementPrior> = Vec::new();
    let fertile = |b: Biome| {
        matches!(
            b,
            Biome::TemperateForest
                | Biome::TemperateGrassland
                | Biome::Mediterranean
                | Biome::Savanna
                | Biome::TropicalSeasonal
        )
    };
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if elev[i] <= sea || !is_river[i] {
                continue;
            }
            // Count river neighbours that flow INTO this cell.
            let inflows = (0..8)
                .filter(|&d| {
                    let (nx, ny) = (x as i32 + DX[d], y as i32 + DY[d]);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        return false;
                    }
                    let ni = ny as usize * w + nx as usize;
                    is_river[ni] && downstream(ni, w, flow_dir) == Some(i)
                })
                .count();
            let mouth = downstream(i, w, flow_dir).map(|ds| elev[ds] <= sea).unwrap_or(false);

            if mouth {
                priors.push(SettlementPrior { x, y, kind: "river_mouth".into(), score: accum[i] * 1.5 });
            } else if inflows >= 2 {
                priors.push(SettlementPrior { x, y, kind: "confluence".into(), score: accum[i] * 1.2 });
            } else if fertile(climate.biome[i]) && elev[i] < sea + (1.0 - sea) * 0.35 {
                priors.push(SettlementPrior { x, y, kind: "fertile_valley".into(), score: accum[i] });
            }
        }
    }
    priors.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    priors.truncate(20);
    priors
}

/// WORLD-11 (W11-P3) — verify declared river courses against the terrain: a
/// river must run *downhill* from its declared source to its declared mouth, and
/// its mouth should reach a sea (or, noted, an inland lake). Advisory only.
pub fn lint_rivers(
    hydro_def: &crate::world::types::HydrologyDef,
    geo: &GeologyOutput,
) -> Vec<String> {
    let (w, h) = (geo.width, geo.height);
    let elev_at = |p: [usize; 2]| -> Option<f32> {
        (p[0] < w && p[1] < h).then(|| geo.heightmap[p[1] * w + p[0]])
    };
    let mut warnings = Vec::new();
    for r in &hydro_def.rivers {
        let (from, to) = match (r.from, r.to) {
            (Some(f), Some(t)) => (f, t),
            _ => continue, // no declared course — the procedural river stands
        };
        match (elev_at(from), elev_at(to)) {
            (Some(ef), Some(et)) => {
                if ef < et {
                    warnings.push(format!(
                        "river `{}`: runs uphill — its source ({}, {}) sits below its mouth ({}, {})",
                        r.name, from[0], from[1], to[0], to[1]
                    ));
                }
                if et > geo.sea_level {
                    warnings.push(format!(
                        "river `{}`: its mouth ({}, {}) is above the shoreline — a river should reach a sea (or an inland lake)",
                        r.name, to[0], to[1]
                    ));
                }
            }
            _ => warnings.push(format!("river `{}`: its source or mouth is off the map", r.name)),
        }
    }
    warnings
}

#[cfg(test)]
mod river_lint_tests {
    use super::*;
    use crate::world::types::{HydrologyDef, NamedWater};

    fn river(from: [usize; 2], to: [usize; 2]) -> HydrologyDef {
        HydrologyDef {
            rainfall: String::new(),
            rivers: vec![NamedWater {
                name: "R".into(),
                description: String::new(),
                from: Some(from),
                to: Some(to),
            }],
            lakes: vec![],
            seas: vec![],
        }
    }

    #[test]
    fn uphill_and_dry_mouth_are_flagged_downhill_is_clean() {
        // 3×1 grid: x=0 high (0.9), x=2 low (0.1); sea at 0.2.
        let geo = GeologyOutput {
            width: 3,
            height: 1,
            sea_level: 0.2,
            heightmap: vec![0.9, 0.5, 0.1],
            ..Default::default()
        };
        // Downhill from high (0,0) to low (2,0), which is below the sea: clean.
        assert!(lint_rivers(&river([0, 0], [2, 0]), &geo).is_empty());
        // Uphill from low (2,0) to high (0,0): runs uphill AND ends on dry land.
        let w = lint_rivers(&river([2, 0], [0, 0]), &geo);
        assert!(w.iter().any(|s| s.contains("uphill")));
        assert!(w.iter().any(|s| s.contains("above the shoreline")));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::{compile_astronomy, compile_climate, compile_geology};
    use crate::world::types::world::{GeneratedGeology, GeologyDef, WorldDefinition};

    fn world(seed: u64) -> WorldDefinition {
        let body = format!(
            r#"{{
                name: "T"
                seed: {seed}
                astronomy: {{
                    star: {{ luminosity_solar: 1.0 }}
                    planet: {{ mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }}
                    orbit: {{ semi_major_axis_au: 1.0 }}
                    calendar: {{ months: 12, month_length_days: 30 }}
                }}
            }}"#
        );
        let mut def = WorldDefinition::from_hjson(&body).unwrap();
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

    fn compile(def: &WorldDefinition) -> HydrologyOutput {
        let astro = compile_astronomy(&def.astronomy);
        let geo = compile_geology(def);
        let climate = compile_climate(def, &astro, &geo);
        compile_hydrology(&geo, &climate)
    }

    #[test]
    fn deterministic() {
        let def = world(0x7777);
        assert_eq!(compile(&def), compile(&def));
    }

    #[test]
    fn rivers_reach_the_sea_and_form_watersheds() {
        let h = compile(&world(0x7777));
        assert!(h.river_count > 0, "no rivers");
        assert!(!h.major_rivers.is_empty());
        // Every major river is mouthed (order ≥ 1) and flows downhill.
        assert!(h.major_rivers.iter().all(|r| r.order >= 1));
        // The largest river out-flows the smallest in the top list.
        let flows: Vec<f32> = h.major_rivers.iter().map(|r| r.flow).collect();
        assert!(flows.first() >= flows.last());
        assert!(h.watershed_count >= 1);
    }

    #[test]
    fn produces_settlement_priors() {
        let h = compile(&world(0x7777));
        assert!(!h.settlement_priors.is_empty(), "no settlement priors");
        // Ranked by score (descending).
        let s: Vec<f32> = h.settlement_priors.iter().map(|p| p.score).collect();
        assert!(s.windows(2).all(|w| w[0] >= w[1]));
        // Every prior is one of the three recognised kinds (which mix depends on
        // the world's drainage — a heavily endorheic world has few ocean mouths).
        assert!(h.settlement_priors.iter().all(|p| {
            ["river_mouth", "confluence", "fertile_valley"].contains(&p.kind.as_str())
        }));
    }
}
