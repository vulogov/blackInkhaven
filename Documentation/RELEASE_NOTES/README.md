# Release notes

Per-release write-ups for every published Inkhaven version. Each
file walks through what changed at user-facing granularity — new
chords, new modals, new config knobs, new CLI subcommands — plus
migration notes when the on-disk shape moved.

The point of these is **continuity**: a writer coming back to the
TUI after a few months should be able to read one or two notes and
see what they missed without trawling the git log.

| Version  | Released   | Notes                                | Highlights |
| -------- | ---------- | ------------------------------------ | ---------- |
| **1.2.6** | 2026-05   | [`1.2.6.md`](1.2.6.md)               | **Tools for the working novelist.** Tag triad (project-wide picker, rename, `export --tag`, tree pips, `ink.tag.*`, Scrivener keyword import). Diagnostic UX (gutter `●`, F8 list, Ctrl+V N/Shift+N, Ctrl+F12 AI explain, `hook.on_diagnostic`). AI as a partner (F12 critique, per-paragraph memory, diff modal). Snapshot annotations (F5 prompt, F6 picker `✎` rendering). Story view split (Ctrl+V w paragraph mini + Ctrl+V Shift+W book). Render preview live zoom. **Story timeline (1.2.7 preview, ships in 1.2.6)** — calendar-aware events, Ctrl+V e picker, Ctrl+V t swim lanes with scope nav, y/Y/Ctrl+Y AI critique, seven `ink.event.*` Bund words + 2 hooks. F11 → Ctrl+F12 (macOS workaround). Seven new tutorials (25 — 31). New **Book of Inkhaven** companion volume. |
| **1.2.5** | 2026-05   | [`1.2.5.md`](1.2.5.md)               | **Typst goes in-process** — `typst_compile.engine = "inprocess"` runs `typst::compile + typst-pdf` inside inkhaven (no shell-out, bundled fonts, `@preview` package fetch). Phase-1 `typst-syntax` parse diagnostics on save/idle; opt-in semantic diagnostics via full compile. `Ctrl+V R` renders the open paragraph to a floating PNG preview with page navigation (←/→) + save-current (S) + save-all (A). `Ctrl+V N` jumps the cursor to the next diagnostic. Esc cancels in-flight compiles; autosave fires before Ctrl+B A/B/O. New `inkhaven doctor` CLI. Embedded logo banners the credits pane. |
| **1.2.4** | 2026-05   | [`1.2.4.md`](1.2.4.md)               | Wiki-links + backlinks (Ctrl+V A/I/L/K), navigation pack (Ctrl+V P/B/M, AI Up history), snapshot diff (F6 V) + pre-restore safety snapshot, per-paragraph word targets + auto-promote, active-time tracking, per-book bar chart, `inkhaven export --status`, `inkhaven stats`, Scrivener importer, tree multi-select + bulk T/O, save-as picker, theme persistence, Bund stdlib gaps closed (`ink.fs.*`, `ink.editor.replace_all`, `ink.search.load`, `ink.ai.poll`, `ink.ai.send_blocking`), F-keys in keybind table, startup splash, Windows CI re-enabled. |
| **1.2.3** | 2026-05   | [`1.2.3.md`](1.2.3.md)               | Multi-format export (markdown / TeX via tylax / EPUB) + `--book-name`, writing-progress subsystem (Ctrl+V G modal, status-bar widget), similar-paragraph side-by-side mode (Ctrl+V S), Bund output pane + Ctrl+Z ? script picker + `ink.input`, dynamic Quick Help. |
| **1.2.1** | 2026-05   | [`1.2.1.md`](1.2.1.md)               | bdslib + tree-sitter-typst absorbed in-tree (crates.io-publishable), Bund scripting (`ink.*` stdlib, hook points, `.bund` Script nodes, Scripts system book), data-driven keymap with HJSON + Bund rebinding, `Ctrl+B M` cycle-type, dirty-flag sync. |
| **1.1**  | 2026-05    | [`1.1.md`](1.1.md)                   | Images first-class, Book assembly / build / take, HJSON-driven `settings.typ`, six LLM providers, AI full-screen + typewriter layouts, document-status workflow, HJSON data nodes, 1000+ commits of polish. |
| **1.0.3** | 2026-04    | (tag-only — see GitHub Releases)     | First public binary release: Linux x86_64 + macOS aarch64. |
| 1.0.2 / 1.0.1 / 1.0.0 | 2026-04 | (tag-only — release-pipeline iteration) | Build / matrix-shaping commits, no user-facing change between them. |

## Reading order

If you're upgrading from an earlier version, read the notes for
every version between the one you came from and the one you're on,
in order. Each release-notes file is self-contained but assumes
you've read the previous one's "Breaking changes" section, if any.

## What we *don't* put here

- Per-commit changelogs — `git log` is canonical for that.
- Internal refactors with no user-visible effect.
- Bugfixes that simply restore advertised behaviour (those land in
  point releases without notes; major behavioural changes get
  notes regardless of size).
