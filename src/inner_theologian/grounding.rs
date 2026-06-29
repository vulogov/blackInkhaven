//! INNER-THEOLOGIAN-1 (IT-P6) — the grounding pipeline. Before a slow-track
//! session opens, three optional sources are read and composed into a prose
//! prefix (prepended to the session prompt, IT-P5). Each source degrades
//! cleanly: a missing feature / empty store contributes nothing. When all are
//! empty the caller falls back to a Category-6 session (the broadest start).

use std::path::Path;

use super::TheologianFinding;

/// Compose the grounding prefix, or `None` when nothing grounds the session.
pub(crate) fn build_grounding(
    project_root: &Path,
    book_slug: &str,
    scope_findings: &[TheologianFinding],
) -> Option<String> {
    let mut parts: Vec<String> = Vec::new();

    // Source 1 — WORLD-6 theological findings (the reserved domain; usually empty
    // today, but read so it lights up the moment something writes there).
    if let Ok(us) = crate::world::utopia::UtopiaStore::open(project_root) {
        if let Ok(findings) = us.findings(book_slug, true) {
            let theo = findings
                .iter()
                .filter(|f| f.finding_domain.as_code() == "theological")
                .count();
            if theo > 0 {
                parts.push(format!(
                    "The world-coherence check has flagged {theo} tension(s) with a theological \
                     dimension; I will start there."
                ));
            }
        }
    }

    // Source 2 — CHAR-1 belief-system arcs.
    if let Ok(cs) = crate::character::CharStore::open(project_root) {
        if let Ok(decls) = cs.all_declarations(book_slug) {
            if let Some(d) = decls.iter().find(|d| {
                has_belief_vocab(&format!(
                    "{} {} {}",
                    d.arc_type.as_code(),
                    d.desired_state_start,
                    d.desired_state_end
                ))
            }) {
                parts.push(format!(
                    "Character {}'s declared arc involves a change of belief or conviction; I will ask \
                     what the prose shows enabling that transformation.",
                    d.character_name
                ));
            }
        }
    }

    // Source 3 — fast-track signals already open in scope.
    if let Some(f) = scope_findings.first() {
        parts.push(format!(
            "A fast-track signal is open here ({}): {}. Worth engaging with directly.",
            f.signal_type.label(),
            f.description
        ));
    }

    // Source 4 — MYTH-1 declared symbol traditions. A symbol the author roots in a
    // named tradition (e.g. "Norse", "Christian") is a hook the theological reader
    // should honour: it tells us which lens the author is already invoking.
    if let Ok(ms) = crate::myth::MythStore::open(project_root) {
        if let Ok(symbols) = ms.symbols(book_slug) {
            let mut traditions: Vec<String> = symbols
                .iter()
                .flat_map(|s| s.traditions.iter().cloned())
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect();
            traditions.sort();
            traditions.dedup();
            if !traditions.is_empty() {
                let shown: Vec<String> = traditions.iter().take(4).cloned().collect();
                parts.push(format!(
                    "The author's declared symbols draw on {} tradition(s) ({}); I will read with those \
                     in mind rather than imposing another.",
                    traditions.len(),
                    shown.join(", ")
                ));
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("GROUNDING:\n{}", parts.join("\n")))
    }
}

/// Whether arc text describes a belief-system change (Source 2 filter).
fn has_belief_vocab(s: &str) -> bool {
    let lc = s.to_lowercase();
    const V: &[&str] = &[
        "faith", "doubt", "redemption", "conversion", "convert", "belief", "spiritual",
        "awakening", "grace", "sin", "soul", "disillusion", "loss of faith", "lost faith",
    ];
    V.iter().any(|w| lc.contains(w))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inner_theologian::SignalType;

    #[test]
    fn belief_vocab_detection() {
        assert!(has_belief_vocab("corruption a loss of faith in the order"));
        assert!(has_belief_vocab("positive_change finds redemption"));
        assert!(!has_belief_vocab("flat remains a stubborn farmer throughout"));
    }

    #[test]
    fn scope_signal_grounds_even_without_world_or_char() {
        // A temp project root has no utopia/char data; a scope finding still
        // grounds the session via Source 3.
        let dir = std::env::temp_dir().join(format!("it-ground-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        let f = TheologianFinding {
            signal_type: SignalType::ConsequenceGap,
            chapter_ord: 3,
            para_id: "p1".into(),
            description: "lethal violence without depicted consequence".into(),
            suppressed: false,
        };
        let g = build_grounding(&dir, "bk", std::slice::from_ref(&f)).unwrap();
        assert!(g.contains("GROUNDING:"));
        assert!(g.contains("consequence gap"));
        // Empty scope + empty stores → None.
        assert!(build_grounding(&dir, "bk", &[]).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
