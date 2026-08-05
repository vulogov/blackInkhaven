# READERS-1 — implementation plan (grounded, file-by-file)

*Companion to `READERS-1_PLAN.md`. Every anchor verified against the tree on
2026-08-04. Phases RE-P0→P6; value core P1+P2+P3.*

---

## Grounded substrate (what READERS builds on — almost all of it exists)

**Reader notes are comments** — `src/tui/comments.rs`:
- `Comment { id: Uuid, char_start, char_end: usize, author: String, created_at,
  resolved: bool, resolved_at, text: String, replies: Vec<CommentReply> }` (`:48-68`).
  **No paragraph-id in the record** — binding is the sidecar file path.
- `CommentsFile { schema_version: u32, comments: Vec<Comment> }` (`:81-86`); per-
  paragraph sidecar `foo.typ → foo.comments.json` (`sidecar_path` `:109`);
  `load_from_sidecar` / `save_to_sidecar` (atomic; deletes when empty) `:127-168`;
  `resolve_author(Option<&str>)` `:178`.
- `ink.review.{list,add_comment,resolve}` (`src/scripting/stdlib/review.rs`) already
  read/write these; `comment_dict` (`:106`) exposes `paragraph_id, paragraph_slug,
  char_start/end, author, created_at, resolved, text, reply_count`.
- The comment panel (`Ctrl+V Shift+C`) + AI clustering `compose_comments_digest_prompt`
  (`src/tui/app/comments_impl.rs:263`, already framed "beta-reader / co-author
  comments", buckets STRUCTURAL/PROSE/FACTUAL/QUESTION) render them today.

**Anchoring free text → paragraph** — `crate::book_rag::retrieval::retrieve(store,
hierarchy, cfg: &BookRagConfig, book_id: Uuid, query: &str) -> Result<Vec<
RetrievedPassage>, String>` (`src/book_rag/retrieval.rs:21`). `RetrievedPassage { id:
Uuid /*paragraph*/, breadcrumb, body, score: f64, is_hit }` (`src/book_rag/mod.rs:19`).
Top-scored `is_hit` = the nearest paragraph. Deterministic chapter/path anchoring:
`resolve_locations` (`src/cli/editorial.rs:418`, chapter title/slug → first paragraph).

**Materialise-from-file template** — `import-epub`: command `ImportEpub { path,
book_name, dry_run }` (`src/cli/mod.rs:515`), `create_paragraph(store, cfg, parent,
title, body)` (`src/epub_import/import.rs:277`: create_node + io_atomic::write +
update_paragraph_content), `EpubImportReport { …created, errors: Vec<String> }`
(per-item errors, `dry_run` early-return). This is the shape for `readers import`.

**Worklist ingestion** — `collect` (`src/cli/editorial.rs:22`) accumulates `raw:
Vec<EditorialFinding>`; source-push blocks at `:108-153` (continuity/lector/stylist/
editor), each `raw.push(editorial::from_X_finding(&f, para))` with `para =
f.anchor.or_else(|| first_para(f.chapter))` (`first_para` `:102`). `EditorialFinding
{ category, severity, location, message, hint, source: &'static str, autofixable }`
(`src/editorial.rs:97`); converters at `:527-614`.

**REDLINE promotion** — `response_kind` default arm `_ => Brief` (`src/editorial.rs:188`)
so an unknown reader category routes to the Brief path (`redline::brief`,
`src/redline/mod.rs:78`); `confusion`/`unpaid_setup` → Decision. The `Ctrl+V Shift+R`
`f` handler already routes Rewrite/Decision/Brief (`src/tui/app.rs:13427`).

**Reconciliation input** — `collect`'s `EditorialReport.findings` carry
`location.paragraph: Option<Uuid>` + `location.chapter` + `category` + `source` — enough
to match a reader note anchored to paragraph P against AI findings at P (or its chapter).

---

## RE-P0 — the reader-note model (pure + convention)

New `src/readers/mod.rs` (+ `mod readers;` in `main.rs`).
- Reader notes reuse `Comment`, distinguished by a **role marker** so they're separable
  from the writer's own comments: reserve an author convention (`reader:<name>`, or a
  `ReaderNote` newtype wrapping `(paragraph: Uuid, Comment)` with an `is_reader()`
  predicate = author starts with `reader:` / a dedicated marker). Decide the marker
  here; everything downstream filters on it.
- `pub struct ReaderNote { paragraph: Uuid, reader: String, text: String, chapter:
  Option<String>, anchored: Anchor }` where `Anchor = { Explicit | Retrieved(f64) |
  Unplaced }` records how it was anchored (for the confirm-fuzzy-anchors UX).
- `pub struct ReaderFinding { kind: String, chapter: u32, paragraph: Option<Uuid>,
  readers: Vec<String>, message: String, corroboration: Vec<String> /*AI sources*/ }`
  — the reconciled unit (converts to `EditorialFinding` in P3).
- Pure helpers + tests (parse a `reader:<name>` marker; `Anchor` classification).

## RE-P1 — import (value enabler)

New `src/readers/import.rs` + `src/cli/readers.rs` + `Command::Readers{ReadersCommand}`.
- `readers import <file> --reader <name> [--book-name] [--dry-run]`:
  1. **parse** the feedback file into `(chapter_hint: Option<String>, text)` notes —
     support a flat `ch N: …` / `Chapter N — …` line grammar, a markdown `## ch`
     sectioned doc, and a blank-line-separated plain-notes fallback (pure `parse_notes`,
     unit-tested).
  2. **anchor** each: a `ch N` hint → `resolve_locations`-style chapter→paragraph; else
     `book_rag::retrieve(store, h, cfg, book_id, text)` → top `is_hit` paragraph (record
     `Anchor::Retrieved(score)`); below a score floor → `Anchor::Unplaced` (lands on the
     chapter's first paragraph, flagged for confirmation).
  3. **land** as a `Comment { author: "reader:<name>", text, char_start/end: 0..0 or the
     retrieved-hit span }` appended to the paragraph's sidecar via `save_to_sidecar`
     (reuse verbatim). Report = `EpubImportReport`-shape `{ notes, anchored,
     retrieved, unplaced, errors }`; `--dry-run` prints the anchor plan without writing.
- Reuses the comment sidecar contract entirely — imported notes are immediately visible
  in `Ctrl+V Shift+C` and `ink.review.list`.

## RE-P2 — reconcile (THE VALUE CORE)

New `src/readers/reconcile.rs`.
- `gather_reader_notes(store, h) -> Vec<ReaderNote>` — walk paragraph sidecars (like
  `walk_all_comments`, `src/cli/comments.rs:50`), keep `is_reader()` comments, attach the
  owning paragraph id + chapter.
- `reconcile(notes: &[ReaderNote], report: &EditorialReport) -> Reckoning` (pure,
  unit-tested): group notes by paragraph; for each cluster compute `readers: distinct
  authors`, and `corroboration = report.findings.filter(|f| f.location.paragraph == P ||
  same chapter).map(|f| f.source)`. Classify:
  - **confirmed** = `readers.len() >= 2 || !corroboration.is_empty()`;
  - **felt** = a single reader, no corroboration;
  - **unwitnessed** = `report` findings at a paragraph with **no** reader note (the AI-only
    side view — candidate false positives).
- `Reckoning { confirmed: Vec<ReaderFinding>, felt: Vec<ReaderFinding>, unwitnessed:
  Vec<EditorialFinding> }`, sorted by reader-convergence then severity.
- `inkhaven readers reconcile [--json]` renders the three groups (see the PLAN mockup).

## RE-P3 — promote into the worklist (value)

- `from_reader_finding(f: &ReaderFinding) -> EditorialFinding` (`src/editorial.rs`, beside
  the other converters ~`:565`): `category = f.kind` (e.g. `reader-confusion`, or the
  bare reader kind), `severity` from convergence (≥3 readers → Warn, else Info),
  `location { chapter, paragraph }`, `message`, `hint = corroboration summary`,
  `source: "reader"`. `response_kind` routes unknown reader kinds to **Brief**; map
  reader-confusion → `confusion` (Decision) where it fits.
- Wire a **reader source block** into `collect` (`src/cli/editorial.rs` ~`:120`, beside
  the lector block): gather reader notes → `reconcile` against the *rest of* the report
  → push the **confirmed** (opt-in: + felt) `ReaderFinding`s via `from_reader_finding`.
  Gated so a project with no reader notes adds nothing (the self-gating idiom).
- Now confirmed reader findings are rows in `Ctrl+V Shift+R`, actionable as Brief/Decision
  through the existing `f` handler — no new prose-write path.

## RE-P4 — surfaces

- `inkhaven readers list` (the imported notes by reader/chapter) + `reconcile` (P2).
- A reconciliation dashboard on the last free meta_sub chord **`Ctrl+B Shift+Z`**
  (`Modal`, rows+anchors like the CHRONICLE/ledger dashboards): the three groups,
  Enter → jump to the paragraph. + a `resolve_in` guard test (the shadow lesson). The
  imported notes themselves already surface in `Ctrl+V Shift+C`.

## RE-P5 — Bund + policy

- `src/scripting/stdlib/readers.rs` (mirror `stdlib/chronicle.rs`): `ink.readers.notes`
  ( -- list ) the reader notes; `ink.readers.reckoning` ( -- dict ) confirmed/felt/
  unwitnessed counts + lists; `ink.readers.check` ( -- dict ) `{ confirmed, felt, clean }`
  (`clean` = no *confirmed* reader finding outstanding — a pre-submit gate). **Import is
  a write → NOT exposed** (as chronicle `mark` isn't). Classify STORE_READ ×3 in
  `policy.rs`; the `every_registered_word_is_classified` guard enforces it.

## RE-P6 — capstone (docs + e2e)

- `Documentation/READERS.md` (mirror REDLINE/CHRONICLE); `Tutorials/115-*.md` + index;
  `KEYBINDING.md` (Ctrl+B Shift+Z); top-level `README.md` "Latest release"; `RELEASE_NOTES/
  2.6.0.md` + index; DEVELOPING-book audit (the fiction "revision" arc gains the
  reader-reckoning beat). CONFIGURATION: an optional `readers.anchor_score_floor` knob
  (else none). e2e: import a notes file → reconcile shows confirmed w/ corroboration →
  promoted into `Ctrl+V Shift+R`; `ink.readers.check` gate.

---

## Open decisions (resolve as we build)

- **The reader marker** — `author = "reader:<name>"` (simple, reuses the author field,
  visible in the panel) vs a dedicated comment field (a schema bump). Lean: the author
  convention (zero migration). Decide at RE-P0.
- **Promote felt findings too, or confirmed-only** — default confirmed-only into the
  worklist (high precision); `felt` stays a reconcile-view signal. A `--include-felt`
  flag. Decide at RE-P3.
- **Anchor score floor** — the retrieve-score below which a note is `Unplaced` and needs
  confirmation. Config knob, sensible default (~0.3).
- **Codename** — READERS (descriptive) vs an evocative one (WITNESS / RECKONING), as
  REDLINE settled REDLINE-vs-REVISE. Rename before RE-P1 if wanted.
