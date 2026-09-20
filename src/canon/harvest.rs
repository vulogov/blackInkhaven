//! CL-P2 — deterministic harvest of authored annotations into canon units.
//!
//! No LLM: this reads the **author's own** structured tags on a paragraph and
//! turns each into a canon decision. Untagged paragraphs harvest to nothing, so
//! running it on every save costs ~nothing and never floods the ledger — the
//! author's tags are the opt-in. LLM extraction from prose is a separate,
//! opt-in phase (CL-P7).
//!
//! CL-P2 ships one producer — `rel:<kind>:<A>:<B>` relationship tags (reusing
//! [`crate::bonds::parse_rel_tag`]) → [`NarrativeKind::CharacterTrait`] decisions.
//! The `secret:` / `know:` (KEN) and declared world-fact producers slot in here
//! the same way later.

use super::NarrativeKind;

/// Run the deterministic producers over a paragraph's authored `tags`, returning
/// `(kind, gist)` for each canon decision found. Pure and order-preserving; the
/// caller attaches the node source-reference and appends to the ledger.
pub fn harvest_tags(tags: &[String]) -> Vec<(NarrativeKind, String)> {
    let mut out = Vec::new();
    for tag in tags {
        // `rel:<kind>:<A>:<B>` — a canonical relationship between two characters.
        if let Some((kind, a, b)) = crate::bonds::parse_rel_tag(tag) {
            out.push((NarrativeKind::CharacterTrait, format!("{a} — {kind} — {b}")));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rel_tags_become_character_trait_decisions() {
        let tags = vec![
            "rel:friend:Alice:Bob".to_string(),
            "pov:Alice".to_string(),      // not a rel tag → ignored
            "rel:mentor:Bob:Cara".to_string(),
            "rel:malformed".to_string(),  // malformed → ignored
        ];
        let got = harvest_tags(&tags);
        assert_eq!(got.len(), 2, "only well-formed rel: tags harvest");
        assert_eq!(got[0].0, NarrativeKind::CharacterTrait);
        // parse_rel_tag preserves the author's character names and carries the kind.
        assert!(
            got[0].1.contains("Alice") && got[0].1.contains("friend") && got[0].1.contains("Bob"),
            "gist names both characters and the relationship: {:?}",
            got[0].1
        );
        assert!(got[1].1.contains("mentor"), "second gist carries its kind: {:?}", got[1].1);
    }

    #[test]
    fn untagged_or_irrelevant_tags_harvest_to_nothing() {
        assert!(harvest_tags(&[]).is_empty());
        assert!(harvest_tags(&["pov:Alice".into(), "bookmark".into()]).is_empty());
    }
}
