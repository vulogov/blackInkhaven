# 35 — Embedded shell pane

Inkhaven 1.2.8 adds an embedded **nushell** pane that lives
entirely inside the TUI window. Run nu pipelines without
leaving the editor; select any command's output and either
copy it to the clipboard or insert it into the manuscript
wrapped as a typst raw block.

```
Ctrl+Z o      open / close the shell pane (engine state
              + history preserved across close)
Ctrl+Z O      drop the cached engine + turn buffer and
              open fresh (use when env / scope drifted)
Ctrl+Z h      inside the pane, toggle history-selection
              mode (↑↓ pick turn · c copy · i insert)
Esc           close the pane (state preserved)
```

The pane is **opt-in via HJSON** — enabled by default, off
when `shell.enabled: false`.

## The flow

```
1. Ctrl+Z o            → shell opens, prompt is "$ ".
2. ls *.md | length    → "12"
3. date now            → "Sun, 17 May 2026 14:32:08"
4. echo "Aerin started writing" | str length  → "23"
5. Ctrl+Z h            → selection mode; cursor on turn 4.
6. ↑                   → cursor on turn 3.
7. i                   → output "Sun, 17 May…" inserts into
                         the editor as #raw(block: true,
                         lang: "shell", `Sun, 17 May…`).
                         Pane closes, editor focused.
8. (write more prose around the inserted block)
9. Ctrl+S              → paragraph saved.
```

`Ctrl+Z o` again on step 9 brings the shell back with the
same engine state (env vars, defs) and turn buffer intact.

## What the engine supports

The pane runs **real nushell** — `nu_engine::eval_block` with
`nu_command`'s default declarations registered:

- Arithmetic, strings, ranges: `1 + 1`, `"foo" + "bar"`,
  `1..10 | each { $in * 2 }`.
- Filesystem: `ls`, `cd`, `glob`, `path expand`, `open`,
  `save`.
- Data shaping: `where`, `select`, `sort-by`, `group-by`,
  `histogram`, `into int / string / record`.
- Text: `str length`, `str upcase`, `lines`, `parse`, `split`,
  `regex`.
- Process: `complete`, `^external-cmd args` for one-shot
  external invocations.
- Variables and scope: `let x = 42` (within one invocation),
  `$env.X = "y"` (persists across invocations).
- Pipelines: `cat README.md | lines | first 10`.

### What it doesn't support

- **Long-running TTY apps** (`vim`, `top`, `less`, `htop`,
  `ssh`). The pane has no PTY backing — these will hang or
  exit immediately. Use a separate terminal.
- **Stdin pipes that read from the user** mid-run. EOF is
  sent immediately when the command tries to read stdin.
- **REPL `let` persistence across separate prompts.** Each
  `Enter`-eval opens its own parse + scope; `let x = 42`
  followed by `$x` on the next line returns an error.
  Nu's interactive REPL handles this via accumulated-buffer
  re-parsing; we'd need to replicate that.  Persistent
  *env vars* (`$env.X = ...`) DO survive because they live
  on the engine's `Stack`, not on the parse-tree scope.
- **Plugins.** Out of scope for 1.2.8.

These limitations are deliberate scope cuts — the pane is
for *simple shell tasks without leaving the TUI, and for
capturing non-interactive command output into the
manuscript*. For full nu-REPL behaviour, run `nu` in a
separate terminal.

## Selection mode (Ctrl+Z h)

`Ctrl+Z h` toggles a frozen view of the turn buffer where
you navigate turn-by-turn (like the AI chat selection mode):

```
↑ / ↓         step turn cursor
Home / End    first / last turn
c             copy selected turn's output to clipboard
              (stderr included below stdout when failure)
i             insert selected turn's stdout into editor at
              cursor, wrapped in cfg.shell.insert_template;
              pane closes + editor refocuses afterwards
Esc           exit selection mode (keep pane open)
Ctrl+Z h      same — toggle back to normal shell mode
```

The selected turn renders in reversed-video so you always
know which `c`/`i` will act on.

### Insert template

The default template is:

```typst
#raw(block: true, lang: "shell", `{output}`)
```

`{output}` is replaced verbatim with the captured stdout —
no escaping, because the backtick-delimited typst `raw`
block preserves everything except the closing backtick
sequence.  Customise via HJSON for a framed presentation or
a custom show-rule.

## Persistence + history

**Per-project SQLite** at `<project>/.inkhaven/shell_history.db`:

- Up / Down arrows in the prompt walk the command history
  ring (deduped against the immediate predecessor).
- The DB persists across TUI restarts. First `Ctrl+Z o` of
  each session loads the most-recent N commands (where N =
  `shell.max_buffered_turns`) into the in-memory ring.
- `Ctrl+Z O` (fresh) wipes the engine + the in-memory ring
  but **leaves the on-disk DB alone**. The rationale is
  "fresh engine, not amnesia about what I typed last week".
  To wipe disk too: `rm .inkhaven/shell_history.db` from
  another terminal.

**Engine state** (env vars, defs, scope changes) survives
modal close/reopen as long as you stick to `Ctrl+Z o`. Only
`Ctrl+Z O` (Shift) builds a fresh engine.

## HJSON configuration

```hjson
shell: {
  // Default true.  Set false to make Ctrl+Z o a no-op
  // (status hint stays helpful).
  enabled: true
  // How many recent (command, output) pairs the pane
  // retains.  Older pairs roll off the front.  The
  // SQLite history is uncapped — this only bounds
  // working-memory + the Up-arrow recall ring's seed.
  max_buffered_turns: 50
  // Per-command cap on captured output lines.  A single
  // command that emits more than this many stdout lines
  // is truncated with a "(N more lines truncated)"
  // marker on the last kept line.  Same rule applied to
  // stderr.  Raise if you rely on `cat large_file` or
  // `git log` showing in full; lower to keep memory and
  // PgUp/PgDn responsiveness tight on slow machines.
  max_output_lines: 1000
  // 1.2.8+ — full-screen TUI apps refused before
  // spawn.  vim/less/top/tmux/… would otherwise open
  // /dev/tty and corrupt ratatui's alt-screen.  Default
  // list covers ~45 common offenders; HJSON override
  // adds/removes by basename (case-insensitive).
  blocked_externals: ["vim", "nvim", "less", "top", "tmux", ...]
  // 1.2.8+ — wall-clock budget per eval.  After timeout,
  // nu interrupt fires; if the worker stays wedged
  // through the 2-second grace window, the engine is
  // rebuilt (lose env/defs/cd, keep TUI alive).
  external_timeout_secs: 30
  // The typst markup wrapping a Ctrl+Z h → i insert.
  // `{output}` is substituted verbatim.  Default uses
  // a typst raw block with `lang: "shell"` for
  // monospace and no markdown reinterpretation.
  insert_template: "#raw(block: true, lang: \"shell\", `{output}`)"
}
```

## Scrolling the pane

The turn buffer scrolls when its accumulated lines exceed
the pane height.  By default the newest output is anchored
flush against the bottom; older content lives just above
the viewport.

| Key            | Effect                                       |
| -------------- | -------------------------------------------- |
| `PgUp`         | Scroll backward by 10 lines                  |
| `PgDown`       | Scroll forward by 10 lines (toward newest)   |
| `Shift+Home`   | Jump to the top of the buffer                |
| `Shift+End`    | Jump back to the newest output (bottom)      |

Running a new command via `Enter` automatically resets the
scroll position so fresh output is always visible.  Entering
or leaving `Ctrl+Z h` selection mode preserves scroll.

The title bar displays `· ↑ scrolled (End→bottom)` while you
are above the newest turn — a reminder that more recent
output exists below.

Custom template examples:

```hjson
// Framed grey box (useful in a printed book):
insert_template: "#block(fill: rgb(\"#f4f4f4\"), inset: 6pt, radius: 2pt)[#raw(\"{output}\")]"

// Inline shell snippet (single line — no `block: true`):
insert_template: "`{output}`"

// Two-column with a label:
insert_template: "#grid(columns: (auto, 1fr), gutter: 8pt)[*shell*][#raw(\"{output}\")]"
```

## Use cases

- **Word-count snapshot in the manuscript.** `wc -w
  manuscript/*.md` then `i` to drop the count into a
  pre-pub note.
- **Quick file inspection.** `ls (Book/00-prologue/*.typ)`
  | `length` to confirm the chapter exists; nothing
  inserted.
- **Capture diff for a comment.** `git diff HEAD~1
  Book/03-chapter` then `c` to copy + paste into a Slack
  thread.
- **One-off transformation.** Have a snippet of broken
  data?  `($paste) | from csv | to json` then `i`.

## Cwd, environment

The shell opens with `$env.PWD` set to the **project root**
(same path inkhaven was launched against). Relative paths
in `ls`, `glob`, `path expand`, etc. resolve from there.
`cd` mutates the engine's PWD env var so subsequent
commands inherit the new cwd; the change persists across
`Ctrl+Z o` close/reopen.

## Trade-offs (read before reporting bugs)

The pane is **not a terminal emulator**.  No ANSI escape
parsing, no scrollback selection by mouse, no
keyboard-shortcut chord forwarding to commands.  If you
type something that expects an interactive TTY, it dies.

This is deliberate.  The goal is "one-shot pipelines whose
output I want to capture or inspect," not "replace my main
terminal."  For the latter, use a real shell.

## See also

- [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md)
  — AI chat selection mode shares the `c` / `i` UX.
- [`33-navigation-history.md`](33-navigation-history.md) —
  the Alt+←/→ ring uses the same per-session-then-restored
  pattern as the shell command history.
