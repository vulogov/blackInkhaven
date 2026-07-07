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
/// `declared` pins cultures by nation name, overriding the generated fields.
pub fn compile_culture(
    pol: &PolitiesOutput,
    capital_biomes: &[String],
    declared: &[crate::world::types::CultureDef],
    seed: u64,
) -> CultureOutput {
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
            let mut c = Culture {
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
            };
            // A declared culture for this nation overrides the generated fields.
            if let Some(d) = declared.iter().find(|d| d.nation.eq_ignore_ascii_case(&p.name)) {
                if !d.ethos.trim().is_empty() {
                    c.ethos = d.ethos.clone();
                }
                if !d.belief.trim().is_empty() {
                    c.belief = d.belief.clone();
                }
                if !d.language.trim().is_empty() {
                    c.language_profile = d.language.clone();
                }
            }
            c
        })
        .collect();
    CultureOutput { cultures }
}

/// WORLD-14 — a settlement name in a culture's phonic style. Each realm draws a
/// small onset/tail palette (seeded from its polity name), recombined by `index`,
/// so a realm's towns share a family sound and differ from another realm's. This
/// fulfils the deferred "elaborate names in the world's style" (proposals.rs) —
/// deterministic; the author adopts what rings true. (Not tied to a built conlang;
/// an accepted ConLang can later supersede these.)
pub fn culture_style_name(culture: &Culture, seed: u64, index: usize) -> String {
    const ONSET: &[&str] = &[
        "Ka", "Ser", "Tho", "Vae", "Mor", "Ilu", "Bra", "Nen", "Kor", "Ael", "Dun", "Syl", "Tor", "Vel",
    ];
    const TAIL: &[&str] =
        &["ra", "eth", "is", "un", "ora", "ai", "el", "or", "yn", "ath", "ir", "une", "wen", "dar"];
    // A per-culture base from the polity name, so each realm names distinctly.
    let base = culture.polity.bytes().fold(seed ^ 0x5EED, |a, b| h(a, b as u64));
    let on = |k: u64| ONSET[(h(base, k) as usize) % ONSET.len()];
    let ta = |k: u64| TAIL[(h(base, 100 + k) as usize) % TAIL.len()];
    let i = index as u64;
    if h(base, 900 + i) % 3 == 0 {
        // A longer, two-onset name.
        format!("{}{}{}", on(i % 3), on((i + 1) % 3).to_lowercase(), ta(i % 3))
    } else {
        format!("{}{}", on(i % 3), ta((i + 1) % 3))
    }
}

/// WORLD-15 — elaborate a generic social role (from demographics' `role_archetypes`
/// — farmer, priest, warrior, …) into a realm's own term, coloured by its ethos,
/// belief, and capital biome. Deterministic; fulfils the deferred role-elaboration
/// noted in `demographics.rs`. The world names the same roles differently in each
/// realm — a Karon `factor` where a forest people keep a `keeper of the founding
/// dead`.
pub fn elaborate_role(base: &str, culture: &Culture, biome: &str) -> String {
    let biome_word = biome.split('_').next().filter(|s| !s.is_empty()).unwrap_or("home");
    let ethos_head = culture.ethos.split([',', ' ']).find(|s| !s.is_empty()).unwrap_or("");
    let belief = culture.belief.trim();
    match base {
        "priest" => {
            if belief.is_empty() {
                "priest".into()
            } else {
                format!("keeper of {belief}")
            }
        }
        "warrior" => {
            if ethos_head.is_empty() {
                "warrior".into()
            } else {
                format!("{ethos_head} warrior")
            }
        }
        "farmer" => format!("{biome_word}-tiller"),
        "fisher" => "netcaster".into(),
        "herder" => format!("{biome_word}-herder"),
        "woodcutter" => "forester".into(),
        "smith" => "forge-master".into(),
        "merchant" => {
            if culture.ethos.contains("mercantile") {
                "factor".into()
            } else {
                "trader".into()
            }
        }
        "vintner" => "vine-keeper".into(),
        other => other.to_string(),
    }
}

/// WORLD-11 (W11-P4) — verify pinned cultures: a maritime ethos on a dry inland
/// capital, or a culture pinned to a nation that does not exist, is flagged.
pub fn lint_culture(
    declared: &[crate::world::types::CultureDef],
    pol: &PolitiesOutput,
    capital_biomes: &[String],
) -> Vec<String> {
    let mut w = Vec::new();
    for d in declared {
        match pol.polities.iter().position(|p| p.name.eq_ignore_ascii_case(&d.nation)) {
            None => w.push(format!("culture: pinned to nation `{}`, which is not among the world's realms", d.nation)),
            Some(i) => {
                let biome = capital_biomes.get(i).map(String::as_str).unwrap_or("");
                let e = d.ethos.to_lowercase();
                let maritime = ["seafaring", "maritime", "naval", "coastal", "harbour", "harbor", "sea-"]
                    .iter()
                    .any(|k| e.contains(k));
                let dry_inland = biome.contains("desert") || biome.contains("grassland") || biome == "savanna";
                if maritime && dry_inland {
                    w.push(format!(
                        "culture `{}`: a seafaring ethos, but its capital sits in a dry inland biome ({biome})",
                        d.nation
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
        let c = compile_culture(&p, &biomes, &[], 0x33);
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
        assert_eq!(compile_culture(&p, &b, &[], 9), compile_culture(&p, &b, &[], 9));
    }

    #[test]
    fn roles_elaborate_into_realm_terms() {
        let p = pol(&["Karon"]);
        let b = vec!["mediterranean".to_string()];
        let c = compile_culture(&p, &b, &[], 0x33);
        let cu = &c.cultures[0];
        // priest folds in the belief; warrior the ethos; farmer the biome.
        assert!(elaborate_role("priest", cu, "mediterranean").starts_with("keeper of "));
        assert!(elaborate_role("warrior", cu, "mediterranean").ends_with(" warrior"));
        assert_eq!(elaborate_role("farmer", cu, "temperate_forest"), "temperate-tiller");
        assert_eq!(elaborate_role("smith", cu, "mediterranean"), "forge-master");
        // A mercantile culture's merchant is a factor.
        let merc = Culture {
            polity: "Karon".into(),
            ethos: "mercantile and civic".into(),
            belief: "a sky-pantheon".into(),
            language_profile: String::new(),
            naming_sample: String::new(),
        };
        assert_eq!(elaborate_role("merchant", &merc, "mediterranean"), "factor");
        // An unknown role passes through unchanged.
        assert_eq!(elaborate_role("scribe", cu, "mediterranean"), "scribe");
    }

    #[test]
    fn culture_style_names_are_deterministic_and_realm_distinct() {
        let p = pol(&["Karon", "Serai"]);
        let b = vec!["mediterranean".to_string(), "temperate_forest".to_string()];
        let c = compile_culture(&p, &b, &[], 0x33);
        let (k0, s0) = (&c.cultures[0], &c.cultures[1]);
        // Deterministic.
        assert_eq!(culture_style_name(k0, 7, 3), culture_style_name(k0, 7, 3));
        // Non-empty, capitalised, alphabetic.
        let n = culture_style_name(k0, 7, 0);
        assert!(n.chars().next().unwrap().is_uppercase() && n.chars().all(|ch| ch.is_alphabetic()));
        // Two realms with the same seed+index usually name differently (distinct palettes).
        let differ = (0..8).filter(|&i| culture_style_name(k0, 7, i) != culture_style_name(s0, 7, i)).count();
        assert!(differ >= 5, "realms name too alike: {differ}/8 differ");
    }

    #[test]
    fn declared_culture_overrides_and_maritime_desert_is_flagged() {
        use crate::world::types::CultureDef;
        let p = pol(&["Karon"]);
        let b = vec!["hot_desert".to_string()];
        let declared = vec![CultureDef {
            nation: "Karon".into(),
            ethos: "seafaring and bold".into(),
            belief: "the tide-mother".into(),
            language: "VSO · fusional · tonal".into(),
        }];
        let c = compile_culture(&p, &b, &declared, 1);
        assert_eq!(c.cultures[0].ethos, "seafaring and bold");
        assert_eq!(c.cultures[0].belief, "the tide-mother");
        assert_eq!(c.cultures[0].language_profile, "VSO · fusional · tonal");
        // A seafaring people whose capital is a hot desert is flagged.
        assert!(lint_culture(&declared, &p, &b).iter().any(|s| s.contains("seafaring")));
    }
}
