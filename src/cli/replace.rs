//! 1.2.22 R.5 — `inkhaven replace` subcommand.
//!
//! Project-wide find & replace.  Lexical matching — literal +
//! whole-word by default (`--substring` opts out, `--regex` for a
//! regex), optional `--ignore-case` — over a linear scan of paragraph
//! bodies (no index; see `crate::replace`).  Safety: `--dry-run`
//! previews every match and changes nothing; without `--yes` it refuses
//! to write; with `--yes` it applies all matches, **snapshotting each
//! touched paragraph first** (restore via `F6` in the editor).
//!
//! Per-match accept/skip is the TUI's job (R.3); the CLI is the
//! preview-then-apply-all path for scripted / CI renames.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::replace::{self, Hit, ReplaceOpts, ScanScope};
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    pattern: &str,
    replacement: &str,
    regex: bool,
    substring: bool,
    ignore_case: bool,
    book: Option<&str>,
    include_system: bool,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let opts = ReplaceOpts {
        regex,
        word_boundary: !substring,
        ignore_case,
    };
    let scope = if let Some(name) = book {
        let b = crate::cli::resolve_user_book(&hierarchy, Some(name), "replace")
            .map_err(Error::Store)?;
        ScanScope::Book(b.id)
    } else if include_system {
        ScanScope::IncludingSystem
    } else {
        ScanScope::UserBooks
    };

    let matches = replace::scan_project(&store, &hierarchy, &scope, pattern, replacement, opts)
        .map_err(|e| Error::Store(format!("replace: {e}")))?;
    if matches.is_empty() {
        println!("replace: no matches for `{pattern}`");
        return Ok(());
    }
    let total: usize = matches.iter().map(|m| m.hits.len()).sum();
    let plural_m = if total == 1 { "" } else { "es" };
    let plural_p = if matches.len() == 1 { "" } else { "s" };

    // Preview the matches for --dry-run and for the un-confirmed case.
    if dry_run || !yes {
        for pm in &matches {
            println!("{}", pm.slug_path);
            for h in &pm.hits {
                println!("  {}:{}  {}", h.line, h.col, preview(h));
            }
        }
        println!();
    }

    if dry_run {
        println!(
            "replace: {total} match{plural_m} in {} paragraph{plural_p} — dry run, nothing changed",
            matches.len(),
        );
        return Ok(());
    }
    if !yes {
        println!(
            "replace: {total} match{plural_m} in {} paragraph{plural_p}. \
             Re-run with --yes to apply (each touched paragraph is snapshotted first).",
            matches.len(),
        );
        return Ok(());
    }

    let annotation = format!("replace: {pattern} → {replacement}");
    let report = replace::apply_project(&store, &hierarchy, &matches, &annotation)
        .map_err(Error::Store)?;
    println!(
        "replace: applied {} replacement{} in {} paragraph{}; {} snapshot{} taken (restore via F6 in the editor)",
        report.occurrences,
        if report.occurrences == 1 { "" } else { "s" },
        report.paragraphs,
        if report.paragraphs == 1 { "" } else { "s" },
        report.snapshots,
        if report.snapshots == 1 { "" } else { "s" },
    );
    Ok(())
}

/// Mark the matched span in its line with `»…«`, trimming long context
/// on either side to ~30 chars.
fn preview(h: &Hit) -> String {
    let chars: Vec<char> = h.line_text.chars().collect();
    let start = h.col.saturating_sub(1).min(chars.len());
    let end = (start + h.matched.chars().count()).min(chars.len());
    let lead = start.saturating_sub(30);
    let trail = (end + 30).min(chars.len());
    let before: String = chars[lead..start].iter().collect();
    let matched: String = chars[start..end].iter().collect();
    let after: String = chars[end..trail].iter().collect();
    format!(
        "{}{before}»{matched}«{after}{}",
        if lead > 0 { "…" } else { "" },
        if trail < chars.len() { "…" } else { "" },
    )
}
