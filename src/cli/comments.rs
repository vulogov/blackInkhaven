//! 1.2.14+ Phase C.2 — `inkhaven comments …`
//! headless management of per-paragraph sidecar
//! comment files (`.comments.json`).
//!
//! Mirrors the in-TUI `Ctrl+V Shift+C` panel for
//! shell pipelines, CI / beta-reader bots, and
//! programmatic export.
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! §4.5.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;
use crate::tui::comments::{self, CommentsFile};

use super::CommentsCommand;

pub fn run(project: &Path, cmd: CommentsCommand) -> Result<()> {
    match cmd {
        CommentsCommand::List {
            paragraph,
            unresolved_only,
        } => list(project, paragraph.as_deref(), unresolved_only),
        CommentsCommand::Resolve { id } => set_resolved(project, &id, true),
        CommentsCommand::Reopen { id } => set_resolved(project, &id, false),
        CommentsCommand::Delete { id } => delete(project, &id),
        CommentsCommand::Export { output } => export(project, output.as_deref()),
    }
}

/// One materialised comment + the surrounding
/// paragraph metadata.  Lives here because it's
/// the shape the CLI walker emits + the export
/// JSON serialises.
#[derive(Debug, Clone, serde::Serialize)]
struct CliCommentRow {
    paragraph_slug_path: String,
    paragraph_title: String,
    typ_rel_path: String,
    #[serde(flatten)]
    comment: crate::tui::comments::Comment,
}

fn walk_all_comments(
    store: &Store,
    hierarchy: &Hierarchy,
    layout: &ProjectLayout,
) -> Vec<CliCommentRow> {
    let _ = store;
    let mut out: Vec<CliCommentRow> = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &node.file else { continue; };
        let typ_abs = layout.root.join(rel);
        let file = match comments::load_from_sidecar(&typ_abs) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if file.comments.is_empty() {
            continue;
        }
        let slug_path = hierarchy.slug_path(node);
        for c in file.comments {
            out.push(CliCommentRow {
                paragraph_slug_path: slug_path.clone(),
                paragraph_title: node.title.clone(),
                typ_rel_path: rel.clone(),
                comment: c,
            });
        }
    }
    out.sort_by(|a, b| {
        a.paragraph_slug_path
            .cmp(&b.paragraph_slug_path)
            .then(b.comment.created_at.cmp(&a.comment.created_at))
    });
    out
}

fn list(
    project: &Path,
    paragraph_filter: Option<&str>,
    unresolved_only: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let all = walk_all_comments(&store, &hierarchy, &layout);
    let filtered: Vec<&CliCommentRow> = all
        .iter()
        .filter(|row| {
            if unresolved_only && row.comment.resolved {
                return false;
            }
            if let Some(slug) = paragraph_filter {
                if !row
                    .paragraph_slug_path
                    .eq_ignore_ascii_case(slug)
                {
                    return false;
                }
            }
            true
        })
        .collect();
    if filtered.is_empty() {
        eprintln!("no comments match");
        return Ok(());
    }
    let max_slug = filtered
        .iter()
        .map(|r| r.paragraph_slug_path.chars().count())
        .max()
        .unwrap_or(20);
    let max_author = filtered
        .iter()
        .map(|r| r.comment.author.chars().count())
        .max()
        .unwrap_or(8);
    println!(
        "  {:<slug_w$}  {:<au_w$}  {:>10}  {:<6}  {}",
        "paragraph", "author", "age", "status", "text",
        slug_w = max_slug,
        au_w = max_author,
    );
    println!(
        "  {}",
        "-".repeat(max_slug + max_author + 50)
    );
    for r in filtered {
        let status = if r.comment.resolved {
            "[r]"
        } else {
            ""
        };
        let age = humanise_age(r.comment.created_at);
        let snippet: String = r.comment.text.chars().take(60).collect();
        let snippet = if r.comment.text.chars().count() > 60 {
            format!("{snippet}…")
        } else {
            snippet
        };
        println!(
            "  {:<slug_w$}  {:<au_w$}  {:>10}  {:<6}  {}",
            r.paragraph_slug_path,
            r.comment.author,
            age,
            status,
            snippet,
            slug_w = max_slug,
            au_w = max_author,
        );
        // Print UUID on the second line so the
        // `resolve <id>` / `delete <id>` chords are
        // easy to copy.
        println!(
            "  {:<slug_w$}    id: {}",
            "",
            r.comment.id,
            slug_w = max_slug,
        );
    }
    Ok(())
}

fn set_resolved(project: &Path, id: &str, resolved: bool) -> Result<()> {
    let uuid =
        uuid::Uuid::parse_str(id).map_err(|e| {
            Error::Config(format!("invalid UUID `{id}`: {e}"))
        })?;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let (typ_abs, mut file) = locate_sidecar_containing(&hierarchy, &layout, uuid)?;
    let comment = file
        .comments
        .iter_mut()
        .find(|c| c.id == uuid)
        .ok_or_else(|| {
            Error::Config(format!("comment `{id}` not found"))
        })?;
    comment.resolved = resolved;
    comment.resolved_at = if resolved {
        Some(chrono::Utc::now())
    } else {
        None
    };
    comments::save_to_sidecar(&typ_abs, &file)
        .map_err(Error::Config)?;
    eprintln!(
        "comment {} {}",
        id,
        if resolved { "resolved" } else { "reopened" }
    );
    Ok(())
}

fn delete(project: &Path, id: &str) -> Result<()> {
    let uuid =
        uuid::Uuid::parse_str(id).map_err(|e| {
            Error::Config(format!("invalid UUID `{id}`: {e}"))
        })?;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let (typ_abs, mut file) = locate_sidecar_containing(&hierarchy, &layout, uuid)?;
    let before = file.comments.len();
    file.comments.retain(|c| c.id != uuid);
    if file.comments.len() == before {
        return Err(Error::Config(format!(
            "comment `{id}` not found in {}",
            typ_abs.display()
        )));
    }
    comments::save_to_sidecar(&typ_abs, &file)
        .map_err(Error::Config)?;
    eprintln!("comment {} deleted", id);
    Ok(())
}

fn export(project: &Path, output: Option<&Path>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let all = walk_all_comments(&store, &hierarchy, &layout);
    let raw = serde_json::to_string_pretty(&all)
        .map_err(|e| Error::Config(format!("serialise: {e}")))?;
    match output {
        Some(p) => {
            std::fs::write(p, &raw).map_err(Error::Io)?;
            eprintln!("wrote {} bytes to {}", raw.len(), p.display());
        }
        None => {
            use std::io::Write;
            std::io::stdout()
                .write_all(raw.as_bytes())
                .map_err(Error::Io)?;
            println!();
        }
    }
    Ok(())
}

/// human-readable comment age
/// for the CLI list table.  Same shape the TUI
/// panel uses; duplicated rather than re-exported
/// to keep the CLI module independent.
fn humanise_age(when: chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let delta = now.signed_duration_since(when);
    let secs = delta.num_seconds();
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{}m ago", delta.num_minutes())
    } else if secs < 86_400 {
        format!("{}h ago", delta.num_hours())
    } else if secs < 86_400 * 30 {
        format!("{}d ago", delta.num_days())
    } else {
        when.format("%Y-%m-%d").to_string()
    }
}

/// Walk the hierarchy looking for the sidecar
/// that contains the named comment UUID.  Returns
/// `(typ_abs_path, parsed CommentsFile)` so the
/// caller can mutate the file in-place + write it
/// back via `save_to_sidecar`.
fn locate_sidecar_containing(
    hierarchy: &Hierarchy,
    layout: &ProjectLayout,
    uuid: uuid::Uuid,
) -> Result<(std::path::PathBuf, CommentsFile)> {
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = &node.file else { continue; };
        let typ_abs = layout.root.join(rel);
        let file = match comments::load_from_sidecar(&typ_abs) {
            Ok(f) => f,
            Err(_) => continue,
        };
        if file.comments.iter().any(|c| c.id == uuid) {
            return Ok((typ_abs, file));
        }
    }
    Err(Error::Config(format!(
        "comment `{uuid}` not found in any paragraph sidecar"
    )))
}
