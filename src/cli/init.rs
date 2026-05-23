use std::io::{self, Write};
use std::path::Path;

use tracing::info;

use crate::config::{Config, DEFAULT_PROJECT_CONFIG, DEFAULT_PROMPTS};
use crate::error::{Error, Result};
use crate::project::{PROMPTS_FILE_DEFAULT, ProjectLayout};
use crate::store::Store;

/// Initialise a new project at `path`. If the directory already exists we
/// require explicit consent before wiping it — either the `--force` flag or
/// a `y` answer to the interactive prompt. After confirmation the entire
/// directory is removed and freshly re-created so the new database starts
/// from a clean slate (stale `metadata.db` + `vectors/` from a previous
/// install never trip up the schema).
pub fn run(path: &Path, force: bool) -> Result<()> {
    let layout = ProjectLayout::new(path);

    if path.exists() {
        // Either the user passed --force (non-interactive overwrite) or
        // we must ask. Anything else aborts cleanly.
        let confirmed = if force {
            true
        } else {
            confirm_overwrite(path)?
        };
        if !confirmed {
            return Err(Error::Store(format!(
                "init aborted — `{}` left untouched",
                path.display()
            )));
        }
        // Refuse to recursively delete the project root if the cwd lives
        // inside it (Mac/Linux happily wipes itself out of the cwd and
        // hands back an EINVAL on every subsequent operation).
        if let Ok(cwd) = std::env::current_dir() {
            if let Ok(abs_target) = std::fs::canonicalize(path) {
                if cwd.starts_with(&abs_target) {
                    return Err(Error::Store(format!(
                        "refusing to wipe `{}` — your current directory lives inside it",
                        abs_target.display()
                    )));
                }
            }
        }
        std::fs::remove_dir_all(path).map_err(Error::Io)?;
    }

    layout.create_layout()?;

    let config_path = layout.config_path();
    std::fs::write(&config_path, DEFAULT_PROJECT_CONFIG)?;
    info!(path = %config_path.display(), "wrote project config");

    let prompts_path = layout.root.join(PROMPTS_FILE_DEFAULT);
    std::fs::write(&prompts_path, DEFAULT_PROMPTS)?;
    info!(path = %prompts_path.display(), "wrote prompt library");

    // Round-trip parse the config to validate it.
    let cfg = Config::load(&config_path)?;

    // Open the document store. This creates `metadata.db` + `vecstore/`.
    // First-run embedding-model download (if needed) happens here.
    let store = Store::open(layout.clone(), &cfg)?;

    // 1.2.6+ — seed the Prompts book with `<name>.example`
    // paragraphs carrying every embedded default prompt
    // inkhaven knows about (F7 grammar-check, F11 explain-
    // diagnostic, F12 critique-edit + critique-changes). The
    // user reviews / tunes the body, then renames to drop the
    // `.example` suffix to take effect — without that suffix,
    // inkhaven keeps using the built-in default. Gated on
    // `ai.reseed_prompt_examples` (default true).
    if cfg.ai.reseed_prompt_examples {
        if let Err(e) = seed_prompt_examples(&cfg, &store) {
            // Non-fatal — the user can `inkhaven add ¶` these
            // later if seeding hiccups for any reason.
            tracing::warn!(
                target: "inkhaven::init",
                "could not seed Prompts.book examples: {e}",
            );
        }
    }

    eprintln!("Initialized inkhaven project at {}", layout.root.display());
    eprintln!("  config:    {}", layout.config_path().display());
    eprintln!("  prompts:   {}", layout.root.join(PROMPTS_FILE_DEFAULT).display());
    eprintln!("  store db:  {}", layout.metadata_db_path().display());
    eprintln!("  vecstore:  {}", layout.vecstore_path().display());
    eprintln!("  books:     {}", layout.books_path().display());
    Ok(())
}

/// 1.2.6+ — seed every embedded prompt as a `<name>.example`
/// paragraph in the Prompts system book. The paragraph body is
/// the embedded fallback prompt verbatim, preceded by a short
/// `// ` Typst-comment intro that explains the lookup rule. The
/// user reviews, tunes, then renames the paragraph to drop the
/// `.example` suffix — at that point the resolver picks it up
/// and the F-key uses the user's prompt instead of the
/// embedded default.
pub(crate) fn seed_prompt_examples(cfg: &Config, store: &Store) -> Result<()> {
    use crate::store::hierarchy::Hierarchy;
    use crate::store::{
        InsertPosition, NodeKind, SYSTEM_TAG_PROMPTS,
    };

    let lang = if cfg.language.trim().is_empty() {
        "English".to_owned()
    } else {
        cfg.language.trim().to_owned()
    };

    // (paragraph_title, body) tuples. Title carries the `.example`
    // suffix so it's clearly inert until the user renames.
    let seeds: [(&str, String); 4] = [
        (
            "grammar-check.example",
            format!(
                "// F7 — grammar check the open paragraph.\n\
                 // Rename this paragraph to `grammar-check` (drop `.example`)\n\
                 // to take effect; until then inkhaven uses the built-in default.\n\n\
                 {}\n",
                crate::tui::app::grammar_check_default_prompt(&lang),
            ),
        ),
        (
            "explain-diagnostic.example",
            format!(
                "// F11 — AI-explain the typst diagnostic at the cursor.\n\
                 // Rename to `explain-diagnostic` to take effect.\n\n\
                 {}\n",
                crate::tui::app::explain_diagnostic_default_prompt(),
            ),
        ),
        (
            "critique-edit.example",
            format!(
                "// F12 (editor mode) — what's weak about the open paragraph.\n\
                 // Rename to `critique-edit` to take effect.\n\n\
                 {}\n",
                crate::tui::app::critique_edit_default_prompt(),
            ),
        ),
        (
            "critique-changes.example",
            format!(
                "// F12 (split-edit mode) — evaluate the changes from the snapshot.\n\
                 // Rename to `critique-changes` to take effect.\n\n\
                 {}\n",
                crate::tui::app::critique_changes_default_prompt(),
            ),
        ),
    ];

    let hierarchy = Hierarchy::load(store)?;
    // Find the Prompts system book.
    let prompts_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_PROMPTS)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("Prompts system book missing after Store::open".into())
        })?;

    for (title, body) in &seeds {
        // Reload hierarchy each pass so subsequent lookups see
        // freshly-added siblings (mirrors the typst-skeleton
        // seeding pattern in store/mod.rs).
        let h = Hierarchy::load(store)?;
        let already = h.iter().any(|n| {
            n.kind == NodeKind::Paragraph
                && n.parent_id == Some(prompts_book.id)
                && n.title.eq_ignore_ascii_case(title)
        });
        if already {
            continue;
        }
        let mut created = store.create_node(
            cfg,
            &h,
            NodeKind::Paragraph,
            title,
            Some(&prompts_book),
            None,
            InsertPosition::End,
        )?;
        // Overwrite the auto-`= Title\n\n` skeleton with the
        // embedded prompt.
        if let Some(rel) = &created.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
            store.update_paragraph_content(&mut created, body.as_bytes())?;
        }
    }
    Ok(())
}

/// Interactive y/N prompt on stderr. Returns true only when the user types
/// `y` / `yes` (case-insensitive). Any other input — including an empty
/// line, EOF, or `n` — returns false so we never wipe by accident.
fn confirm_overwrite(path: &Path) -> Result<bool> {
    eprint!(
        "Directory `{}` already exists. Remove it and re-initialise? [y/N] ",
        path.display()
    );
    io::stderr().flush().ok();
    let mut buf = String::new();
    if io::stdin().read_line(&mut buf).map_err(Error::Io)? == 0 {
        return Ok(false);
    }
    let answer = buf.trim().to_ascii_lowercase();
    Ok(matches!(answer.as_str(), "y" | "yes"))
}
