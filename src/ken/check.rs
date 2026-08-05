//! KEN-1 (KEN-P2) — the epistemic check: *could this character know this yet?*
//!
//! Detect **uses** of known topics (a topic named in a character's attributed
//! dialogue, or referenced in their POV scene's narration), then check each use
//! against the character's [`earliest_grant`](super::earliest_grant). A use before
//! the earliest grant is a `premature_knowledge` break; a `secret:` topic used by
//! an ungranted character is a `leaked_secret`. Silent where it can't ground —
//! KEN never invents a break.

use std::collections::{BTreeSet, HashMap};

use super::grants;
use super::walk::ParaRef;
use super::{
    earliest_grant, Grant, GrantSource, KnowledgeFinding, KnowledgeItem, Severity, Use, UseVia,
};
use crate::config::Config;
use crate::prose::ProseLanguage;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

/// The topics KEN watches: every granted topic plus every declared secret,
/// deduped. Only these are matched in prose. Pure.
pub(crate) fn known_topics(grants: &[Grant], secrets: &BTreeSet<String>) -> Vec<String> {
    let mut set: BTreeSet<String> = grants.iter().map(|g| g.topic.clone()).collect();
    set.extend(secrets.iter().cloned());
    set.into_iter().collect()
}

/// Whether a `pov:` value names a concrete character (not omniscient / first /
/// third-person markers, whose "knower" isn't a specific character).
fn concrete_pov(declared: &Option<String>) -> Option<String> {
    let name = declared.as_deref()?.trim();
    match name.to_lowercase().as_str() {
        "" | "omniscient" | "omni" | "first" | "third" | "limited" => None,
        _ => Some(grants::normalize_topic(name)),
    }
}

/// Detect every use of a known topic across the walk — impure (runs DIALOG-1
/// attribution). `topics` is the watched set; `names` the character roster.
pub(crate) fn detect_uses(
    paras: &[ParaRef],
    topics: &[String],
    names: &[String],
    lang: &ProseLanguage,
) -> Vec<Use> {
    let convention = crate::dialogue::dialogue_convention(lang);
    let lex = crate::dialogue::lexicon_for_with(lang, &[], &[]);
    let windows = crate::dialogue::AttributionWindows::default();
    let topics_lc: Vec<(String, String)> =
        topics.iter().map(|t| (t.clone(), t.to_lowercase())).collect();

    let mut uses = Vec::new();
    let mut prev_named: Option<String> = None;
    for p in paras {
        // Dialogue uses — a character speaks a topic's name (the strong signal).
        let mut spans = crate::dialogue::detect_spans(&p.id.to_string(), &p.text, convention, lang);
        crate::dialogue::attribute_spans(
            &mut spans,
            &p.text,
            names,
            prev_named.as_deref(),
            lex,
            lang,
            windows,
        );
        for span in &spans {
            let Some(speaker) = span.attribution_name.as_deref() else { continue };
            let line_lc = span.speech_text.to_lowercase();
            for (topic, topic_lc) in &topics_lc {
                if crate::drift::mentions(&line_lc, topic_lc) {
                    uses.push(Use {
                        character: grants::normalize_topic(speaker),
                        topic: topic.clone(),
                        at: p.at,
                        via: UseVia::Dialogue,
                        anchor: p.id,
                    });
                }
            }
        }
        if let Some(last) = spans.iter().rev().find_map(|s| s.attribution_name.clone()) {
            prev_named = Some(last);
        }

        // POV uses — a concrete-POV scene references a topic in narration (the
        // POV character is aware of it). Skipped for omniscient/first-person.
        if let Some(pov) = concrete_pov(&p.declared_pov) {
            let text_lc = p.text.to_lowercase();
            for (topic, topic_lc) in &topics_lc {
                if crate::drift::mentions(&text_lc, topic_lc) {
                    uses.push(Use {
                        character: pov.clone(),
                        topic: topic.clone(),
                        at: p.at,
                        via: UseVia::Pov,
                        anchor: p.id,
                    });
                }
            }
        }
    }
    uses
}

/// The pure epistemic check: one finding per (character, topic) whose *earliest*
/// use precedes the character's earliest grant (or who has no grant at all). A
/// `secret:` topic escalates to `leaked_secret`; everything else is
/// `premature_knowledge`. Both are `Break` severity. Pure.
pub(crate) fn check(
    grants: &[Grant],
    uses: &[Use],
    secrets: &BTreeSet<String>,
) -> Vec<KnowledgeFinding> {
    // The earliest premature use per (character, topic) — later repeats are noise.
    let mut first: HashMap<(String, String), &Use> = HashMap::new();
    for u in uses {
        let premature = match earliest_grant(grants, &u.character, &u.topic) {
            None => true,
            Some(g) => g.at > u.at,
        };
        if !premature {
            continue;
        }
        first
            .entry((u.character.clone(), u.topic.clone()))
            .and_modify(|e| {
                if u.at < e.at {
                    *e = u;
                }
            })
            .or_insert(u);
    }

    let mut out: Vec<KnowledgeFinding> = first
        .into_values()
        .map(|u| {
            let grant = earliest_grant(grants, &u.character, &u.topic);
            let kind = if secrets.contains(&u.topic) { "leaked_secret" } else { "premature_knowledge" };
            let verb = match u.via {
                UseVia::Dialogue => "speaks of",
                UseVia::Pov => "references",
            };
            let message = match grant {
                Some(g) => format!(
                    "{} {verb} \u{201c}{}\u{201d} in ch. {} — before learning it in ch. {}",
                    u.character, u.topic, u.at.chapter_ord, g.at.chapter_ord
                ),
                None => format!(
                    "{} {verb} \u{201c}{}\u{201d} in ch. {} — never established to know it",
                    u.character, u.topic, u.at.chapter_ord
                ),
            };
            KnowledgeFinding {
                kind,
                severity: Severity::Break,
                chapter: u.at.chapter_ord,
                anchor: Some(u.anchor),
                character: u.character.clone(),
                topic: u.topic.clone(),
                message,
            }
        })
        .collect();
    out.sort_by(|a, b| {
        a.chapter
            .cmp(&b.chapter)
            .then(a.character.cmp(&b.character))
            .then(a.topic.cmp(&b.topic))
    });
    out
}

/// A declared reveal (`know:<topic>@<char>`) whose topic never surfaces again — the
/// epistemic `unpaid_setup`. Only **declared** grants count (the author opted in by
/// tagging the reveal); presence grants don't imply a topic must resurface. A
/// `Notice` (dangling, not a hard break). Pure.
pub(crate) fn dropped_reveals(grants: &[Grant], uses: &[Use]) -> Vec<KnowledgeFinding> {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut out = Vec::new();
    for g in grants.iter().filter(|g| g.source == GrantSource::Declared) {
        if !seen.insert((g.character.clone(), g.topic.clone())) {
            continue;
        }
        let surfaces = uses.iter().any(|u| u.topic == g.topic && u.at >= g.at);
        if surfaces {
            continue;
        }
        out.push(KnowledgeFinding {
            kind: "dropped_reveal",
            severity: Severity::Notice,
            chapter: g.at.chapter_ord,
            anchor: g.anchor,
            character: g.character.clone(),
            topic: g.topic.clone(),
            message: format!(
                "{} is told \u{201c}{}\u{201d} in ch. {} — it never surfaces again",
                g.character, g.topic, g.at.chapter_ord
            ),
        });
    }
    out
}

/// The impure driver KEN-P4's CLI calls: build grants → detect uses → the check
/// (`premature_knowledge` / `leaked_secret`) + dropped reveals. Self-gating — a
/// project with no `secret:`/`know:` tags and no events adds no topics and returns
/// nothing.
pub(crate) fn run(
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
) -> Vec<KnowledgeFinding> {
    let (grants, items, paras) = grants::build_grants(layout, h, book);
    let secrets: BTreeSet<String> =
        items.iter().filter(|i: &&KnowledgeItem| i.secret).map(|i| i.topic.clone()).collect();
    let topics = known_topics(&grants, &secrets);
    if topics.is_empty() {
        return Vec::new();
    }
    let lang = ProseLanguage::from_label(&cfg.language);
    let names = crate::dialogue::character_names(h);
    let uses = detect_uses(&paras, &topics, &names, &lang);

    let mut findings = check(&grants, &uses, &secrets);
    findings.extend(dropped_reveals(&grants, &uses));
    findings.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then(a.chapter.cmp(&b.chapter))
            .then(a.character.cmp(&b.character))
            .then(a.topic.cmp(&b.topic))
    });
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{GrantSource, ScenePos};
    use uuid::Uuid;

    fn pos(ch: u32, sc: u32) -> ScenePos {
        ScenePos { chapter_ord: ch, scene_index: sc }
    }
    fn grant(c: &str, t: &str, ch: u32) -> Grant {
        Grant {
            character: c.into(),
            topic: t.into(),
            at: pos(ch, 1),
            source: GrantSource::Declared,
            anchor: Some(Uuid::from_u128(1000 + ch as u128)),
        }
    }
    fn usage(c: &str, t: &str, ch: u32) -> Use {
        Use { character: c.into(), topic: t.into(), at: pos(ch, 1), via: UseVia::Dialogue, anchor: Uuid::from_u128(ch as u128) }
    }

    #[test]
    fn flags_use_before_grant_and_never_granted() {
        let grants = vec![grant("Mara", "the betrayal", 7)];
        let uses = vec![
            usage("Mara", "the betrayal", 4), // before grant (ch7) → premature
            usage("Bob", "the murder", 3),    // never granted → premature
        ];
        let out = check(&grants, &uses, &BTreeSet::new());
        assert_eq!(out.len(), 2);
        let mara = out.iter().find(|f| f.character == "Mara").unwrap();
        assert_eq!(mara.kind, "premature_knowledge");
        assert_eq!(mara.chapter, 4);
        assert!(mara.message.contains("before learning it in ch. 7"));
        let bob = out.iter().find(|f| f.character == "Bob").unwrap();
        assert!(bob.message.contains("never established"));
    }

    #[test]
    fn granted_before_or_at_use_is_silent() {
        let grants = vec![grant("Mara", "the betrayal", 4)];
        // used at ch4 (== grant) and ch9 (after) — both legitimate.
        let uses = vec![usage("Mara", "the betrayal", 4), usage("Mara", "the betrayal", 9)];
        assert!(check(&grants, &uses, &BTreeSet::new()).is_empty());
    }

    #[test]
    fn secret_topic_escalates_to_leaked_secret_and_dedups_to_earliest() {
        let secrets: BTreeSet<String> = ["the heir".to_string()].into_iter().collect();
        // Sella references the secret at ch5 and again ch6, never granted.
        let uses = vec![usage("Sella", "the heir", 6), usage("Sella", "the heir", 5)];
        let out = check(&[], &uses, &secrets);
        assert_eq!(out.len(), 1, "deduped to one finding per (character, topic)");
        assert_eq!(out[0].kind, "leaked_secret");
        assert_eq!(out[0].chapter, 5, "the earliest premature use");
        assert_eq!(out[0].severity, Severity::Break);
    }

    #[test]
    fn dropped_reveal_flags_a_declared_topic_that_never_surfaces() {
        let grants = vec![
            grant("Mara", "the betrayal", 7), // surfaces later → not dropped
            grant("Joren", "the map", 3),     // never used → dropped
        ];
        let uses = vec![usage("Mara", "the betrayal", 9)];
        let out = dropped_reveals(&grants, &uses);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "dropped_reveal");
        assert_eq!((out[0].character.as_str(), out[0].topic.as_str()), ("Joren", "the map"));
        assert_eq!(out[0].severity, Severity::Notice);
        assert!(out[0].anchor.is_some());
    }

    #[test]
    fn dropped_reveal_ignores_presence_grants() {
        let g = Grant {
            character: "Alice".into(),
            topic: "the murder".into(),
            at: pos(6, 1),
            source: GrantSource::Presence, // not author-declared → not a dropped reveal
            anchor: Some(Uuid::from_u128(1)),
        };
        assert!(dropped_reveals(&[g], &[]).is_empty());
    }

    #[test]
    fn known_topics_unions_grants_and_secrets_deduped() {
        let grants = vec![grant("A", "x", 1), grant("B", "x", 1), grant("A", "y", 1)];
        let secrets: BTreeSet<String> = ["y".to_string(), "z".to_string()].into_iter().collect();
        assert_eq!(known_topics(&grants, &secrets), vec!["x", "y", "z"]);
    }

    #[test]
    fn concrete_pov_skips_reserved_markers() {
        assert_eq!(concrete_pov(&Some("Mara".into())).as_deref(), Some("Mara"));
        assert!(concrete_pov(&Some("omniscient".into())).is_none());
        assert!(concrete_pov(&Some("first".into())).is_none());
        assert!(concrete_pov(&None).is_none());
    }
}
