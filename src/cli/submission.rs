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
        } => {
            let layout = ProjectLayout::new(project);
            layout.require_initialized()?;
            let cfg = Config::load_layered(&layout.config_path())?;
            let store =
                Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
            let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
            let book = super::resolve_user_book(&h, book_name.as_deref(), "submission digest")
                .map_err(Error::Store)?
                .clone();
            let d = ensure_digest(&layout, &cfg, &store, &h, &book, provider.as_deref(), refresh)?;
            print!("{}", d.as_context());
            Ok(())
        }
        SubmissionCommand::Query {
            book_name,
            provider,
        } => generate(project, Gen::Query, book_name.as_deref(), provider.as_deref()),
        SubmissionCommand::Synopsis {
            book_name,
            long,
            provider,
        } => generate(
            project,
            if long { Gen::SynopsisLong } else { Gen::SynopsisShort },
            book_name.as_deref(),
            provider.as_deref(),
        ),
        SubmissionCommand::Comps {
            book_name,
            provider,
        } => generate(project, Gen::Comps, book_name.as_deref(), provider.as_deref()),
        SubmissionCommand::Logline {
            book_name,
            provider,
        } => generate(project, Gen::Logline, book_name.as_deref(), provider.as_deref()),
    }
}

/// Load the cached digest if it still matches the live manuscript, else
/// (re)build it with an AI summary pass.  Shared by the `digest`
/// subcommand, every generator, and `plan analyze`.
pub(crate) fn ensure_digest(
    layout: &ProjectLayout,
    cfg: &Config,
    store: &Store,
    h: &Hierarchy,
    book: &crate::store::node::Node,
    provider: Option<&str>,
    refresh: bool,
) -> Result<BookDigest> {
    // Deterministic skeleton (shared with `manuscript` / `docx`).
    let (meta, chapters) = super::manuscript::build_model(layout, cfg, h, book, None, None, None)?;
    let titles: Vec<String> = chapters.iter().map(|c| c.title.clone()).collect();
    let characters = system_book_titles(store, h, crate::store::SYSTEM_TAG_CHARACTERS, 30);
    let threads = system_book_titles(store, h, crate::store::SYSTEM_TAG_THREADS, 30);

    if !refresh {
        if let Some(cached) = BookDigest::load(&layout.root, &book.slug) {
            if cached.matches(&meta.title, meta.word_count, &titles, &characters, &threads) {
                eprintln!("(cached digest; --refresh to rebuild)");
                return Ok(cached);
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
    let hash =
        BookDigest::compute_hash(&meta.title, meta.word_count, &titles, &characters, &threads);
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
    Ok(digest)
}

// ── generators (P3.2) ───────────────────────────────────────────────

use crate::submission_gen::SubmissionKind as Gen;

fn generate(
    project: &Path,
    kind: Gen,
    book_name: Option<&str>,
    provider: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let book = super::resolve_user_book(&h, book_name, "submission")
        .map_err(Error::Store)?
        .clone();

    let digest = ensure_digest(&layout, &cfg, &store, &h, &book, provider, false)?;

    let ai = AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    // prompts.hjson override → built-in.
    let system = resolve_system(&layout, kind);
    let prompt = kind.user_prompt(&digest.as_context());
    eprintln!("inkhaven submission {} · model: {model}", kind.slug());
    let raw = run_blocking(&ai, model, &system, &prompt)?;
    let draft = raw.trim().to_string();
    if draft.is_empty() {
        return Err(Error::Store("submission: empty response".into()));
    }

    // Reload the hierarchy (ensure_digest didn't mutate it) and upsert the
    // draft as a paragraph in the Submissions book.
    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let slug = write_draft(&store, &cfg, &layout, &h, kind.title(), &draft)?;
    println!("{draft}\n");
    eprintln!(
        "submission {}: saved to Submissions/{slug} — reference it with `inkhaven submissions add --draft {slug} …`",
        kind.slug(),
    );
    Ok(())
}

/// Resolve the system prompt: a matching `prompts.hjson` entry overrides
/// the built-in (the Prompts-book paragraph tier is a P3.3 follow-up).
fn resolve_system(layout: &ProjectLayout, kind: Gen) -> String {
    let path = layout.root.join("prompts.hjson");
    crate::ai::prompts::PromptLibrary::load(&path)
        .ok()
        .and_then(|lib| lib.find(kind.slug()).map(|p| p.template.clone()))
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| kind.builtin_system().to_string())
}

/// Upsert a draft paragraph (by title) into the `Submissions` book.
/// Returns the paragraph slug.
fn write_draft(
    store: &Store,
    cfg: &Config,
    layout: &ProjectLayout,
    h: &Hierarchy,
    title: &str,
    body_text: &str,
) -> Result<String> {
    let book = h
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_SUBMISSIONS)
        })
        .cloned()
        .ok_or_else(|| Error::Store("submission: no Submissions book (reopen the project)".into()))?;
    let body = format!("= {title}\n\n{}\n", body_text.trim());

    // Overwrite an existing draft of the same kind rather than piling up.
    if let Some(existing) = h.collect_subtree(book.id).into_iter().find_map(|id| {
        h.get(id).filter(|n| {
            n.kind == NodeKind::Paragraph && n.title.trim().eq_ignore_ascii_case(title)
        })
    }) {
        let mut node = existing.clone();
        if let Some(rel) = &node.file {
            let abs = layout.root.join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        }
        store.update_paragraph_content(&mut node, body.as_bytes())?;
        return Ok(node.slug);
    }

    let mut node = store.create_node(
        cfg,
        h,
        NodeKind::Paragraph,
        title,
        Some(&book),
        None,
        crate::store::InsertPosition::End,
    )?;
    if let Some(rel) = &node.file {
        let abs = layout.root.join(rel);
        std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        store.update_paragraph_content(&mut node, body.as_bytes())?;
    }
    Ok(node.slug)
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
