//! CHORUS-1 (CH-P4) — POV & head-hop discipline.
//!
//! A scene has a point of view — declared (a `pov:<name>` paragraph tag) or, if
//! undeclared, inferred as the most-mentioned character (the existing POV-chip
//! heuristic, decoupled from the TUI). A **head-hop** is a named character other
//! than the scene POV shown accessing their own inner life — the subject of an
//! interiority verb (`Joren wondered…` in a Mara-POV scene). Advisory and
//! heuristic: the tree has no parser, so this catches interiority attributed to
//! a *named* subject, not pronoun antecedents (`she thought` where "she" isn't
//! the POV needs antecedent resolution CHORUS deliberately doesn't attempt).

use std::collections::{BTreeMap, HashSet};

use uuid::Uuid;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::vocab::interiority_verbs;

/// A scene's point of view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ScenePov {
    /// A single named POV character (declared `pov:<name>` or inferred).
    Single(String),
    /// First person (`pov:first`) — any NAMED character's interiority is a leak.
    First,
    /// Deliberately omniscient / multi-POV (`pov:omniscient`) — head-hop off.
    Omniscient,
    /// Nothing declared and no character mentioned — can't judge.
    Unknown,
}

impl ScenePov {
    pub(crate) fn describe(&self) -> String {
        match self {
            ScenePov::Single(n) => format!("POV {n}"),
            ScenePov::First => "first person".into(),
            ScenePov::Omniscient => "omniscient".into(),
            ScenePov::Unknown => "POV unknown".into(),
        }
    }
}

/// One head-hop: a named character (not the scene POV) shown accessing their own
/// inner life, `count` times in the scene.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadHop {
    pub experiencer: String,
    pub count: usize,
}

fn norm(tok: &str) -> String {
    tok.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase()
}

fn name_parts(name: &str) -> Vec<String> {
    name.split_whitespace().map(|p| p.to_lowercase()).collect()
}

fn matches_at(tokens: &[String], i: usize, parts: &[String]) -> bool {
    i + parts.len() <= tokens.len() && parts.iter().enumerate().all(|(k, p)| &tokens[i + k] == p)
}

/// Determine a scene's POV: a declared tag wins; else the most-mentioned roster
/// character; else Unknown.
pub(crate) fn scene_pov(text: &str, roster: &[String], declared: Option<&str>) -> ScenePov {
    if let Some(d) = declared {
        let dl = d.trim().to_lowercase();
        return match dl.as_str() {
            "omniscient" | "omni" | "multi" => ScenePov::Omniscient,
            "first" | "1st" | "i" => ScenePov::First,
            _ => {
                // Resolve to the roster's canonical spelling if it matches.
                let canonical = roster.iter().find(|n| n.to_lowercase() == dl).cloned();
                ScenePov::Single(canonical.unwrap_or_else(|| d.trim().to_string()))
            }
        };
    }
    match most_mentioned(text, roster) {
        Some(name) => ScenePov::Single(name),
        None => ScenePov::Unknown,
    }
}

/// The most-mentioned roster character in `text` (ties → first appearance).
fn most_mentioned(text: &str, roster: &[String]) -> Option<String> {
    let tokens: Vec<String> = text.split_whitespace().map(|t| norm(&t)).collect();
    let mut best: Option<(String, usize, usize)> = None; // (name, count, first_idx)
    for name in roster {
        let parts = name_parts(name);
        if parts.is_empty() {
            continue;
        }
        let (mut count, mut first) = (0usize, usize::MAX);
        for i in 0..tokens.len() {
            if matches_at(&tokens, i, &parts) {
                if count == 0 {
                    first = i;
                }
                count += 1;
            }
        }
        if count > 0 {
            let better = match &best {
                None => true,
                Some((_, bc, bf)) => count > *bc || (count == *bc && first < *bf),
            };
            if better {
                best = Some((name.clone(), count, first));
            }
        }
    }
    best.map(|(n, _, _)| n)
}

/// Detect head-hops: a named roster character who is the subject of an
/// interiority verb (`<Name> <verb>`) and is not the scene POV. Deduped by
/// experiencer with an occurrence count. Empty for Omniscient / Unknown.
pub(crate) fn head_hops(
    text: &str,
    roster: &[String],
    verbs: &HashSet<String>,
    pov: &ScenePov,
) -> Vec<HeadHop> {
    if matches!(pov, ScenePov::Omniscient | ScenePov::Unknown) {
        return Vec::new();
    }
    let tokens: Vec<String> = text.split_whitespace().map(|t| norm(&t)).collect();
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for name in roster {
        let parts = name_parts(name);
        if parts.is_empty() || !is_leak(pov, name) {
            continue;
        }
        for i in 0..tokens.len() {
            if matches_at(&tokens, i, &parts) {
                let verb_idx = i + parts.len();
                if verb_idx < tokens.len() && verbs.contains(&tokens[verb_idx]) {
                    *counts.entry(name.clone()).or_default() += 1;
                }
            }
        }
    }
    counts.into_iter().map(|(experiencer, count)| HeadHop { experiencer, count }).collect()
}

/// Is a named experiencer's interiority a POV leak?
fn is_leak(pov: &ScenePov, experiencer: &str) -> bool {
    match pov {
        ScenePov::First => true,
        ScenePov::Single(name) => name.to_lowercase() != experiencer.to_lowercase(),
        ScenePov::Omniscient | ScenePov::Unknown => false,
    }
}

/// A scene's head-hop findings (only scenes WITH leaks are returned).
pub(crate) struct SceneHeadHops {
    pub chapter_ord: u32,
    pub scene_index: u32,
    pub pov: ScenePov,
    pub first_para: Uuid,
    pub hops: Vec<HeadHop>,
}

/// Walk `book`'s chapters, split each into scenes (`is_scene_break`), and report
/// the scenes with head-hops. Impure (reads paragraph files); the judgement is
/// the pure [`scene_pov`] + [`head_hops`].
pub(crate) fn scan_head_hops(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
) -> Vec<SceneHeadHops> {
    let (lang, _) = crate::prose::resolve_prose_language(None, &cfg.language);
    let roster = crate::dialogue::character_names(h);
    let verbs: HashSet<String> = interiority_verbs(&lang).iter().map(|s| s.to_string()).collect();

    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();

    let mut out = Vec::new();
    for (ci, ch) in chapters.iter().enumerate() {
        let chapter_ord = (ci + 1) as u32;
        let mut scene_idx = 0u32;
        let mut cur: Vec<(Uuid, String, Vec<String>)> = Vec::new();
        for id in h.collect_subtree(ch.id) {
            let Some(n) = h.get(id) else { continue };
            if n.kind != NodeKind::Paragraph || n.content_type.as_deref() == Some("jinja") {
                continue;
            }
            let Some(rel) = n.file.as_ref() else { continue };
            let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) else { continue };
            let text = crate::audiobook::typst_to_plain(&raw);
            if crate::manuscript::is_scene_break(&text) {
                finish_scene(&mut cur, chapter_ord, &mut scene_idx, &roster, &verbs, &mut out);
                continue;
            }
            cur.push((n.id, text, n.tags.clone()));
        }
        finish_scene(&mut cur, chapter_ord, &mut scene_idx, &roster, &verbs, &mut out);
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn finish_scene(
    cur: &mut Vec<(Uuid, String, Vec<String>)>,
    chapter_ord: u32,
    scene_idx: &mut u32,
    roster: &[String],
    verbs: &HashSet<String>,
    out: &mut Vec<SceneHeadHops>,
) {
    if cur.is_empty() {
        return;
    }
    *scene_idx += 1;
    let first_para = cur[0].0;
    let declared = cur
        .iter()
        .flat_map(|(_, _, tags)| tags.iter())
        .find_map(|t| t.strip_prefix("pov:").map(|s| s.to_string()));
    let text: String = cur.iter().map(|(_, t, _)| t.as_str()).collect::<Vec<_>>().join("\n");
    let pov = scene_pov(&text, roster, declared.as_deref());
    let hops = head_hops(&text, roster, verbs, &pov);
    if !hops.is_empty() {
        out.push(SceneHeadHops {
            chapter_ord,
            scene_index: *scene_idx,
            pov,
            first_para,
            hops,
        });
    }
    cur.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prose::ProseLanguage;

    fn en_verbs() -> HashSet<String> {
        interiority_verbs(&ProseLanguage::En).iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn scene_pov_declared_and_inferred() {
        let roster = vec!["Mara".to_string(), "Joren".to_string()];
        assert_eq!(scene_pov("", &roster, Some("Mara")), ScenePov::Single("Mara".into()));
        // Canonical spelling resolved from a lowercase tag.
        assert_eq!(scene_pov("", &roster, Some("mara")), ScenePov::Single("Mara".into()));
        assert_eq!(scene_pov("", &roster, Some("omniscient")), ScenePov::Omniscient);
        assert_eq!(scene_pov("", &roster, Some("first")), ScenePov::First);
        // Inferred: Mara mentioned twice, Joren once → Mara.
        assert_eq!(
            scene_pov("Mara went in. Joren waited. Mara sighed.", &roster, None),
            ScenePov::Single("Mara".into())
        );
        assert_eq!(scene_pov("The wind rose.", &roster, None), ScenePov::Unknown);
    }

    #[test]
    fn head_hop_flags_a_named_non_pov_experiencer() {
        let roster = vec!["Mara".to_string(), "Joren".to_string()];
        let v = en_verbs();
        let pov = ScenePov::Single("Mara".into());
        // Joren (not POV) is the subject of an interiority verb → head-hop.
        let hops = head_hops("Mara looked out. Joren wondered whether she knew.", &roster, &v, &pov);
        assert_eq!(hops, vec![HeadHop { experiencer: "Joren".into(), count: 1 }]);
        // Mara (the POV) accessing her own interiority is fine.
        assert!(head_hops("Mara thought about the tide.", &roster, &v, &pov).is_empty());
    }

    #[test]
    fn omniscient_and_first_person() {
        let roster = vec!["Mara".to_string(), "Joren".to_string()];
        let v = en_verbs();
        // Omniscient scene: no head-hops, ever.
        assert!(head_hops("Joren wondered.", &roster, &v, &ScenePov::Omniscient).is_empty());
        // First person: any named character's interiority leaks.
        let hops = head_hops("Joren wondered. Mara realised.", &roster, &v, &ScenePov::First);
        assert_eq!(hops.len(), 2);
    }

    #[test]
    fn works_in_russian() {
        let roster = vec!["Мара".to_string(), "Джорен".to_string()];
        let v: HashSet<String> =
            interiority_verbs(&ProseLanguage::Ru).iter().map(|s| s.to_string()).collect();
        // Мара-POV scene; Джорен подумал → head-hop on Джорен.
        let pov = scene_pov("Мара смотрела в окно. Джорен подумал о ней.", &roster, None);
        assert_eq!(pov, ScenePov::Single("Мара".into()));
        let hops = head_hops("Мара смотрела в окно. Джорен подумал о ней.", &roster, &v, &pov);
        assert_eq!(hops, vec![HeadHop { experiencer: "Джорен".into(), count: 1 }]);
    }
}
