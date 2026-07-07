//! WORLD-15 — the Trade layer. Deterministic *connectivity*, not simulated
//! economics: realms that are not rivals trade with their nearest neighbours, and
//! the route is a land road or a sea lane depending on whether the two capitals
//! sit on a coast. Pure function of `(polities, demographics, geology)` — it
//! invents no prices, goods, or volumes, only which realms are linked and how.
//!
//! The routes surface three ways: `realworld trade` (CLI), a `Trade` chapter in
//! the World book, and roads drawn on the plakat map (hub landmarks + `RoadSpec`).

use crate::world::compile::polities_layer::PolitiesOutput;
use crate::world::types::GeologyOutput;

/// A trade link between two realms (indices into `polities`), undirected with
/// `from < to`.
#[derive(Debug, Clone, PartialEq)]
pub struct TradeRoute {
    pub from: usize,
    pub to: usize,
    /// `"land"` (overland road) or `"sea"` (a coastal lane).
    pub mode: String,
    /// `"allied"` or `"neutral"` — rivals never get a route.
    pub stance: String,
    pub distance_km: f64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TradeOutput {
    pub routes: Vec<TradeRoute>,
}

const DX: [i32; 8] = [1, 1, 0, -1, -1, -1, 0, 1];
const DY: [i32; 8] = [0, 1, 1, 1, 0, -1, -1, -1];

/// Is the cell coastal — land with at least one sea neighbour (or itself water)?
/// A river-mouth / coastal capital trades by sea.
fn is_coastal(geo: &GeologyOutput, x: usize, y: usize) -> bool {
    let (w, h) = (geo.width, geo.height);
    if w == 0 || h == 0 || geo.heightmap.len() != w * h || x >= w || y >= h {
        return false;
    }
    let sea = geo.sea_level;
    if geo.heightmap[y * w + x] <= sea {
        return true; // on the water itself (a port on an inlet)
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

fn stance_of(pol: &PolitiesOutput, a: usize, b: usize) -> &str {
    let (lo, hi) = if a < b { (a, b) } else { (b, a) };
    pol.relations
        .iter()
        .find(|r| (r.a, r.b) == (lo, hi) || (r.a, r.b) == (hi, lo))
        .map(|r| r.stance.as_str())
        .unwrap_or("neutral")
}

/// How many nearest non-rival neighbours each realm links to. Keeps the network a
/// plausible web rather than a dense complete graph.
const MAX_LINKS_PER_REALM: usize = 3;

/// Compile the trade network: each realm links to its nearest non-rival realms,
/// classified land or sea by the two capitals' coasts. Deterministic.
pub fn compile_trade(pol: &PolitiesOutput, geo: &GeologyOutput, radius_earth: f64) -> TradeOutput {
    let n = pol.polities.len();
    if n < 2 {
        return TradeOutput::default();
    }
    let (w, h) = (geo.width, geo.height);
    let coastal: Vec<bool> =
        pol.polities.iter().map(|p| is_coastal(geo, p.capital_pos.0, p.capital_pos.1)).collect();

    // Collect undirected non-rival links, keyed (lo, hi) → route, so a link that
    // both endpoints choose is stored once.
    use std::collections::BTreeMap;
    let mut links: BTreeMap<(usize, usize), TradeRoute> = BTreeMap::new();
    for a in 0..n {
        // Rank the other realms by distance, drop rivals, keep the nearest few.
        let mut nbrs: Vec<(usize, f64, &str)> = (0..n)
            .filter(|&b| b != a)
            .map(|b| {
                let dx = pol.polities[a].capital_pos.0 as f64 - pol.polities[b].capital_pos.0 as f64;
                let dy = pol.polities[a].capital_pos.1 as f64 - pol.polities[b].capital_pos.1 as f64;
                (b, crate::world::travel::distance_km(radius_earth, w, h, dx, dy), stance_of(pol, a, b))
            })
            .filter(|(_, _, stance)| *stance != "rival")
            .collect();
        nbrs.sort_by(|x, y| {
            x.1.partial_cmp(&y.1).unwrap_or(std::cmp::Ordering::Equal).then(x.0.cmp(&y.0))
        });
        for (b, km, stance) in nbrs.into_iter().take(MAX_LINKS_PER_REALM) {
            let (lo, hi) = if a < b { (a, b) } else { (b, a) };
            let mode = if coastal[lo] && coastal[hi] { "sea" } else { "land" };
            links.entry((lo, hi)).or_insert(TradeRoute {
                from: lo,
                to: hi,
                mode: mode.to_string(),
                stance: stance.to_string(),
                distance_km: km,
            });
        }
    }
    TradeOutput { routes: links.into_values().collect() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::polities_layer::{Polity, Relation};
    use crate::world::types::GeologyOutput;

    fn polity(name: &str, x: usize, y: usize) -> Polity {
        Polity {
            name: name.into(),
            capital: "cap".into(),
            capital_pos: (x, y),
            member_count: 1,
            population: 10_000,
        }
    }

    /// A tiny all-land world (heightmap above sea everywhere).
    fn land_geo() -> GeologyOutput {
        let (w, h) = (10, 10);
        GeologyOutput {
            width: w,
            height: h,
            sea_level: 0.0,
            heightmap: vec![1.0; w * h],
            ..Default::default()
        }
    }

    #[test]
    fn rivals_never_get_a_route_non_rivals_nearby_do() {
        let pol = PolitiesOutput {
            polities: vec![polity("A", 1, 1), polity("B", 2, 1), polity("C", 8, 8)],
            relations: vec![
                Relation { a: 0, b: 1, stance: "rival".into() }, // A–B rivals
                Relation { a: 0, b: 2, stance: "allied".into() },
                Relation { a: 1, b: 2, stance: "neutral".into() },
            ],
        };
        let t = compile_trade(&pol, &land_geo(), 1.0);
        // No A–B route (rivals); A–C and B–C exist.
        assert!(!t.routes.iter().any(|r| (r.from, r.to) == (0, 1)));
        assert!(t.routes.iter().any(|r| (r.from, r.to) == (0, 2)));
        assert!(t.routes.iter().any(|r| (r.from, r.to) == (1, 2)));
        // All-land world → land routes.
        assert!(t.routes.iter().all(|r| r.mode == "land"));
    }

    #[test]
    fn coastal_capitals_trade_by_sea() {
        // Put a strip of sea so both capitals sit on a coast.
        let (w, h) = (10, 10);
        let mut hm = vec![1.0f32; w * h];
        for y in 0..h {
            hm[y * w] = -1.0; // west column is ocean
        }
        let geo = GeologyOutput { width: w, height: h, sea_level: 0.0, heightmap: hm, ..Default::default() };
        let pol = PolitiesOutput {
            polities: vec![polity("A", 1, 2), polity("B", 1, 7)], // both beside the ocean column
            relations: vec![Relation { a: 0, b: 1, stance: "allied".into() }],
        };
        let t = compile_trade(&pol, &geo, 1.0);
        assert_eq!(t.routes.len(), 1);
        assert_eq!(t.routes[0].mode, "sea");
        assert_eq!(t.routes[0].stance, "allied");
    }

    #[test]
    fn is_deterministic_and_dedups_undirected() {
        let pol = PolitiesOutput {
            polities: vec![polity("A", 1, 1), polity("B", 2, 2), polity("C", 3, 3)],
            relations: Vec::new(),
        };
        let a = compile_trade(&pol, &land_geo(), 1.0);
        let b = compile_trade(&pol, &land_geo(), 1.0);
        assert_eq!(a, b);
        // Each undirected pair appears once.
        assert!(a.routes.iter().all(|r| r.from < r.to));
    }
}
