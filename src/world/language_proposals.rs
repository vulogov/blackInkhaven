//! WORLD-13 — the World→ConLang bridge. The culture layer already proposes a
//! *language profile* per people (`compile_culture`: word order · morphology ·
//! sound) plus a naming sample; today the author carries that across to the
//! ConLang suite (`inkhaven language`) by hand. This turns the profile into a
//! **language proposal** the author accepts, which scaffolds a language book
//! seeded with the world's brief — the automated handoff the book has always
//! described as "the World × ConLang bridge".
//!
//! Grounded: the profile and naming sample already exist; accepting only runs
//! the same `scaffold_language_chapters` as `language init` and records the
//! brief. It does not invent a phonology or a lexicon — that stays the author's
//! craft in the ConLang suite.

use crate::world::compile::culture_layer::CultureOutput;
use crate::world::compile::polities_layer::PolitiesOutput;
use crate::world::proposals::{now_secs, PlaceProposal};

/// A stable slug for a realm name, for the proposal signature (dedup key).
fn realm_slug(name: &str) -> String {
    crate::world::proposals::stable_slug(name, '-')
}

/// Split a `"SVO · agglutinative · tonal"` profile into its three axes
/// (word order, morphology, sound). Missing axes come back empty.
fn split_profile(profile: &str) -> (String, String, String) {
    let mut parts = profile.split('·').map(|p| p.trim().to_string());
    (parts.next().unwrap_or_default(), parts.next().unwrap_or_default(), parts.next().unwrap_or_default())
}

/// One language proposal per polity whose culture carries a profile. The
/// language is named for its people (the Karon speak Karon); the payload keeps
/// the profile axes + naming sample for the accept-time brief.
pub fn language_proposals(pol: &PolitiesOutput, cultures: &CultureOutput, _seed: u64) -> Vec<PlaceProposal> {
    let now = now_secs();
    let mut out = Vec::new();
    for (i, p) in pol.polities.iter().enumerate() {
        let Some(c) = cultures.cultures.get(i) else { continue };
        if c.language_profile.trim().is_empty() {
            continue;
        }
        let (order, morph, sound) = split_profile(&c.language_profile);
        let payload = serde_json::json!({
            "realm": p.name,
            "profile": c.language_profile,
            "word_order": order,
            "morphology": morph,
            "sound": sound,
            "naming_sample": c.naming_sample,
        });
        let rationale = format!(
            "The tongue of {} — {} · sample “{}”.",
            p.name, c.language_profile, c.naming_sample,
        );
        out.push(PlaceProposal {
            id: uuid::Uuid::new_v4(),
            signature: format!("language:{}", realm_slug(&p.name)),
            kind: "language".into(),
            name: p.name.clone(),
            payload,
            rationale,
            status: "pending".into(),
            created_at: now,
        });
    }
    out
}

/// The design-brief body seeded into the new language's `Meta` chapter — the
/// world's profile, written where the author will build the real language.
pub fn language_brief_body(p: &PlaceProposal) -> String {
    let s = |k: &str| p.payload.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    format!(
        "The world proposes this tongue for {}. It is a *profile*, not a finished \
         language — realise it in the ConLang suite (Phonology, Grammar, Dictionary).\n\n\
         Word order: {}\nMorphology: {}\nSound: {}\nNaming sample: {}\n\n\
         // World × ConLang bridge — from `realworld propose-language`\n",
        s("realm"),
        if s("word_order").is_empty() { "—".into() } else { s("word_order") },
        if s("morphology").is_empty() { "—".into() } else { s("morphology") },
        if s("sound").is_empty() { "—".into() } else { s("sound") },
        if s("naming_sample").is_empty() { "—".into() } else { s("naming_sample") },
    )
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
                    member_count: 1,
                    population: 1000,
                })
                .collect(),
            relations: Vec::new(),
        }
    }

    fn cultures(profiles: &[(&str, &str, &str)]) -> CultureOutput {
        CultureOutput {
            cultures: profiles
                .iter()
                .map(|(realm, prof, sample)| Culture {
                    polity: (*realm).into(),
                    ethos: "settled".into(),
                    belief: "a sky-pantheon".into(),
                    language_profile: (*prof).into(),
                    naming_sample: (*sample).into(),
                })
                .collect(),
        }
    }

    #[test]
    fn one_language_per_culture_named_for_its_people() {
        let props = language_proposals(
            &pol(&["Karon", "Serai"]),
            &cultures(&[("Karon", "SVO · fusional · tonal", "Kaeth"), ("Serai", "SOV · agglutinative · liquid", "Seren")]),
            7,
        );
        assert_eq!(props.len(), 2);
        assert_eq!(props[0].kind, "language");
        assert_eq!(props[0].name, "Karon");
        assert_eq!(props[0].signature, "language:karon");
        // The brief keeps the three axes + the sample.
        let brief = language_brief_body(&props[0]);
        assert!(brief.contains("SVO"));
        assert!(brief.contains("fusional"));
        assert!(brief.contains("tonal"));
        assert!(brief.contains("Kaeth"));
    }

    #[test]
    fn a_culture_without_a_profile_is_skipped() {
        let props = language_proposals(&pol(&["Mute"]), &cultures(&[("Mute", "", "")]), 1);
        assert!(props.is_empty());
    }

    #[test]
    fn is_deterministic() {
        let a = language_proposals(&pol(&["Karon"]), &cultures(&[("Karon", "VSO · isolating · guttural", "Kor")]), 9);
        let b = language_proposals(&pol(&["Karon"]), &cultures(&[("Karon", "VSO · isolating · guttural", "Kor")]), 9);
        assert_eq!(language_brief_body(&a[0]), language_brief_body(&b[0]));
        assert_eq!(a[0].signature, b[0].signature);
    }
}
