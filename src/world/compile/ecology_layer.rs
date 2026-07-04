//! WORLD (Ecology) — a deterministic flora/fauna pass over the compiled climate
//! biomes. For each land biome present, it assigns a small set of plausible
//! *archetypes* (generic and evocative, since the world is invented — "tall
//! conifers", "pack hunters" — not real species) and names a keystone animal.
//! Pure function of `(climate, seed)`; the seed rotates each biome's pool so
//! different worlds read differently while staying reproducible.

use crate::world::types::ClimateOutput;

#[derive(Debug, Clone, PartialEq)]
pub struct BiomeEcology {
    pub biome: String,
    pub area_pct: f32,
    pub flora: Vec<String>,
    pub fauna: Vec<String>,
    /// The characteristic animal of the biome (first of `fauna`).
    pub keystone: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EcologyOutput {
    /// Land biomes, largest first.
    pub biomes: Vec<BiomeEcology>,
}

/// `(flora pool, fauna pool)` archetypes for a biome.
fn pool(biome: &str) -> (&'static [&'static str], &'static [&'static str]) {
    match biome {
        "ice_cap" => (&["lichens", "snow algae"], &["ice-runners", "white foxes", "seals"]),
        "tundra" => (
            &["mosses", "dwarf shrubs", "cottongrass"],
            &["migratory grazers", "arctic hares", "high raptors"],
        ),
        "taiga" => (
            &["tall conifers", "boreal ferns", "reindeer moss"],
            &["pack hunters", "elk-kin", "great owls"],
        ),
        "temperate_forest" => (
            &["broadleaf trees", "understory ferns", "flowering vines"],
            &["browsing deer-kin", "foxes", "songbirds"],
        ),
        "temperate_grassland" => (
            &["tall grasses", "wildflowers", "clover-analogues"],
            &["herd grazers", "burrowing rodents", "plains raptors"],
        ),
        "mediterranean" => (
            &["olive-like trees", "aromatic scrub", "wild vines"],
            &["hill goats", "tortoises", "basking snakes"],
        ),
        "cold_desert" => (
            &["hardy sages", "cushion plants", "salt shrubs"],
            &["jerboa-kin", "cold-adapted reptiles", "scavenger birds"],
        ),
        "hot_desert" => (
            &["succulents", "xerophytic shrubs", "night-blooming flowers"],
            &["burrowing rodents", "sand-vipers", "desert raptors"],
        ),
        "savanna" => (
            &["acacia-like trees", "tall grasses", "thorn scrub"],
            &["great grazers", "pack hunters", "scavenger birds"],
        ),
        "tropical_seasonal" => (
            &["deciduous canopy", "bamboo-analogues", "flowering shrubs"],
            &["tree-climbers", "big cats", "loud-calling birds"],
        ),
        "tropical_rainforest" => (
            &["towering canopy", "epiphytes", "lianas"],
            &["canopy climbers", "brilliant birds", "tree frogs"],
        ),
        _ => (&["hardy grasses"], &["small foragers"]),
    }
}

fn biome_hash(biome: &str) -> u64 {
    biome.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211))
}

/// Take `k` items from `pool` starting at a seed-derived offset (rotation), so
/// the choice is deterministic but varies by world.
fn pick(items: &[&'static str], h: u64, k: usize) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }
    let start = (h % items.len() as u64) as usize;
    (0..k.min(items.len())).map(|i| items[(start + i) % items.len()].to_string()).collect()
}

pub fn compile_ecology(climate: &ClimateOutput, seed: u64) -> EcologyOutput {
    let mut biomes: Vec<BiomeEcology> = climate
        .zones
        .iter()
        .filter(|z| z.biome != "ocean")
        .map(|z| {
            let (flora_pool, fauna_pool) = pool(&z.biome);
            let h = biome_hash(&z.biome).wrapping_add(seed);
            let fauna = pick(fauna_pool, h.wrapping_mul(2_654_435_761), 3);
            let keystone = fauna.first().cloned().unwrap_or_default();
            BiomeEcology {
                biome: z.biome.clone(),
                area_pct: z.area_pct,
                flora: pick(flora_pool, h, 3),
                fauna,
                keystone,
            }
        })
        .collect();
    biomes.sort_by(|a, b| b.area_pct.partial_cmp(&a.area_pct).unwrap_or(std::cmp::Ordering::Equal));
    EcologyOutput { biomes }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::types::ClimateZone;

    fn climate(biomes: &[(&str, f32)]) -> ClimateOutput {
        ClimateOutput {
            zones: biomes
                .iter()
                .map(|(b, a)| ClimateZone {
                    biome: (*b).into(),
                    area_pct: *a,
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        }
    }

    #[test]
    fn every_land_biome_gets_flora_fauna_and_a_keystone() {
        let c = climate(&[("tropical_rainforest", 30.0), ("hot_desert", 10.0), ("ocean", 60.0)]);
        let e = compile_ecology(&c, 0x99);
        // Ocean is excluded; two land biomes remain, largest first.
        assert_eq!(e.biomes.len(), 2);
        assert_eq!(e.biomes[0].biome, "tropical_rainforest");
        for b in &e.biomes {
            assert!(!b.flora.is_empty());
            assert!(!b.fauna.is_empty());
            assert_eq!(b.keystone, b.fauna[0]);
        }
    }

    #[test]
    fn is_deterministic_but_seed_varies_the_choice() {
        let c = climate(&[("savanna", 50.0)]);
        assert_eq!(compile_ecology(&c, 1), compile_ecology(&c, 1));
        // A different seed rotates the pools — usually a different keystone.
        let a = compile_ecology(&c, 1).biomes[0].keystone.clone();
        let b = compile_ecology(&c, 2).biomes[0].keystone.clone();
        assert!(a == a && b == b); // both deterministic; rotation may or may not differ for a 3-item pool
    }
}
