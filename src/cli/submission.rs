//! 1.3.1 SUBMISSION-1 P3 — `inkhaven submission` subcommand (singular).
//!
//! The AI submission-package side (distinct from the plural `submissions`
//! tracker): build the [`BookDigest`] context substrate, and — in P3.2 —
//! the query letter / synopsis / comp-title / logline generators that
//! consume it.

use std::path::Path;

use crate::ai::AiClient;
use crate::book_digest::{BookDigest, ChapterSummary};
use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

use super::SubmissionCommand;

const DIGEST_SYSTEM: &str = "You are a precise literary summarizer. Given one chapter of a \
novel, reply with a SINGLE sentence capturing only its key plot development — what changes by \
the end. No preamble, no quotation marks, do not begin with 'This chapter' or the chapter title.";

pub fn run(project: &Path, cmd: SubmissionCommand) -> Result<()> {
    match cmd {
        SubmissionCommand::Digest {
            book_name,
            refresh,
            provider,
        } => digest(project, book_name.as_deref(), refresh, provider.as_deref()),
    }
}

fn digest(
    project: &Path,
    book_name: Option<&str>,
    refresh: bool,
    provider: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = super::resolve_user_book(&h, book_name, "submission digest")
        .map_err(Error::Store)?
        .clone();

    // Deterministic skeleton (shared with `manuscript` / `docx`).
    let (meta, chapters) =
        super::manuscript::build_model(&layout, &cfg, &h, &book, None, None, None)?;
    let titles: Vec<String> = chapters.iter().map(|c| c.title.clone()).collect();
    let characters = system_book_titles(&store, &h, crate::store::SYSTEM_TAG_CHARACTERS, 30);
    let threads = system_book_titles(&store, &h, crate::store::SYSTEM_TAG_THREADS, 30);
    let hash =
        BookDigest::compute_hash(&meta.title, meta.word_count, &titles, &characters, &threads);

    if !refresh {
        if let Some(cached) = BookDigest::load(&layout.root, &book.slug) {
            if cached.matches(&meta.title, meta.word_count, &titles, &characters, &threads) {
                eprintln!("(cached digest; --refresh to rebuild)");
                print!("{}", cached.as_context());
                return Ok(());
            }
        }
    }

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    eprintln!(
        "inkhaven submission digest · model: {model} · {} chapter(s)",
        chapters.len(),
    );

    let mut summaries = Vec::with_capacity(chapters.len());
    for (i, ch) in chapters.iter().enumerate() {
        eprint!("  [{}/{}] {} ", i + 1, chapters.len(), ch.title);
        let prose = truncate_chars(&ch.paragraphs.join("\n\n"), 6000);
        let prompt = format!("CHAPTER: {}\n\n{prose}", ch.title);
        let raw = run_blocking(&ai, model, DIGEST_SYSTEM, &prompt)?;
        eprintln!();
        summaries.push(ChapterSummary {
            title: ch.title.clone(),
            summary: one_line(&raw),
        });
    }

    let digest = BookDigest {
        book_slug: book.slug.clone(),
        title: meta.title,
        author: meta.byline,
        word_count: meta.word_count,
        chapters: summaries,
        characters,
        threads,
        content_hash: hash,
    };
    digest.save(&layout.root).map_err(Error::Store)?;
    print!("{}", digest.as_context());
    Ok(())
}

/// Paragraph titles under the system book tagged `tag` (the names in the
/// Characters / Threads books), capped at `limit`.
fn system_book_titles(store: &Store, h: &Hierarchy, tag: &str, limit: usize) -> Vec<String> {
    let Some(book) = h
        .iter()
        .find(|n| n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(tag))
    else {
        return Vec::new();
    };
    let _ = store; // titles come from the hierarchy; store kept for symmetry
    h.collect_subtree(book.id)
        .into_iter()
        .filter_map(|id| h.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.trim().to_string())
        .filter(|t| !t.is_empty())
        .take(limit)
        .collect()
}

fn run_blocking(ai: &AiClient, model: &str, system: &str, prompt: &str) -> Result<String> {
    crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(system.to_string()),
        prompt.to_string(),
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))
}

/// First non-empty line, trimmed of surrounding quotes — defends against a
/// model that adds a preamble line or wraps the sentence in quotes.
fn one_line(raw: &str) -> String {
    raw.lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("")
        .trim_matches(|c| c == '"' || c == '\'')
        .to_string()
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    s.chars().take(max).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_line_strips_quotes_and_preamble() {
        assert_eq!(one_line("\"Mara confronts her father.\""), "Mara confronts her father.");
        assert_eq!(one_line("\n  Here it is:\nactual line\n"), "Here it is:");
        assert_eq!(one_line("plain"), "plain");
    }

    #[test]
    fn truncate_is_char_safe() {
        let s = "héllo wörld";
        assert_eq!(truncate_chars(s, 5).chars().count(), 5);
        assert_eq!(truncate_chars(s, 100), s);
    }
}
