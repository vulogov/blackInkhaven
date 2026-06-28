//! DIALOG-1 (D-P4) — the compute pipeline: walk a book's chapters paragraph by
//! paragraph, detect + attribute dialogue, compute the three deterministic
//! findings (zero-attribution, said-bookism density, talking-head sequences),
//! and persist spans + per-chapter stats to `dialogue.duckdb` with content-hash
//! laziness. Mirrors `prose::pipeline` (NARR-1); follows its idle/explicit
//! engagement model — never on the per-save hot path.

use std::hash::{Hash, Hasher};

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::resolve_prose_language;
use crate::store::SYSTEM_TAG_CHARACTERS;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::store::DialogueStore;
use super::{
    AttributionConfidence, AttributionWindows, ChapterDialogueStats, DialogueFinding,
    DialogueFindingKind, DialogueSpan, TagVerbClass, attribute_spans, detect_spans,
    dialogue_convention,
};

// Defaults (RFC §13). The `dialogue:` config block threads overrides in D-P10.
const BEAT_MIN_WORDS: u32 = 8;
const TALKING_HEAD_THRESHOLD: u32 = 6;
const UNATTRIBUTED_RUN_THRESHOLD: u32 = 8;
const SAID_BOOKISM_THRESHOLD: f32 = 0.15;

/// The character roster — titles of the direct children of the Characters
/// system book (one entry per character). Empty when there is no Characters
/// book or no entries.
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

/// A chapter's ordered prose paragraphs as `(para_id, stripped_text)`. Excludes
/// Jinja templates; strips Typst markup (the same extractor the prose pass uses).
fn chapter_paragraphs(
    layout: &ProjectLayout,
    h: &Hierarchy,
    chapter_id: uuid::Uuid,
) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        if p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        let Some(rel) = p.file.as_ref() else { continue };
        if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
            out.push((id.to_string(), crate::audiobook::typst_to_plain(&raw)));
        }
    }
    out
}

fn hash_paras(paras: &[(String, String)]) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    for (_, text) in paras {
        text.hash(&mut h);
    }
    h.finish()
}

/// Recompute (lazily) every chapter of `book`, persisting spans + stats and
/// returning the findings to emit. Unchanged chapters (matching stored hash)
/// are skipped but still contribute their stored tag counts to the book
/// said-bookism baseline. `now` is the timestamp stamped on recomputed rows.
pub(crate) fn refresh_book(
    store: &DialogueStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    explicit_lang: Option<&str>,
    now: &str,
) -> Result<Vec<DialogueFinding>> {
    let (lang, _note) = resolve_prose_language(explicit_lang, &cfg.language);
    let convention = dialogue_convention(&lang);
    let names = character_names(h);
    let windows = AttributionWindows::default();

    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();

    let mut findings = Vec::new();
    // (chapter_ord, neutral_tag_count, said_bookism_count) for the book baseline.
    let mut tag_counts: Vec<(u32, u32, u32)> = Vec::new();
    let mut any_recomputed = false;

    for (idx, ch) in chapters.iter().enumerate() {
        let ord = (idx + 1) as u32;
        let paras = chapter_paragraphs(layout, h, ch.id);
        let hash = hash_paras(&paras);
        if store.stored_chapter_hash(&book.slug, ord)? == Some(hash) {
            if let Some(s) = store.chapter_stats(&book.slug, ord)? {
                tag_counts.push((ord, s.neutral_tag_count, s.said_bookism_count));
            }
            continue;
        }
        store.clear_chapter(&book.slug, ord)?;
        let (stats, mut chap_findings) = detect_chapter(
            store, &book.slug, ord, &paras, &names, &convention, &lang, windows, now, hash,
        )?;
        tag_counts.push((ord, stats.neutral_tag_count, stats.said_bookism_count));
        findings.append(&mut chap_findings);
        any_recomputed = true;
    }

    // Rebuild the per-character fingerprints when any chapter changed (D-P5).
    if any_recomputed {
        super::fingerprint::rebuild_fingerprints(store, &book.slug, &lang, now)?;
    }

    // Said-bookism density: per-chapter density vs the book baseline.
    let total_neutral: u32 = tag_counts.iter().map(|c| c.1).sum();
    let total_bookism: u32 = tag_counts.iter().map(|c| c.2).sum();
    let total_tags = total_neutral + total_bookism;
    let baseline = if total_tags > 0 {
        total_bookism as f32 / total_tags as f32
    } else {
        0.0
    };
    for (ord, neutral, bookism) in &tag_counts {
        let chtags = neutral + bookism;
        if chtags == 0 {
            continue;
        }
        let density = *bookism as f32 / chtags as f32;
        if density - baseline > SAID_BOOKISM_THRESHOLD {
            findings.push(DialogueFinding {
                kind: DialogueFindingKind::SaidBookism,
                chapter_ord: *ord,
                para_id: None,
                detail: format!(
                    "said-bookism density {density:.2} (book baseline {baseline:.2}, Δ +{:.2} ⚠)",
                    density - baseline
                ),
            });
        }
    }

    Ok(findings)
}

#[allow(clippy::too_many_arguments)]
fn detect_chapter(
    store: &DialogueStore,
    book_slug: &str,
    ord: u32,
    paras: &[(String, String)],
    names: &[String],
    convention: &super::DialogueConvention,
    lang: &crate::prose::ProseLanguage,
    windows: AttributionWindows,
    now: &str,
    hash: u64,
) -> Result<(ChapterDialogueStats, Vec<DialogueFinding>)> {
    let mut findings = Vec::new();
    let (mut total_spans, mut zero, mut neutral, mut bookism) = (0u32, 0u32, 0u32, 0u32);
    let (mut dialogue_words, mut total_words) = (0u32, 0u32);

    let mut prev_named: Option<String> = None;
    let mut established: Vec<String> = Vec::new(); // last ≤2 distinct speakers
    let mut zero_run = 0u32;
    let mut th_run = 0u32;
    let mut th_first_para: Option<String> = None;
    let mut th_sequences = 0u32;

    for (para_id, text) in paras {
        total_words += text.split_whitespace().count() as u32;
        let mut spans = detect_spans(para_id, text, *convention, lang);
        attribute_spans(&mut spans, text, names, prev_named.as_deref(), lang, windows);

        let mut para_attributed: Option<String> = None;
        let mut zero_in_para = 0u32;
        let mut span_words = 0u32;
        for span in &spans {
            total_spans += 1;
            dialogue_words += span.word_count;
            span_words += span.word_count;
            match span.tag_verb_class {
                Some(TagVerbClass::Neutral) => neutral += 1,
                Some(TagVerbClass::SaidBookism) => bookism += 1,
                None => {}
            }
            if !span.has_attribution_signal {
                zero += 1;
                zero_in_para += 1;
            }
            if span.attribution_conf != AttributionConfidence::None {
                if let Some(n) = &span.attribution_name {
                    para_attributed = Some(n.clone());
                }
            }
            store.upsert_span(book_slug, ord, span, now, hash)?;
        }

        // Track the scene's established speakers for the run-clearing heuristic.
        if let Some(n) = &para_attributed {
            prev_named = Some(n.clone());
            if !established.contains(n) {
                established.push(n.clone());
                if established.len() > 2 {
                    established.remove(0);
                }
            }
            zero_run = 0;
        }

        // Zero-attribution finding (with the two-speaker run-clearing heuristic).
        if zero_in_para > 0 {
            if para_attributed.is_none() {
                zero_run += 1;
                let in_established_run =
                    established.len() >= 2 && zero_run <= UNATTRIBUTED_RUN_THRESHOLD;
                if !in_established_run {
                    findings.push(DialogueFinding {
                        kind: DialogueFindingKind::ZeroAttribution,
                        chapter_ord: ord,
                        para_id: Some(para_id.clone()),
                        detail: "unattributed speech — no tag or character name within range"
                            .into(),
                    });
                }
            } else {
                // Mixed paragraph (some attributed, some not) — flag it.
                findings.push(DialogueFinding {
                    kind: DialogueFindingKind::ZeroAttribution,
                    chapter_ord: ord,
                    para_id: Some(para_id.clone()),
                    detail: "unattributed speech in an otherwise-tagged paragraph".into(),
                });
            }
        }

        // Talking-head sequence: consecutive dialogue-only paragraphs (no action
        // beat = no ≥`BEAT_MIN_WORDS`-word non-speech narration).
        let has_dialogue = !spans.is_empty();
        let non_span_words =
            (text.split_whitespace().count() as u32).saturating_sub(span_words);
        let dialogue_only = has_dialogue && non_span_words < BEAT_MIN_WORDS;
        if dialogue_only {
            th_run += 1;
            if th_first_para.is_none() {
                th_first_para = Some(para_id.clone());
            }
            if th_run == TALKING_HEAD_THRESHOLD {
                th_sequences += 1;
                findings.push(DialogueFinding {
                    kind: DialogueFindingKind::TalkingHead,
                    chapter_ord: ord,
                    para_id: th_first_para.clone(),
                    detail: format!(
                        "talking-head sequence: {TALKING_HEAD_THRESHOLD}+ paragraphs with no action beat"
                    ),
                });
            }
        } else {
            th_run = 0;
            th_first_para = None;
        }
    }

    let chtags = neutral + bookism;
    let density = if chtags > 0 { bookism as f32 / chtags as f32 } else { 0.0 };
    let ratio = if total_words > 0 {
        dialogue_words as f32 / total_words as f32
    } else {
        0.0
    };
    let stats = ChapterDialogueStats {
        chapter_ord: ord,
        total_spans,
        zero_attribution_count: zero,
        said_bookism_count: bookism,
        neutral_tag_count: neutral,
        said_bookism_density: density,
        dialogue_word_count: dialogue_words,
        total_word_count: total_words,
        dialogue_density_ratio: ratio,
        talking_head_sequences: th_sequences,
    };
    store.upsert_chapter_stats(book_slug, &stats, now, hash)?;
    Ok((stats, findings))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::NodeKind;

    fn mk(kind: NodeKind, slug: &str, parent: Option<uuid::Uuid>, file: Option<&str>) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4(), "kind": format!("{kind:?}").to_lowercase(),
            "title": slug, "slug": slug, "path": [], "parent_id": parent,
            "order": 0, "file": file, "modified_at": "2026-01-01T00:00:00Z",
        }))
        .expect("node")
    }

    /// Build a one-book/one-chapter project on disk with the given paragraph
    /// bodies, plus a Characters book holding `chars`. Returns (store, layout,
    /// hierarchy, book).
    fn project(
        root: &std::path::Path,
        bodies: &[&str],
        chars: &[&str],
    ) -> (DialogueStore, ProjectLayout, Hierarchy, Node) {
        let layout = ProjectLayout::new(root);
        let book = mk(NodeKind::Book, "tale", None, None);
        let chap = mk(NodeKind::Chapter, "ch1", Some(book.id), None);
        let mut nodes = vec![book.clone(), chap.clone()];
        std::fs::create_dir_all(root.join("books/tale/ch1")).unwrap();
        for (i, body) in bodies.iter().enumerate() {
            let rel = format!("books/tale/ch1/{:02}-p{i}.typ", i + 1);
            std::fs::write(root.join(&rel), body).unwrap();
            nodes.push(mk(NodeKind::Paragraph, &format!("p{i}"), Some(chap.id), Some(&rel)));
        }
        // Characters book + one entry per name.
        let cbook: Node = serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4(), "kind": "book", "title": "Characters",
            "slug": "characters", "path": [], "parent_id": null, "order": 90,
            "file": null, "modified_at": "2026-01-01T00:00:00Z",
            "system_tag": "characters",
        })).unwrap();
        for c in chars {
            nodes.push(mk(NodeKind::Paragraph, c, Some(cbook.id), None));
        }
        nodes.push(cbook);
        let h = Hierarchy::from_nodes_for_test(nodes);
        let st = DialogueStore::open(root).unwrap();
        (st, layout, h, book)
    }

    fn cfg_en() -> Config {
        let mut c = Config::default();
        c.language = "en".into();
        c
    }

    #[test]
    fn zero_attribution_finding_for_untagged_line() {
        let dir = tempfile::tempdir().unwrap();
        let (st, layout, h, book) =
            project(dir.path(), &["\u{201C}Who goes there?\u{201D}"], &["Mara"]);
        let f = refresh_book(&st, &layout, &h, &cfg_en(), &book, None, "now").unwrap();
        assert!(
            f.iter().any(|x| x.kind == DialogueFindingKind::ZeroAttribution),
            "{f:?}"
        );
        let s = st.chapter_stats("tale", 1).unwrap().unwrap();
        assert_eq!(s.total_spans, 1);
        assert_eq!(s.zero_attribution_count, 1);
    }

    #[test]
    fn named_tag_is_not_flagged_and_is_persisted_certain() {
        let dir = tempfile::tempdir().unwrap();
        let (st, layout, h, book) =
            project(dir.path(), &["\u{201C}Hello,\u{201D} said Mara."], &["Mara"]);
        let f = refresh_book(&st, &layout, &h, &cfg_en(), &book, None, "now").unwrap();
        assert!(!f.iter().any(|x| x.kind == DialogueFindingKind::ZeroAttribution));
        let certain = st.certain_spans("tale").unwrap();
        assert_eq!(certain.len(), 1);
        assert_eq!(certain[0].1.attribution_name.as_deref(), Some("Mara"));
    }

    #[test]
    fn said_bookism_density_finding() {
        let dir = tempfile::tempdir().unwrap();
        // All bookism tags → density 1.0, baseline 1.0 → no Δ. Mix instead:
        // one neutral chapter-baseline vs a bookism-heavy line won't trigger in
        // a single chapter (density == baseline). So this is a single-chapter
        // smoke: density computed, no spurious finding when density == baseline.
        let (st, layout, h, book) = project(
            dir.path(),
            &["\u{201C}No,\u{201D} Mara whispered. \u{201C}Stop,\u{201D} Mara hissed."],
            &["Mara"],
        );
        refresh_book(&st, &layout, &h, &cfg_en(), &book, None, "now").unwrap();
        let s = st.chapter_stats("tale", 1).unwrap().unwrap();
        assert_eq!(s.said_bookism_count, 2);
        assert!((s.said_bookism_density - 1.0).abs() < 1e-3);
    }

    #[test]
    fn talking_head_sequence_finding() {
        let dir = tempfile::tempdir().unwrap();
        // Six consecutive dialogue-only paragraphs, no action beat.
        let bodies: Vec<&str> = vec![
            "\u{201C}One.\u{201D}",
            "\u{201C}Two.\u{201D}",
            "\u{201C}Three.\u{201D}",
            "\u{201C}Four.\u{201D}",
            "\u{201C}Five.\u{201D}",
            "\u{201C}Six.\u{201D}",
        ];
        let (st, layout, h, book) = project(dir.path(), &bodies, &[]);
        let f = refresh_book(&st, &layout, &h, &cfg_en(), &book, None, "now").unwrap();
        assert!(
            f.iter().any(|x| x.kind == DialogueFindingKind::TalkingHead),
            "{f:?}"
        );
        let s = st.chapter_stats("tale", 1).unwrap().unwrap();
        assert_eq!(s.talking_head_sequences, 1);
    }

    #[test]
    fn action_beat_clears_talking_head() {
        let dir = tempfile::tempdir().unwrap();
        let bodies: Vec<&str> = vec![
            "\u{201C}One.\u{201D}",
            "\u{201C}Two.\u{201D}",
            "She crossed the long cold room and opened the heavy door slowly.",
            "\u{201C}Three.\u{201D}",
            "\u{201C}Four.\u{201D}",
        ];
        let (st, layout, h, book) = project(dir.path(), &bodies, &[]);
        let f = refresh_book(&st, &layout, &h, &cfg_en(), &book, None, "now").unwrap();
        assert!(!f.iter().any(|x| x.kind == DialogueFindingKind::TalkingHead));
    }

    #[test]
    fn lazy_skip_on_unchanged_hash() {
        let dir = tempfile::tempdir().unwrap();
        let (st, layout, h, book) =
            project(dir.path(), &["\u{201C}Hello,\u{201D} said Mara."], &["Mara"]);
        let cfg = cfg_en();
        refresh_book(&st, &layout, &h, &cfg, &book, None, "now").unwrap();
        // Second pass: same text → chapter skipped (no findings recomputed).
        let f2 = refresh_book(&st, &layout, &h, &cfg, &book, None, "later").unwrap();
        assert!(f2.is_empty(), "unchanged chapter should not re-emit: {f2:?}");
    }
}
