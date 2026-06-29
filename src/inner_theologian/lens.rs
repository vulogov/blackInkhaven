//! INNER-THEOLOGIAN-1 (IT-P5) — tradition-lens selection hints (RFC §9.1).
//!
//! The final lens selection is the LLM's, made from what is present in the
//! passage. This module gives the prompt a *deterministic head start*: it scans
//! the passage for the markers in the §9.1 table and suggests the lenses those
//! markers make most revealing. The persona is always explicit about which lens
//! raises which question and always invites the author to say a lens is
//! irrelevant — so a wrong hint costs nothing.

use super::TraditionLens;

/// One row of the §9.1 signal→lens table: lowercased English markers that, when
/// present, make `lenses` more illuminating.
struct LensSignal {
    markers: &'static [&'static str],
    lenses: &'static [TraditionLens],
}

use TraditionLens::*;

const LENS_SIGNALS: &[LensSignal] = &[
    // Hidden/revealed reality structure
    LensSignal { markers: &["veil", "illusion", "awaken", "true reality", "hidden world", "the real world"], lenses: &[Gnostic, Buddhism, SecularPhilosophy] },
    // Callings from before the story
    LensSignal { markers: &["destiny", "prophecy", "chosen", "foretold", "born to", "calling"], lenses: &[Lds, Gnostic, Hinduism] },
    // Suffering without function
    LensSignal { markers: &["suffering", "agony", "torment", "innocent", "undeserved", "why me"], lenses: &[Judaism, Buddhism, Catholic] },
    // Familial salvation across generations
    LensSignal { markers: &["family", "ancestor", "generations", "bloodline", "covenant", "lineage"], lenses: &[Lds, Judaism, Confucianism] },
    // Embodied divine / semi-divine
    LensSignal { markers: &["god", "goddess", "divine", "avatar", "incarnate", "demigod"], lenses: &[Lds, Hinduism, SecularPhilosophy] },
    // Self-sacrifice by a central character
    LensSignal { markers: &["sacrifice", "gave his life", "gave her life", "laid down", "for the others"], lenses: &[Orthodox, Catholic, Protestant] },
    // Moral awakening / knowledge-as-salvation
    LensSignal { markers: &["awakening", "realised the truth", "enlighten", "saw clearly", "knowledge"], lenses: &[Gnostic, Buddhism, SecularPhilosophy] },
    // Institutional power and its corruption
    LensSignal { markers: &["empire", "the council", "the church", "authority", "corrupt", "regime"], lenses: &[Gnostic, Islam, Confucianism] },
    // Relational obligations broken or honoured
    LensSignal { markers: &["promise", "duty", "oath", "betrayed", "obligation", "owed"], lenses: &[Confucianism, Judaism, SecularPhilosophy] },
    // Violence in service of duty
    LensSignal { markers: &["war", "battle", "kill", "for duty", "had to", "command"], lenses: &[Hinduism, Islam, SecularPhilosophy] },
];

/// Marker-suggested lenses for a passage, in priority order, de-duplicated.
/// These are a *soft hint* — the prompt presents all eleven lenses as the menu
/// and the persona chooses; an empty result simply means "no markers fired, pick
/// from the whole set." (No default trio: forcing the same three every time made
/// the other eight traditions look ignored.)
pub(crate) fn suggest_lenses(passage: &str) -> Vec<TraditionLens> {
    let lc = passage.to_lowercase();
    let mut out: Vec<TraditionLens> = Vec::new();
    for sig in LENS_SIGNALS {
        if sig.markers.iter().any(|m| marker_hit(&lc, m)) {
            for &lens in sig.lenses {
                if !out.contains(&lens) {
                    out.push(lens);
                }
            }
        }
    }
    out.truncate(5);
    out
}

/// A marker matches: multi-word markers as substrings, single tokens only on a
/// whole-word boundary (so "war" doesn't fire on "warm").
fn marker_hit(lc: &str, marker: &str) -> bool {
    if marker.contains(' ') {
        lc.contains(marker)
    } else {
        lc.split(|c: char| !c.is_alphanumeric()).any(|tok| tok == marker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_suggest_their_lenses() {
        let s = suggest_lenses("She awakened and saw the veil over the true reality.");
        assert!(s.contains(&Gnostic));
        let d = suggest_lenses("He gave his life as a sacrifice for the others.");
        assert!(d.contains(&Orthodox));
    }

    #[test]
    fn no_marker_yields_empty_so_persona_picks_from_all() {
        let s = suggest_lenses("The afternoon was warm and the tea had gone cold.");
        assert!(s.is_empty());
    }

    #[test]
    fn dedups_and_caps_at_five() {
        // A passage hitting many rows still returns at most five distinct lenses.
        let s = suggest_lenses(
            "war and sacrifice and prophecy and family and the corrupt empire and a broken promise",
        );
        assert!(s.len() <= 5);
        let mut uniq = s.clone();
        uniq.dedup();
        assert_eq!(uniq.len(), s.len());
    }
}
