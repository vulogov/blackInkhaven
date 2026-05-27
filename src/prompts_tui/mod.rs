//! 1.2.11+ — standalone TUI prompts editor.
//!
//! Launched as `inkhaven prompts-editor -p <dir>`.
//! Four-pane workbench:
//!
//!   * **Left** — prompts list.
//!   * **Centre** — prompt editor (same chord set as
//!     the main inkhaven editor).
//!   * **Right** — AI response (display-only).
//!   * **Bottom** — AI prompt input (single-line).
//!
//! Send semantics: typing in the AI prompt + pressing
//! Enter renders the editor's body as a template
//! (substituting `{{selection}}` with the AI prompt
//! input + `{{context}}` with empty), sends the
//! result as the user message to the configured LLM
//! with no system prompt, and streams the response
//! into the AI pane.
//!
//! **Phase 1**: read-only walk-through.  CLI plumbing,
//! the four-pane shell, list navigation,
//! show-on-focus editor display, help pane.  No
//! mutation, no save, no AI send — those land in
//! Phases 2 and 3.
//!
//! See `Documentation/PROPOSALS/PROMPTS_EDITOR_TUI.md`
//! for the full design.

mod app;
mod backup;

use std::path::Path;

use anyhow::Result;

/// Entry point — called from the `inkhaven
/// prompts-editor` subcommand dispatcher.  Initialises
/// the terminal, loads `prompts.hjson` (or the
/// embedded defaults when missing), runs the event
/// loop, restores the terminal on exit.
pub fn run(project: &Path) -> Result<()> {
    app::run(project)
}
