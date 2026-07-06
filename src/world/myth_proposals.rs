//! WORLD-12 (MYTH-2) — the World→Mythology bridge. The culture layer already
//! proposes a *belief* per people (`compile_culture`); this turns those beliefs
//! into **Mythology proposals** — a `para:myth-symbol` or `para:myth-motif`
//! entry the author accepts into the Mythology book, exactly like Place
//! proposals. Deterministic (a fixed belief→seed table + a generic fallback):
//! pure function of the compiled cultures + seed, so `realworld variants` stays
//! reproducible and no model call touches the compile path.
//!
//! The `traditions` field on each proposed symbol carries the peoples who hold
//! it — the cross-reference back to the world that generated it. Nothing commits
//! until the author accepts (the authority-discipline spine of the RFC).

use std::collections::BTreeMap;

use crate::world::compile::culture_layer::CultureOutput;
use crate::world::proposals::{now_secs, PlaceProposal};

/// A seeded Mythology entry: either a symbol (a named vocabulary with a meaning)
/// or a motif (a recurring pattern). `valence` is one of the `MythValence` codes.
struct MythSeed {
    /// `"symbol"` or `"motif"` — selects the target `para:myth-*` tag + block.
    kind: &'static str,
    /// Symbol vocabulary (the words that carry the meaning). Empty for a motif.
    vocabulary: Vec<&'static str>,
    /// Symbol meaning / motif description.
    gloss: &'static str,
    /// Motif name. Empty for a symbol.
    name: &'static str,
    valence: &'static str,
}

/// Map a culture's declared belief to a Mythology seed. The six generated
/// beliefs (from `compile_culture`'s `BELIEF` table) get a curated seed; an
/// author-declared belief the table doesn't know falls through to a generic
/// symbol whose vocabulary is drawn from the belief's own words.
fn belief_to_seed(belief: &str) -> Option<MythSeed> {
    let key = belief.trim().to_lowercase();
    if key.is_empty() {
        return None;
    }
    let seed = match key.as_str() {
        "ancestor veneration" => MythSeed {
            kind: "symbol",
            vocabulary: vec!["ancestor", "ancestors", "forebears", "the dead"],
            gloss: "the forebears watch, guide, and judge the living",
            name: "",
            valence: "positive",
        },
        "a sky-pantheon" => MythSeed {
            kind: "symbol",
            vocabulary: vec!["sky", "sun", "storm", "thunder", "the heavens"],
            gloss: "the high gods who rule from the sky",
            name: "",
            valence: "positive",
        },
        "one hidden god" => MythSeed {
            kind: "symbol",
            vocabulary: vec!["the hidden", "the unseen", "the one"],
            gloss: "a single god who withholds its face",
            name: "",
            valence: "ambiguous",
        },
        "nature spirits of river and stone" => MythSeed {
            kind: "symbol",
            vocabulary: vec!["river", "stone", "spring", "grove", "the old places"],
            gloss: "the small gods bound to features of the land",
            name: "",
            valence: "ambiguous",
        },
        "a cult of the seasons" => MythSeed {
            kind: "motif",
            vocabulary: vec![],
            gloss: "rites and reversals bound to the turning of the seasons",
            name: "the turning year",
            valence: "ambiguous",
        },
        "reverence for the founding dead" => MythSeed {
            kind: "motif",
            vocabulary: vec![],
            gloss: "the first ancestors whose choices still bind the living",
            name: "the founder's shadow",
            valence: "positive",
        },
        _ => return None, // declared / unknown belief → generic fallback
    };
    Some(seed)
}

/// A stable, deterministic slug for a belief, for the proposal signature. Keeps
/// re-`propose-myth` from re-proposing a belief the author already accepted.
fn belief_slug(belief: &str) -> String {
    let mut s = String::new();
    for c in belief.trim().to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            s.push(c);
        } else if !s.ends_with('-') {
            s.push('-');
        }
    }
    s.trim_matches('-').to_string()
}

/// Words from a declared belief worth using as a symbol vocabulary: lowercased,
/// de-articled, length > 3, in order, de-duplicated, capped at four.
fn belief_vocabulary(belief: &str) -> Vec<String> {
    const STOP: &[&str] = &["the", "and", "with", "from", "into", "that", "this", "their", "a", "an", "of", "for"];
    let mut out: Vec<String> = Vec::new();
    for raw in belief.split(|c: char| !c.is_ascii_alphanumeric()) {
        let w = raw.to_lowercase();
        if w.len() > 3 && !STOP.contains(&w.as_str()) && !out.contains(&w) {
            out.push(w);
        }
        if out.len() == 4 {
            break;
        }
    }
    out
}

/// Turn the compiled cultures into Mythology proposals — one per *distinct*
/// belief, with the peoples who share it gathered into `traditions`. Merging by
/// belief means two cultures that both revere a sky-pantheon yield a single
/// symbol the author declares once, credited to both.
pub fn myth_proposals(cultures: &CultureOutput, _seed: u64) -> Vec<PlaceProposal> {
    // Group culture (polity) names by their belief, preserving first-seen order.
    let mut order: Vec<String> = Vec::new();
    let mut by_belief: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for c in &cultures.cultures {
        let belief = c.belief.trim().to_string();
        if belief.is_empty() {
            continue;
        }
        if !by_belief.contains_key(&belief) {
            order.push(belief.clone());
        }
        by_belief.entry(belief).or_default().push(c.polity.clone());
    }

    let now = now_secs();
    let mut out = Vec::new();
    for belief in &order {
        let peoples = by_belief.get(belief).cloned().unwrap_or_default();
        let (kind, vocabulary, gloss, name, valence) = match belief_to_seed(belief) {
            Some(s) => (
                s.kind,
                s.vocabulary.iter().map(|v| v.to_string()).collect::<Vec<_>>(),
                s.gloss.to_string(),
                s.name.to_string(),
                s.valence.to_string(),
            ),
            None => {
                // Declared / unknown belief → a generic symbol keyed to its own words.
                let vocab = belief_vocabulary(belief);
                (
                    "symbol",
                    if vocab.is_empty() { vec![belief.to_lowercase()] } else { vocab },
                    format!("what these peoples hold sacred: {belief}"),
                    String::new(),
                    "ambiguous".to_string(),
                )
            }
        };

        let target_tag = if kind == "motif" { "para:myth-motif" } else { "para:myth-symbol" };
        let display = if kind == "motif" { name.clone() } else { vocabulary.first().cloned().unwrap_or_default() };
        let rationale = format!(
            "{} held by {} — a {} for the Mythology book.",
            belief,
            peoples.join(", "),
            if kind == "motif" { "motif" } else { "symbol" },
        );
        let payload = serde_json::json!({
            "myth_kind": kind,
            "tag": target_tag,
            "vocabulary": vocabulary,
            "gloss": gloss,
            "name": name,
            "valence": valence,
            "traditions": peoples,
            "belief": belief,
        });
        out.push(PlaceProposal {
            id: uuid::Uuid::new_v4(),
            signature: format!("myth:{}", belief_slug(belief)),
            kind: format!("myth-{kind}"),
            name: if display.is_empty() { belief.clone() } else { display },
            payload,
            rationale,
            status: "pending".into(),
            created_at: now,
        });
    }
    out
}

/// Render a proposal's payload as the HJSON body of a `para:myth-*` paragraph —
/// the exact block `src/myth/parse.rs` reads back. Emits strict JSON, which the
/// HJSON parser accepts, so quoting is always correct.
pub fn myth_entry_body(p: &PlaceProposal) -> String {
    let get_str = |k: &str| p.payload.get(k).and_then(|v| v.as_str()).unwrap_or("").to_string();
    let get_vec = |k: &str| {
        p.payload
            .get(k)
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect::<Vec<_>>())
            .unwrap_or_default()
    };
    let valence = get_str("valence");
    let block = if get_str("myth_kind") == "motif" {
        let desc = {
            let peoples = get_vec("traditions");
            let base = get_str("gloss");
            if peoples.is_empty() { base } else { format!("{base} — held by {}", peoples.join(", ")) }
        };
        serde_json::json!({
            "myth_motif": { "name": get_str("name"), "description": desc, "valence": valence }
        })
    } else {
        serde_json::json!({
            "myth_symbol": {
                "vocabulary": get_vec("vocabulary"),
                "meaning": get_str("gloss"),
                "valence": valence,
                "traditions": get_vec("traditions"),
            }
        })
    };
    serde_json::to_string_pretty(&block).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::compile::culture_layer::Culture;

    fn culture(polity: &str, belief: &str) -> Culture {
        Culture {
            polity: polity.into(),
            ethos: "settled and pragmatic".into(),
            belief: belief.into(),
            language_profile: "SVO · fusional · liquid".into(),
            naming_sample: "Kaeth".into(),
        }
    }

    #[test]
    fn known_belief_maps_to_a_curated_symbol() {
        let cul = CultureOutput { cultures: vec![culture("Karon", "a sky-pantheon")] };
        let props = myth_proposals(&cul, 7);
        assert_eq!(props.len(), 1);
        assert_eq!(props[0].kind, "myth-symbol");
        assert_eq!(props[0].signature, "myth:a-sky-pantheon");
        // The body is a parseable myth_symbol block crediting the people.
        let body = myth_entry_body(&props[0]);
        let s = crate::myth::parse_symbol_block_for_test("p", &body).unwrap();
        assert!(s.vocabulary.iter().any(|v| v == "sky"));
        assert_eq!(s.traditions, vec!["Karon"]);
    }

    #[test]
    fn seasonal_belief_maps_to_a_motif() {
        let cul = CultureOutput { cultures: vec![culture("Serai", "a cult of the seasons")] };
        let props = myth_proposals(&cul, 1);
        assert_eq!(props[0].kind, "myth-motif");
        let body = myth_entry_body(&props[0]);
        let m = crate::myth::parse_motif_block_for_test("p", &body).unwrap();
        assert_eq!(m.name, "the turning year");
        assert!(m.description.contains("Serai")); // peoples folded into the description
    }

    #[test]
    fn cultures_sharing_a_belief_merge_into_one_proposal() {
        let cul = CultureOutput {
            cultures: vec![
                culture("Karon", "a sky-pantheon"),
                culture("Serai", "a sky-pantheon"),
            ],
        };
        let props = myth_proposals(&cul, 3);
        assert_eq!(props.len(), 1, "one symbol, credited to both peoples");
        let body = myth_entry_body(&props[0]);
        let s = crate::myth::parse_symbol_block_for_test("p", &body).unwrap();
        assert_eq!(s.traditions, vec!["Karon", "Serai"]);
    }

    #[test]
    fn declared_belief_falls_back_to_a_generic_symbol() {
        let cul = CultureOutput { cultures: vec![culture("Vael", "the tide-mother and her drowned choir")] };
        let props = myth_proposals(&cul, 5);
        assert_eq!(props[0].kind, "myth-symbol");
        let body = myth_entry_body(&props[0]);
        let s = crate::myth::parse_symbol_block_for_test("p", &body).unwrap();
        // Vocabulary drawn from the belief's own words (de-articled, len > 3).
        assert!(s.vocabulary.iter().any(|v| v == "tide" || v == "mother" || v == "drowned" || v == "choir"));
    }

    #[test]
    fn is_deterministic() {
        let cul = CultureOutput { cultures: vec![culture("Karon", "one hidden god")] };
        let a = myth_proposals(&cul, 9);
        let b = myth_proposals(&cul, 9);
        assert_eq!(myth_entry_body(&a[0]), myth_entry_body(&b[0]));
        assert_eq!(a[0].signature, b[0].signature);
    }
}
