//! KEN-1 (KEN-P6) — the opt-in `implied_irony` pass.
//!
//! The deterministic core catches *named* epistemic breaks (a character speaks a
//! topic before their grant). The subtle case — a character *acting* informed or
//! conspicuously ignorant **without naming** the topic — is what a reader feels but
//! no heuristic sees. This pass hands each scene, plus a knowledge ledger (who
//! learns what, when), to the model and asks for those implied breaks.
//!
//! **Explicit and cost-capped** — never automatic, never free. It rides the world
//! fact-checker's `slow_llm_call` (soft cost cap + daily cap + JSON-finding parse),
//! the same rail as SENTINEL's coherence pass. Self-gating: no grants → no call.

use std::collections::BTreeMap;
use std::path::Path;

use super::grants;
use super::walk::ParaRef;
use super::{Grant, KnowledgeFinding, ScenePos, Severity};
use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub(crate) const IRONY_SYSTEM: &str = "You are a meticulous continuity editor for a work of \
fiction, watching one axis only: what each character KNOWS. You are given a knowledge ledger (which \
character learns which fact, and in which chapter) and a reading-order sequence of scenes (each \
noting whose point of view we are in). Find IMPLIED epistemic breaks — a character ACTING on \
knowledge they could not have yet, or acting conspicuously UNAWARE of something they already know — \
where the fact is NOT named outright (obvious named references are handled by another pass). Be \
conservative: only flag a genuine break you can tie to the ledger. Respond ONLY with a JSON array; \
each item is {\"category\": \"implied_irony\", \"severity\": \"warning\"|\"info\", \"explanation\": \
a one-sentence reason naming the character and the chapter}. Return [] if nothing is off.";

/// Run the opt-in LLM implied-irony pass. `max_cost` is the soft token budget;
/// `force` skips the cost preflight prompt. Self-gating (no grants → empty).
pub(crate) fn run(
    project: &Path,
    book_name: Option<&str>,
    max_cost: usize,
    force: bool,
) -> Result<Vec<KnowledgeFinding>, String> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(|e| e.to_string())?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| e.to_string())?;
    let h = Hierarchy::load(&store).map_err(|e| e.to_string())?;
    let book = crate::cli::resolve_user_book(&h, book_name, "knowledge")?;

    let (grants, _items, paras) = grants::build_grants(&layout, &h, book);
    if grants.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = build_irony_prompt(&build_ledger(&grants), &group_scenes(&paras));
    let findings = crate::cli::realworld::slow_llm_call(
        project,
        "knowledge-irony",
        IRONY_SYSTEM,
        prompt,
        max_cost,
        force,
    )
    .map_err(|e| e.to_string())?;

    Ok(findings.into_iter().map(map_finding).collect())
}

/// A per-character "knows: topic (ch. N)" ledger, sorted, for the prompt. Pure.
fn build_ledger(grants: &[Grant]) -> String {
    let mut by_char: BTreeMap<&str, Vec<(&str, u32)>> = BTreeMap::new();
    for g in grants {
        by_char.entry(g.character.as_str()).or_default().push((g.topic.as_str(), g.at.chapter_ord));
    }
    let mut out = String::new();
    for (character, mut topics) in by_char {
        topics.sort();
        topics.dedup();
        let items: Vec<String> = topics.iter().map(|(t, ch)| format!("{t} (ch. {ch})")).collect();
        out.push_str(&format!("- {character} knows: {}\n", items.join("; ")));
    }
    out
}

/// Group the paragraph walk into scenes (concatenated text + the scene POV), in
/// reading order. Pure.
fn group_scenes(paras: &[ParaRef]) -> Vec<(ScenePos, Option<String>, String)> {
    let mut m: BTreeMap<ScenePos, (Option<String>, String)> = BTreeMap::new();
    for p in paras {
        let e = m.entry(p.at).or_insert((p.declared_pov.clone(), String::new()));
        if e.0.is_none() {
            e.0 = p.declared_pov.clone();
        }
        if !e.1.is_empty() {
            e.1.push('\n');
        }
        e.1.push_str(p.text.trim());
    }
    m.into_iter().map(|(at, (pov, text))| (at, pov, text)).collect()
}

fn build_irony_prompt(ledger: &str, scenes: &[(ScenePos, Option<String>, String)]) -> String {
    let mut body = String::new();
    for (at, pov, text) in scenes {
        if text.trim().is_empty() {
            continue;
        }
        let pov = pov.as_deref().unwrap_or("—");
        body.push_str(&format!(
            "[ch. {} · scene {} · POV: {pov}]\n{}\n\n",
            at.chapter_ord,
            at.scene_index,
            text.trim()
        ));
    }
    format!(
        "KNOWLEDGE LEDGER (who learns what, and when):\n{ledger}\n\n\
         SCENES (reading order; find IMPLIED knowledge breaks against the ledger):\n{body}"
    )
}

/// Map an LLM finding to a soft `implied_irony` finding. The LLM never produces a
/// hard `Break` — the deterministic layer owns those; implied irony is advisory.
fn map_finding(f: crate::world::fact_check::Finding) -> KnowledgeFinding {
    let severity = match f.severity.as_str() {
        "warning" | "contradiction" => Severity::Notice,
        _ => Severity::Info,
    };
    KnowledgeFinding {
        kind: "implied_irony",
        severity,
        chapter: 0,
        anchor: None,
        character: String::new(),
        topic: String::new(),
        message: f.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::GrantSource;
    use uuid::Uuid;

    fn grant(c: &str, t: &str, ch: u32) -> Grant {
        Grant {
            character: c.into(),
            topic: t.into(),
            at: ScenePos { chapter_ord: ch, scene_index: 1 },
            source: GrantSource::Declared,
            anchor: Some(Uuid::from_u128(1)),
        }
    }

    #[test]
    fn ledger_groups_and_sorts_per_character() {
        let grants = vec![grant("Mara", "the map", 3), grant("Mara", "the betrayal", 7), grant("Bob", "the murder", 6)];
        let l = build_ledger(&grants);
        assert!(l.contains("- Bob knows: the murder (ch. 6)"));
        // Mara's topics sorted (the betrayal < the map alphabetically).
        assert!(l.contains("- Mara knows: the betrayal (ch. 7); the map (ch. 3)"));
    }

    #[test]
    fn scenes_group_by_position_in_reading_order() {
        let mk = |ch, sc, pov: Option<&str>, text: &str| ParaRef {
            id: Uuid::from_u128(ch as u128 * 10 + sc as u128),
            at: ScenePos { chapter_ord: ch, scene_index: sc },
            tags: vec![],
            text: text.into(),
            declared_pov: pov.map(str::to_string),
        };
        let paras = vec![
            mk(1, 1, Some("Mara"), "First."),
            mk(1, 1, Some("Mara"), "Second."),
            mk(2, 1, None, "Later."),
        ];
        let scenes = group_scenes(&paras);
        assert_eq!(scenes.len(), 2);
        assert_eq!(scenes[0].0, ScenePos { chapter_ord: 1, scene_index: 1 });
        assert_eq!(scenes[0].1.as_deref(), Some("Mara"));
        assert_eq!(scenes[0].2, "First.\nSecond.");
    }

    #[test]
    fn implied_irony_maps_soft_severity() {
        let mk = |sev: &str| crate::world::fact_check::Finding {
            category: "implied_irony".into(),
            severity: sev.into(),
            body: "Mara acts unaware of the betrayal she learned in ch. 7.".into(),
            body_en: String::new(),
            suppressed_by: None,
        };
        assert_eq!(map_finding(mk("warning")).severity, Severity::Notice);
        assert_eq!(map_finding(mk("info")).severity, Severity::Info);
        assert_eq!(map_finding(mk("info")).kind, "implied_irony");
    }
}
