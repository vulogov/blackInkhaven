//! `inkhaven style` — run the editor's deterministic style-warning detectors
//! (filter-words, repeated-phrase, show-don't-tell, anachronism) over the whole
//! manuscript and print a report. CLI/CI parity for the in-editor overlay
//! (`Ctrl+V w`): zero AI, deterministic, offline. Reads the same
//! `editor.style_warnings.*` config the overlay uses.

use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;
use crate::tui::style_warnings::{
    AnachronismDetector, FilterWordsDetector, RepeatedPhraseDetector, ShowDontTellDetector,
};

#[derive(Default, Clone)]
struct Counts {
    filter: usize,
    repeated: usize,
    show: usize,
    anachronism: usize,
}

impl Counts {
    fn total(&self) -> usize {
        self.filter + self.repeated + self.show + self.anachronism
    }
}

pub fn run(
    project: &Path,
    book_name: Option<String>,
    language: Option<String>,
    json: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let h = Hierarchy::load(&store)?;

    let lang = language.unwrap_or_else(|| cfg.language.clone());
    let sw = &cfg.editor.style_warnings;
    // The per-line detectors are built once; the repeated-phrase detector is
    // windowed per paragraph, so it's rebuilt in the loop.
    let filter = FilterWordsDetector::new(&sw.filter_words, &lang);
    let show = ShowDontTellDetector::new(&sw.show_dont_tell, &lang);
    let anach = AnachronismDetector::new(&sw.anachronism);

    // Non-system manuscript books (optionally just the named one).
    let books: Vec<_> = h
        .iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .filter(|n| {
            book_name
                .as_deref()
                .is_none_or(|bn| n.title.eq_ignore_ascii_case(bn) || n.slug.eq_ignore_ascii_case(bn))
        })
        .map(|n| (n.id, n.title.clone()))
        .collect();
    if books.is_empty() {
        println!(
            "no manuscript book found{}.",
            book_name.as_deref().map(|b| format!(" named `{b}`")).unwrap_or_default()
        );
        return Ok(());
    }

    let mut totals = Counts::default();
    let mut per_para: Vec<(String, Counts)> = Vec::new();
    let mut paragraphs = 0usize;
    // 1.8.1 — quantitative readability (lexical diversity + sentence rhythm),
    // accumulated per book alongside the span warnings.
    let mut read_by_book: Vec<(String, crate::tui::readability::ReadabilityReport)> = Vec::new();

    for (book_id, book_title) in &books {
        let mut rd = crate::tui::readability::Readability::default();
        for id in h.collect_subtree(*book_id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            paragraphs += 1;
            let Ok(Some(bytes)) = store.get_content(id) else { continue };
            let text = String::from_utf8_lossy(&bytes);
            rd.add(&text);
            let lines: Vec<String> = text.lines().map(str::to_string).collect();

            let mut c = Counts::default();
            for line in &lines {
                c.filter += filter.detect(line).len();
                c.show += show.detect(line).len();
                c.anachronism += anach.detect(line).len();
            }
            c.repeated += RepeatedPhraseDetector::new(&sw.repeated_phrases, &lang, &lines).total_hits();

            if c.total() > 0 {
                totals.filter += c.filter;
                totals.repeated += c.repeated;
                totals.show += c.show;
                totals.anachronism += c.anachronism;
                per_para.push((node.title.clone(), c));
            }
        }
        if !rd.is_empty() {
            read_by_book.push((book_title.clone(), rd.finish()));
        }
    }

    per_para.sort_by(|a, b| b.1.total().cmp(&a.1.total()));
    let scope = book_name.as_deref().unwrap_or("manuscript");

    if json {
        let paras: Vec<_> = per_para
            .iter()
            .map(|(title, c)| {
                serde_json::json!({
                    "title": title,
                    "filter_words": c.filter,
                    "repeated_phrase": c.repeated,
                    "show_dont_tell": c.show,
                    "anachronism": c.anachronism,
                    "total": c.total(),
                })
            })
            .collect();
        let readability: Vec<_> = read_by_book
            .iter()
            .map(|(title, r)| {
                serde_json::json!({
                    "book": title,
                    "tokens": r.tokens,
                    "types": r.types,
                    "ttr": r.ttr,
                    "sentences": r.sentences,
                    "mean_sentence_len": r.mean_sentence_len,
                    "stdev_sentence_len": r.stdev_sentence_len,
                    "rhythm_cv": r.rhythm_cv(),
                    "longest_sentence": r.longest_sentence,
                })
            })
            .collect();
        let out = serde_json::json!({
            "scope": scope,
            "language": lang,
            "paragraphs_total": paragraphs,
            "paragraphs_flagged": per_para.len(),
            "totals": {
                "filter_words": totals.filter,
                "repeated_phrase": totals.repeated,
                "show_dont_tell": totals.show,
                "anachronism": totals.anachronism,
                "total": totals.total(),
            },
            "paragraphs": paras,
            "readability": readability,
        });
        println!("{}", serde_json::to_string_pretty(&out).unwrap_or_default());
        return Ok(());
    }

    println!(
        "style · {scope} ({lang}) — {} of {paragraphs} paragraph(s) flagged",
        per_para.len()
    );
    println!("  filter-words:    {}", totals.filter);
    println!("  repeated-phrase: {}", totals.repeated);
    println!("  show-dont-tell:  {}", totals.show);
    println!("  anachronism:     {}", totals.anachronism);
    println!("  total:           {}", totals.total());

    if !per_para.is_empty() {
        println!("\ntop paragraphs (f=filter r=repeat s=show-tell a=anachronism):");
        for (title, c) in per_para.iter().take(15) {
            println!(
                "  {:>4}  {}  (f{} r{} s{} a{})",
                c.total(),
                title,
                c.filter,
                c.repeated,
                c.show,
                c.anachronism
            );
        }
    }

    if !read_by_book.is_empty() {
        println!("\nreadability (per book):");
        for (title, r) in &read_by_book {
            let monotone = if r.sentences >= 8 && r.rhythm_cv() < 0.4 { "  ⚠ monotone rhythm" } else { "" };
            println!("  {title}");
            println!(
                "    lexical diversity · {} types / {} tokens · TTR {:.2}",
                r.types, r.tokens, r.ttr
            );
            println!(
                "    sentence rhythm   · {} sentences · {:.1} words avg (±{:.1}, CV {:.2}) · longest {}{}",
                r.sentences,
                r.mean_sentence_len,
                r.stdev_sentence_len,
                r.rhythm_cv(),
                r.longest_sentence,
                monotone,
            );
        }
    }
    Ok(())
}
