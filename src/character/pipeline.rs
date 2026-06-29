//! CHAR-1 (C-P4) — the compute pipeline. Two passes:
//!
//! - **Agency** (deterministic, no LLM): per character per chapter, scan the
//!   character's relevant paragraphs and compute the agency score. Runs in the
//!   review pass.
//! - **State extraction** (LLM, sliding-window, CLI-explicit): per chapter the
//!   character appears in, summarise their observable state, feeding the prior
//!   chapter's summary forward. Content-hash lazy; an edit re-runs from that
//!   chapter onward. Enriched with DIALOG-1 utterance/hedge and NARR-1
//!   interiority signals.

use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::{CompiledLexicon, VoiceScope, resolve_prose_language};
use crate::store::NodeKind;
use crate::store::SYSTEM_TAG_CHARACTERS;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::agency::{AgencyWindows, compute_agency};
use super::llm::char_llm_call;
use super::store::CharStore;
use super::{ArcDeclaration, CharacterState, verbs_for_with};

/// The character roster — titles of the Characters book's direct children.
pub(crate) fn character_names(h: &Hierarchy) -> Vec<String> {
    let Some(book) = h.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_CHARACTERS)
    }) else {
        return Vec::new();
    };
    let mut names: Vec<String> = h
        .children_of(Some(book.id))
        .iter()
        .map(|n| n.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .collect();
    names.sort();
    names.dedup();
    names
}

/// A chapter's ordered prose paragraphs `(para_id, stripped_text)` (Jinja
/// excluded; Typst stripped).
fn chapter_paragraphs(layout: &ProjectLayout, h: &Hierarchy, chapter_id: uuid::Uuid) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph || p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        if let Some(rel) = p.file.as_ref() {
            if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
                out.push((id.to_string(), crate::audiobook::typst_to_plain(&raw)));
            }
        }
    }
    out
}

/// Whole-word, case-insensitive mention test (a single-token name also matches
/// a 5-char-stem token, for declined forms).
pub(super) fn mentions(text: &str, name: &str) -> bool {
    let lc = text.to_lowercase();
    let nm = name.to_lowercase();
    if nm.contains(' ') {
        return lc.contains(&nm);
    }
    let stem: String = nm.chars().take(5).collect();
    let use_stem = nm.chars().count() >= 5;
    lc.split(|c: char| !c.is_alphanumeric()).any(|tok| {
        tok == nm || (use_stem && tok.chars().take(5).collect::<String>() == stem && !tok.is_empty())
    })
}

/// The text of a chapter's paragraphs mentioning `name` (empty if none).
fn mention_text(paras: &[(String, String)], name: &str) -> String {
    paras
        .iter()
        .filter(|(_, t)| mentions(t, name))
        .map(|(_, t)| t.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn chapters_of<'a>(h: &'a Hierarchy, book: &Node) -> Vec<&'a Node> {
    h.children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect()
}

fn hash_str(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ── agency pass (deterministic) ───────────────────────────────────────────────

/// Compute + persist the agency score for every character in every chapter they
/// appear in. Returns the number of (character, chapter) cells written.
pub(crate) fn run_agency(
    store: &CharStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
) -> Result<usize> {
    let (lang, _note) = resolve_prose_language(cfg.char.language.as_deref(), &cfg.language);
    let lx = CompiledLexicon::for_language_with(&lang, &[], &[]);
    let av = verbs_for_with(&lang, &cfg.char.extra_action_verbs);
    let roster = character_names(h);
    let now = chrono::Utc::now().to_rfc3339();
    let win = AgencyWindows {
        before: cfg.char.active_window_before,
        after: cfg.char.active_window_after,
    };

    let mut count = 0;
    for (idx, ch) in chapters_of(h, book).iter().enumerate() {
        let ord = (idx + 1) as u32;
        let paras = chapter_paragraphs(layout, h, ch.id);
        for name in &roster {
            let text = mention_text(&paras, name);
            if text.trim().is_empty() {
                continue;
            }
            let others: Vec<String> = roster.iter().filter(|n| *n != name).cloned().collect();
            let (score, active, passive) =
                compute_agency(&text, name, &others, &lang, &lx, &av, win);
            store.upsert_agency(&book.slug, name, ord, score, active, passive, &now)?;
            count += 1;
        }
    }
    Ok(count)
}

// ── state extraction (LLM) ────────────────────────────────────────────────────

const EXTRACTION_SYSTEM: &str = "You are extracting the observable state of a fictional character \
at the end of a chapter. Observable state means what their behaviour, speech, and reactions \
demonstrate. Do NOT speculate about hidden psychology or motivations not shown in the text. Do NOT \
add information from your training data about the character or story. A 'change' means something \
demonstrably different in behaviour, speech, decisions, or situation — not a minor incident with no \
lasting effect; be conservative. Return ONLY JSON: \
{\"state_summary\":\"2-3 sentences\",\"changed\":true|false,\"change_description\":\"...|null\"}";

/// Build the sliding-window user prompt for one chapter.
pub(super) fn build_extraction_prompt(
    name: &str,
    arc_type: Option<&str>,
    prev_state: Option<&str>,
    char_paras: &str,
) -> String {
    format!(
        "Character name: {name}\nArc type declared by author: {}\n\nPrevious state summary: {}\n\n\
         Paragraphs from this chapter where {name} appears:\n{char_paras}",
        arc_type.unwrap_or("not declared"),
        prev_state.unwrap_or("no prior state"),
    )
}

/// Parse the extraction response into `(state_summary, changed, change_description)`.
pub(super) fn parse_state(raw: &str) -> Option<(String, bool, Option<String>)> {
    let json = super::llm::extract_json_object(raw);
    let v: serde_json::Value = serde_json::from_str(json).ok()?;
    let summary = v.get("state_summary").and_then(|x| x.as_str())?.trim().to_string();
    if summary.is_empty() {
        return None;
    }
    let changed = v.get("changed").and_then(|x| x.as_bool()).unwrap_or(false);
    let change = v
        .get("change_description")
        .and_then(|x| x.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "null")
        .map(str::to_string);
    Some((summary, changed, if changed { change } else { None }))
}

/// Cross-system enrichment for one character+chapter: DIALOG-1 utterance count
/// (+ book hedge density) and NARR-1 chapter interiority. Best-effort; missing
/// stores → `None` fields.
fn enrich(
    project_root: &std::path::Path,
    book_slug: &str,
    name: &str,
    chapter_ord: u32,
    from_dialogue: bool,
    from_voice: bool,
) -> (Option<u32>, Option<f32>, Option<f32>) {
    let mut utt = None;
    let mut hedge = None;
    let mut interiority = None;
    // DIALOG-1: per-chapter utterance count + book hedge density.
    if from_dialogue {
        if let Ok(ds) = crate::dialogue::DialogueStore::open(project_root) {
            if let Ok(spans) = ds.spans_for_chapter(book_slug, chapter_ord) {
                let n = spans
                    .iter()
                    .filter(|s| s.attribution_name.as_deref() == Some(name))
                    .count();
                utt = Some(n as u32);
            }
            if let Ok(Some(fp)) = ds.fingerprint(book_slug, name) {
                hedge = Some(fp.hedge_density);
            }
        }
    }
    // NARR-1: chapter interiority ratio.
    if from_voice {
        if let Ok(ps) = crate::prose::ProseStore::open(project_root) {
            if let Ok(profiles) = ps.get_all(book_slug) {
                interiority = profiles
                    .iter()
                    .find(|p| p.scope == VoiceScope::Chapter(chapter_ord))
                    .and_then(|p| p.interiority_ratio);
            }
        }
    }
    (utt, hedge, interiority)
}

/// Extract the state chain for one character (LLM, sliding-window, lazy). Drops
/// and re-extracts from the first changed chapter forward. Returns the number of
/// chapters (re)extracted.
pub(crate) fn run_extraction(
    store: &CharStore,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    name: &str,
    arc: Option<&ArcDeclaration>,
) -> Result<usize> {
    let arc_type = arc.map(|a| a.arc_type.as_code());
    let chapters = chapters_of(h, book);
    let now = chrono::Utc::now().to_rfc3339();

    // Find the first chapter whose mention-text hash changed; re-extract from
    // there forward (the window feeds N→N+1).
    let mut dirty_from: Option<u32> = None;
    let mut chapter_texts: Vec<(u32, String)> = Vec::new();
    for (idx, ch) in chapters.iter().enumerate() {
        let ord = (idx + 1) as u32;
        let text = mention_text(&chapter_paragraphs(layout, h, ch.id), name);
        if text.trim().is_empty() {
            continue;
        }
        let hash = hash_str(&text);
        if dirty_from.is_none() && store.stored_state_hash(&book.slug, name, ord)? != Some(hash) {
            dirty_from = Some(ord);
        }
        chapter_texts.push((ord, text));
    }
    let Some(from) = dirty_from else {
        return Ok(0); // nothing changed
    };
    store.clear_states_from(&book.slug, name, from)?;

    let mut prev_state: Option<String> = None;
    let mut extracted = 0;
    for (ord, text) in &chapter_texts {
        if *ord < from {
            // Carry forward the existing summary as the window's prior state.
            if let Some(s) = store.states_for_character(&book.slug, name)?.into_iter().find(|s| s.chapter_ord == *ord) {
                prev_state = Some(s.state_summary);
            }
            continue;
        }
        let user = build_extraction_prompt(name, arc_type, prev_state.as_deref(), text);
        let raw = char_llm_call(cfg, EXTRACTION_SYSTEM, &user)?;
        let (summary, changed, change_desc) =
            parse_state(&raw).unwrap_or_else(|| ("(no state extracted)".into(), false, None));
        let (utt, hedge, interiority) = enrich(
            layout.root.as_path(),
            &book.slug,
            name,
            *ord,
            cfg.char.enrich_from_dialogue,
            cfg.char.enrich_from_voice,
        );
        let state = CharacterState {
            character_name: name.to_string(),
            chapter_ord: *ord,
            state_summary: summary.clone(),
            changed,
            change_description: change_desc,
            agency_score: None,
            active_count: 0,
            passive_count: 0,
            utterance_count: utt,
            chapter_hedge_density: hedge,
            chapter_interiority_ratio: interiority,
        };
        store.upsert_state(&book.slug, &state, &now, hash_str(text))?;
        prev_state = Some(summary);
        extracted += 1;
    }
    Ok(extracted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mentions_whole_word_and_stem() {
        assert!(mentions("Mara crossed the room", "Mara"));
        assert!(mentions("the room was Mara's", "Mara"));
        assert!(!mentions("Marandole spoke", "Mar")); // <5 char name = exact only
        assert!(mentions("Aldrics blade fell", "Aldric")); // 5-char stem (aldri…)
        assert!(mentions("Владимира позвали", "Владимир")); // RU declined → 5-char stem
        assert!(!mentions("the wall stood", "Mara"));
    }

    #[test]
    fn mention_text_filters_paragraphs() {
        let paras = vec![
            ("p1".into(), "Mara opened the door.".to_string()),
            ("p2".into(), "The hall was empty.".to_string()),
            ("p3".into(), "Aldric watched Mara leave.".to_string()),
        ];
        let t = mention_text(&paras, "Mara");
        assert!(t.contains("opened"));
        assert!(t.contains("watched"));
        assert!(!t.contains("hall was empty"));
    }

    #[test]
    fn extraction_prompt_has_window_fields() {
        let p = build_extraction_prompt("Mara", Some("positive_change"), Some("defers"), "Mara acted.");
        assert!(p.contains("Character name: Mara"));
        assert!(p.contains("positive_change"));
        assert!(p.contains("Previous state summary: defers"));
        assert!(p.contains("Mara acted."));
        // No prior state for chapter 1.
        let p1 = build_extraction_prompt("Mara", None, None, "x");
        assert!(p1.contains("no prior state"));
        assert!(p1.contains("not declared"));
    }

    #[test]
    fn parse_state_tolerant() {
        let raw = "sure: {\"state_summary\":\"Defers to family.\",\"changed\":false,\"change_description\":null}";
        let (s, changed, cd) = parse_state(raw).unwrap();
        assert_eq!(s, "Defers to family.");
        assert!(!changed);
        assert!(cd.is_none());
        // changed=true with a description.
        let raw2 = "{\"state_summary\":\"Defies him.\",\"changed\":true,\"change_description\":\"first defiance\"}";
        let (_s, c2, cd2) = parse_state(raw2).unwrap();
        assert!(c2);
        assert_eq!(cd2.as_deref(), Some("first defiance"));
        // Empty summary / no json → None.
        assert!(parse_state("{\"state_summary\":\"\"}").is_none());
        assert!(parse_state("no json").is_none());
    }
}
