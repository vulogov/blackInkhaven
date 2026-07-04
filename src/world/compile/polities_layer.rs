//! WORLD-9 (Polities) — aggregate the compiled settlements into nations. The
//! largest settlements become capitals; every other settlement joins the nearest
//! capital's realm. Each polity gets a generated name, a capital, a population,
//! and a member count; pairwise relations (allied / rival / neutral) are seeded.
//! Pure function of `(demographics, seed)`.

use crate::world::types::{DemographicsOutput, Settlement};

#[derive(Debug, Clone, PartialEq)]
pub struct Polity {
    pub name: String,
    /// A descriptor of the capital settlement (settlements are positional).
    pub capital: String,
    pub capital_pos: (usize, usize),
    pub member_count: usize,
    pub population: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Relation {
    /// Indices into `polities`.
    pub a: usize,
    pub b: usize,
    /// "allied" | "rival" | "neutral".
    pub stance: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PolitiesOutput {
    pub polities: Vec<Polity>,
    pub relations: Vec<Relation>,
}

fn dist2(a: (usize, usize), b: (usize, usize)) -> i64 {
    let dx = a.0 as i64 - b.0 as i64;
    let dy = a.1 as i64 - b.1 as i64;
    dx * dx + dy * dy
}

fn hash3(a: u64, b: u64, seed: u64) -> u64 {
    a.wrapping_mul(2_654_435_761)
        .wrapping_add(b.wrapping_mul(40_503))
        .wrapping_add(seed.wrapping_mul(2_246_822_519))
}

/// A short pronounceable realm name from a seed.
fn realm_name(seed: u64) -> String {
    const ONSET: &[&str] = &["K", "R", "Th", "V", "M", "S", "D", "N", "L", "Br", "Ael", "Gor"];
    const NUCLEUS: &[&str] = &["a", "e", "i", "o", "u", "ae", "ia"];
    const CODA: &[&str] = &["n", "r", "th", "l", "s", "m", "nd", "ka"];
    let pick = |slice: &[&'static str], salt: u64| -> &'static str {
        slice[(hash3(seed, salt, 7) % slice.len() as u64) as usize]
    };
    format!(
        "{}{}{}{}",
        pick(ONSET, 1),
        pick(NUCLEUS, 2),
        pick(CODA, 3),
        pick(NUCLEUS, 4)
    )
}

fn describe(s: &Settlement) -> String {
    format!("the {} {}", s.biome, s.class)
}

pub fn compile_polities(demo: &DemographicsOutput, seed: u64) -> PolitiesOutput {
    let settlements = &demo.settlements;
    if settlements.is_empty() {
        return PolitiesOutput { polities: Vec::new(), relations: Vec::new() };
    }

    // One realm per ~6 settlements, at least 1, capped at 8.
    let k = ((settlements.len() + 5) / 6).clamp(1, 8).min(settlements.len());

    // Capitals = the k largest settlements (stable order by population, then pos).
    let mut ranked: Vec<&Settlement> = settlements.iter().collect();
    ranked.sort_by(|a, b| {
        b.population
            .cmp(&a.population)
            .then((a.x, a.y).cmp(&(b.x, b.y)))
    });
    let capitals: Vec<&Settlement> = ranked.iter().take(k).copied().collect();

    let mut members = vec![0usize; k];
    let mut pops = vec![0u64; k];
    for s in settlements {
        // Nearest capital.
        let (mut best, mut best_d) = (0usize, i64::MAX);
        for (ci, cap) in capitals.iter().enumerate() {
            let d = dist2((s.x, s.y), (cap.x, cap.y));
            if d < best_d {
                best_d = d;
                best = ci;
            }
        }
        members[best] += 1;
        pops[best] += s.population;
    }

    let polities: Vec<Polity> = capitals
        .iter()
        .enumerate()
        .map(|(i, cap)| Polity {
            name: realm_name(seed.wrapping_add((cap.x as u64) << 16).wrapping_add(cap.y as u64)),
            capital: describe(cap),
            capital_pos: (cap.x, cap.y),
            member_count: members[i],
            population: pops[i],
        })
        .collect();

    // Seeded pairwise relations.
    let mut relations = Vec::new();
    for a in 0..k {
        for b in (a + 1)..k {
            let stance = match hash3(a as u64, b as u64, seed) % 5 {
                0 => "allied",
                1 => "rival",
                2 => "rival",
                _ => "neutral",
            };
            relations.push(Relation { a, b, stance: stance.to_string() });
        }
    }

    PolitiesOutput { polities, relations }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settle(x: usize, y: usize, pop: u64) -> Settlement {
        Settlement {
            x,
            y,
            population: pop,
            class: "city".into(),
            basis: "river_mouth".into(),
            biome: "temperate".into(),
        }
    }

    fn demo(s: Vec<Settlement>) -> DemographicsOutput {
        DemographicsOutput {
            total_population: s.iter().map(|x| x.population).sum(),
            habitable_fraction: 0.5,
            settlements: s,
            size_classes: Default::default(),
            role_archetypes: Vec::new(),
        }
    }

    #[test]
    fn clusters_settlements_around_the_largest_capitals() {
        // Two population centres far apart + satellites → two realms.
        let d = demo(vec![
            settle(0, 0, 90_000),
            settle(1, 1, 2_000),
            settle(2, 0, 1_500),
            settle(50, 50, 80_000),
            settle(51, 49, 3_000),
            settle(49, 51, 1_000),
            settle(48, 48, 900),
            settle(52, 52, 800),
            settle(53, 51, 700),
            settle(47, 53, 600),
            settle(50, 47, 500),
            settle(46, 46, 400),
        ]);
        let p = compile_polities(&d, 0x2024);
        assert_eq!(p.polities.len(), 2);
        // Every settlement is accounted for.
        let total: usize = p.polities.iter().map(|x| x.member_count).sum();
        assert_eq!(total, 12);
        // Relations cover every pair.
        assert_eq!(p.relations.len(), 1);
    }

    #[test]
    fn is_deterministic() {
        let d = demo(vec![settle(0, 0, 5000), settle(9, 9, 4000)]);
        assert_eq!(compile_polities(&d, 3), compile_polities(&d, 3));
    }
}
