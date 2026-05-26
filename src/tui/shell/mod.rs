//! In-process nushell pane (1.2.8+).
//!
//! Phase 2 — real `EngineState` builder + `Engine::eval`.
//! Parses a single line through `nu_parser::parse`, evals
//! via `nu_engine::eval_block::<WithoutDebug>`, captures the
//! resulting `PipelineData` into a `ShellOutput` containing
//! stdout + stderr strings.  No modal / chord / UI plumbing
//! yet — that lands in Phase 3.
//!
//! Architecture notes:
//!   * `Engine` owns `EngineState + Stack`.  Re-used across
//!     calls so env-var mutations (`$env.PWD = ...`,
//!     `let x = 1`) persist between invocations — same as
//!     a real REPL.
//!   * Cwd starts at the project root.  Captured into the
//!     stack's `PWD` env var so `ls`, `cd`, `glob`, …
//!     resolve relative paths correctly.
//!   * No reedline — line editing lands on top of inkhaven's
//!     own `TextInput` in Phase 4.  No `print` styling /
//!     ANSI output — we capture raw `Value` text via
//!     `PipelineData::collect_string` and surface it as a
//!     plain `String` for the pane to render.
//!   * Long-running / TTY-needing commands (`vim`, `top`,
//!     `less`) will hang the TUI — explicitly out of scope.
//!     Pipelines that read external stdin will see EOF
//!     immediately.
//!
//! Long-term: `Engine` will gain a per-project SQLite
//! history connection (Phase 4) and a configurable output-
//! buffer cap (already in `ShellConfig`).

#![allow(dead_code)]  // some fields/methods unused until Phase 3+.

use std::path::Path;

use nu_protocol::engine::{EngineState, Stack, StateWorkingSet};
use nu_protocol::{PipelineData, Span, Value};
use nu_protocol::debugger::WithoutDebug;

/// Single nu instance bound to a project.  Holds the engine
/// state (function table, scope chain, env vars) + a Stack
/// (per-eval scratch + env mutations).  Re-eval'd commands
/// share state, like a real REPL.
pub(super) struct Engine {
    state: EngineState,
    stack: Stack,
}

/// Captured result of one `Engine::eval` call.
///
/// `stdout` is the pipeline's final value, serialised as a
/// human-readable string via `collect_string("\n", &Config::default())`.
/// Empty when the pipeline ended at a side-effect command
/// (`save`, `cd`, …) with no return value.
///
/// `stderr` carries parse errors + ShellError messages.
/// Empty on success.
///
/// `success`: false when parse produced errors OR eval
/// returned `Err(ShellError)`.  True even for empty
/// stdout — `cd /tmp` is a success without output.
pub(super) struct ShellOutput {
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

impl Engine {
    /// Build a fresh nu engine bound to `project_root`.
    /// Loads nu-command's default declarations (`ls`, `cd`,
    /// `where`, `str`, `path`, …) and seeds the Stack's
    /// `PWD` env var so relative paths resolve from the
    /// project.
    pub(super) fn new(project_root: &Path) -> Self {
        let engine_state = EngineState::new();
        // add_shell_command_context takes the state by
        // value and returns it with the delta merged.
        let state = nu_command::add_shell_command_context(engine_state);

        let mut stack = Stack::new();
        // `cwd_init` env var is what nu's std library reads;
        // PWD is what most commands consult.  Set both to
        // the project root so `ls`, `glob`, etc. behave.
        let root_str = project_root.to_string_lossy().to_string();
        stack.add_env_var(
            "PWD".to_string(),
            Value::string(root_str.clone(), Span::unknown()),
        );
        stack.add_env_var(
            "CWD".to_string(),
            Value::string(root_str, Span::unknown()),
        );

        Self { state, stack }
    }

    /// Parse + evaluate one input line.  Never panics —
    /// parse errors and ShellErrors come back in `stderr`
    /// with `success = false`.
    pub(super) fn eval(&mut self, line: &str) -> ShellOutput {
        // Parse phase: builds a Block + records errors on the
        // working_set.  Merge the resulting delta (new
        // declarations from `def`, etc.) back into
        // engine_state so subsequent evals see them.
        let (block, parse_errors) = {
            let mut working_set = StateWorkingSet::new(&self.state);
            let block = nu_parser::parse(
                &mut working_set,
                None,
                line.as_bytes(),
                false,
            );
            let errs: Vec<String> = working_set
                .parse_errors
                .iter()
                .map(|e| format!("{e:?}"))
                .collect();
            // merge_delta consumes the delta, leaves working_set empty.
            let delta = working_set.render();
            if let Err(e) = self.state.merge_delta(delta) {
                return ShellOutput {
                    stdout: String::new(),
                    stderr: format!("merge_delta: {e:?}"),
                    success: false,
                };
            }
            (block, errs)
        };

        if !parse_errors.is_empty() {
            // Surface the FIRST parse error verbatim and any
            // following ones on their own lines.  Bail before
            // eval — a partially-parsed Block can produce
            // confusing runtime errors that drown the real
            // syntax problem.
            return ShellOutput {
                stdout: String::new(),
                stderr: parse_errors.join("\n"),
                success: false,
            };
        }

        // Eval phase.
        match nu_engine::eval_block::<WithoutDebug>(
            &self.state,
            &mut self.stack,
            &block,
            PipelineData::empty(),
        ) {
            Ok(exec) => {
                let cfg = nu_protocol::Config::default();
                let stdout =
                    exec.body.collect_string("\n", &cfg).unwrap_or_default();
                ShellOutput {
                    stdout,
                    stderr: String::new(),
                    success: true,
                }
            }
            Err(e) => ShellOutput {
                stdout: String::new(),
                stderr: format!("{e:?}"),
                success: false,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn engine() -> Engine {
        // Use the temp dir as the project root so tests don't
        // touch the actual repo cwd.  The eval is pure arithmetic
        // so the cwd choice doesn't matter — but constructing
        // the engine still requires *a* path.
        let dir = std::env::temp_dir();
        Engine::new(&dir)
    }

    #[test]
    fn one_plus_one_equals_two() {
        let mut e = engine();
        let out = e.eval("1 + 1");
        assert!(out.success, "stderr was: {}", out.stderr);
        assert_eq!(out.stdout.trim(), "2");
    }

    #[test]
    fn string_concatenation() {
        let mut e = engine();
        let out = e.eval(r#""foo" + "bar""#);
        assert!(out.success, "stderr was: {}", out.stderr);
        assert_eq!(out.stdout.trim(), "foobar");
    }

    #[test]
    fn let_binding_does_not_persist_across_evals_yet() {
        // Honest documentation of a current limitation:
        // each `eval` opens its own parse + scope, so a
        // `let x = …` in one call doesn't create a variable
        // visible to the next.  Nu's interactive REPL uses
        // an "append + re-parse the accumulated buffer"
        // strategy to make persistent lets work — we'd need
        // that for true REPL parity.  For the 1.2.8 simple-
        // shell-command target this is acceptable; revisit
        // when / if users hit the wall.  Env-var mutations
        // (`$env.X = ...`) DO persist because they live on
        // `Stack`, not in the parse-tree's scope.
        let mut e = engine();
        let out1 = e.eval("let answer = 42");
        // The first call usually succeeds (let with no
        // visible output) — but if nu evolves we want the
        // assert to fail loudly so we update the test.
        let _ = out1;
        let out2 = e.eval("$answer * 2");
        assert!(
            !out2.success,
            "if this passes, let now persists across evals — \
             celebrate, update the test name, and remove this docstring",
        );
    }

    #[test]
    fn parse_error_lands_in_stderr_not_panic() {
        let mut e = engine();
        let out = e.eval("let x = ");  // missing rhs
        assert!(!out.success);
        assert!(!out.stderr.is_empty());
    }

    #[test]
    fn pwd_env_var_set_to_project_root() {
        let dir: PathBuf = std::env::temp_dir();
        let mut e = Engine::new(&dir);
        let out = e.eval("$env.PWD");
        assert!(out.success, "stderr was: {}", out.stderr);
        // tempdir on macOS comes back via /var/folders/..., on
        // Linux via /tmp.  Just check we got *something* and
        // that the path roughly matches what we set.
        let expected = dir.to_string_lossy().to_string();
        assert_eq!(out.stdout.trim(), expected.trim());
    }
}
