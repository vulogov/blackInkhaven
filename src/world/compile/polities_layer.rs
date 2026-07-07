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
    // Saturating: a wildly-out-of-range declared capital coordinate must not
    // panic (debug) or wrap to a negative "nearest" (release).
    let dx = a.0 as i64 - b.0 as i64;
    let dy = a.1 as i64 - b.1 as i64;
    dx.saturating_mul(dx).saturating_add(dy.saturating_mul(dy))
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

pub fn compile_polities(
    demo: &DemographicsOutput,
    declared: &[crate::world::types::NationDef],
    seed: u64,
) -> PolitiesOutput {
    use std::collections::{HashMap, HashSet};
    let settlements = &demo.settlements;
    if settlements.is_empty() {
        return PolitiesOutput { polities: Vec::new(), relations: Vec::new() };
    }

    // Target realm count: ~one per six settlements (1..8), but at least enough for
    // every declared nation.
    let generated_k = ((settlements.len() + 5) / 6).clamp(1, 8).min(settlements.len());
    let k_target = generated_k.max(declared.len()).min(settlements.len());

    // Capitals: each declared nation first (the settlement nearest its declared
    // capital cell), then the largest remaining settlements fill to the target.
    let mut used: HashSet<(usize, usize)> = HashSet::new();
    let mut caps: Vec<(&Settlement, Option<&crate::world::types::NationDef>)> = Vec::new();
    for nd in declared {
        // The nearest settlement not already seated by an earlier declared
        // capital — so two declared nations near one settlement don't collapse
        // into one (the second would otherwise be silently dropped).
        if let Some(s) = settlements
            .iter()
            .filter(|s| !used.contains(&(s.x, s.y)))
            .min_by_key(|s| dist2((s.x, s.y), (nd.capital[0], nd.capital[1])))
        {
            used.insert((s.x, s.y));
            caps.push((s, Some(nd)));
        }
    }
    let mut ranked: Vec<&Settlement> = settlements.iter().collect();
    ranked.sort_by(|a, b| b.population.cmp(&a.population).then((a.x, a.y).cmp(&(b.x, b.y))));
    for s in ranked {
        if caps.len() >= k_target {
            break;
        }
        if used.insert((s.x, s.y)) {
            caps.push((s, None));
        }
    }
    let k = caps.len();

    let mut members = vec![0usize; k];
    let mut pops = vec![0u64; k];
    for s in settlements {
        let (mut best, mut best_d) = (0usize, i64::MAX);
        for (ci, (cap, _)) in caps.iter().enumerate() {
            let d = dist2((s.x, s.y), (cap.x, cap.y));
            if d < best_d {
                best_d = d;
                best = ci;
            }
        }
        members[best] += 1;
        pops[best] += s.population;
    }

    let polities: Vec<Polity> = caps
        .iter()
        .enumerate()
        .map(|(i, (cap, nd))| Polity {
            name: match nd {
                Some(n) => n.name.clone(),
                None => realm_name(seed.wrapping_add((cap.x as u64) << 16).wrapping_add(cap.y as u64)),
            },
            capital: describe(cap),
            capital_pos: (cap.x, cap.y),
            member_count: members[i],
            population: pops[i],
        })
        .collect();

    // Declared relations override the seeded pairwise ones (matched by name).
    let mut declared_pairs: HashMap<(usize, usize), String> = HashMap::new();
    for (i, (_, nd)) in caps.iter().enumerate() {
        if let Some(n) = nd {
            for r in &n.relations {
                if let Some(j) = polities.iter().position(|p| p.name.eq_ignore_ascii_case(&r.with)) {
                    let (a, b) = if i < j { (i, j) } else { (j, i) };
                    if a != b {
                        declared_pairs.insert((a, b), r.stance.clone());
                    }
                }
            }
        }
    }
    let mut relations = Vec::new();
    for a in 0..k {
        for b in (a + 1)..k {
            let stance = declared_pairs.get(&(a, b)).cloned().unwrap_or_else(|| {
                match hash3(a as u64, b as u64, seed) % 5 {
                    0 => "allied",
                    1 | 2 => "rival",
                    _ => "neutral",
                }
                .to_string()
            });
            relations.push(Relation { a, b, stance });
        }
    }

    PolitiesOutput { polities, relations }
}

/// WORLD-11 (W11-P2) — verify declared nations for plausibility. Advisory.
pub fn lint_polities(
    declared: &[crate::world::types::NationDef],
    demo: &DemographicsOutput,
) -> Vec<String> {
    let mut w = Vec::new();
    // BUG-14 — two declared nations sharing a name make relations ambiguous: a
    // `with: <name>` reference binds to the first match only. We cannot guess
    // which was meant, so we warn (deterministic order via BTreeMap).
    let mut name_counts: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();
    for nd in declared {
        *name_counts.entry(nd.name.trim().to_lowercase()).or_insert(0) += 1;
    }
    for (name, count) in &name_counts {
        if *count > 1 {
            w.push(format!(
                "nation name `{name}` is declared {count} times — a `with` relation to it binds to the first realm only; give each realm a distinct name"
            ));
        }
    }
    for nd in declared {
        match demo
            .settlements
            .iter()
            .map(|s| dist2((s.x, s.y), (nd.capital[0], nd.capital[1])))
            .min()
        {
            None => w.push(format!("nation `{}`: the world has no settlements to seat a capital", nd.name)),
            Some(d2) => {
                let d = (d2 as f64).sqrt();
                if d > 10.0 {
                    w.push(format!(
                        "nation `{}`: capital cell ({}, {}) is {:.0} cells from the nearest settlement — is it in the wilderness?",
                        nd.name, nd.capital[0], nd.capital[1], d
                    ));
                }
            }
        }
    }
    w
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
    fn duplicate_declared_nation_names_are_flagged() {
        use crate::world::types::NationDef;
        let d = demo(vec![settle(0, 0, 90_000), settle(40, 40, 80_000)]);
        let declared = vec![
            NationDef { name: "Karon".into(), capital: [0, 0], relations: Vec::new() },
            NationDef { name: "karon".into(), capital: [40, 40], relations: Vec::new() },
        ];
        let w = lint_polities(&declared, &d);
        assert!(w.iter().any(|s| s.contains("declared 2 times")), "no dup-name warning: {w:?}");
        // A distinct-named set produces no dup warning.
        let ok = vec![
            NationDef { name: "Karon".into(), capital: [0, 0], relations: Vec::new() },
            NationDef { name: "Serai".into(), capital: [40, 40], relations: Vec::new() },
        ];
        assert!(!lint_polities(&ok, &d).iter().any(|s| s.contains("times")));
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
        let p = compile_polities(&d, &[], 0x2024);
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
        assert_eq!(compile_polities(&d, &[], 3), compile_polities(&d, &[], 3));
    }

    #[test]
    fn declared_nations_name_their_realms_and_relations() {
        use crate::world::types::{NationDef, NationRelation};
        let d = demo(vec![
            settle(0, 0, 90_000),
            settle(1, 1, 2_000),
            settle(50, 50, 80_000),
            settle(51, 49, 3_000),
        ]);
        let declared = vec![
            NationDef {
                name: "Karon".into(),
                capital: [0, 0],
                relations: vec![NationRelation { with: "Serai".into(), stance: "rival".into() }],
            },
            NationDef { name: "Serai".into(), capital: [50, 50], relations: vec![] },
        ];
        let p = compile_polities(&d, &declared, 0x1);
        assert!(p.polities.iter().any(|x| x.name == "Karon"));
        assert!(p.polities.iter().any(|x| x.name == "Serai"));
        let ki = p.polities.iter().position(|x| x.name == "Karon").unwrap();
        let si = p.polities.iter().position(|x| x.name == "Serai").unwrap();
        let (a, b) = if ki < si { (ki, si) } else { (si, ki) };
        assert!(p.relations.iter().any(|r| r.a == a && r.b == b && r.stance == "rival"));
    }

    #[test]
    fn a_wilderness_capital_is_flagged() {
        use crate::world::types::NationDef;
        let d = demo(vec![settle(0, 0, 5000)]);
        let declared = vec![NationDef { name: "Faraway".into(), capital: [900, 900], relations: vec![] }];
        assert!(lint_polities(&declared, &d).iter().any(|s| s.contains("wilderness")));
    }

    #[test]
    fn two_declared_nations_near_one_settlement_both_survive() {
        use crate::world::types::NationDef;
        // Both declared capitals are nearest to (0,0); each must seat a distinct
        // settlement rather than the second being silently dropped.
        let d = demo(vec![settle(0, 0, 50_000), settle(80, 80, 40_000)]);
        let declared = vec![
            NationDef { name: "Aa".into(), capital: [1, 1], relations: vec![] },
            NationDef { name: "Bb".into(), capital: [2, 2], relations: vec![] },
        ];
        let p = compile_polities(&d, &declared, 1);
        assert!(p.polities.iter().any(|x| x.name == "Aa"));
        assert!(p.polities.iter().any(|x| x.name == "Bb"));
    }

    #[test]
    fn an_extreme_declared_capital_does_not_panic() {
        use crate::world::types::NationDef;
        let d = demo(vec![settle(0, 0, 5000)]);
        let declared =
            vec![NationDef { name: "Far".into(), capital: [3_000_000_000, 0], relations: vec![] }];
        let p = compile_polities(&d, &declared, 1); // must not overflow-panic
        assert_eq!(p.polities.len(), 1);
        assert!(lint_polities(&declared, &d).iter().any(|s| s.contains("wilderness")));
    }
}
