//! WORLD-8 (W8-P1) — the History & Chronology layer. A deterministic pass that
//! gives the world a *past*: it infers a founding chronology from the compiled
//! demographics (larger, better-sited settlements are older) and divides that
//! span into epochs. Pure function of `(demographics, seed)` — same world, same
//! history — so it can be re-derived on demand like the physical layers.
//!
//! Settlements carry no names (they're positional), so foundings are labelled
//! descriptively ("the river-mouth city in the forest"); the author renames them
//! when adopting the events into the story Timeline.

use crate::world::types::{DemographicsOutput, Settlement};

/// One settlement's founding, dated relative to the story's "present" (year 0;
/// negative years are before it).
#[derive(Debug, Clone, PartialEq)]
pub struct Founding {
    pub year: i64,
    pub label: String,
    pub class: String,
    pub population: u64,
}

/// A named span of the world's past.
#[derive(Debug, Clone, PartialEq)]
pub struct Epoch {
    pub name: String,
    pub start_year: i64,
    pub end_year: i64,
    pub note: String,
}

/// A generated historical event beyond the foundings (a realm's rise or fall, a
/// migration) — the narrative texture of the world's past.
#[derive(Debug, Clone, PartialEq)]
pub struct HistEvent {
    pub year: i64,
    /// "rise" | "fall" | "migration".
    pub kind: String,
    pub description: String,
}

/// The compiled chronology.
#[derive(Debug, Clone, PartialEq)]
pub struct HistoryOutput {
    /// Foundings, oldest first.
    pub foundings: Vec<Founding>,
    pub epochs: Vec<Epoch>,
    /// Realm rises/falls + migrations, chronologically ordered.
    pub events: Vec<HistEvent>,
    /// Total span of recorded history, in years.
    pub span_years: i64,
}

fn mix(a: u64, seed: u64) -> u64 {
    a.wrapping_mul(2_654_435_761).wrapping_add(seed.wrapping_mul(40_503)).wrapping_add(0x9E37)
}

/// Weight of a settlement's siting toward antiquity — the best sites were
/// settled first.
fn basis_weight(basis: &str) -> f64 {
    match basis {
        "river_mouth" => 3.0,
        "confluence" => 2.0,
        "fertile_valley" => 1.5,
        _ => 1.0,
    }
}

/// A deterministic 0..1 jitter from a settlement's position + the world seed, so
/// equally-scored settlements still get a stable order without a real RNG.
fn jitter(s: &Settlement, seed: u64) -> f64 {
    let h = (s.x as u64)
        .wrapping_mul(2_654_435_761)
        .wrapping_add((s.y as u64).wrapping_mul(40_503))
        .wrapping_add(seed);
    (h % 1000) as f64 / 1000.0
}

/// Antiquity score — higher is older. Population (log) scaled by siting quality,
/// plus the stable jitter as a tiebreak.
fn antiquity(s: &Settlement, seed: u64) -> f64 {
    (s.population.max(1) as f64).ln() * basis_weight(&s.basis) + jitter(s, seed)
}

pub fn compile_history(demo: &DemographicsOutput, seed: u64) -> HistoryOutput {
    let mut settlements: Vec<&Settlement> = demo.settlements.iter().collect();
    settlements.sort_by(|a, b| {
        antiquity(b, seed)
            .partial_cmp(&antiquity(a, seed))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // A larger, more-settled world has a longer recorded past (capped).
    let span = (200 + demo.settlements.len() as i64 * 20).min(4000);
    let n = settlements.len().max(1) as i64;

    let foundings: Vec<Founding> = settlements
        .iter()
        .enumerate()
        .map(|(i, s)| Founding {
            // Oldest at -span; newest approaches the present (year 0).
            year: -span + (span * i as i64) / n,
            label: format!("the {} {} on the {}", s.biome, s.class, s.basis.replace('_', " ")),
            class: s.class.clone(),
            population: s.population,
        })
        .collect();

    let third = (span / 3).max(1);
    let epochs = vec![
        Epoch {
            name: "Founding Age".into(),
            start_year: -span,
            end_year: -span + third,
            note: "the first settlements take root on the best sites".into(),
        },
        Epoch {
            name: "Age of Expansion".into(),
            start_year: -span + third,
            end_year: -third,
            note: "towns spread along the rivers and coasts".into(),
        },
        Epoch {
            name: "Present Age".into(),
            start_year: -third,
            end_year: 0,
            note: "the world as the story finds it".into(),
        },
    ];

    // Richer events: polity rise/fall + migrations between biomes.
    let mut events: Vec<HistEvent> = Vec::new();
    let pol = super::polities_layer::compile_polities(demo, seed);
    for (i, p) in pol.polities.iter().enumerate() {
        // Realms rise across the early epochs, spread out.
        let rise = -span + third * ((mix(i as u64, seed) % 3) as i64);
        events.push(HistEvent {
            year: rise,
            kind: "rise".into(),
            description: format!("the realm of {} rises around {}", p.name, p.capital),
        });
        // Roughly one in three wanes in a later age.
        if mix(i as u64, seed.wrapping_add(11)) % 3 == 0 {
            events.push(HistEvent {
                year: -third + span / 6,
                kind: "fall".into(),
                description: format!("the realm of {} wanes", p.name),
            });
        }
    }
    // Distinct biomes present → seeded migrations.
    let mut biomes: Vec<String> = Vec::new();
    for s in &demo.settlements {
        if !biomes.contains(&s.biome) {
            biomes.push(s.biome.clone());
        }
    }
    if biomes.len() >= 2 {
        for e in 0..2u64 {
            let a = &biomes[(mix(e, seed) % biomes.len() as u64) as usize];
            let b = &biomes[(mix(e.wrapping_add(7), seed.wrapping_mul(3)) % biomes.len() as u64) as usize];
            if a != b {
                events.push(HistEvent {
                    year: -span + third + (e as i64) * (third / 2),
                    kind: "migration".into(),
                    description: format!("a people migrate from the {a} to the {b}"),
                });
            }
        }
    }
    events.sort_by_key(|e| e.year);

    HistoryOutput { foundings, epochs, events, span_years: span }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(x: usize, y: usize, pop: u64, class: &str, basis: &str) -> Settlement {
        Settlement {
            x,
            y,
            population: pop,
            class: class.into(),
            basis: basis.into(),
            biome: "temperate".into(),
        }
    }

    fn demo(settlements: Vec<Settlement>) -> DemographicsOutput {
        DemographicsOutput {
            total_population: settlements.iter().map(|s| s.population).sum(),
            habitable_fraction: 0.5,
            settlements,
            size_classes: Default::default(),
            role_archetypes: Vec::new(),
        }
    }

    #[test]
    fn oldest_settlement_is_the_largest_best_sited() {
        let d = demo(vec![
            settle(1, 1, 500, "village", "coast"),
            settle(2, 2, 90_000, "city", "river_mouth"),
            settle(3, 3, 5_000, "town", "fertile_valley"),
        ]);
        let h = compile_history(&d, 0x1234);
        // The big river-mouth city is the oldest (most negative year).
        assert_eq!(h.foundings.first().unwrap().class, "city");
        assert!(h.foundings.first().unwrap().year < h.foundings.last().unwrap().year);
        assert_eq!(h.foundings.len(), 3);
        assert_eq!(h.epochs.len(), 3);
    }

    #[test]
    fn generates_rise_and_migration_events() {
        let d = demo(vec![
            settle(0, 0, 90_000, "city", "river_mouth"),
            settle(40, 40, 80_000, "city", "confluence"),
            settle(1, 1, 3_000, "town", "fertile_valley"),
            settle(41, 39, 2_000, "town", "coast"),
        ]);
        // Give the settlements distinct biomes so a migration can fire.
        let mut d = d;
        d.settlements[1].biome = "hot_desert".into();
        d.settlements[3].biome = "taiga".into();
        let h = compile_history(&d, 0x55);
        assert!(h.events.iter().any(|e| e.kind == "rise"));
        assert!(h.events.iter().any(|e| e.kind == "migration"));
        // Chronologically ordered.
        assert!(h.events.windows(2).all(|w| w[0].year <= w[1].year));
    }

    #[test]
    fn is_deterministic() {
        let d = demo(vec![
            settle(1, 1, 500, "village", "coast"),
            settle(2, 2, 90_000, "city", "river_mouth"),
        ]);
        assert_eq!(compile_history(&d, 7), compile_history(&d, 7));
    }

    #[test]
    fn span_scales_with_settlement_count_and_is_capped() {
        let one = demo(vec![settle(1, 1, 100, "village", "coast")]);
        let many = demo((0..500).map(|i| settle(i, i, 100, "village", "coast")).collect());
        assert!(compile_history(&one, 0).span_years < compile_history(&many, 0).span_years);
        assert!(compile_history(&many, 0).span_years <= 4000);
    }
}
