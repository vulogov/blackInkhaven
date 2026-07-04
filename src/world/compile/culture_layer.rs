//! WORLD-9 (Culture & Society) — one culture per polity. The ethos draws on the
//! capital's biome; the belief, the language *profile* (a typology sketch), and a
//! naming sample are seeded. Pure function of `(polities, capital biomes, seed)`.
//!
//! The language profile is a proposal, not a built language: the author realises
//! it in the ConLang suite (`inkhaven language`) and assigns it to the culture —
//! the same "sim proposes, author adopts" discipline the rest of WORLD uses.

use crate::world::compile::polities_layer::PolitiesOutput;

#[derive(Debug, Clone, PartialEq)]
pub struct Culture {
    pub polity: String,
    pub ethos: String,
    pub belief: String,
    /// A conlang typology sketch (word order · morphology · sound) to realise.
    pub language_profile: String,
    /// A sample name in the culture's phonic style.
    pub naming_sample: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CultureOutput {
    pub cultures: Vec<Culture>,
}

fn h(a: u64, salt: u64) -> u64 {
    a.wrapping_mul(2_654_435_761).wrapping_add(salt.wrapping_mul(40_503)).wrapping_add(0x9E37)
}

fn pick<'a>(items: &[&'a str], k: u64) -> &'a str {
    items[(k % items.len() as u64) as usize]
}

fn biome_ethos(biome: &str) -> &'static str {
    match biome {
        b if b.contains("forest") || b == "taiga" => "woodland-reverent",
        b if b.contains("desert") => "austere and hospitable",
        b if b.contains("grassland") || b == "savanna" => "roaming and herd-proud",
        b if b.contains("tropical") => "vivid and ceremonious",
        b if b == "tundra" || b == "ice_cap" => "hardy and close-knit",
        b if b == "mediterranean" => "mercantile and civic",
        _ => "settled and pragmatic",
    }
}

/// `capital_biomes[i]` is polity `i`'s capital biome (parallel to `pol.polities`).
pub fn compile_culture(pol: &PolitiesOutput, capital_biomes: &[String], seed: u64) -> CultureOutput {
    const TEMPER: &[&str] =
        &["proud", "cautious", "curious", "devout", "stubborn", "generous", "secretive", "boisterous"];
    const BELIEF: &[&str] = &[
        "ancestor veneration",
        "a sky-pantheon",
        "one hidden god",
        "nature spirits of river and stone",
        "a cult of the seasons",
        "reverence for the founding dead",
    ];
    const ORDER: &[&str] = &["SVO", "SOV", "VSO"];
    const MORPH: &[&str] = &["isolating", "agglutinative", "fusional"];
    const SOUND: &[&str] = &["tonal", "guttural", "liquid and vowel-rich", "clipped and consonantal"];
    const ONSET: &[&str] = &["Ka", "Ser", "Tho", "Vae", "Mor", "Ilu", "Bra", "Nen"];
    const TAIL: &[&str] = &["ra", "eth", "is", "un", "ora", "ai", "el"];

    let cultures = pol
        .polities
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let biome = capital_biomes.get(i).map(String::as_str).unwrap_or("");
            let s = seed.wrapping_add((i as u64) << 8);
            Culture {
                polity: p.name.clone(),
                ethos: format!("{}, {}", biome_ethos(biome), pick(TEMPER, h(s, 1))),
                belief: pick(BELIEF, h(s, 2)).to_string(),
                language_profile: format!(
                    "{} · {} · {}",
                    pick(ORDER, h(s, 3)),
                    pick(MORPH, h(s, 4)),
                    pick(SOUND, h(s, 5))
                ),
                naming_sample: format!("{}{}", pick(ONSET, h(s, 6)), pick(TAIL, h(s, 7))),
            }
        })
        .collect();
    CultureOutput { cultures }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::polities_layer::Polity;

    fn pol(names: &[&str]) -> PolitiesOutput {
        PolitiesOutput {
            polities: names
                .iter()
                .map(|n| Polity {
                    name: (*n).into(),
                    capital: "the capital".into(),
                    capital_pos: (0, 0),
                    member_count: 1,
                    population: 1000,
                })
                .collect(),
            relations: Vec::new(),
        }
    }

    #[test]
    fn one_culture_per_polity_with_all_facets() {
        let p = pol(&["Karon", "Serai"]);
        let biomes = vec!["hot_desert".to_string(), "temperate_forest".to_string()];
        let c = compile_culture(&p, &biomes, 0x33);
        assert_eq!(c.cultures.len(), 2);
        // Ethos reflects the capital biome.
        assert!(c.cultures[0].ethos.starts_with("austere and hospitable"));
        assert!(c.cultures[1].ethos.starts_with("woodland-reverent"));
        for cu in &c.cultures {
            assert!(!cu.belief.is_empty());
            assert!(cu.language_profile.contains('·'));
            assert!(!cu.naming_sample.is_empty());
        }
    }

    #[test]
    fn is_deterministic() {
        let p = pol(&["Karon"]);
        let b = vec!["savanna".to_string()];
        assert_eq!(compile_culture(&p, &b, 9), compile_culture(&p, &b, 9));
    }
}
