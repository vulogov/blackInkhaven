//! WORLD-4 P2 — the proposal queue. Layer 5 is the first compiler layer that
//! *proposes* rather than asserts: its settlements become **Place proposals**
//! the author accepts, edits, or rejects. Nothing commits without acceptance
//! (the authority-discipline spine of the RFC).
//!
//! A proposal carries a stable `signature` (derived from what it proposes, e.g.
//! `place:60:69`) so re-running the compiler doesn't re-propose something the
//! author already accepted or rejected.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::world::types::DemographicsOutput;

/// A compiler-proposed record awaiting the author's decision.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct PlaceProposal {
    pub id: Uuid,
    /// Stable identity for dedup across re-compiles (e.g. `place:60:69`).
    pub signature: String,
    /// The target system book — `"place"` for now.
    pub kind: String,
    pub name: String,
    /// The proposed record content.
    pub payload: serde_json::Value,
    /// Why the compiler proposed this (shown to the author).
    pub rationale: String,
    /// `"pending"` | `"accepted"` | `"rejected"`.
    pub status: String,
    pub created_at: i64,
}

/// Generate Place proposals from a demographics output. The settlement grid
/// coordinates make each proposal's `signature` stable across re-compiles.
pub fn place_proposals(demo: &DemographicsOutput, seed: u64) -> Vec<PlaceProposal> {
    let now = now_secs();
    demo.settlements
        .iter()
        .map(|s| {
            let name = settlement_name(seed, s.x, s.y);
            let payload = serde_json::json!({
                "name": name,
                "x": s.x,
                "y": s.y,
                "population": s.population,
                "class": s.class,
                "basis": s.basis,
                "biome": s.biome,
            });
            let rationale = format!(
                "A {} of ~{} at a {} in a {} zone.",
                s.class,
                fmt_pop(s.population),
                s.basis.replace('_', " "),
                s.biome.replace('_', " "),
            );
            PlaceProposal {
                id: Uuid::new_v4(),
                signature: format!("place:{}:{}", s.x, s.y),
                kind: "place".into(),
                name,
                payload,
                rationale,
                status: "pending".into(),
                created_at: now,
            }
        })
        .collect()
}

fn fmt_pop(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 10_000 {
        format!("{:.0}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

/// Deterministic, pronounceable settlement name from `(seed, x, y)`. Not meant
/// to be linguistically tied to any conlang — the author renames freely (and a
/// later phase can AI-elaborate names in the manuscript language).
pub fn settlement_name(seed: u64, x: usize, y: usize) -> String {
    const ONSET: &[&str] =
        &["V", "K", "M", "T", "S", "L", "R", "N", "Th", "Br", "Dr", "El", "Ar", "Or", "Vel", "Kor"];
    const NUCLEUS: &[&str] = &["a", "e", "i", "o", "u", "ae", "ia", "ar", "en"];
    const CODA: &[&str] = &["n", "r", "l", "s", "th", "m", "", "k", "ndor", "mar"];

    let mut rng =
        SplitMix64::new(seed ^ (x as u64).wrapping_mul(73856093) ^ (y as u64).wrapping_mul(19349663));
    let syllables = 2 + (rng.next_u64() % 2) as usize; // 2 or 3
    let mut name = String::new();
    for s in 0..syllables {
        name.push_str(ONSET[(rng.next_u64() as usize) % ONSET.len()]);
        name.push_str(NUCLEUS[(rng.next_u64() as usize) % NUCLEUS.len()]);
        // Only the last syllable may take a coda (keeps names pronounceable).
        if s + 1 == syllables {
            name.push_str(CODA[(rng.next_u64() as usize) % CODA.len()]);
        }
    }
    // Lower-case the tail so e.g. "VelKor" → "Velkor".
    let mut chars = name.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
        None => name,
    }
}

pub(crate) fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

struct SplitMix64(u64);
impl SplitMix64 {
    fn new(seed: u64) -> Self {
        SplitMix64(seed)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_deterministic_and_vary_by_position() {
        assert_eq!(settlement_name(42, 60, 69), settlement_name(42, 60, 69));
        assert_ne!(settlement_name(42, 60, 69), settlement_name(42, 61, 69));
        // Capitalised, non-empty, alphabetic.
        let n = settlement_name(42, 60, 69);
        assert!(n.chars().next().unwrap().is_uppercase());
        assert!(n.len() >= 3 && n.chars().all(|c| c.is_alphabetic()));
    }
}
