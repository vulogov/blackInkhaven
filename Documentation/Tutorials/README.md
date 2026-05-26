# Tutorials

Each tutorial walks through one concrete writing workflow end to end.
They assume nothing about your prior experience with Inkhaven, AI, or
RAG; they do assume you have already installed the binary per
[`../FIRST_STEPS.md`](../FIRST_STEPS.md).

Read in order if you are new. Skip around once you know what each
covers.

| # | Tutorial | Workflow |
| --- | -------- | -------- |
| 1 | [`01-getting-started.md`](01-getting-started.md) | First five minutes: launch the TUI, build a tiny book, save a paragraph. |
| 2 | [`02-organising-your-manuscript.md`](02-organising-your-manuscript.md) | The hierarchy, system books, folding, reordering, renaming. |
| 3 | [`03-the-editor.md`](03-the-editor.md) | Movement, selection, find / replace, undo / redo, snapshots, split-edit. |
| 4 | [`04-search-and-discovery.md`](04-search-and-discovery.md) | Semantic and full-text search; finding prose you have forgotten the words for. |
| 5 | [`05-ai-writing-assistant.md`](05-ai-writing-assistant.md) | Sending text to the LLM, scopes (F9), inference modes (F10), chat history, prompt picker. |
| 6 | [`06-grammar-check.md`](06-grammar-check.md) | The F7 grammar workflow and the `g`-apply pipeline that preserves Typst markup. |
| 7 | [`07-places-and-characters.md`](07-places-and-characters.md) | Building a lexicon of worldbuilding, using the yellow / cyan overlays, asking the AI via `Ctrl+B P` / `Ctrl+B C`. |
| 8 | [`08-importing-existing-docs.md`](08-importing-existing-docs.md) | `inkhaven import-help`, the F3 file picker, adopting a directory of `.md` / `.typ` files. |
| 9 | [`09-exporting-to-typst-and-pdf.md`](09-exporting-to-typst-and-pdf.md) | `inkhaven export typst` / `export pdf`; what a book-level config paragraph looks like. |
| 10 | [`10-backups-and-recovery.md`](10-backups-and-recovery.md) | Manual backup, restore, the auto-backup-on-exit hook, recovery from drift. |
| 11 | [`11-theming.md`](11-theming.md) | The Catppuccin Mocha defaults and every colour knob in the HJSON. |
| 12 | [`12-configuring-ai-providers.md`](12-configuring-ai-providers.md) | The six bundled provider stanzas (Gemini / Claude / OpenAI / DeepSeek / Grok / Ollama), Ctrl+B L live-switching, model upgrades. |
| 13 | [`13-ai-full-screen-mode.md`](13-ai-full-screen-mode.md) | Ctrl+B K layout, persistent chat history, Ctrl+F search, Ctrl+C selection mode that copies / inserts turns. |
| 14 | [`14-document-status.md`](14-document-status.md) | Ctrl+B R workflow ring, status badges in tree + editor header, Ctrl+B 1..7 scoped filter modal. |
| 15 | [`15-multi-format-export.md`](15-multi-format-export.md) | `inkhaven export markdown / tex / epub`, `--book-name` for multi-book projects, Ctrl+B O extra formats, Ctrl+V markdown extraction. |
| 16 | [`16-similar-paragraphs.md`](16-similar-paragraphs.md) | Ctrl+V S — vector-similarity picker + side-by-side editor with the AI pane replaced by a second editor. |
| 17 | [`17-writing-goals.md`](17-writing-goals.md) | Writing-progress tracking, the `goals:` HJSON stanza, daily-words vs morning-baseline, streak with grace, per-book deadlines, the Ctrl+V G modal. |
| 18 | [`18-bund-pane-and-script-picker.md`](18-bund-pane-and-script-picker.md) | The floating Bund output pane (`ink.pane.*`), the Ctrl+Z ? script picker, the `ink.input` prompt modal. |
| 19 | [`19-wiki-links.md`](19-wiki-links.md) | Metadata-only outgoing / incoming paragraph links, Ctrl+V A / I / L / K chords, AI-inference integration, status-bar count. |
| 20 | [`20-snapshot-diff.md`](20-snapshot-diff.md) | F6 V snapshot diff (snapshot vs current), pre-restore safety snapshot on Enter, recovery flow. |
| 21 | [`21-navigation.md`](21-navigation.md) | Ctrl+V P fuzzy paragraph picker, Ctrl+V B / M bookmarks, AI prompt Up-arrow history, slash-command prefix ranking. |
| 22 | [`22-tree-multiselect.md`](22-tree-multiselect.md) | Tree-pane mark set (Space), T cycles type, O cycles status — both work on single OR multi-selection. |
| 23 | [`23-scrivener-import.md`](23-scrivener-import.md) | `inkhaven import-scrivener` — single-binary `.scriv` ingest with RTF→Typst conversion, dry-run, mapping rules. |
| 24 | [`24-typst-in-process.md`](24-typst-in-process.md) | `typst_compile.engine = "inprocess"` — bundled compiler + fonts + `@preview` packages, parse + semantic diagnostics, Ctrl+V R render preview, Ctrl+V N diagnostic navigation, `inkhaven doctor`. |
| 25 | [`25-tag-workflows.md`](25-tag-workflows.md) | Project-wide tags — `Ctrl+B ]` picker, `Ctrl+B }` search, R rename, `inkhaven export --tag`, tree pips, `ink.tag.*` Bund, Scrivener keyword import. |
| 26 | [`26-story-view.md`](26-story-view.md) | `Ctrl+V Shift+W` book view + `Ctrl+V w` paragraph mini view — radial graph of hierarchy / paragraph links / lexicon mentions; `S` saves PNG; `ink.story.render` Bund word. |
| 27 | [`27-diagnostics.md`](27-diagnostics.md) | Typst diagnostics surface — gutter `●` markers, `F8` list modal, `Ctrl+V N` next-diagnostic, `Ctrl+F12` AI explain, `hook.on_diagnostic`, `ink.editor.set_cursor`. |
| 28 | [`28-ai-critique-and-memory.md`](28-ai-critique-and-memory.md) | `F12` mode-aware critique, opt-in per-paragraph AI memory, diff-review modal on apply (`a`/`r`/`e`), smart marker extraction for grammar replies. |
| 29 | [`29-snapshot-annotations.md`](29-snapshot-annotations.md) | `F5` annotation prompt; `F6` picker rendering with `✎` indent; when to label which snapshots. |
| 30 | [`30-render-preview.md`](30-render-preview.md) | `Ctrl+V R` render preview with `+/-` live zoom (1.2.6+); `S` saves current page, `A` saves all. |
| 31 | [`31-story-timeline.md`](31-story-timeline.md) | The full timeline feature (1.2.6+) — calendars, CLI `event add/list/show`, `Ctrl+V e` picker, `Ctrl+V Shift+T` swim lanes with scope nav, `Ctrl+V Shift+E` new event from any pane, `Ctrl+V Shift+I` edit event timing, AI health critique (y/Y/Ctrl+Y). 1.2.7+ adds tree-style Tab/Enter, Space collapse, span-aware ↑/↓ select, grid stripes, F12 full critique, session persistence, book-slug prefix. |
| 32 | [`32-paragraph-undelete.md`](32-paragraph-undelete.md) | `Ctrl+B U` restores the most recently deleted paragraph — body, tags, status, paragraph links, event data round-trip. Single-slot kill-ring (1.2.7+). |
| 33 | [`33-navigation-history.md`](33-navigation-history.md) | `Alt+←` / `Alt+→` step through visited paragraphs; `Ctrl+V Shift+P` recent-paragraph picker (1.2.7+). Browser-style back/forward across every pane that opens a paragraph. |
| 34 | [`34-mouse-and-external-changes.md`](34-mouse-and-external-changes.md) | `Ctrl+Shift+M` toggles mouse capture so the terminal's native text-select + copy works; passive watcher reloads the open paragraph when the on-disk file changes (CLI / sed / git pull / Bund script). (1.2.7+) |
| 35 | [`35-embedded-shell.md`](35-embedded-shell.md) | Embedded **nushell** pane (1.2.8+) — `Ctrl+Z o` opens a floating shell inside the TUI; `Ctrl+Z h` selection mode lets you `c` copy a command's output to clipboard or `i` insert it into the editor wrapped as a typst raw block. Per-project SQLite history; HJSON-configurable insert template. |
| 36 | [`36-config-editor.md`](36-config-editor.md) | Edit `<project>/inkhaven.hjson` from inside the TUI (1.2.8+) — `Ctrl+B 0` opens a full-screen modal editor with HJSON syntax highlighting. `Ctrl+S` saves; when bytes change a *Restart required* overlay pops up. Mirrors the paragraph editor's chord set. |
| 37 | [`37-help-book-viewer.md`](37-help-book-viewer.md) | The Help book as a rendered-markdown viewer (1.2.8+) — paragraphs render via pulldown-cmark (headings, lists, emphasis, code fences, blockquotes, links) instead of showing source. Read-only; scroll keys + mouse wheel only. |

## Scripting + chord customisation

Two narrative docs sit outside the numbered tutorial sequence
because they cross-cut every other topic:

| Topic | Doc |
| ----- | --- |
| **Bund — the embedded scripting language** | [`../Bund/BUND_TUTORIAL.md`](../Bund/BUND_TUTORIAL.md) — stack model, lambdas, hooks, `ink.*` stdlib, sandbox. |
| **Reassigning chord keys** | [`../KEYS_REASSIGNMENT.md`](../KEYS_REASSIGNMENT.md) — HJSON `keys.bindings` + `ink.key.*`. Includes the full action table. |

## Conventions used in these tutorials

- Commands you should run are in fenced code blocks. Lines starting
  with `$` indicate a shell prompt; the `$` itself is not part of
  the command.
- Keystrokes are written `Ctrl+S`, `F1`, `Ctrl+B` then `P` (the
  meta prefix needs two keypresses).
- The cursor on the TUI is represented by `│` in plain-text mockups.
- Status-bar messages and Inkhaven output are shown in code blocks
  with no leading `$`.
- When a tutorial references another doc, the link is relative — open
  it in another tab; nothing else here depends on it.

## What if I get lost?

- Inside the TUI, press `Ctrl+B H` for the pane-aware quick reference
  overlay.
- Press `F1` for the help-manual query pane (once you have populated
  the Help book — `inkhaven import-help` ingests a directory).
- Print the [`cheat sheet`](../INKHAVEN_CHEAT_SHEET.typ) (`typst
  compile Documentation/INKHAVEN_CHEAT_SHEET.typ`) — two-column A4
  with every chord, hook, and CLI subcommand.
- See the canonical reference: [`../KEYBINDING.md`](../KEYBINDING.md).
- For database / backup / recovery questions:
  [`../MAINTENANCE.md`](../MAINTENANCE.md).
- For configuration: [`../CONFIGURATION.md`](../CONFIGURATION.md).
