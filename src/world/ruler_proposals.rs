//! WORLD-12 — the World→Characters bridge. Every polity implies a leader; this
//! proposes one **ruler stub per polity** as a Character the author accepts,
//! named in the same procedural style the world already uses for settlements
//! (`settlement_name`) — a pronounceable placeholder the author renames freely.
//!
//! Grounded, not invented: the person is entailed by the realm existing, and the
//! only generated datum (the name) is minted by the same discipline as place
//! names. The stub carries the realm, capital, population, and the culture's
//! ethos + belief so the accepted Character starts life rooted in its people.
//! The world models no individuals beyond this — deeper cast is the author's.

use crate::world::compile::culture_layer::CultureOutput;
use crate::world::compile::polities_layer::PolitiesOutput;
use crate::world::proposals::{now_secs, settlement_name, PlaceProposal};

/// A stable slug for a realm name, for the proposal signature (dedup key).
fn realm_slug(name: &str) -> String {
    crate::world::proposals::stable_slug(name, '-')
}

/// Mint a ruler's name in the world's naming style. Reuses `settlement_name`
/// (the world's pronounceable-name generator) with a per-polity ruler salt so
/// each realm's ruler is distinct and the whole set is deterministic.
fn ruler_name(seed: u64, i: usize) -> String {
    const RULER_SALT: u64 = 0x52_55_4C_45_52; // "RULER"
    settlement_name(seed ^ RULER_SALT, i, i.wrapping_add(1))
}

/// One Character proposal per polity — the realm's ruler, named in style and
/// rooted in the culture. `cultures.cultures[i]` is parallel to `pol.polities[i]`.
pub fn ruler_proposals(pol: &PolitiesOutput, cultures: &CultureOutput, seed: u64) -> Vec<PlaceProposal> {
    let now = now_secs();
    pol.polities
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let culture = cultures.cultures.get(i);
            let ethos = culture.map(|c| c.ethos.clone()).unwrap_or_default();
            let belief = culture.map(|c| c.belief.clone()).unwrap_or_default();
            let name = ruler_name(seed, i);
            let payload = serde_json::json!({
                "realm": p.name,
                "capital": p.capital,
                "population": p.population,
                "ethos": ethos,
                "belief": belief,
            });
            let rationale = format!(
                "The ruler of {} — a people who are {} and believe in {}.",
                p.name,
                if ethos.is_empty() { "settled" } else { &ethos },
                if belief.is_empty() { "their own ways" } else { &belief },
            );
            PlaceProposal {
                id: uuid::Uuid::new_v4(),
                signature: format!("character:ruler:{}", realm_slug(&p.name)),
                kind: "character".into(),
                name,
                payload,
                rationale,
                status: "pending".into(),
                created_at: now,
            }
        })
        .collect()
}

/// The prose body of an accepted ruler Character — a factual stub the author
/// fleshes out. No invented arc or backstory: only what the world entails.
pub fn ruler_body(p: &PlaceProposal) -> String {
    let s = |k: &str| p.payload.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let pop = p.payload.get("population").and_then(|v| v.as_u64()).unwrap_or(0);
    let realm = s("realm");
    let capital = s("capital").replace('_', " ");
    let ethos = s("ethos");
    let belief = s("belief");
    let mut body = format!("{} rules {realm}", p.name);
    if !capital.trim().is_empty() {
        body.push_str(&format!(", from {capital}"));
    }
    body.push_str(&format!(", a realm of roughly {pop} people.\n\n"));
    if !ethos.is_empty() || !belief.is_empty() {
        body.push_str(&format!(
            "Their people are {}, and they hold to {}.\n\n",
            if ethos.is_empty() { "settled and pragmatic" } else { &ethos },
            if belief.is_empty() { "their own ways" } else { &belief },
        ));
    }
    body.push_str(&format!("// world-compiler proposal {}\n", p.signature));
    body
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::culture_layer::Culture;
    use crate::world::compile::polities_layer::Polity;

    fn pol(names: &[&str]) -> PolitiesOutput {
        PolitiesOutput {
            polities: names
                .iter()
                .map(|n| Polity {
                    name: (*n).into(),
                    capital: "the capital".into(),
                    capital_pos: (0, 0),
                    member_count: 2,
                    population: 42_000,
                })
                .collect(),
            relations: Vec::new(),
        }
    }

    fn cultures(realms: &[&str]) -> CultureOutput {
        CultureOutput {
            cultures: realms
                .iter()
                .map(|n| Culture {
                    polity: (*n).into(),
                    ethos: "mercantile and civic".into(),
                    belief: "a sky-pantheon".into(),
                    language_profile: "SVO · fusional · liquid".into(),
                    naming_sample: "Kaeth".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn one_ruler_per_polity_named_in_style() {
        let props = ruler_proposals(&pol(&["Karon", "Serai"]), &cultures(&["Karon", "Serai"]), 7);
        assert_eq!(props.len(), 2);
        for p in &props {
            assert_eq!(p.kind, "character");
            assert!(p.signature.starts_with("character:ruler:"));
            assert!(!p.name.is_empty() && p.name.chars().next().unwrap().is_uppercase());
        }
        // Distinct rulers, distinct signatures.
        assert_ne!(props[0].name, props[1].name);
        assert_ne!(props[0].signature, props[1].signature);
    }

    #[test]
    fn body_is_grounded_in_the_realm_and_culture() {
        let props = ruler_proposals(&pol(&["Karon"]), &cultures(&["Karon"]), 3);
        let body = ruler_body(&props[0]);
        assert!(body.contains("rules Karon"));
        assert!(body.contains("mercantile and civic"));
        assert!(body.contains("a sky-pantheon"));
        assert!(body.contains("world-compiler proposal character:ruler:karon"));
    }

    #[test]
    fn is_deterministic() {
        let a = ruler_proposals(&pol(&["Karon"]), &cultures(&["Karon"]), 9);
        let b = ruler_proposals(&pol(&["Karon"]), &cultures(&["Karon"]), 9);
        assert_eq!(a[0].name, b[0].name);
        assert_eq!(ruler_body(&a[0]), ruler_body(&b[0]));
    }
}
