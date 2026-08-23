//! BONDS-1 (BD-P6) — the opt-in `implied_cooling` pass.
//!
//! The deterministic core catches *declared* relationship faults (an
//! unwritten / unearned / dropped bond, measured against the `rel:` tags). The
//! subtle case — a relationship the prose lets *warm* or *cool* on the page with
//! **no** `rel:` tag marking the change — is what a reader feels but no heuristic
//! sees. This pass hands the model the declared-bond ledger plus the shared
//! scenes and asks for those implied, undeclared shifts.
//!
//! **Explicit and cost-capped** — never automatic, never free. It rides the world
//! fact-checker's `slow_llm_call` (soft cost cap + daily cap + JSON-finding
//! parse), the same rail as KEN's `implied_irony`. Self-gating: no declared bonds
//! → no call. Mirrors `ken::deep`.

use std::collections::BTreeMap;
use std::path::Path;

use super::gather;
use super::{BondFinding, Declared, ScenePos, Severity};
use crate::config::Config;
use crate::ken::walk::ParaRef;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

pub(crate) const COOLING_SYSTEM: &str = "You are a meticulous continuity editor for a work of \
fiction, watching one axis only: how two characters RELATE. You are given a bond ledger (which pair \
is declared to be in what relationship, and in which chapter) and a reading-order sequence of scenes \
those characters share. Find IMPLIED relationship shifts — a pair whose behaviour on the page clearly \
WARMS or COOLS (allies turning cold, enemies softening, a growing intimacy or a quiet estrangement) \
where NO declared change marks it (declared shifts are handled by another pass). Be conservative: \
only flag a genuine, on-the-page shift you can tie to the scenes. Respond ONLY with a JSON array; \
each item is {\"category\": \"implied_cooling\", \"severity\": \"warning\"|\"info\", \"explanation\": a \
one-sentence reason naming the two characters and the chapter}. Return [] if nothing is off.";

/// Run the opt-in LLM implied-cooling pass. `max_cost` is the soft token budget;
/// `force` skips the cost preflight prompt. Self-gating (no declared bonds →
/// empty).
pub(crate) fn run(
    project: &Path,
    book_name: Option<&str>,
    max_cost: usize,
    force: bool,
) -> Result<Vec<BondFinding>, String> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized().map_err(|e| e.to_string())?;
    let cfg = Config::load_layered(&layout.config_path()).map_err(|e| e.to_string())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| e.to_string())?;
    let h = Hierarchy::load(&store).map_err(|e| e.to_string())?;
    let book = crate::cli::resolve_user_book(&h, book_name, "bonds")?;

    let (declared, _coscenes, paras) = gather::build_bonds(&layout, &h, book);
    if declared.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = build_cooling_prompt(&build_ledger(&declared), &group_scenes(&paras));
    let findings = crate::cli::realworld::slow_llm_call(
        project,
        "bonds-cooling",
        COOLING_SYSTEM,
        prompt,
        max_cost,
        force,
    )
    .map_err(|e| e.to_string())?;

    Ok(findings.into_iter().map(map_finding).collect())
}

/// A per-pair "declared state → state" ledger, sorted, for the prompt. Pure.
fn build_ledger(declared: &[Declared]) -> String {
    let mut by_pair: BTreeMap<(&str, &str), Vec<(&str, u32)>> = BTreeMap::new();
    for d in declared {
        by_pair
            .entry((d.a.as_str(), d.b.as_str()))
            .or_default()
            .push((d.kind.as_str(), d.at.chapter_ord));
    }
    let mut out = String::new();
    for ((a, b), mut states) in by_pair {
        states.sort_by_key(|(_, ch)| *ch);
        states.dedup();
        let items: Vec<String> = states.iter().map(|(k, ch)| format!("{k} (ch. {ch})")).collect();
        out.push_str(&format!("- {a} & {b}: {}\n", items.join(" \u{2192} ")));
    }
    out
}

/// Group the paragraph walk into scenes (concatenated text + the scene POV), in
/// reading order. Pure. (Same shape as `ken::deep::group_scenes`.)
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

fn build_cooling_prompt(ledger: &str, scenes: &[(ScenePos, Option<String>, String)]) -> String {
    let mut body = String::new();
    for (at, pov, text) in scenes {
        if text.trim().is_empty() {
            continue;
        }
        let pov = pov.as_deref().unwrap_or("\u{2014}");
        body.push_str(&format!(
            "[ch. {} \u{b7} scene {} \u{b7} POV: {pov}]\n{}\n\n",
            at.chapter_ord,
            at.scene_index,
            text.trim()
        ));
    }
    format!(
        "BOND LEDGER (which pair is declared what, and when):\n{ledger}\n\n\
         SCENES (reading order; find IMPLIED, undeclared relationship shifts):\n{body}"
    )
}

/// Map an LLM finding to a soft `implied_cooling` finding. The LLM never produces
/// a hard `Break` — the deterministic layer owns those; implied cooling is
/// advisory. (No pair is parsed back out; the reason names the characters.)
fn map_finding(f: crate::world::fact_check::Finding) -> BondFinding {
    let severity = match f.severity.as_str() {
        "warning" | "contradiction" => Severity::Notice,
        _ => Severity::Info,
    };
    BondFinding {
        kind: "implied_cooling",
        severity,
        chapter: 0,
        anchor: None,
        a: String::new(),
        b: String::new(),
        message: f.body,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn decl(kind: &str, ch: u32) -> Declared {
        Declared::new(kind, "Mara", "Kell", ScenePos { chapter_ord: ch, scene_index: 1 }, Uuid::from_u128(ch as u128))
    }

    #[test]
    fn ledger_shows_the_declared_state_trail_per_pair() {
        let declared = vec![decl("ally", 1), decl("enemy", 9)];
        let l = build_ledger(&declared);
        // Canonical pair (Kell & Mara), states in chapter order joined by →.
        assert!(l.contains("- Kell & Mara: ally (ch. 1) \u{2192} enemy (ch. 9)"), "{l}");
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
        assert_eq!(scenes[0].2, "First.\nSecond.");
    }

    #[test]
    fn implied_cooling_maps_soft_severity() {
        let mk = |sev: &str| crate::world::fact_check::Finding {
            category: "implied_cooling".into(),
            severity: sev.into(),
            body: "Mara and Kell grow cold across ch. 5 with no declared break.".into(),
            body_en: String::new(),
            suppressed_by: None,
        };
        assert_eq!(map_finding(mk("warning")).severity, Severity::Notice);
        assert_eq!(map_finding(mk("info")).severity, Severity::Info);
        assert_eq!(map_finding(mk("info")).kind, "implied_cooling");
    }
}
