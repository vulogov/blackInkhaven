//! `inkhaven language doctor` — a health report for a language sub-book:
//! phonotactic/homophone/coverage checks over its dictionary + manuscript usage.
//! Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// health report for a language
/// sub-book.  Walks every chapter, counts entries +
/// rules + samples, computes coverage metrics, and
/// emits a human-readable summary on stdout.  Exit
/// code 0 always — informational, not a gate.
///
/// Coverage gap analysis (§13 of the proposal):
///   * count manuscript words (working language) that
///     don't appear as translations in this language's
///     dictionary.  Surfaces vocabulary the author has
///     written in prose but hasn't yet defined a
///     translation for.
///   * count dictionary entries that lack examples —
///     half-finished work.
///   * count entries that lack inflection paradigms —
///     hint that the lexicon overlay won't catch
///     inflected forms for those words.
pub(crate) fn doctor(project: &Path, language: &str, json: bool) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    // Walk each chapter's paragraphs.  We don't reach
    // for the in-memory TUI helpers because doctor /
    // export need to run from a headless CLI process.
    let chapters = hierarchy.children_of(Some(lang_book.id));
    let mut dict_entries: Vec<(String, crate::language_entry::DictionaryEntry)> =
        Vec::new();
    let mut dict_unparseable = 0usize;
    let mut grammar_count = 0usize;
    let mut phonology_count = 0usize;
    let mut sample_count = 0usize;
    let mut meta: Option<crate::language_entry::MetaOverview> = None;
    for chapter in &chapters {
        let title_lc = chapter.title.to_lowercase();
        let paragraphs: Vec<_> = hierarchy
            .collect_subtree(chapter.id)
            .into_iter()
            .filter_map(|id| hierarchy.get(id))
            .filter(|n| n.kind == NodeKind::Paragraph)
            .cloned()
            .collect();
        match title_lc.as_str() {
            "dictionary" => {
                for p in &paragraphs {
                    let Ok(Some(bytes)) = store.get_content(p.id) else {
                        continue;
                    };
                    let Ok(body) = std::str::from_utf8(&bytes) else {
                        continue;
                    };
                    match crate::language_entry::parse(body) {
                        Ok(Some(e)) => dict_entries.push((p.title.clone(), e)),
                        Ok(None) => dict_unparseable += 1,
                        Err(_) => dict_unparseable += 1,
                    }
                }
            }
            "grammar" => grammar_count = paragraphs.len(),
            "phonology" => phonology_count = paragraphs.len(),
            "sample texts" => sample_count = paragraphs.len(),
            "meta" => {
                for p in &paragraphs {
                    if p.title.eq_ignore_ascii_case("overview") {
                        let Ok(Some(bytes)) = store.get_content(p.id) else {
                            continue;
                        };
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            if let Ok(Some(m)) =
                                crate::language_entry::parse_meta_overview(body)
                            {
                                meta = Some(m);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let total_entries = dict_entries.len();
    let with_examples = dict_entries
        .iter()
        .filter(|(_, e)| !e.example.trim().is_empty())
        .count();
    let with_inflection = dict_entries
        .iter()
        .filter(|(_, e)| !e.inflection.is_empty())
        .count();
    let missing_examples = total_entries.saturating_sub(with_examples);
    let missing_inflection = total_entries.saturating_sub(with_inflection);

    // Coverage-gap analysis: which working-language
    // words in the manuscript have no dictionary
    // translation?  Walk every paragraph in user
    // books (skip system books — Notes / Places /
    // Characters / Artefacts / Prompts / Language /
    // Typst are reference material, not manuscript
    // prose) and collect their words.
    use unicode_segmentation::UnicodeSegmentation;
    let dictionary_translations: std::collections::HashSet<String> = dict_entries
        .iter()
        .filter_map(|(_, e)| {
            let t = e.translation.trim().to_lowercase();
            if t.is_empty() { None } else { Some(t) }
        })
        .collect();
    let mut manuscript_words: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        // Skip system-book content.
        let mut cursor = Some(node.id);
        let mut is_system = false;
        while let Some(id) = cursor {
            if let Some(n) = hierarchy.get(id) {
                if n.system_tag.is_some() {
                    is_system = true;
                    break;
                }
                cursor = n.parent_id;
            } else {
                break;
            }
        }
        if is_system {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(node.id) {
            if let Ok(body) = std::str::from_utf8(&bytes) {
                for w in UnicodeSegmentation::unicode_words(body) {
                    let lc = w.to_lowercase();
                    // Stop-word-ish filter: drop
                    // 1-letter "words" (a, I) — most
                    // are noise; the rest are too
                    // common to be worth flagging.
                    if lc.chars().count() < 2 {
                        continue;
                    }
                    manuscript_words.insert(lc);
                }
            }
        }
    }
    let manuscript_word_count = manuscript_words.len();
    let undefined_words: Vec<String> = manuscript_words
        .difference(&dictionary_translations)
        .cloned()
        .collect();

    // 1.2.13+ Phase D.1 — JSON mode emits the same
    // numbers in a structured form so CI pipelines
    // can gate on `coverage.with_example_pct < 80`
    // etc.  Returns early; the text render below
    // stays unchanged.
    if json {
        use serde_json::{json, Map, Value};
        let mut sorted_undefined: Vec<String> =
            undefined_words.iter().take(50).cloned().collect();
        sorted_undefined.sort();
        let example_pct = if total_entries > 0 {
            with_examples * 100 / total_entries
        } else {
            0
        };
        let inflection_pct = if total_entries > 0 {
            with_inflection * 100 / total_entries
        } else {
            0
        };
        let coverage_pct = if manuscript_word_count > 0 {
            manuscript_word_count.saturating_sub(undefined_words.len()) * 100
                / manuscript_word_count
        } else {
            0
        };
        let mut report = Map::new();
        report.insert("language".into(), Value::String(lang_book.title.clone()));
        report.insert(
            "meta".into(),
            meta.as_ref()
                .map(|m| json!({
                    "name": m.name,
                    "language_kind": m.language_kind,
                    "family": m.family,
                    "iso_code": m.iso_code,
                    "alphabet_count": m.alphabet.len(),
                    "reading_direction": m.reading_direction,
                }))
                .unwrap_or(Value::Null),
        );
        report.insert(
            "chapters".into(),
            json!({
                "dictionary_parseable": total_entries,
                "dictionary_unparseable": dict_unparseable,
                "grammar": grammar_count,
                "phonology": phonology_count,
                "sample_texts": sample_count,
            }),
        );
        report.insert(
            "coverage".into(),
            json!({
                "with_example": with_examples,
                "with_example_pct": example_pct,
                "with_paradigm": with_inflection,
                "with_paradigm_pct": inflection_pct,
                "missing_example": missing_examples,
                "missing_paradigm": missing_inflection,
            }),
        );
        report.insert(
            "manuscript_gap".into(),
            json!({
                "unique_words": manuscript_word_count,
                "uncovered_count": undefined_words.len(),
                "coverage_pct": coverage_pct,
                "uncovered_sample": sorted_undefined,
            }),
        );
        let s = serde_json::to_string_pretty(&Value::Object(report))
            .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
        println!("{s}");
        return Ok(());
    }

    // Emit the human-readable report.
    println!("Language doctor — `{}`", lang_book.title);
    println!();
    if let Some(m) = meta.as_ref() {
        if !m.name.is_empty() {
            println!("  name           : {}", m.name);
        }
        if !m.language_kind.is_empty() {
            println!("  kind           : {}", m.language_kind);
        }
        if !m.family.is_empty() {
            println!("  family         : {}", m.family);
        }
        if !m.iso_code.is_empty() {
            println!("  iso_code       : {}", m.iso_code);
        }
        if !m.alphabet.is_empty() {
            println!("  alphabet       : {} entries", m.alphabet.len());
        }
        if !m.reading_direction.is_empty() {
            println!("  direction      : {}", m.reading_direction);
        }
        println!();
    } else {
        println!("  Meta/overview  : MISSING or unparseable");
        println!();
    }
    println!("Chapters");
    println!("  Dictionary     : {total_entries} parseable entries");
    if dict_unparseable > 0 {
        println!(
            "                   {dict_unparseable} unparseable (no HJSON block — pre-Phase-B authoring)"
        );
    }
    println!("  Grammar        : {grammar_count} rules");
    println!("  Phonology      : {phonology_count} rules");
    println!("  Sample texts   : {sample_count} samples");
    println!();
    println!("Dictionary coverage");
    if total_entries > 0 {
        let example_pct = with_examples * 100 / total_entries;
        let inflection_pct = with_inflection * 100 / total_entries;
        println!(
            "  with example   : {with_examples}/{total_entries} ({example_pct}%)"
        );
        println!(
            "  with paradigm  : {with_inflection}/{total_entries} ({inflection_pct}%)"
        );
        if missing_examples > 0 {
            println!("  missing example: {missing_examples}");
        }
        if missing_inflection > 0 {
            println!(
                "  missing paradigm: {missing_inflection} (overlay won't catch inflected forms)"
            );
        }
    } else {
        println!("  no dictionary entries yet — try `inkhaven language add-word`");
    }
    println!();
    println!("Manuscript gap analysis");
    println!("  unique words (≥2 chars) in manuscript prose: {manuscript_word_count}");
    let undefined_count = undefined_words.len();
    if total_entries > 0 {
        let covered = manuscript_word_count.saturating_sub(undefined_count);
        let pct = if manuscript_word_count > 0 {
            covered * 100 / manuscript_word_count
        } else {
            0
        };
        println!("  covered by dictionary: {covered}/{manuscript_word_count} ({pct}%)");
        if undefined_count > 0 {
            println!("  uncovered words (sample, max 15):");
            let mut sample: Vec<&String> = undefined_words.iter().take(15).collect();
            sample.sort();
            for w in sample {
                println!("    · {w}");
            }
            if undefined_count > 15 {
                println!("    ... and {} more", undefined_count - 15);
            }
        }
    } else {
        println!("  (skipping — no dictionary entries to compare against)");
    }
    Ok(())
}
