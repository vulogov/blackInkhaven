# SUBMISSION-1 — Submission track (1.3.1) implementation plan

_Status: planning. Target: **1.3.1**. Completes pillar A of the 1.3 theme
("From Draft to Submission") — the print track shipped in 1.3.0 (PDF-1);
this is the **submission** track._

## What this is

1.3.0 took a finished book to a printable, bindable artefact. It did not
help the more common path: getting the manuscript in front of an agent or
editor. The Shunn manuscript **format** already ships (`inkhaven
manuscript`, 1.2.19 — a typst render), so the gap is not the format. It is:

1. **The format agents actually require** — Word (`.docx`), not PDF/typst.
2. **The package around the manuscript** — query letter, synopsis, comp
   titles, logline.
3. **Keeping track of where it went** — a submission log.

Three tracks, sequenced so the concrete unblocker (`.docx`) lands first,
the structured-data piece (tracker) second, and the AI design risk
(generators) third.

Everything stays in the house style: pure-Rust, single binary, no external
apps; CLI + `ink.*` Bund + TUI + the HJSON cascade; multilingual
(en/ru/de/fr/es) where prose is generated; the 1.2.15 stability bar (no
panic surfaces, atomic `io_atomic` writes, poison recovery).

## Builds on (already in tree)

- **`src/manuscript.rs`** — `ManuscriptMeta`, `round_word_count`,
  `header_keyword`, `is_scene_break`, `build_typst`. The `.docx` writer
  reuses the metadata + the chapter collector (lifted to shared code).
- **`src/cli/manuscript.rs::collect_chapters`** — the book→chapter→
  paragraph walk; promote to `manuscript::collect_chapters` so typst and
  docx share one source of truth.
- **`SYSTEM_BOOKS` + `ensure_system_books`** (`src/store/mod.rs:36`,`:414`)
  — adding `("submissions", "Submissions")` auto-seeds it on every project
  open, including existing projects (the same path that creates a missing
  Facts book). NB: users on a stale binary won't see it — see Risk 5.
- **The AI scope + prompt resolver** — `AiMode` (`src/tui/inference.rs`),
  the `Facts`/`Book` whole-context scopes, and the F7-style prompt
  precedence (Prompts-book paragraph → `prompts.hjson` → built-in). The
  generators are new `submission-*` prompt slugs on the same resolver.
- **Sidecar-JSON precedent** — inline comments (1.2.14) and threads store
  structured records in `.inkhaven/*.json`; the tracker follows suit.

## Dependency audit (gating)

- **`docx-rs`** (pure-Rust `.docx` writer: zip + OOXML). The only candidate
  new runtime dep. Audit exactly as `lopdf` was: confirm pure-Rust, no
  `-sys`/external-app pulls, MIT/Apache, and a transitive-tree diff
  (`zip` is already in tree from the EPUB work — likely shared). If it
  drags a heavy or non-pure dep, fall back to **hand-rolling OOXML**
  (`.docx` is a zip of a fixed set of XML parts; we already emit EPUB
  XHTML by hand with the in-tree `zip`, so this is a known quantity).
- No other new deps. Generators reuse the existing LLM stack; the tracker
  is `serde` + sidecar JSON.

## Phases

### P0 — `.docx` dependency audit + fidelity gate (landed)

The make-or-break risk for track 1 is **whether Word honours what the
library emits** — running header, double-spacing, page breaks, title-page
layout. De-risked first, before building on top (the PDF-1 lesson).

**Audit outcome — hand-roll, zero new deps.** `docx-rs v0.4.20` (even with
`--no-default-features`) hard-pulls **`zip v0.6.6`** — a second major
version alongside the in-tree `zip v2` (EPUB) — plus its own `flate2`
chain. That duplicate is exactly the tech debt the lopdf `embed_image`
audit rejected, and `quick-xml`/`serde`/`thiserror` are already shared, so
the lib buys little. A `.docx` is six small XML parts in a zip, and the
EPUB writer already hand-rolls a zip container over `zip v2`. Decision:
**hand-roll OOXML**, no new dependency.

**Writer landed.** `src/export/docx.rs` — `build_docx(&ManuscriptMeta,
&[ManuscriptChapter], DocxFont) -> Result<Vec<u8>>`: the six OOXML parts
(`[Content_Types].xml`, `_rels/.rels`, `word/_rels/document.xml.rels`,
`word/styles.xml`, `word/header2.xml`, `word/document.xml`), Shunn layout —
title page (contact corner + rounded word count + centred title/byline),
double-spaced 12 pt Times/Courier via `docDefaults`, 1″ margins, ½″
first-line indent, `<w:titlePg/>` so page 1 has no header, a
`Surname / KEYWORD / PAGE`-field running header from page 2, chapter
`pageBreakBefore`, scene breaks as a centred `#`. Reuses `ManuscriptMeta` +
`round_word_count` / `header_keyword` / `is_scene_break`.

**Gate — passed.** Structural unzip tests assert every part, the font +
`w:line="480"` double-spacing, the header keyword + live `PAGE` field,
`titlePg` + header ref + page breaks + scene break + XML-escaping;
`document.xml` parses well-formed via `quick-xml`. `file(1)` identifies the
output as *Microsoft Word 2007+*. An `#[ignore]`d
`emit_sample_docx_for_manual_word_check` writes
`/tmp/inkhaven-shunn-sample.docx` for the manual Word open (the one check a
headless box can't do; LibreOffice headless convert is the CI option when
available).

### P1 — Shunn `.docx` surfaces

The writer (`src/export/docx.rs::build_docx`) landed in P0. P1 wires it to
the surfaces and shares the chapter walk:

- **Shared collector** — promote `cli/manuscript.rs::collect_chapters` to
  `manuscript::collect_chapters` so the typst and `.docx` paths build their
  `ManuscriptChapter` list from one place (word count excludes scene-break
  markers, as today).
- **CLI** — `inkhaven docx [book] [--out --title --author --contact
  --font times|courier]`, mirroring `inkhaven manuscript`.
- **Book-take** — `docx` in `output.extra_formats` (source-derived, like
  `markdown`/`tex`/`epub`; lands next to the PDF with the book stem).
- **Bund** — `ink.export.docx` (or extend the existing export word set).
- Tests: unzip → assert paragraph/section structure, header text, double-
  spacing run property, title-page fields; empty-book guard; font switch.

### P2 — `Submissions` book + tracker (landed)

- **P2.1** — `("submissions", "Submissions")` added to `SYSTEM_BOOKS`
  (after Language; auto-seeds via `ensure_system_books`, existing projects
  included). The book holds generated **prose drafts**. Tracker:
  `src/submissions.rs` over the `.inkhaven/submissions.json` sidecar —
  `SubmissionStatus` / `SubmissionRecord` `{ id, market, agent, draft_ref,
  date_sent, status (drafting|sent|rejected|offer|withdrawn),
  response_date, next_action_date, notes, log }` / `SubmissionLog` (atomic
  `io_atomic`, sequential `S<n>` ids). CLI `inkhaven submissions
  add|list|status|remove` (`list --json/--status/--open`).
- **P2.2** — TUI tracker modal on **`Ctrl+V u`** (free in the view table;
  distinct from `Ctrl+V Shift+U` kill-ring): colour-coded status, yellow
  next-action dates, cursor row reversed; ↑↓ navigate, `Space`/`s` cycle
  status (stamps a response date for rejected/offer), `d` remove — both
  persist. Keybind test pins it vs the kill-ring.
- **Timestamped note trail** (added on request) — `SubmissionRecord.log:
  Vec<NoteEntry { date, text }>` (append-only, serde-default,
  backward-compatible). `inkhaven submissions add-note <id> <text>` stamps
  today; `list` prints the trail; the modal expands the selected record's
  trail + chips a 📝N count. Tracks a submission's progression — got a
  call, requested edits, moving to round two.
- **Deferred** — the optional `stale-submission` doctor scan (sent, no
  response past N days → nudge); a small follow-up, not blocking.

### P3 — Submission-package generators (AI, RAG-grounded)

The design risk is **whole-book context** — a novel doesn't fit a prompt.

- **Book digest** (the context substrate) — cached per-chapter one-line AI
  summaries + the existing blurb/logline + Characters + Threads, stored in
  the Submissions book (or a sidecar) and regenerated on demand. Generators
  consume the digest + RAG-pulled passages, never the raw manuscript.
- **Generators** — each a resolvable prompt (precedence Prompts-book →
  `prompts.hjson` → built-in; slugs `submission-query`,
  `submission-synopsis-short`, `submission-synopsis-long`,
  `submission-comps`, `submission-logline`):
  - **query letter** — hook + mini-synopsis + bio + comps.
  - **short synopsis** (~1 pp) / **long synopsis** (2–3 pp) — full arc,
    *including the ending* (synopses spoil by design).
  - **comp titles** — suggestions + rationale, **Local-pinned** (no web)
    and clearly labelled *suggestions* — never assert sales figures
    (Risk 3).
  - **logline / pitch**.
- **Surfaces** — `inkhaven submission <kind>` writes a draft into the
  Submissions book and links it to a tracker record; TUI chords stream into
  the AI pane with `I`-lift into the Submissions book (reuse the
  translate/extract apply pattern). Multilingual prompt fragments.

### P3.5 — `ink.export.*` Bund surface (planned)

P1 deferred a lone `ink.export.docx` because there was no export Bund
surface to slot into.  This phase adds the whole family, so a release /
automation script — the thing Bund exists for — can emit every artefact in
one pass (`ink.pdf.*` already lets a script impose + cover; this completes
it for the prose formats).

**Feasibility (confirmed).** Bund words reach the project exactly like the
existing `ink.node.*` words: `active_store(tag)` → `store.project_root()` →
`ProjectLayout` + `Hierarchy::load(store)`; `active_config(tag)` for
defaults; `resolve_fs_path` sandboxes the output path (same as
`ink.pdf.save`).  Book resolution reuses `cli::resolve_user_book`
(case-insensitive title / slug).  So the words are thin glue over the same
`build_model` / `build_docx` / `build_typst` / `build_epub` the CLI calls.

**Surface** — new `src/scripting/stdlib/export.rs`, registered in
`stdlib/mod.rs`:

- `ink.export.docx ( book path -- )` — Shunn Word (Times default).
- `ink.export.manuscript ( book path -- )` — Shunn typst.
- `ink.export.epub ( book path -- )`.
- `ink.export.markdown ( book path -- )` / `ink.export.tex ( book path -- )`.

Each: pull `path` (sandboxed via `resolve_fs_path`), pull `book` (string →
`resolve_user_book`; empty / `NODATA` → the sole user book when
unambiguous, mirroring the CLI's optional `--book-name`), build, write via
`io_atomic`.  v1 is positional; font / title / author overrides via a
trailing options dict (`ink.export.docx ( book path opts -- )`) is a
follow-up — the CLI flags already cover the interactive path.

**Policy.** Every `ink.export.*` writes a file → category `FS_WRITE` in
`WORD_CATEGORIES` (like `ink.pdf.save`), pinned by an
`export_disk_words_classified` test (mirror `pdf_disk_words_classified`).

**Tests.** A register-on-a-VM smoke (mirror the existing stdlib register
tests) + the policy classification; export correctness is already covered
by the `build_docx` / `build_typst` / `build_epub` unit tests.

Small, no new deps, low risk — lands after the generators (P3) or
independently.

### P4 — Docs + release

- Tutorial 66 (submission workflow: `.docx` → package → tracker).
- KEYBINDING (tracker chord), CONFIGURATION (`submission:` block — default
  font, contact block, generator scope/model).
- RELEASE_NOTES/1.3.1 finalize, top-level README (last release only),
  version bump → 1.3.1, signed tag, `cargo publish`, merge to main.

## Risks / decisions

1. **`docx-rs` fidelity** — *the* gating risk; de-risk in P0 with a Word
   round-trip before building. Fallback: hand-rolled OOXML over the in-tree
   `zip` (same approach as EPUB).
2. **Whole-book context for generators** — the digest + RAG strategy; weigh
   digest regeneration cost + cache invalidation (chapter edits dirty the
   digest).
3. **Comp-title hallucination** — Local-pinned, labelled as suggestions,
   never assert sales/market claims; a power author curates them.
4. **Tracker storage shape** — prose drafts live in the Submissions book;
   structured records in the sidecar. Don't put structured state in prose
   or prose in JSON.
5. **New system book vs stale binaries** — `Submissions` auto-seeds via
   `ensure_system_books`, but users on a binary < 1.3.1 won't see it (the
   Facts-on-1.2.7 lesson). Optional small item: record last-opened version
   in the project and surface a "binary N releases behind" nudge.

## Out of scope (1.3.1)

- Web comp-title research / live market data (no external apps/web by
  default).
- Direct agent-database / submission-portal integration.
- `.docx` **import** (export only).
- The Whole-Book AI Editor (RAG over the whole manuscript) → 1.4+.

## Sequencing

`.docx` (P1) is the concrete unblocker and ships first; the tracker (P2) is
small and sets up where drafts live; the generators (P3) carry the AI
design risk and land last. Each phase is self-contained and preflight-clean
on its own — a partial 1.3.1 (say P0–P2) is still a shippable cut if P3
needs more room.
