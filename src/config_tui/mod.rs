//! 1.2.10+ — standalone TUI configuration editor.
//!
//! Launched as `inkhaven config -p <dir>`.  Provides a
//! schema-aware, tree-pane + edit-pane view of
//! `<dir>/inkhaven.hjson`.
//!
//! **Phase 1**: read-only walk-through — tree + detail
//! pane + help pane + unknown-fields chip.  No widgets
//! that mutate, no save, no backup.
//!
//! See `Documentation/PROPOSALS/CONFIG_TUI.md` for the
//! full design.

mod annotations;
mod app;
mod backup;
mod help;
mod hjson_index;
mod save;
mod schema;
mod widgets;

use std::path::Path;

use anyhow::{Context, Result};
use std::path::PathBuf;

/// Entry point — called from the `inkhaven config`
/// subcommand dispatcher.  Initialises the terminal,
/// builds the schema, parses the live HJSON, runs the
/// event loop, restores the terminal on exit.
pub fn run(project: &Path) -> Result<()> {
    app::run(project)
}

/// 1.2.11+ — outcome of a successful in-place HJSON
/// patch.  `config_path` is the path written; `backup`
/// is the versioned snapshot of the **pre-patch**
/// contents (under `<project>/.config-backups/`) so a
/// roll-back is one `cp` away.
pub struct InPlacePatchOutcome {
    pub config_path: PathBuf,
    pub backup: PathBuf,
}

/// 1.2.11+ — apply a set of dotted-path leaf updates
/// to `<project>/inkhaven.hjson` in place.  Used by
/// the CLI surface (`inkhaven show-dont-tell bootstrap
/// --update`) and any future caller that needs to
/// surgically patch a handful of leaves without
/// launching the full config TUI.
///
/// Semantics:
///
///   * Each `(path, value)` pair targets a single leaf
///     by dotted schema path (e.g.
///     `editor.style_warnings.show_dont_tell.russian_emotion_adjectives`).
///   * Paths already present in the live HJSON are
///     **spliced** — comments + formatting around the
///     leaf survive.
///   * Paths NOT present are **appended** inside the
///     parent stanza's closing brace.  Parent must
///     exist (it does for every leaf reachable from
///     the schema tree, since `serde(default)` makes
///     the entire `Config` shape round-trippable).
///   * A versioned backup of the **pre-patch** file
///     lands under `<project>/.config-backups/` first;
///     the new file is written atomically (.tmp +
///     rename) so an interrupted run can't truncate
///     `inkhaven.hjson`.
///   * Updates is `&[(path, value)]`, order is
///     irrelevant — internal sorting handles
///     splice-shift safety.
pub fn apply_in_place_edits(
    project_root: &Path,
    updates: &[(String, serde_json::Value)],
) -> Result<InPlacePatchOutcome> {
    if updates.is_empty() {
        anyhow::bail!("apply_in_place_edits: no updates supplied");
    }
    let config_path = project_root.join("inkhaven.hjson");
    let source = std::fs::read_to_string(&config_path)
        .with_context(|| format!("read {}", config_path.display()))?;
    // Snapshot the **pre-patch** contents first.  If
    // anything below fails the user still has the
    // original under `.config-backups/`.
    let backup = save::write_backup(project_root, &source)
        .context("write pre-patch backup")?;
    let index = hjson_index::parse(&source)
        .map_err(|e| anyhow::anyhow!("parse HJSON: {e}"))?;
    let edits: Vec<save::Edit> = updates
        .iter()
        .map(|(path, value)| {
            let kind = if index.leaves.contains_key(path) {
                save::EditKind::Splice
            } else {
                save::EditKind::Append
            };
            save::Edit {
                path: path.clone(),
                new_value: value.clone(),
                kind,
            }
        })
        .collect();
    let new_source = save::apply_edits(&index, &edits)
        .context("apply HJSON edits")?;
    let written = save::write_atomic(&config_path, &new_source)
        .context("write inkhaven.hjson")?;
    Ok(InPlacePatchOutcome {
        config_path: written,
        backup,
    })
}
