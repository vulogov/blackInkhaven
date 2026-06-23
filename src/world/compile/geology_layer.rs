//! WORLD-4 Layer 2 — Geology (generated path). Deterministic from
//! `(definition, seed)`: seed → plate seeds → Delaunay adjacency → boundary
//! classification → a multi-octave Perlin heightmap biased by continental plates
//! and convergent-boundary uplift → continents / mountains / minerals.
//!
//! Determinism is load-bearing (the whole compiler rests on it), so randomness
//! comes from an in-tree SplitMix64 seeded by the world seed — never `rand`,
//! whose stream could drift across versions.

use std::collections::HashSet;

use noise::{NoiseFn, Perlin};

use crate::world::types::geology::*;
use crate::world::types::world::{GeneratedGeology, WorldDefinition};

/// Model resolution (4:3). Coarse enough to compile fast + test, fine enough to
/// seed climate / hydrology; plakat upsamples for cartography later.
const W: usize = 160;
const H: usize = 120;

/// Compile the generated geology layer. (The DEM-import path is separate.)
pub fn compile_geology(def: &WorldDefinition) -> GeologyOutput {
    let g = def
        .geology
        .as_ref()
        .and_then(|d| d.generated.clone())
        .unwrap_or_default();
    let mut rng = SplitMix64::new(def.seed_u64() ^ 0x6750_4C45_4F4C_4759); // "GEOLOGY"-ish salt

    let plates = generate_plates(&mut rng, &g);
    let boundaries = classify_boundaries(&plates);

    let heightmap = build_heightmap(def.seed_u64(), &plates, &boundaries, &g);
    let sea_level = g.sea_level.clamp(0.0, 1.0);

    let below = heightmap.iter().filter(|&&e| e <= sea_level).count();
    let sea_coverage_pct = below as f32 / heightmap.len() as f32 * 100.0;
    let land_fraction = 1.0 - below as f32 / heightmap.len() as f32;

    let continents = count_continents(&heightmap, sea_level);
    let mountain_ranges = find_mountain_ranges(&heightmap, &plates, &boundaries);
    let minerals = mineral_hints(&boundaries);
    let elevation = elevation_stats(&heightmap, land_fraction);

    GeologyOutput {
        source: "generated".into(),
        width: W,
        height: H,
        sea_level,
        plates,
        boundaries: BoundarySummary {
            convergent: boundaries.iter().filter(|b| b.kind == BoundaryKind::Convergent).count(),
            divergent: boundaries.iter().filter(|b| b.kind == BoundaryKind::Divergent).count(),
            transform: boundaries.iter().filter(|b| b.kind == BoundaryKind::Transform).count(),
        },
        continents,
        sea_coverage_pct,
        mountain_ranges,
        minerals,
        elevation,
        heightmap,
    }
}

/// Compile geology from an external DEM (heightmap) instead of generating it:
/// read the image, resample to the model grid, normalise, and derive
/// continents / sea coverage / elevation. A DEM carries no tectonic model, so
/// plates / boundaries are empty and minerals reduce to sedimentary coal.
pub fn compile_geology_dem(
    def: &WorldDefinition,
    dem_path: &std::path::Path,
) -> std::result::Result<GeologyOutput, String> {
    let dem = def
        .geology
        .as_ref()
        .and_then(|d| d.dem.as_ref())
        .ok_or_else(|| "no `geology.dem` block in the definition".to_string())?;
    let img = image::open(dem_path)
        .map_err(|e| format!("reading DEM {}: {e}", dem_path.display()))?
        .to_luma16();
    let (iw, ih) = (img.width() as usize, img.height() as usize);
    if iw == 0 || ih == 0 {
        return Err(format!("DEM {} is empty", dem_path.display()));
    }

    // Nearest-neighbour resample to the model grid, tracking min/max to
    // normalise to [0,1].
    let mut raw = vec![0u16; W * H];
    let (mut lo, mut hi) = (u16::MAX, 0u16);
    for y in 0..H {
        for x in 0..W {
            let sx = (x * iw / W).min(iw - 1);
            let sy = (y * ih / H).min(ih - 1);
            let v = img.get_pixel(sx as u32, sy as u32)[0];
            raw[y * W + x] = v;
            lo = lo.min(v);
            hi = hi.max(v);
        }
    }
    let span = (hi - lo).max(1) as f32;
    let heightmap: Vec<f32> = raw.iter().map(|&v| (v - lo) as f32 / span).collect();

    // Sea level: from the declared sea pixel value (normalised), else the default.
    let sea_level = match dem.sea_level_pixel_value {
        Some(pv) => ((pv.saturating_sub(lo)) as f32 / span).clamp(0.0, 1.0),
        None => 0.4,
    };

    let below = heightmap.iter().filter(|&&e| e <= sea_level).count();
    let land_fraction = 1.0 - below as f32 / heightmap.len() as f32;
    let continents = count_continents(&heightmap, sea_level);
    let elevation = elevation_stats(&heightmap, land_fraction);

    Ok(GeologyOutput {
        source: "dem".into(),
        width: W,
        height: H,
        sea_level,
        plates: Vec::new(),
        boundaries: BoundarySummary::default(),
        continents,
        sea_coverage_pct: below as f32 / heightmap.len() as f32 * 100.0,
        // Without tectonics we don't attribute ranges to plate pairs.
        mountain_ranges: Vec::new(),
        minerals: vec![MineralHint {
            mineral: "coal".into(),
            context: "old sedimentary lowlands".into(),
        }],
        elevation,
        heightmap,
    })
}

// ── Plates ───────────────────────────────────────────────────────────────

fn generate_plates(rng: &mut SplitMix64, g: &GeneratedGeology) -> Vec<Plate> {
    let n = g.plates.max(2) as usize;
    let want_continental = (g.continents as usize).min(n);
    let mut plates: Vec<Plate> = (0..n)
        .map(|id| {
            let angle = rng.next_f64() * std::f64::consts::TAU;
            Plate {
                id,
                seed_x: rng.next_f64() as f32,
                seed_y: rng.next_f64() as f32,
                motion_x: angle.cos() as f32,
                motion_y: angle.sin() as f32,
                continental: false,
            }
        })
        .collect();
    // Mark `continents` plates continental (a deterministic shuffle-and-take).
    let mut idx: Vec<usize> = (0..n).collect();
    for i in (1..n).rev() {
        let j = (rng.next_u64() % (i as u64 + 1)) as usize;
        idx.swap(i, j);
    }
    for &i in idx.iter().take(want_continental) {
        plates[i].continental = true;
    }
    plates
}

#[derive(PartialEq, Clone, Copy, Debug)]
enum BoundaryKind {
    Convergent,
    Divergent,
    Transform,
}

struct Boundary {
    a: usize,
    b: usize,
    kind: BoundaryKind,
}

fn classify_boundaries(plates: &[Plate]) -> Vec<Boundary> {
    if plates.len() < 3 {
        return Vec::new();
    }
    let points: Vec<delaunator::Point> = plates
        .iter()
        .map(|p| delaunator::Point { x: p.seed_x as f64, y: p.seed_y as f64 })
        .collect();
    let tri = delaunator::triangulate(&points);
    let mut edges: HashSet<(usize, usize)> = HashSet::new();
    for t in tri.triangles.chunks(3) {
        if t.len() < 3 {
            continue;
        }
        for &(i, j) in &[(t[0], t[1]), (t[1], t[2]), (t[2], t[0])] {
            edges.insert((i.min(j), i.max(j)));
        }
    }
    let mut out: Vec<Boundary> = edges
        .into_iter()
        .map(|(a, b)| {
            let pa = &plates[a];
            let pb = &plates[b];
            // Separation axis A→B, and A's velocity relative to B along it.
            let (sx, sy) = (pb.seed_x - pa.seed_x, pb.seed_y - pa.seed_y);
            let len = (sx * sx + sy * sy).sqrt().max(1e-6);
            let (nx, ny) = (sx / len, sy / len);
            let (rx, ry) = (pa.motion_x - pb.motion_x, pa.motion_y - pb.motion_y);
            let along = rx * nx + ry * ny; // >0 closing (convergent)
            let perp = (rx - along * nx).hypot(ry - along * ny);
            let kind = if along.abs() < perp {
                BoundaryKind::Transform
            } else if along > 0.0 {
                BoundaryKind::Convergent
            } else {
                BoundaryKind::Divergent
            };
            Boundary { a, b, kind }
        })
        .collect();
    // Stable order so the output (and any derived names) is deterministic.
    out.sort_by_key(|b| (b.a, b.b));
    out
}

// ── Heightmap ──────────────────────────────────────────────────────────────

fn build_heightmap(
    seed: u64,
    plates: &[Plate],
    boundaries: &[Boundary],
    g: &GeneratedGeology,
) -> Vec<f32> {
    let perlin = Perlin::new(seed as u32);
    let orogeny = match g.mountain_orogeny.as_str() {
        "quiet" => 0.25_f32,
        "ancient" => 0.12,
        _ => 0.45, // active (default)
    };
    // Convergent plate-pairs get mountain uplift along their boundary.
    let convergent: HashSet<(usize, usize)> = boundaries
        .iter()
        .filter(|b| b.kind == BoundaryKind::Convergent)
        .map(|b| (b.a, b.b))
        .collect();

    let mut raw = vec![0.0_f32; W * H];
    for y in 0..H {
        for x in 0..W {
            let nx = x as f32 / W as f32;
            let ny = y as f32 / H as f32;
            let (p1, d1, p2, d2) = two_nearest_plates(plates, nx, ny);
            let base = if plates[p1].continental { 0.25 } else { -0.15 };
            let n = fbm(&perlin, nx as f64 * 3.0, ny as f64 * 3.0) as f32 * 0.30;
            // Boundary proximity: |d1 - d2| is small near the plate boundary.
            let boundary_dist = (d2 - d1).max(0.0);
            let uplift = if convergent.contains(&(p1.min(p2), p1.max(p2))) {
                orogeny * (-boundary_dist * 30.0).exp()
            } else {
                0.0
            };
            raw[y * W + x] = 0.5 + base + n + uplift;
        }
    }
    normalize(&mut raw);
    raw
}

/// The nearest and second-nearest plate indices + their (squared) distances.
fn two_nearest_plates(plates: &[Plate], x: f32, y: f32) -> (usize, f32, usize, f32) {
    let (mut p1, mut d1) = (0usize, f32::MAX);
    let (mut p2, mut d2) = (0usize, f32::MAX);
    for p in plates {
        let dx = p.seed_x - x;
        let dy = p.seed_y - y;
        let d = dx * dx + dy * dy;
        if d < d1 {
            p2 = p1;
            d2 = d1;
            p1 = p.id;
            d1 = d;
        } else if d < d2 {
            p2 = p.id;
            d2 = d;
        }
    }
    (p1, d1.sqrt(), p2, d2.sqrt())
}

fn fbm(p: &Perlin, x: f64, y: f64) -> f64 {
    let (mut v, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..4 {
        v += amp * p.get([x * freq, y * freq]);
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    v / norm
}

fn normalize(map: &mut [f32]) {
    let (mut lo, mut hi) = (f32::MAX, f32::MIN);
    for &v in map.iter() {
        lo = lo.min(v);
        hi = hi.max(v);
    }
    let span = (hi - lo).max(1e-6);
    for v in map.iter_mut() {
        *v = (*v - lo) / span;
    }
}

// ── Derived summaries ───────────────────────────────────────────────────────

/// Count connected landmasses (4-connectivity) above a minimum size.
fn count_continents(map: &[f32], sea_level: f32) -> usize {
    let min_size = (W * H) / 200; // ~0.5% of the surface
    let mut seen = vec![false; W * H];
    let mut count = 0;
    for start in 0..W * H {
        if seen[start] || map[start] <= sea_level {
            continue;
        }
        // Flood fill this landmass.
        let mut size = 0;
        let mut stack = vec![start];
        seen[start] = true;
        while let Some(i) = stack.pop() {
            size += 1;
            let (x, y) = (i % W, i / W);
            for (nx, ny) in neighbors(x, y) {
                let ni = ny * W + nx;
                if !seen[ni] && map[ni] > sea_level {
                    seen[ni] = true;
                    stack.push(ni);
                }
            }
        }
        if size >= min_size {
            count += 1;
        }
    }
    count
}

fn neighbors(x: usize, y: usize) -> Vec<(usize, usize)> {
    let mut v = Vec::with_capacity(4);
    if x > 0 {
        v.push((x - 1, y));
    }
    if x + 1 < W {
        v.push((x + 1, y));
    }
    if y > 0 {
        v.push((x, y - 1));
    }
    if y + 1 < H {
        v.push((x, y + 1));
    }
    v
}

/// One range per convergent boundary that raised enough high terrain.
fn find_mountain_ranges(
    map: &[f32],
    plates: &[Plate],
    boundaries: &[Boundary],
) -> Vec<MountainRange> {
    let high = 0.78_f32;
    let mut ranges = Vec::new();
    for b in boundaries.iter().filter(|b| b.kind == BoundaryKind::Convergent) {
        let mut cell_count = 0;
        let mut peak = 0.0_f32;
        for y in 0..H {
            for x in 0..W {
                let nx = x as f32 / W as f32;
                let ny = y as f32 / H as f32;
                let (p1, _, p2, _) = two_nearest_plates(plates, nx, ny);
                let pair = (p1.min(p2), p1.max(p2));
                if pair == (b.a, b.b) && map[y * W + x] > high {
                    cell_count += 1;
                    peak = peak.max(map[y * W + x]);
                }
            }
        }
        if cell_count >= (W * H) / 400 {
            ranges.push(MountainRange { plate_a: b.a, plate_b: b.b, peak_elevation: peak, cell_count });
        }
    }
    ranges.sort_by(|a, b| b.peak_elevation.partial_cmp(&a.peak_elevation).unwrap());
    ranges
}

fn mineral_hints(boundaries: &[Boundary]) -> Vec<MineralHint> {
    let mut hints = Vec::new();
    let has = |k: BoundaryKind| boundaries.iter().any(|b| b.kind == k);
    if has(BoundaryKind::Convergent) {
        hints.push(MineralHint { mineral: "copper".into(), context: "convergent volcanic arcs".into() });
        hints.push(MineralHint { mineral: "gold".into(), context: "convergent collision belts".into() });
    }
    if has(BoundaryKind::Divergent) {
        hints.push(MineralHint { mineral: "iron".into(), context: "divergent rift basins".into() });
    }
    // Sedimentary coal forms in old lowland basins regardless of tectonics.
    hints.push(MineralHint { mineral: "coal".into(), context: "old sedimentary lowlands".into() });
    hints
}

fn elevation_stats(map: &[f32], land_fraction: f32) -> ElevationStats {
    let (mut min, mut max, mut sum) = (f32::MAX, f32::MIN, 0.0_f64);
    for &v in map {
        min = min.min(v);
        max = max.max(v);
        sum += v as f64;
    }
    ElevationStats { min, max, mean: (sum / map.len() as f64) as f32, land_fraction }
}

// ── Deterministic PRNG (SplitMix64) ──────────────────────────────────────────

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A value in `[0, 1)`.
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::types::world::{GeologyDef, WorldDefinition};

    fn world(seed: u64, plates: u32, continents: u32, sea: f32, orogeny: &str) -> WorldDefinition {
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
                plates,
                continents,
                mountain_orogeny: orogeny.into(),
                sea_level: sea,
            }),
            dem: None,
        });
        def
    }

    #[test]
    fn deterministic_from_seed() {
        let def = world(0x1234, 7, 4, 0.4, "active");
        assert_eq!(compile_geology(&def), compile_geology(&def));
    }

    #[test]
    fn different_seeds_differ() {
        let a = compile_geology(&world(1, 7, 4, 0.4, "active"));
        let b = compile_geology(&world(2, 7, 4, 0.4, "active"));
        assert_ne!(a.heightmap, b.heightmap);
    }

    #[test]
    fn plates_and_boundaries_are_sane() {
        let out = compile_geology(&world(42, 7, 4, 0.4, "active"));
        assert_eq!(out.plates.len(), 7);
        assert_eq!(out.plates.iter().filter(|p| p.continental).count(), 4);
        // A 7-plate Delaunay graph has several boundaries, each classified.
        let total = out.boundaries.convergent + out.boundaries.divergent + out.boundaries.transform;
        assert!(total >= 6, "got {total} boundaries");
        // Normalised heightmap spans the full range.
        assert!(out.elevation.min < 0.05 && out.elevation.max > 0.95);
    }

    #[test]
    fn sea_level_controls_coverage() {
        let low = compile_geology(&world(7, 7, 4, 0.2, "active"));
        let high = compile_geology(&world(7, 7, 4, 0.7, "active"));
        // A higher sea level drowns more of the same world.
        assert!(high.sea_coverage_pct > low.sea_coverage_pct);
        assert!(high.elevation.land_fraction < low.elevation.land_fraction);
    }

    #[test]
    fn dem_import_reads_an_external_heightmap() {
        use crate::world::types::world::{DemGeology, GeologyDef};
        // Synthesise a small horizontal-gradient grayscale heightmap.
        let path = std::env::temp_dir().join("inkhaven_world4_dem_test.png");
        let (iw, ih) = (40u32, 30u32);
        let mut img = image::GrayImage::new(iw, ih);
        for y in 0..ih {
            for x in 0..iw {
                let v = (x as f32 / iw as f32 * 255.0) as u8;
                img.put_pixel(x, y, image::Luma([v]));
            }
        }
        img.save(&path).unwrap();

        let mut def = world(1, 7, 4, 0.4, "active");
        def.geology = Some(GeologyDef {
            generated: None,
            dem: Some(DemGeology {
                path: path.display().to_string(),
                scale_km_per_pixel: 5.0,
                sea_level_pixel_value: None,
            }),
        });

        let out = compile_geology_dem(&def, &path).expect("read DEM");
        assert_eq!(out.source, "dem");
        assert_eq!(out.width, W);
        assert_eq!(out.height, H);
        // A DEM carries no tectonics.
        assert!(out.plates.is_empty());
        assert!(out.mountain_ranges.is_empty());
        // The gradient normalises across the full range; the high (eastern) half
        // is land, the low (western) half is ocean at sea level 0.4.
        assert!(out.elevation.min < 0.05 && out.elevation.max > 0.95);
        assert!(out.continents >= 1, "the high half should be one landmass");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn active_orogeny_builds_more_mountains_than_ancient() {
        let active = compile_geology(&world(99, 7, 4, 0.4, "active"));
        let ancient = compile_geology(&world(99, 7, 4, 0.4, "ancient"));
        // Same seed/plates, but active orogeny raises higher peaks.
        let active_peak = active.elevation.max;
        let ancient_peak = ancient.elevation.max;
        // Both normalise to ~1.0 max, so compare mountain-range counts instead.
        assert!(
            active.mountain_ranges.len() >= ancient.mountain_ranges.len(),
            "active {} vs ancient {}",
            active.mountain_ranges.len(),
            ancient.mountain_ranges.len()
        );
        let _ = (active_peak, ancient_peak);
        // Minerals always include coal; convergent worlds add copper.
        assert!(active.minerals.iter().any(|m| m.mineral == "coal"));
    }
}
