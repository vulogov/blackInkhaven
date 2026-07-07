//! WORLD-4 Branch A — the **map compiler** (P3.2). Inkhaven's five deterministic
//! layers (astronomy → geology → climate → hydrology → demographics) already know
//! the shape of the world: where the mountains rose, where the rivers reach the
//! sea, which biome covers which band, where the people settled. This module
//! turns that knowledge into a [plakat](https://crates.io/crates/plakat) **MapSpec
//! v2** and hands it to the `plakat` binary to render — `plakat map --map-spec`
//! loads our spec and skips its own LLM entirely, so the cartography stays a pure
//! function of our compiled layers and the world seed.
//!
//! The flow is: [`build_map_spec`] (pure, from the compiled outputs) →
//! [`render`] (writes the spec, runs `plakat`, collects the PNG + GeoJSON) →
//! [`parse_landmark_coords`] (pure, reads plakat's resolved landmark positions
//! back out of the GeoJSON so the caller can refine each Place's coordinates).
//!
//! Everything here is deterministic and dependency-free at the type level: the
//! spec is emitted as a `serde_json::Value` (we mirror plakat's schema by shape,
//! not by importing its structs), and the GeoJSON readback is plain `serde_json`.
//! The only impurity is the subprocess in [`render`]; the map geometry it produces
//! is itself seeded and reproducible.

use crate::world::compile::polities_layer::PolitiesOutput;
use crate::world::compile::trade_layer::TradeOutput;
use crate::world::proposals::PlaceLink;
use crate::world::types::{ClimateOutput, DemographicsOutput, GeologyOutput, HydrologyOutput};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// The MapSpec schema version this module emits (plakat 1.10's `SPEC_VERSION`).
const SPEC_VERSION: u32 = 2;

/// Hard caps so a dense world doesn't emit a thousand landmarks / ranges. The map
/// stays legible and the spec stays small.
const MAX_LANDMARKS: usize = 48;
const MAX_RANGES: usize = 6;
const MAX_REGIONS: usize = 6;
const MAX_RIVERS: usize = 8;

// ── plakat detection ─────────────────────────────────────────────────────────

/// Is the `plakat` binary on `PATH`, and what version? Returns the trimmed first
/// line of `plakat --version` (e.g. `"plakat 1.10.0"`), or `None` if it isn't
/// installed or doesn't run. Never errors — map rendering is an optional feature.
pub fn detect() -> Option<String> {
    let out = std::process::Command::new("plakat").arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    s.lines().next().map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

// ── MapSpec emission ─────────────────────────────────────────────────────────

/// Build a plakat MapSpec v2 from the compiled world layers. Pure: same inputs →
/// byte-identical spec. `places` (the accepted Place ↔ World links) let the
/// emitted landmarks carry stable `place_<uuid>` ids so [`parse_landmark_coords`]
/// can map plakat's resolved positions back onto the right Place.
/// A coordinate-bearing author-declared landmark (from `geography.landmarks`),
/// pre-projected to a grid cell for the map. Unlike a Place it does not round-trip
/// back to a node — it is drawn where the author placed it.
#[derive(Debug, Clone, PartialEq)]
pub struct DeclaredLandmark {
    pub name: String,
    pub kind: String,
    pub x: usize,
    pub y: usize,
}

#[allow(clippy::too_many_arguments)]
pub fn build_map_spec(
    name: &str,
    geo: &GeologyOutput,
    climate: &ClimateOutput,
    hydro: &HydrologyOutput,
    demo: &DemographicsOutput,
    places: &[PlaceLink],
    declared: &[DeclaredLandmark],
    pol: &PolitiesOutput,
    trade: &TradeOutput,
) -> Value {
    // Landmarks first, so roads can reference only hubs that were actually
    // emitted (a dense world past the landmark cap drops some hubs — H7).
    let landmarks = landmark_features(geo, demo, places, declared, pol);
    let emitted_hubs: std::collections::HashSet<String> = landmarks
        .iter()
        .filter_map(|l| l.get("id").and_then(|v| v.as_str()))
        .filter(|id| id.starts_with("hub_"))
        .map(str::to_string)
        .collect();
    json!({
        "version": SPEC_VERSION,
        "name": name,
        "scale_tier": 0,
        "tile_grid": { "cols": 1, "rows": 1 },
        "world_extent_km": null,
        "climate": climate_label(climate),
        "era": null,
        "language": null,
        "terrain": {
            "dominant_elevation": dominant_elevation(geo),
            "mountain_ranges": mountain_features(geo),
            "plateaus": [],
            "rift_valleys": [],
        },
        "water": {
            "seas": [],
            "rivers": river_features(geo, hydro),
            "lakes": [],
        },
        "regions": region_features(climate),
        "landmarks": landmarks,
        "infrastructure": { "roads": road_features(trade, &emitted_hubs), "walls": [], "bridges": [] },
        "bund_hooks": null,
    })
}

/// Trade routes → plakat `RoadSpec`s connecting realm-capital hubs (`hub_<i>`). A
/// sea route is a `sea-lane`; a land route a `road`. A route is dropped when
/// either capital's hub was not emitted (the landmark cap was hit), so no road
/// points at a landmark that isn't on the map.
fn road_features(trade: &TradeOutput, emitted_hubs: &std::collections::HashSet<String>) -> Vec<Value> {
    trade
        .routes
        .iter()
        .enumerate()
        .filter_map(|(i, r)| {
            let (from, to) = (format!("hub_{}", r.from), format!("hub_{}", r.to));
            if !emitted_hubs.contains(&from) || !emitted_hubs.contains(&to) {
                return None;
            }
            Some(json!({
                "id": format!("trade_{i}"),
                "from": from,
                "to": to,
                "kind": if r.mode == "sea" { "sea-lane" } else { "road" },
            }))
        })
        .collect()
}

/// One-word climate hint from the mean land temperature.
fn climate_label(climate: &ClimateOutput) -> &'static str {
    match climate.mean_land_temp_c {
        t if t < -2.0 => "polar",
        t if t < 8.0 => "cold",
        t if t < 18.0 => "temperate",
        t if t < 26.0 => "warm",
        _ => "tropical",
    }
}

/// Dominant elevation descriptor from the land fraction + mean elevation.
fn dominant_elevation(geo: &GeologyOutput) -> &'static str {
    let mean = geo.elevation.mean;
    let sea = geo.sea_level;
    if mean <= sea {
        "lowland"
    } else {
        // How high does the land sit above sea level, on the 0..1 scale?
        let relief = (mean - sea) / (geo.elevation.max - sea).max(1e-3);
        if relief > 0.45 {
            "highland"
        } else if relief > 0.2 {
            "rolling"
        } else {
            "lowland"
        }
    }
}

/// A small cluster of high cells on the heightfield — one emitted mountain range.
struct RangeCluster {
    sum_x: f64,
    sum_y: f64,
    count: usize,
    min_x: usize,
    max_x: usize,
    min_y: usize,
    max_y: usize,
    peak: f32,
}

impl RangeCluster {
    fn new(x: usize, y: usize, e: f32) -> Self {
        RangeCluster {
            sum_x: x as f64,
            sum_y: y as f64,
            count: 1,
            min_x: x,
            max_x: x,
            min_y: y,
            max_y: y,
            peak: e,
        }
    }
    fn centroid(&self) -> (f64, f64) {
        (self.sum_x / self.count as f64, self.sum_y / self.count as f64)
    }
    fn add(&mut self, x: usize, y: usize, e: f32) {
        self.sum_x += x as f64;
        self.sum_y += y as f64;
        self.count += 1;
        self.min_x = self.min_x.min(x);
        self.max_x = self.max_x.max(x);
        self.min_y = self.min_y.min(y);
        self.max_y = self.max_y.max(y);
        self.peak = self.peak.max(e);
    }
}

/// Derive mountain-range features by clustering the highest land cells of the
/// heightfield. The geology summary names how many ranges exist but not where;
/// the heightfield knows where the high ground is, so we read positions from it.
/// Deterministic: cells are visited in elevation-descending, then raster order.
fn mountain_features(geo: &GeologyOutput) -> Vec<Value> {
    let (w, h) = (geo.width, geo.height);
    let hm = &geo.heightmap;
    if w == 0 || h == 0 || hm.len() != w * h {
        return Vec::new();
    }
    let sea = geo.sea_level;
    let max = geo.elevation.max;
    if max <= sea {
        return Vec::new();
    }
    // High ground: cells in the top slice of the land elevation range.
    let threshold = sea + (max - sea) * 0.7;
    let mut high: Vec<(usize, usize, f32)> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let e = hm[y * w + x];
            if e >= threshold {
                high.push((x, y, e));
            }
        }
    }
    if high.is_empty() {
        return Vec::new();
    }
    // Visit highest first (ties broken by the raster order already in `high`),
    // greedily assigning each to the nearest existing cluster within a radius
    // scaled to the grid, else opening a new one.
    high.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap_or(std::cmp::Ordering::Equal));
    let radius = (w.max(h) as f64) * 0.12;
    let cap = geo.mountain_ranges.len().clamp(1, MAX_RANGES);
    let mut clusters: Vec<RangeCluster> = Vec::new();
    for (x, y, e) in high {
        let mut best: Option<(usize, f64)> = None;
        for (i, c) in clusters.iter().enumerate() {
            let (cx, cy) = c.centroid();
            let d = ((cx - x as f64).powi(2) + (cy - y as f64).powi(2)).sqrt();
            if d <= radius && best.map(|(_, bd)| d < bd).unwrap_or(true) {
                best = Some((i, d));
            }
        }
        match best {
            Some((i, _)) => clusters[i].add(x, y, e),
            None if clusters.len() < cap => clusters.push(RangeCluster::new(x, y, e)),
            None => {
                // At the cap: fold into whichever cluster is nearest regardless.
                if let Some((i, _)) = clusters
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        let (cx, cy) = c.centroid();
                        (i, ((cx - x as f64).powi(2) + (cy - y as f64).powi(2)).sqrt())
                    })
                    .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
                {
                    clusters[i].add(x, y, e);
                }
            }
        }
    }
    // Largest (most cells) first, so the dominant ranges lead the list.
    clusters.sort_by(|a, b| b.count.cmp(&a.count));
    clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let (cx, cy) = c.centroid();
            let span_x = (c.max_x - c.min_x) as f64;
            let span_y = (c.max_y - c.min_y) as f64;
            json!({
                "id": format!("range_{i}"),
                "anchor": canvas_anchor(cx, cy, w, h),
                "orientation": orientation(span_x, span_y),
                "length_fraction": (span_x.max(span_y) / w.max(h) as f64).clamp(0.05, 1.0),
                "height": height_bucket((c.peak - sea) / (max - sea).max(1e-3)),
            })
        })
        .collect()
}

/// Cardinal-ish orientation from a bounding box's aspect.
fn orientation(span_x: f64, span_y: f64) -> &'static str {
    let ratio = (span_x + 1.0) / (span_y + 1.0);
    if ratio > 1.6 {
        "east-west"
    } else if ratio < 0.625 {
        "north-south"
    } else {
        "northeast"
    }
}

/// Relief fraction (0..1 above sea) → plakat height bucket.
fn height_bucket(relief: f32) -> &'static str {
    if relief > 0.85 {
        "extreme"
    } else if relief > 0.6 {
        "high"
    } else if relief > 0.3 {
        "moderate"
    } else {
        "low"
    }
}

/// River features: each major river's mouth comes straight from the hydrology
/// layer; the source is traced upstream along the D8 flow field (the neighbour
/// that drains into the current cell with the most accumulated flow), so the
/// emitted river runs the real watercourse rather than a guessed straight line.
fn river_features(geo: &GeologyOutput, hydro: &HydrologyOutput) -> Vec<Value> {
    let (w, h) = (hydro.width, hydro.height);
    if w == 0 || h == 0 {
        return Vec::new();
    }
    hydro
        .major_rivers
        .iter()
        .take(MAX_RIVERS)
        .enumerate()
        .map(|(i, r)| {
            let (sx, sy) = trace_source(r.mouth_x, r.mouth_y, w, h, &hydro.flow_dir, &hydro.flow_accum);
            json!({
                "id": format!("river_{i}"),
                "source": canvas_anchor(sx as f64, sy as f64, w, h),
                "mouth": canvas_anchor(r.mouth_x as f64, r.mouth_y as f64, w, h),
                "tributaries": [],
                "navigable": r.order >= 4 || r.flow >= median_flow(geo, hydro),
            })
        })
        .collect()
}

/// A flow threshold (the largest river's flow, halved) above which we call a
/// river navigable. Cheap heuristic; only affects plakat's styling.
fn median_flow(_geo: &GeologyOutput, hydro: &HydrologyOutput) -> f32 {
    hydro.major_rivers.iter().map(|r| r.flow).fold(0.0f32, f32::max) * 0.5
}

/// The 8 D8 neighbour offsets, matching `compile_hydrology`'s convention
/// (E, SE, S, SW, W, NW, N, NE).
const DX: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DY: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

/// Walk upstream from a river mouth to a headwater: at each step move to the
/// neighbour whose flow drains *into* the current cell and carries the most
/// accumulated flow. Bounded so a pathological field can't loop forever.
fn trace_source(
    mouth_x: usize,
    mouth_y: usize,
    w: usize,
    h: usize,
    flow_dir: &[i8],
    flow_accum: &[f32],
) -> (usize, usize) {
    let (mut x, mut y) = (mouth_x.min(w.saturating_sub(1)), mouth_y.min(h.saturating_sub(1)));
    let steps_cap = (w + h) * 2;
    for _ in 0..steps_cap {
        let mut best: Option<(usize, usize, f32)> = None;
        for d in 0..8 {
            let nx = x as i32 + DX[d];
            let ny = y as i32 + DY[d];
            if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                continue;
            }
            let (nx, ny) = (nx as usize, ny as usize);
            let ni = ny * w + nx;
            // Does this neighbour flow into `cur`? Its D8 direction must point back
            // at us: neighbour + its offset == current cell. (Bounds-guarded — the
            // sibling flow_accum read below already is; keep the two symmetric.)
            let nd = flow_dir.get(ni).copied().unwrap_or(-1);
            if !(0..8).contains(&nd) {
                continue;
            }
            let nd = nd as usize;
            let bx = nx as i32 + DX[nd];
            let by = ny as i32 + DY[nd];
            if bx == x as i32 && by == y as i32 {
                let acc = flow_accum.get(ni).copied().unwrap_or(0.0);
                if best.map(|(_, _, ba)| acc > ba).unwrap_or(true) {
                    best = Some((nx, ny, acc));
                }
            }
        }
        match best {
            Some((nx, ny, _)) => {
                x = nx;
                y = ny;
            }
            None => break, // headwater: nothing drains into us.
        }
    }
    (x, y)
}

/// Region features: the dominant climate zones become biome regions. The zone
/// summary carries no position, so we read each biome's centroid out of the
/// per-cell biome grid — the region anchors where that biome actually sits.
fn region_features(climate: &ClimateOutput) -> Vec<Value> {
    let (w, h) = (climate.width, climate.height);
    if w == 0 || h == 0 || climate.biome.len() != w * h {
        return Vec::new();
    }
    climate
        .zones
        .iter()
        .filter(|z| z.area_pct >= 3.0)
        .take(MAX_REGIONS)
        .enumerate()
        .filter_map(|(i, z)| {
            let (cx, cy, n) = biome_centroid(climate, &z.biome);
            if n == 0 {
                return None;
            }
            Some(json!({
                "id": format!("region_{i}"),
                "biome": z.biome,
                "anchor": canvas_anchor(cx, cy, w, h),
                "coverage": (z.area_pct / 100.0).clamp(0.0, 1.0),
            }))
        })
        .collect()
}

/// Centroid (and cell count) of every grid cell whose biome label matches.
fn biome_centroid(climate: &ClimateOutput, biome: &str) -> (f64, f64, usize) {
    let (w, h) = (climate.width, climate.height);
    let (mut sx, mut sy, mut n) = (0.0f64, 0.0f64, 0usize);
    for y in 0..h {
        for x in 0..w {
            if climate.biome[y * w + x].as_str() == biome {
                sx += x as f64;
                sy += y as f64;
                n += 1;
            }
        }
    }
    if n == 0 {
        (0.0, 0.0, 0)
    } else {
        (sx / n as f64, sy / n as f64, n)
    }
}

/// Landmark features. Every coordinate-bearing Place link is emitted first as a
/// labeled `place_<uuid>` marker — whether or not a compiler settlement sits on
/// its cell — so a Place positioned by hand (`realworld place set-coords`) draws
/// and round-trips just like a compiler-born one. The remaining budget is filled
/// with the largest settlements not already covered by a link, as anonymous
/// landmarks. A coastal city becomes a `port`.
fn landmark_features(
    geo: &GeologyOutput,
    demo: &DemographicsOutput,
    places: &[PlaceLink],
    declared: &[DeclaredLandmark],
    pol: &PolitiesOutput,
) -> Vec<Value> {
    let (w, h) = (geo.width, geo.height);
    let mut out: Vec<Value> = Vec::new();
    let mut used: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

    // (−1) Realm capitals as trade hubs (`hub_<i>`), labeled by realm, so the
    //      trade roads have stable endpoints to connect. Emitted first so the
    //      capital cell is owned by its hub.
    for (i, p) in pol.polities.iter().enumerate() {
        let (x, y) = p.capital_pos;
        if x >= w || y >= h || out.len() >= MAX_LANDMARKS {
            continue;
        }
        if !used.insert((x, y)) {
            continue;
        }
        out.push(json!({
            "id": format!("hub_{i}"),
            "name": p.name,
            "kind": if is_coastal(geo, x, y) { "port" } else { "city" },
            "anchor": canvas_anchor(x as f64, y as f64, w, h),
            "size": size_hint(p.population),
        }));
    }

    // (0) Author-declared landmarks with a position always draw, labeled by name,
    //     with a stable `decl_<slug>` id (they don't round-trip back to a node).
    for d in declared {
        if d.x >= w || d.y >= h || out.len() >= MAX_LANDMARKS {
            continue;
        }
        if !used.insert((d.x, d.y)) {
            continue;
        }
        let kind = if d.kind.trim().is_empty() { "city" } else { d.kind.trim() };
        out.push(json!({
            "id": format!("decl_{}", slugify(&d.name)),
            "name": d.name,
            "kind": kind,
            "anchor": canvas_anchor(d.x as f64, d.y as f64, w, h),
            "size": "medium",
        }));
    }

    // (1) One guaranteed marker per Place link — headline (most populous) first so
    //     the cap keeps the biggest places; author-positioned Places (population 0)
    //     still appear, only after the populous ones if the cap is tight.
    let mut links: Vec<&PlaceLink> = places.iter().filter(|p| p.x < w && p.y < h).collect();
    links.sort_by(|a, b| b.population.cmp(&a.population));
    for p in links {
        if out.len() >= MAX_LANDMARKS {
            break;
        }
        if !used.insert((p.x, p.y)) {
            continue; // one marker per cell
        }
        out.push(json!({
            "id": format!("place_{}", p.place_id),
            "name": p.name,
            "kind": landmark_kind_for(p.population, is_coastal(geo, p.x, p.y)),
            "anchor": canvas_anchor(p.x as f64, p.y as f64, w, h),
            "size": size_hint(p.population.max(1)),
        }));
    }

    // (2) Fill the rest with the largest settlements no link already covers.
    let mut settlements: Vec<&crate::world::types::Settlement> = demo.settlements.iter().collect();
    settlements.sort_by(|a, b| b.population.cmp(&a.population));
    let mut anon = 0usize;
    for s in settlements {
        if out.len() >= MAX_LANDMARKS {
            break;
        }
        if !used.insert((s.x, s.y)) {
            continue; // a Place link already owns this cell
        }
        let kind = if s.class == "city" && is_coastal(geo, s.x, s.y) {
            "port"
        } else {
            landmark_kind(&s.class)
        };
        out.push(json!({
            "id": format!("settlement_{anon}"),
            "name": format!("{} {}", title(&s.class), anon + 1),
            "kind": kind,
            "anchor": canvas_anchor(s.x as f64, s.y as f64, w, h),
            "size": size_hint(s.population),
        }));
        anon += 1;
    }
    out
}

/// A stable lowercase slug for a declared landmark id (`decl_<slug>`).
fn slugify(name: &str) -> String {
    crate::world::proposals::stable_slug(name, '_')
}

/// Map a demographics size class to a plakat landmark kind.
fn landmark_kind(class: &str) -> &'static str {
    match class {
        "city" => "city",
        "town" => "town",
        _ => "village",
    }
}

/// A plakat landmark kind for a Place link, which carries a population (0 when
/// the Place was positioned by hand) but no size class. A populous coastal Place
/// becomes a `port`; a Place with no population is drawn as a `town` marker.
fn landmark_kind_for(pop: u64, coastal: bool) -> &'static str {
    match pop {
        p if p >= 100_000 => {
            if coastal {
                "port"
            } else {
                "city"
            }
        }
        p if p >= 10_000 => "town",
        1..=9_999 => "village",
        _ => "town", // hand-positioned, no demographics — a plain visible marker
    }
}

/// A landmark size hint plakat understands (`small`/`medium`/`large`).
fn size_hint(pop: u64) -> &'static str {
    if pop >= 100_000 {
        "large"
    } else if pop >= 10_000 {
        "medium"
    } else {
        "small"
    }
}

/// Is the cell on the coast — land with at least one sea neighbour?
fn is_coastal(geo: &GeologyOutput, x: usize, y: usize) -> bool {
    let (w, h) = (geo.width, geo.height);
    if x >= w || y >= h || geo.heightmap.len() != w * h {
        return false;
    }
    let sea = geo.sea_level;
    if geo.heightmap[y * w + x] <= sea {
        return false; // already water
    }
    for d in 0..8 {
        let nx = x as i32 + DX[d];
        let ny = y as i32 + DY[d];
        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
            continue;
        }
        if geo.heightmap[ny as usize * w + nx as usize] <= sea {
            return true;
        }
    }
    false
}

/// Title-case a lowercase class word ("city" → "City").
fn title(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

/// A plakat `Canvas` anchor in normalized 0..1 space. The grid's row 0 is north
/// (D8 `DY+` runs south), which matches plakat's top-down canvas y — so the same
/// fraction round-trips through the north-up GeoJSON readback (see
/// [`parse_landmark_coords`]).
fn canvas_anchor(x: f64, y: f64, w: usize, h: usize) -> Value {
    let fx = if w > 1 { (x / (w - 1) as f64).clamp(0.0, 1.0) } else { 0.5 };
    let fy = if h > 1 { (y / (h - 1) as f64).clamp(0.0, 1.0) } else { 0.5 };
    json!({ "kind": "canvas", "x": fx, "y": fy })
}

// ── GeoJSON readback ─────────────────────────────────────────────────────────

/// A landmark position plakat resolved, mapped back onto the source grid. `x`/`y`
/// are grid cells (north-up GeoJSON re-flipped to the layer's row-0-north grid).
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedLandmark {
    pub id: String,
    pub name: String,
    pub x: usize,
    pub y: usize,
}

impl ResolvedLandmark {
    /// If this landmark came from an accepted Place (`place_<uuid>`), the uuid.
    pub fn place_id(&self) -> Option<uuid::Uuid> {
        self.id.strip_prefix("place_").and_then(|s| uuid::Uuid::parse_str(s).ok())
    }
}

/// Parse plakat's exported GeoJSON, pulling the `landmark` Point features and
/// re-projecting their normalized 0..1 (north-up) coordinates onto a `w×h` grid.
/// Tolerant: non-landmark features, missing fields, and malformed entries are
/// skipped rather than erroring.
pub fn parse_landmark_coords(geojson: &str, w: usize, h: usize) -> Vec<ResolvedLandmark> {
    let Ok(root) = serde_json::from_str::<Value>(geojson) else {
        return Vec::new();
    };
    let Some(features) = root.get("features").and_then(|f| f.as_array()) else {
        return Vec::new();
    };
    features
        .iter()
        .filter_map(|f| {
            let props = f.get("properties")?;
            if props.get("class").and_then(|c| c.as_str()) != Some("landmark") {
                return None;
            }
            let id = props.get("id").and_then(|v| v.as_str())?.to_string();
            let name = props.get("name").and_then(|v| v.as_str()).unwrap_or(&id).to_string();
            let coords = f.get("geometry")?.get("coordinates")?.as_array()?;
            let nx = coords.first()?.as_f64()?;
            let ny = coords.get(1)?.as_f64()?;
            // GeoJSON is north-up normalized; the grid is row-0-north. x maps
            // directly; y flips back (north-up ny=1 ↔ grid row 0).
            let gx = if w > 1 { (nx * (w - 1) as f64).round().clamp(0.0, (w - 1) as f64) as usize } else { 0 };
            let gy = if h > 1 {
                ((1.0 - ny) * (h - 1) as f64).round().clamp(0.0, (h - 1) as f64) as usize
            } else {
                0
            };
            Some(ResolvedLandmark { id, name, x: gx, y: gy })
        })
        .collect()
}

// ── rendering ────────────────────────────────────────────────────────────────

/// The artifacts a render produced.
#[derive(Debug, Clone)]
pub struct MapArtifacts {
    /// The MapSpec we wrote and handed to plakat.
    pub spec_path: PathBuf,
    /// The rendered features PNG.
    pub png_path: PathBuf,
    /// The exported GeoJSON (machine-readable geometry).
    pub geojson_path: PathBuf,
    /// Landmark positions plakat resolved, re-projected onto the source grid.
    pub landmarks: Vec<ResolvedLandmark>,
}

/// Where map artifacts live under a project.
pub fn maps_dir(project: &Path) -> PathBuf {
    project.join("assets").join("maps")
}

/// Render the world map: write `spec` under the project, invoke
/// `plakat map --map-spec … --map-dump-features … --map-export-geojson … --seed`,
/// then read the resolved landmark positions back. `grid_w`/`grid_h` are the
/// compiled layers' grid dimensions (for re-projecting the GeoJSON coordinates).
///
/// Errors if plakat is absent or the subprocess fails; the message names the
/// remedy. Determinism comes from `seed` + the spec.
pub fn render(
    project: &Path,
    spec: &Value,
    seed: u64,
    grid_w: usize,
    grid_h: usize,
) -> Result<MapArtifacts, String> {
    if detect().is_none() {
        return Err(
            "plakat not found on PATH — install it (`cargo install plakat`) to render maps".into(),
        );
    }
    let dir = maps_dir(project);
    std::fs::create_dir_all(&dir).map_err(|e| format!("creating {}: {e}", dir.display()))?;
    let spec_path = dir.join("world.mapspec.json");
    let png_path = dir.join("world.features.png");
    let geojson_path = dir.join("world.geojson");

    let body = serde_json::to_string_pretty(spec).map_err(|e| format!("serializing spec: {e}"))?;
    std::fs::write(&spec_path, body).map_err(|e| format!("writing {}: {e}", spec_path.display()))?;

    let out = std::process::Command::new("plakat")
        .arg("map")
        .args(["--map-spec".as_ref(), spec_path.as_os_str()])
        .args(["--map-dump-features".as_ref(), png_path.as_os_str()])
        .args(["--map-export-geojson".as_ref(), geojson_path.as_os_str()])
        .args(["--seed", &seed.to_string()])
        .output()
        .map_err(|e| format!("running plakat: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(format!("plakat map failed: {}", err.trim()));
    }

    let landmarks = std::fs::read_to_string(&geojson_path)
        .ok()
        .map(|g| parse_landmark_coords(&g, grid_w, grid_h))
        .unwrap_or_default();

    Ok(MapArtifacts { spec_path, png_path, geojson_path, landmarks })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::{compile_climate, compile_demographics, compile_geology, compile_hydrology};
    use crate::world::compile::compile_astronomy;
    use crate::world::types::WorldDefinition;

    fn world() -> WorldDefinition {
        let body = r#"{
            name: "Testworld"
            seed: 12648430
            astronomy: {
                star: { luminosity_solar: 1.0 }
                planet: { mass_earth: 1.0, radius_earth: 1.0, axial_tilt_deg: 23.4, day_length_hours: 24.0 }
                orbit: { semi_major_axis_au: 1.0 }
                calendar: { months: 12, month_length_days: 30 }
            }
        }"#;
        WorldDefinition::from_hjson(body).unwrap()
    }

    /// An empty polities set for map tests that don't exercise trade hubs.
    fn tpol() -> PolitiesOutput {
        PolitiesOutput { polities: Vec::new(), relations: Vec::new() }
    }

    fn layers() -> (GeologyOutput, ClimateOutput, HydrologyOutput, DemographicsOutput) {
        let def = world();
        let astro = compile_astronomy(&def.astronomy);
        let geo = compile_geology(&def);
        let climate = compile_climate(&def, &astro, &geo);
        let hydro = compile_hydrology(&geo, &climate);
        let demo = compile_demographics(&climate, &hydro);
        (geo, climate, hydro, demo)
    }

    #[test]
    fn spec_has_v2_shape() {
        let (geo, climate, hydro, demo) = layers();
        let spec = build_map_spec("Testworld", &geo, &climate, &hydro, &demo, &[], &[], &tpol(), &TradeOutput::default());
        assert_eq!(spec["version"], 2);
        assert_eq!(spec["name"], "Testworld");
        assert!(spec["terrain"]["mountain_ranges"].is_array());
        assert!(spec["water"]["rivers"].is_array());
        assert!(spec["landmarks"].is_array());
        assert!(spec["regions"].is_array());
    }

    #[test]
    fn spec_is_deterministic() {
        let (geo, climate, hydro, demo) = layers();
        let a = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[], &tpol(), &TradeOutput::default());
        let b = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[], &tpol(), &TradeOutput::default());
        assert_eq!(a, b);
    }

    #[test]
    fn landmarks_are_capped_and_anchored() {
        let (geo, climate, hydro, demo) = layers();
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[], &tpol(), &TradeOutput::default());
        let lms = spec["landmarks"].as_array().unwrap();
        assert!(lms.len() <= MAX_LANDMARKS);
        for lm in lms {
            assert_eq!(lm["anchor"]["kind"], "canvas");
            let x = lm["anchor"]["x"].as_f64().unwrap();
            let y = lm["anchor"]["y"].as_f64().unwrap();
            assert!((0.0..=1.0).contains(&x) && (0.0..=1.0).contains(&y));
        }
    }

    #[test]
    fn trade_routes_become_roads_between_capital_hubs() {
        use crate::world::compile::polities_layer::{Polity, Relation};
        use crate::world::compile::trade_layer::TradeOutput;
        let (geo, climate, hydro, demo) = layers();
        let pol = PolitiesOutput {
            polities: vec![
                Polity { name: "Karon".into(), capital: "c".into(), capital_pos: (5, 5), member_count: 1, population: 40_000 },
                Polity { name: "Serai".into(), capital: "c".into(), capital_pos: (30, 30), member_count: 1, population: 30_000 },
            ],
            relations: vec![Relation { a: 0, b: 1, stance: "allied".into() }],
        };
        let trade = crate::world::compile::compile_trade(&pol, &geo, 1.0);
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[], &pol, &trade);
        // Capitals became hub landmarks.
        let lms = spec["landmarks"].as_array().unwrap();
        assert!(lms.iter().any(|l| l["id"] == "hub_0" && l["name"] == "Karon"));
        assert!(lms.iter().any(|l| l["id"] == "hub_1" && l["name"] == "Serai"));
        // The route is a road connecting the two hubs.
        let roads = spec["infrastructure"]["roads"].as_array().unwrap();
        assert_eq!(roads.len(), 1);
        assert_eq!(roads[0]["from"], "hub_0");
        assert_eq!(roads[0]["to"], "hub_1");
        assert_eq!(roads[0]["kind"], "road");
    }

    #[test]
    fn a_declared_landmark_with_coords_is_drawn() {
        let (geo, climate, hydro, demo) = layers();
        let d = DeclaredLandmark { name: "The Ashen Peak".into(), kind: "mountain".into(), x: 3, y: 4 };
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[d], &tpol(), &TradeOutput::default());
        let lms = spec["landmarks"].as_array().unwrap();
        let found = lms.iter().find(|lm| lm["id"] == "decl_the_ashen_peak").expect("declared marker");
        assert_eq!(found["name"], "The Ashen Peak");
        assert_eq!(found["kind"], "mountain");
    }

    #[test]
    fn geo_landmark_projects_lat_lon_to_a_cell() {
        use crate::world::types::world::GeoLandmark;
        let lm = GeoLandmark {
            name: "X".into(), kind: "port".into(), climate_zone: String::new(),
            population: 0, description: String::new(),
            lat: Some(0.0), lon: Some(0.0), x: None, y: None,
        };
        // Equator/prime-meridian on a 160×120 grid → mid grid, in range.
        let (gx, gy) = lm.grid(160, 120).expect("has a position");
        assert!(gx < 160 && gy < 120);
        // Raw x/y takes precedence and clamps.
        let lm2 = GeoLandmark { x: Some(999), y: Some(999), ..lm.clone() };
        assert_eq!(lm2.grid(160, 120), Some((159, 119)));
        // No position → None.
        let lm3 = GeoLandmark { lat: None, lon: None, x: None, y: None, ..lm };
        assert_eq!(lm3.grid(160, 120), None);
    }

    #[test]
    fn hand_placed_link_without_a_settlement_still_gets_a_marker() {
        // A Place positioned by hand (no compiler settlement on its cell,
        // population 0) must still appear as a labeled `place_<uuid>` landmark
        // that round-trips — the WORLD-12 fix.
        let (geo, climate, hydro, demo) = layers();
        // A cell the demographics layer almost certainly has no settlement on.
        let (px, py) = (1usize, 1usize);
        assert!(!demo.settlements.iter().any(|s| s.x == px && s.y == py));
        let id = uuid::Uuid::new_v4();
        let link = PlaceLink {
            place_id: id,
            name: "Hand-Placed Hold".into(),
            biome: String::new(),
            climate_zone: String::new(),
            hydrology_basis: String::new(),
            population: 0,
            x: px,
            y: py,
        };
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[link], &[], &tpol(), &TradeOutput::default());
        let lms = spec["landmarks"].as_array().unwrap();
        let found = lms.iter().find(|lm| lm["id"] == format!("place_{id}")).expect("marker present");
        assert_eq!(found["name"], "Hand-Placed Hold");
        assert_eq!(found["kind"], "town"); // population 0 → plain visible marker
    }

    #[test]
    fn landmarks_dedup_when_a_link_sits_on_a_settlement() {
        let (geo, climate, hydro, demo) = layers();
        let s = demo.settlements.first().cloned().expect("a settlement");
        let id = uuid::Uuid::new_v4();
        let link = PlaceLink {
            place_id: id,
            name: "Capital".into(),
            biome: s.biome.clone(),
            climate_zone: s.biome.clone(),
            hydrology_basis: s.basis.clone(),
            population: s.population,
            x: s.x,
            y: s.y,
        };
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[link], &[], &tpol(), &TradeOutput::default());
        let lms = spec["landmarks"].as_array().unwrap();
        // Exactly one landmark on that cell — the place link, not a duplicate anon.
        let on_cell: Vec<_> = lms
            .iter()
            .filter(|lm| {
                let fx = lm["anchor"]["x"].as_f64().unwrap();
                let fy = lm["anchor"]["y"].as_f64().unwrap();
                let gx = (fx * (geo.width - 1) as f64).round() as usize;
                let gy = (fy * (geo.height - 1) as f64).round() as usize;
                gx == s.x && gy == s.y
            })
            .collect();
        assert_eq!(on_cell.len(), 1);
        assert_eq!(on_cell[0]["id"], format!("place_{id}"));
    }

    #[test]
    fn ranges_capped() {
        let (geo, climate, hydro, demo) = layers();
        let spec = build_map_spec("W", &geo, &climate, &hydro, &demo, &[], &[], &tpol(), &TradeOutput::default());
        assert!(spec["terrain"]["mountain_ranges"].as_array().unwrap().len() <= MAX_RANGES);
    }

    #[test]
    fn geojson_roundtrip_reprojects() {
        // A north-up landmark at the top-centre should land near grid row 0.
        let gj = r#"{
          "type":"FeatureCollection",
          "features":[
            {"type":"Feature","properties":{"class":"landmark","id":"place_550e8400-e29b-41d4-a716-446655440000","name":"Caer"},"geometry":{"type":"Point","coordinates":[0.5,1.0]}},
            {"type":"Feature","properties":{"class":"river","id":"river_0"},"geometry":{"type":"LineString","coordinates":[[0.1,0.1]]}}
          ]
        }"#;
        let lms = parse_landmark_coords(gj, 101, 101);
        assert_eq!(lms.len(), 1, "only the landmark feature is taken");
        assert_eq!(lms[0].x, 50);
        assert_eq!(lms[0].y, 0, "north-up y=1 maps to grid row 0");
        assert_eq!(lms[0].name, "Caer");
        assert!(lms[0].place_id().is_some());
    }

    #[test]
    fn parse_handles_garbage() {
        assert!(parse_landmark_coords("not json", 10, 10).is_empty());
        assert!(parse_landmark_coords("{}", 10, 10).is_empty());
        assert!(parse_landmark_coords(r#"{"features":[]}"#, 10, 10).is_empty());
    }

    #[test]
    fn orientation_and_height_buckets() {
        assert_eq!(orientation(10.0, 1.0), "east-west");
        assert_eq!(orientation(1.0, 10.0), "north-south");
        assert_eq!(orientation(5.0, 5.0), "northeast");
        assert_eq!(height_bucket(0.9), "extreme");
        assert_eq!(height_bucket(0.1), "low");
    }
}
