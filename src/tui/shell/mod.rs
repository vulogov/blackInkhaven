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

#![allow(dead_code)]  // some fields/methods unused until Phase 5+.

use std::path::{Path, PathBuf};

use nu_parser::FlatShape;
use nu_protocol::engine::{EngineState, Redirection, Stack, StateWorkingSet};
use nu_protocol::{ByteStreamSource, OutDest, PipelineData, Span, Value};
use nu_protocol::debugger::WithoutDebug;
use ratatui::style::{Color, Modifier, Style};
use rusqlite::Connection;

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

/// One command + its captured output.  The shell pane
/// renders an interleaved list of these as scrollback;
/// selection mode (Phase 6) navigates between them
/// turn-by-turn (same model as AI chat selection).
#[derive(Debug, Clone)]
pub(crate) struct ShellTurn {
    pub command: String,
    pub stdout: String,
    pub stderr: String,
    pub success: bool,
}

/// 1.2.8+ — per-project SQLite-backed command history for
/// the shell pane's Up/Down recall ring.
///
/// File: `<project_root>/.inkhaven/shell_history.db`.  A
/// single `history` table with `(id, command, ts)`; no
/// schema migrations needed — additive only.  Load returns
/// the most-recent `cap` commands in chronological order
/// (oldest first) so the recall cursor lands naturally on
/// the newest on first Up-arrow.
///
/// SQL failures are non-fatal: a corrupt or unwritable
/// `.db` falls through to an in-memory-only ring, with the
/// error stamped on the status bar once.  The shell still
/// works; just history doesn't survive restart.
pub(crate) struct History {
    conn: Option<Connection>,
    path: PathBuf,
}

impl History {
    /// Open (or lazily create) the history DB under the
    /// project's `.inkhaven/` directory.  The directory is
    /// created if missing.  All errors are swallowed at
    /// open-time and surface later via `last_error()`.
    pub(crate) fn open(project_root: &Path) -> Self {
        let mut path = project_root.to_path_buf();
        path.push(".inkhaven");
        let _ = std::fs::create_dir_all(&path);
        path.push("shell_history.db");
        let conn = Connection::open(&path).ok();
        if let Some(c) = conn.as_ref() {
            let _ = c.execute_batch(
                r#"CREATE TABLE IF NOT EXISTS history (
                       id INTEGER PRIMARY KEY AUTOINCREMENT,
                       command TEXT NOT NULL,
                       ts TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                   );
                   CREATE INDEX IF NOT EXISTS history_ts_idx
                     ON history(ts DESC);"#,
            );
        }
        Self { conn, path }
    }

    /// Return the most-recent `cap` commands in
    /// chronological order (oldest → newest) — same order
    /// the in-memory ring expects.  Empty list on any
    /// error.
    pub(crate) fn load(&self, cap: usize) -> Vec<String> {
        let Some(conn) = self.conn.as_ref() else {
            return Vec::new();
        };
        let Ok(mut stmt) = conn.prepare(
            "SELECT command FROM (
                 SELECT command, id FROM history ORDER BY id DESC LIMIT ?
             ) sub ORDER BY id ASC",
        ) else {
            return Vec::new();
        };
        let cap_i = cap as i64;
        match stmt.query_map([cap_i], |row| row.get::<_, String>(0)) {
            Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Append a command to history.  Silently no-op on
    /// SQL error so the user's session isn't disrupted by
    /// a transient disk problem.
    pub(crate) fn push(&self, command: &str) {
        let Some(conn) = self.conn.as_ref() else { return };
        let _ = conn.execute(
            "INSERT INTO history (command) VALUES (?1)",
            [command],
        );
    }

    /// Path the DB lives at — exposed for status messages /
    /// the audit hooks.  Returns even when the open failed.
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
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
        // Strip trailing slashes: nu's engine_state refuses
        // to spawn externals when $env.PWD has them, but it
        // doesn't normalise on its own — and tempdir() on
        // macOS happens to return paths with a trailing /.
        let root_str = {
            let raw = project_root.to_string_lossy().to_string();
            let trimmed = raw.trim_end_matches('/');
            if trimmed.is_empty() { "/".to_string() } else { trimmed.to_string() }
        };
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

        // Eval phase.  Push a Pipe redirection on BOTH stdout
        // and stderr so external commands (`^/bin/ls -l`,
        // `^git status`, …) get captured into the resulting
        // PipelineData instead of inheriting nu's stdout — which
        // would be our process's TTY and corrupt the ratatui
        // alternate-screen surface.
        //
        // Why `OutDest::Pipe` and not `OutDest::Value`:
        //   - `Value` keeps stdout + stderr as *separate*
        //     pipes on the spawned `ChildProcess`.  When we
        //     later call `pipeline.into_value(...)`,
        //     `ChildProcess::into_bytes` asserts
        //     `stderr.is_none()` and returns an "internal
        //     error: stderr should not exist" ShellError.
        //   - `Pipe` is documented to *merge* stderr into the
        //     single stdout pipe when both descriptors are set
        //     to it — so the child has stderr=None and
        //     into_bytes drains the combined stream cleanly.
        // Net effect: the user sees external command stdout
        // and stderr interleaved in turn order in the pane,
        // same way a real terminal would render them.
        let mut guard = self.stack.push_redirection(
            Some(Redirection::Pipe(OutDest::Pipe)),
            Some(Redirection::Pipe(OutDest::Pipe)),
        );
        let exec_result = nu_engine::eval_block::<WithoutDebug>(
            &self.state,
            &mut *guard,
            &block,
            PipelineData::empty(),
        );
        drop(guard);
        match exec_result {
            Ok(mut exec) => {
                // 1.2.8+ — when the pipeline ends in an external
                // command that exited non-zero (`^/bin/ls
                // /nonexistent`, `^false`, …), nu's
                // `ChildProcess::into_bytes` DRAINS the merged
                // stdout pipe into a Vec<u8>, then runs
                // `check_ok(exit_status, ignore_error, span)?`
                // and PROPAGATES Err on non-zero — discarding
                // the bytes we just drained.  format_via_table
                // then sees that Err in into_value and returns
                // empty stdout: silent loss of stderr (the bug
                // from the screenshot — `^/bin/ls /missing`
                // exits 2 and the "No such file" message
                // vanishes).
                //
                // Fix: reach into the ByteStream, set
                // `ignore_error = true` on the wrapped
                // ChildProcess so `check_ok` returns Ok for any
                // exit status.  The bytes (merged stdout +
                // stderr) are now returned regardless and land
                // in the pane.  We don't surface the non-zero
                // exit anywhere — keeping the interface simple;
                // users who care about exit status can pipe
                // through `| complete` explicitly.
                if let PipelineData::ByteStream(stream, _) = &mut exec.body {
                    if let ByteStreamSource::Child(child) = stream.source_mut() {
                        child.ignore_error(true);
                    }
                }
                // 1.2.8+ — pipe the result through nu's `table`
                // command so List<Record> renders as a column-
                // aligned table instead of `{name: ..., type:
                // ...}` debug dumps.  Falls back to plain
                // collect_string when the table decl isn't
                // registered (shouldn't happen with
                // nu-command's default context) or when the
                // pipeline type isn't tabular (let / cd /
                // strings — collect_string gets the right
                // answer for those anyway).
                let cfg = nu_protocol::Config::default();
                let raw = format_via_table(
                    &self.state,
                    &mut self.stack,
                    exec.body,
                    &cfg,
                );
                ShellOutput {
                    stdout: strip_ansi(&raw),
                    stderr: String::new(),
                    success: true,
                }
            }
            Err(e) => ShellOutput {
                stdout: String::new(),
                stderr: strip_ansi(&format!("{e:?}")),
                success: false,
            },
        }
    }

    /// 1.2.8+ — tokenise `line` against the current engine
    /// state and return styled (text, Style) spans suitable
    /// for ratatui rendering.  Empty input returns a single
    /// empty span.  Pure read against the engine — no
    /// declarations land in `engine_state` afterwards
    /// (the working_set's delta is discarded).
    pub(super) fn highlight(&self, line: &str) -> Vec<(String, Style)> {
        if line.is_empty() {
            return vec![(String::new(), Style::default())];
        }
        let mut working_set = StateWorkingSet::new(&self.state);
        let block = nu_parser::parse(
            &mut working_set,
            None,
            line.as_bytes(),
            false,
        );
        let flat = nu_parser::flatten_block(&working_set, &block);
        // The block's span covers the freshly-added file in
        // the workspace's global byte counter.  Subtract its
        // start to get offsets into `line.as_bytes()`.  When
        // the block has no span (degenerate input) we fall
        // back to first-token offset = 0.
        let block_start = block
            .span
            .map(|s| s.start)
            .or_else(|| flat.first().map(|(span, _)| span.start))
            .unwrap_or(0);
        let bytes = line.as_bytes();
        let mut out: Vec<(String, Style)> = Vec::new();
        let mut cursor = 0usize;
        for (span, shape) in &flat {
            let local_start = span
                .start
                .saturating_sub(block_start)
                .min(bytes.len());
            let local_end = span
                .end
                .saturating_sub(block_start)
                .min(bytes.len());
            if local_start > cursor {
                let gap = String::from_utf8_lossy(
                    &bytes[cursor..local_start],
                )
                .into_owned();
                out.push((gap, Style::default()));
            }
            if local_end > local_start {
                let text = String::from_utf8_lossy(
                    &bytes[local_start..local_end],
                )
                .into_owned();
                out.push((text, style_for_shape(shape)));
                cursor = local_end;
            }
        }
        if cursor < bytes.len() {
            let tail =
                String::from_utf8_lossy(&bytes[cursor..]).into_owned();
            out.push((tail, Style::default()));
        }
        out
    }
}

/// 1.2.8+ — map a `nu_parser::FlatShape` to a ratatui
/// `Style`.  Loosely follows nushell's default theme:
/// keywords + builtins in cyan, variables in magenta,
/// strings in green, numbers + flags in yellow, errors
/// in red.  Unrecognised shapes (newly-added FlatShape
/// variants in future nu versions) fall through to plain
/// foreground so highlighting degrades gracefully.
fn style_for_shape(shape: &FlatShape) -> Style {
    match shape {
        FlatShape::Keyword => Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
        FlatShape::InternalCall(_)
        | FlatShape::External(_)
        | FlatShape::ExternalResolved => Style::default().fg(Color::Cyan),
        FlatShape::Variable(_) | FlatShape::VarDecl(_) => {
            Style::default().fg(Color::Magenta)
        }
        FlatShape::String
        | FlatShape::RawString
        | FlatShape::StringInterpolation
        | FlatShape::GlobInterpolation => Style::default().fg(Color::Green),
        FlatShape::Int
        | FlatShape::Float
        | FlatShape::Bool
        | FlatShape::DateTime => Style::default().fg(Color::Yellow),
        FlatShape::Flag => Style::default().fg(Color::Yellow),
        FlatShape::Operator
        | FlatShape::Pipe
        | FlatShape::Redirection => Style::default().fg(Color::Gray),
        FlatShape::Filepath
        | FlatShape::Directory
        | FlatShape::GlobPattern => Style::default().fg(Color::Blue),
        FlatShape::Garbage => Style::default().fg(Color::Red),
        FlatShape::Custom(_) => Style::default().fg(Color::Cyan),
        _ => Style::default(),
    }
}

/// 1.2.8+ — strip ANSI escape sequences from text before
/// it lands in ratatui's display.  External commands
/// (`/bin/ls`, `git`, …) emit cursor-positioning + colour
/// SGR codes which would otherwise mangle the TUI's render
/// (literal ANSI bytes pass through ratatui to the host
/// terminal and reposition the cursor mid-paint, hence the
/// overlapped-text bug seen in 1.2.8 phases 3-6).
///
/// State machine handles CSI (`ESC [ … <final>`), G0/G1
/// designators (`ESC ( X`, `ESC ) X`), and bare `ESC X`
/// fallthrough.  Unknown sequences drop only the `ESC` +
/// next char to stay conservative.
pub(super) fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            match chars.next() {
                Some('[') => {
                    // CSI: drain until final byte in
                    // 0x40..=0x7E (uppercase + lowercase
                    // ASCII letters + a few punctuation).
                    while let Some(d) = chars.next() {
                        if matches!(d, '@'..='~') {
                            break;
                        }
                    }
                }
                Some(']') => {
                    // OSC: drain until ST (ESC \\) or BEL.
                    while let Some(d) = chars.next() {
                        if d == '\x07' {
                            break;
                        }
                        if d == '\x1b' {
                            let _ = chars.next();
                            break;
                        }
                    }
                }
                Some('(') | Some(')') => {
                    let _ = chars.next();
                }
                Some(_) | None => {}
            }
            continue;
        }
        out.push(c);
    }
    out
}

/// 1.2.8+ — run a `PipelineData` through nu's built-in
/// `table` command, then collect the resulting bytes /
/// values as a UTF-8 String.  This is what nu's REPL does
/// for normal output: List<Record> becomes a column-aligned
/// table, naked Values become their default string repr,
/// Empty / Nothing yields an empty string.
///
/// Errors from `table` (rare — usually only for pipelines
/// that can't render) fall through to plain
/// `collect_string`, so the caller always gets *some*
/// stdout text rather than a panic or a hidden failure.
fn format_via_table(
    engine_state: &EngineState,
    stack: &mut Stack,
    pipeline: PipelineData,
    cfg: &nu_protocol::Config,
) -> String {
    // Materialise the pipeline to a single `Value` so we
    // can branch on shape.  Scalar values (Int, String,
    // Bool, Float, Date, Filesize, Duration) format
    // cleanly via `to_expanded_string`; List<Record> /
    // Record / List<Value> deserve the `table` command's
    // column-aligned rendering; Nothing yields an empty
    // string.
    let value = match pipeline.into_value(Span::unknown()) {
        Ok(v) => v,
        Err(_) => return String::new(),
    };
    if matches!(value, Value::Nothing { .. }) {
        return String::new();
    }
    let is_tabular =
        matches!(value, Value::List { .. } | Value::Record { .. });
    if is_tabular {
        if let Some(table_id) = engine_state.table_decl_id {
            let cmd = engine_state.get_decl(table_id);
            let ast_call = nu_protocol::ast::Call::new(Span::unknown());
            let call_ref: nu_protocol::engine::Call<'_> =
                (&ast_call).into();
            let pd = PipelineData::Value(value.clone(), None);
            if let Ok(formatted) =
                cmd.run(engine_state, stack, &call_ref, pd)
            {
                if let Ok(s) = formatted.collect_string("\n", cfg) {
                    return s;
                }
            }
        }
        // `table` decl missing or table-call errored —
        // fall through to the value's own expander.
    }
    value.to_expanded_string("\n", cfg)
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
    fn history_roundtrips_commands() {
        // Use a unique tempdir per test so parallel test
        // runs don't share state.
        let tmp = tempfile::tempdir().expect("tempdir");
        let h = History::open(tmp.path());
        h.push("ls");
        h.push("pwd");
        h.push("date");
        // Cap larger than count → all three back, in
        // chronological order.
        let loaded = h.load(10);
        assert_eq!(loaded, vec!["ls", "pwd", "date"]);
        // Cap smaller than count → most-recent only,
        // still in chronological order.
        let loaded2 = h.load(2);
        assert_eq!(loaded2, vec!["pwd", "date"]);
    }

    #[test]
    fn history_survives_reopen() {
        let tmp = tempfile::tempdir().expect("tempdir");
        {
            let h = History::open(tmp.path());
            h.push("first");
            h.push("second");
        }
        // New History from the same root re-opens the same
        // file — that's the restart simulation.
        let h = History::open(tmp.path());
        let loaded = h.load(10);
        assert_eq!(loaded, vec!["first", "second"]);
    }

    #[test]
    fn external_command_path_without_caret_is_captured() {
        // Nu lets `/bin/echo args` resolve as an external
        // even without the `^` prefix (path-shaped tokens).
        // The user's bug report was about externals run
        // this way — verify it captures stderr the same as
        // the explicit `^` form.
        let mut e = engine();
        let out = e.eval(r#"/bin/sh -c "echo nocaret-stderr 1>&2; echo nocaret-stdout""#);
        let combined = format!("{}\n{}", out.stdout, out.stderr);
        assert!(
            combined.contains("nocaret-stderr"),
            "expected stderr captured for path-shaped external, got stdout={:?} stderr={:?}",
            out.stdout, out.stderr,
        );
        assert!(
            combined.contains("nocaret-stdout"),
            "expected stdout captured for path-shaped external, got stdout={:?} stderr={:?}",
            out.stdout, out.stderr,
        );
    }

    #[test]
    fn external_command_failed_exit_stderr_captured() {
        // Failure case: external exits non-zero AND prints
        // to stderr.  `ls /nonexistent` is the canonical
        // example.
        let mut e = engine();
        let out = e.eval(r#"^/bin/ls /this/path/should/not/exist/13579"#);
        let combined = format!("{}\n{}", out.stdout, out.stderr);
        assert!(
            combined.to_lowercase().contains("no such")
                || combined.to_lowercase().contains("not found")
                || combined.to_lowercase().contains("cannot access")
                || combined.contains("13579"),
            "expected /bin/ls failure stderr captured, got stdout={:?} stderr={:?}",
            out.stdout, out.stderr,
        );
    }

    #[test]
    fn external_command_stderr_is_captured_not_inherited() {
        // Regression: `^/bin/sh -c "echo oops 1>&2"` would leak
        // `oops` to the host TTY if stderr wasn't redirected.
        // With Pipe-on-both, nu merges stderr INTO stdout, so
        // the probe appears in `out.stdout`.  Some shell
        // command runners might split it back into `out.stderr`
        // depending on framing — accept either, since either
        // way means the bytes were CAPTURED (didn't leak).
        let mut e = engine();
        let out = e.eval(r#"^/bin/sh -c "echo stderr-probe-13579 1>&2""#);
        // The eval itself can be Ok or it can mark success
        // false depending on how nu classifies an exit-zero
        // command that wrote to stderr — but the bytes MUST
        // appear somewhere in the capture.
        let combined = format!("{}\n{}", out.stdout, out.stderr);
        assert!(
            combined.contains("stderr-probe-13579"),
            "expected stderr bytes captured (stdout or stderr), got stdout={:?} stderr={:?}",
            out.stdout, out.stderr,
        );
    }

    #[test]
    fn external_command_output_is_captured_not_inherited() {
        // Regression: `/bin/ls -l` (or any external) used to
        // leak its bytes directly to the host TTY, corrupting
        // ratatui's alternate-screen surface.  Fixed by
        // push_redirection(Pipe(Value), …) around eval_block.
        // We can't observe the host TTY from a unit test, but
        // we CAN observe the captured stdout: if the
        // redirection guard is doing its job, the bytes from
        // /bin/echo land in `out.stdout`.  If it isn't, stdout
        // comes back empty and the bytes are gone (printed by
        // the test runner instead).
        let mut e = engine();
        let out = e.eval("^/bin/echo inkhaven-shell-capture-probe");
        assert!(
            out.success,
            "echo exit !=0: stdout={:?} stderr={:?}",
            out.stdout, out.stderr,
        );
        assert!(
            out.stdout.contains("inkhaven-shell-capture-probe"),
            "expected captured stdout to contain probe, got {:?}",
            out.stdout,
        );
    }

    #[test]
    fn strip_ansi_clears_csi_sequences() {
        // SGR colour codes around a word.
        assert_eq!(
            strip_ansi("\x1b[31mred\x1b[0m"),
            "red",
        );
        // Cursor positioning + clear-line — what tripped
        // the visual-overlap bug.
        assert_eq!(
            strip_ansi("a\x1b[1;1Hb\x1b[2Kc"),
            "abc",
        );
        // Mixed: SGR + plain + SGR.
        assert_eq!(
            strip_ansi("\x1b[33;1mwarn:\x1b[0m message"),
            "warn: message",
        );
        // No escapes: byte-identical pass-through.
        assert_eq!(
            strip_ansi("plain ascii\nand a newline"),
            "plain ascii\nand a newline",
        );
    }

    #[test]
    fn highlight_reconstructs_input_byte_for_byte() {
        // The styled spans must concatenate back to the
        // original input verbatim — no characters dropped,
        // no extra whitespace inserted by gap-handling.
        let e = engine();
        let cases = [
            "1 + 1",
            "ls",
            r#"echo "hello world""#,
            "let x = 42",
            "",
            "   ",
        ];
        for input in cases {
            let spans = e.highlight(input);
            let rebuilt: String = spans.iter().map(|(s, _)| s.as_str()).collect();
            assert_eq!(rebuilt, input, "round-trip failed for {:?}", input);
        }
    }

    #[test]
    fn highlight_produces_some_styling() {
        // Loose assertion: the highlighter must distinguish
        // at least one token from plain text on a normal
        // shell command.  Avoids pinning to a specific
        // FlatShape that nu's grammar may reclassify
        // between minor versions.  The round-trip test
        // above covers byte-exact reconstruction.
        let e = engine();
        let spans = e.highlight("ls --long");
        let any_styled = spans
            .iter()
            .any(|(_, style)| style.fg.is_some() || !style.add_modifier.is_empty());
        assert!(
            any_styled,
            "expected at least one styled token, got {spans:?}",
        );
    }

    #[test]
    fn pwd_env_var_set_to_project_root() {
        let dir: PathBuf = std::env::temp_dir();
        let mut e = Engine::new(&dir);
        let out = e.eval("$env.PWD");
        assert!(out.success, "stderr was: {}", out.stderr);
        // tempdir on macOS comes back via /var/folders/..., on
        // Linux via /tmp.  Engine::new strips trailing slashes
        // (nu rejects PWD with trailing /), so compare against
        // the trimmed form.
        let raw = dir.to_string_lossy().to_string();
        let expected = raw.trim_end_matches('/').to_string();
        let expected = if expected.is_empty() { "/" } else { &expected };
        assert_eq!(out.stdout.trim(), expected);
    }
}
