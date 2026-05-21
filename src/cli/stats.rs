//! `inkhaven stats` — per-paragraph table snapshot of the project
//! (1.2.4+).
//!
//! Walks the hierarchy, emits one row per Paragraph node:
//! title · slug · status · words · target% (when set) · last
//! modified (relative). Default output is a fixed-width text
//! table; future variants (`--format=json`) can land without
//! breaking the default behaviour.
//!
//! Excludes system books (Help / Scripts / Typst / Prompts /
//! Places / Characters / Notes / Artefacts / Research) — they're
//! inkhaven internals, not manuscript content. `--book-name`
//! scopes to one user book the same way `inkhaven export` does.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

pub fn run(project: &Path, book_name: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;

    // Pick a scope, mirroring `inkhaven export`'s rules. Single
    // user book is implicit; multiple books require `--book-name`.
    let root_id = resolve_scope(&h, book_name)?;
    let candidates: Vec<&Node> = match root_id {
        Some(id) => h
            .collect_subtree(id)
            .into_iter()
            .filter_map(|nid| h.get(nid))
            .filter(|n| n.kind == NodeKind::Paragraph)
            .collect(),
        None => h
            .flatten()
            .into_iter()
            .map(|(n, _)| n)
            .filter(|n| n.kind == NodeKind::Paragraph)
            // No book filter: still drop system-book content
            // since those aren't manuscript paragraphs.
            .filter(|n| !is_in_system_book(&h, n))
            .collect(),
    };

    if candidates.is_empty() {
        println!("(no paragraphs)");
        return Ok(());
    }

    // Column widths
    let title_w = candidates
        .iter()
        .map(|n| display_width(&n.title).min(50))
        .max()
        .unwrap_or(20)
        .max(20);
    let slug_w = candidates
        .iter()
        .map(|n| display_width(&n.slug).min(30))
        .max()
        .unwrap_or(10)
        .max(10);
    let status_w = 6; // Napkin / First / etc., max 6
    let words_w = 7;
    let target_w = 8;
    let age_w = 10;

    println!(
        "{:<title_w$}  {:<slug_w$}  {:<status_w$}  {:>words_w$}  {:>target_w$}  {:>age_w$}",
        "TITLE",
        "SLUG",
        "STATUS",
        "WORDS",
        "TARGET",
        "AGE",
        title_w = title_w,
        slug_w = slug_w,
        status_w = status_w,
        words_w = words_w,
        target_w = target_w,
        age_w = age_w,
    );
    println!(
        "{}",
        "─".repeat(title_w + slug_w + status_w + words_w + target_w + age_w + 10)
    );

    // Totals
    let mut total_words: u64 = 0;
    let mut total_target: i64 = 0;
    let mut at_or_above_target: usize = 0;
    let mut with_target: usize = 0;
    let mut by_status: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();

    for n in &candidates {
        total_words += n.word_count;
        let target_str = match n.target_words {
            Some(t) if t > 0 => {
                with_target += 1;
                total_target += t as i64;
                let pct = (n.word_count as i64 * 100 / t as i64).clamp(0, 999);
                if pct >= 100 {
                    at_or_above_target += 1;
                }
                format!("{pct}%")
            }
            _ => "—".to_string(),
        };
        let status = n.status.as_deref().unwrap_or("—").to_string();
        *by_status.entry(status.clone()).or_insert(0) += 1;

        let age = humantime_short(
            chrono::Utc::now().signed_duration_since(n.modified_at).num_seconds().max(0) as u64,
        );

        println!(
            "{:<title_w$}  {:<slug_w$}  {:<status_w$}  {:>words_w$}  {:>target_w$}  {:>age_w$}",
            truncate(&n.title, title_w),
            truncate(&n.slug, slug_w),
            truncate(&status, status_w),
            n.word_count,
            target_str,
            age,
            title_w = title_w,
            slug_w = slug_w,
            status_w = status_w,
            words_w = words_w,
            target_w = target_w,
            age_w = age_w,
        );
    }

    println!();
    println!("Summary");
    println!("  paragraphs:    {}", candidates.len());
    println!("  total words:   {}", total_words);
    if with_target > 0 {
        println!(
            "  target words:  {} ({}/{} paragraphs at-or-above target)",
            total_target, at_or_above_target, with_target,
        );
    }
    println!("  by status:");
    for (k, v) in &by_status {
        println!("    {:<8} {}", k, v);
    }

    Ok(())
}

fn resolve_scope(
    h: &Hierarchy,
    book_name: Option<&str>,
) -> Result<Option<uuid::Uuid>> {
    let user_books: Vec<&Node> = h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();
    match book_name {
        Some(name) => {
            let needle = name.trim().to_ascii_lowercase();
            let pick = user_books.iter().copied().find(|b| {
                b.title.to_ascii_lowercase() == needle
                    || b.slug.to_ascii_lowercase() == needle
            });
            match pick {
                Some(book) => Ok(Some(book.id)),
                None => {
                    let listing = user_books
                        .iter()
                        .map(|b| format!("`{}`", b.title))
                        .collect::<Vec<_>>()
                        .join(", ");
                    Err(Error::Store(format!(
                        "stats: no book matches `--book-name {name}`. Available: {listing}"
                    )))
                }
            }
        }
        None => {
            if user_books.len() > 1 {
                let listing = user_books
                    .iter()
                    .map(|b| format!("`{}`", b.title))
                    .collect::<Vec<_>>()
                    .join(", ");
                return Err(Error::Store(format!(
                    "stats: project has {} user books — pass --book-name <name>. Available: {listing}",
                    user_books.len(),
                )));
            }
            Ok(user_books.first().map(|b| b.id))
        }
    }
}

fn is_in_system_book(h: &Hierarchy, n: &Node) -> bool {
    h.ancestors(n)
        .into_iter()
        .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some())
}

fn display_width(s: &str) -> usize {
    s.chars().count()
}

fn truncate(s: &str, max: usize) -> String {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max {
        return s.to_string();
    }
    let cut: String = chars.iter().take(max.saturating_sub(1)).collect();
    format!("{cut}…")
}

fn humantime_short(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86_400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86_400)
    }
}
