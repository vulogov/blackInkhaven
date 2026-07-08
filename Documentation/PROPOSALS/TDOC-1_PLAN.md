# TDOC-1 — Verified code blocks

| | |
|---|---|
| **RFC** | TDOC-1 |
| **Title** | Fidelity — code listings that can't go stale |
| **Status** | Proposed — targets a 1.6.x point release |
| **Author** | Vladimir Ulogov |
| **New dependency** | none (shell out to author-configured commands) |
| **Program** | TDOC (see `TDOC_ROADMAP.md`) |

## The idea

Fiction's fact-checker measures the prose against the compiled world. TDOC-1 gives
the technical writer the same weapon against *their* ground truth: a code listing
marked `verify` is extracted, run against the current toolchain, and — if it no
longer compiles or passes — flagged in the **Output pane** exactly like any other
Inkhaven finding, on the exact paragraph. Deterministic, zero AI, zero network. The
staleness that rots documentation ("this example was true two releases ago") becomes
a red mark you can see.

## Grounded current-state (what exists to build on)

- **Code listings already exist** — STRUCT-2 subtype `para:code`
  (`src/tui/app.rs`, `STRUCTURAL_SUBTYPES` table). The seed body is
  `#figure(caption: [...])[ ```rust … ``` ]`, so the language + code live in a
  fenced ` ```lang … ``` ` block inside the figure. `para:code`-tagged manuscript
  paragraphs are the scan set.
- **The finding surface** — `crate::pane::output::{Message, kinds, emit}`,
  `Message::new(kind, severity, Lifetime::UntilActedOn, json!({"text":…}))`,
  `.with_source_paragraph(id)`, then `emit`. `src/world/fact_check.rs::emit_finding_impl`
  is the exact pattern to mirror. Findings already route to the Output pane, carry a
  source group (`src/pane/output/filter.rs::message_source`), colour by severity, and
  land a tree badge.
- **Paragraph access** — `App::paragraph_text(id)` (reads the open doc or the store);
  the book-scope walk in `fact_check_scope_book` (collect `NodeKind::Paragraph` under
  a book) is the iteration template.
- **CLI shape** — a subcommand enum + `Command::X(cmd) => x::run(cmd, project)`
  (mirror `SourcesCommand` / `Command::Sources`).

## Design

### Opt-in, twice

Running code is a real risk, so verification is opt-in on **both** ends:

1. **The project must declare runners.** No language runs unless the project's
   config names a command for it. A project with no runners configured verifies
   nothing (and `docs verify` says so).
2. **The block must ask to be verified.** Only a code block whose fence info string
   carries the `verify` flag is run — `` ```rust verify ``. Illustrative or
   pseudo-code blocks (the majority) are never executed. (`no-verify` is accepted as
   an explicit exclusion for symmetry.)

### Config — `docs.verify`

`src/config.rs`, a new `DocsConfig` block (field `docs`):

```hjson
docs: {
  verify: {
    enabled: true            // master switch (default false — opt-in)
    timeout_seconds: 30       // per-block wall-clock cap
    runners: {
      // language → command. `{file}` is replaced by a temp file holding the
      // block's code (with the right extension); `{dir}` by its parent.
      rust:   "rustc --edition 2021 --crate-type lib {file} -o /dev/null"
      python: "python -m py_compile {file}"
      bash:   "bash -n {file}"
      go:     "go build -o /dev/null {file}"
    }
  }
}
```

- Unknown languages (no runner) are **skipped with a note**, never an error.
- `{file}` substitution + a temp file per block (extension from a `lang → ext` map:
  rust→rs, python→py, …; fallback `.txt`).

### Extraction

A pure function `extract_verifiable(body: &str) -> Vec<CodeBlock>` where
`CodeBlock { lang: String, code: String, verify: bool }`:
- Scan the paragraph body for fenced ` ```<info> … ``` ` blocks (reuse the same fence
  handling the split-inline-code scanner / `typst_to_markdown` already rely on).
- Parse the info string: first token = language, remaining space/comma tokens =
  flags; `verify` present → run it.
- Only `para:code` paragraphs are scanned (gate on the `para:code` tag), so ordinary
  prose with incidental fences is ignored.

### The runner

`fn run_block(block, cfg) -> VerifyOutcome` (`VerifyOutcome::{Pass, Fail{stderr},
Skipped{reason}, Errored{reason}}`):
- Resolve the runner for `block.lang`; none → `Skipped`.
- Write `block.code` to a temp file (`{ext}`); substitute `{file}`/`{dir}` into the
  command; `std::process::Command` with the configured argv (split on whitespace, or
  run via the shell for pipelines — decide: run via `sh -c` for flexibility, documented
  as such).
- Enforce `timeout_seconds` (spawn + wait-with-timeout; kill on expiry → `Fail`).
- Capture exit status + stderr; nonzero → `Fail{stderr}`.
- No network is used by TDOC-1 itself; whatever the runner does is the author's.

### Findings → Output pane

A new kind `kinds::DOC_VERIFY` (+ a `"docs"` source group in
`filter.rs::message_source`, and a glyph in `draw_output`'s `kind_glyph` map, e.g.
`⌨`). Per failing block:

```rust
let mut msg = Message::new(
    kinds::DOC_VERIFY,
    Severity::Warning,                    // a stale example is a real defect
    Lifetime::UntilActedOn,
    json!({
        "text": format!("code example failed `{lang}` verification"),
        "category": "doc-verify",
        "lang": lang,
        "detail": stderr_first_lines,     // shown in the expand (o/Space) view
    }),
).with_source_paragraph(node_id);
emit(&msg);
```

- Prior `DOC_VERIFY` findings for a re-checked paragraph are cleared first (mirror
  `clear_paragraph_fact_warnings`).
- `Pass`/`Skipped` emit nothing (quiet on success), but the run **summary** goes to
  the status line: `docs verify · N ran · M failed · K skipped`.

### Surfaces

- **CLI**: `inkhaven docs verify [--book <name>] [--paragraph <id>] [--dry-run]`.
  `--dry-run` lists each block that *would* run and its resolved command (the safety
  preview). Exit non-zero when any block fails (fits CI).
  - New `enum DocsCommand { Verify {…} }` + `Command::Docs(DocsCommand)` +
    `Command::Docs(cmd) => docs::run(cmd, &project)`.
- **TUI**: a `Ctrl+B` chord (a free key — e.g. `Ctrl+B Shift+D`, "docs verify") that
  runs over the book containing the open paragraph, findings to Output; the
  book/recent/paragraph scope mirrors the fact-check scope menu.

## Safety (the load-bearing section)

Executing project-declared commands is the whole risk. Mitigations:

1. **Off by default** (`docs.verify.enabled: false`); nothing runs until the author
   turns it on and names runners.
2. **First-run consent** — the first time `docs verify` finds configured runners in a
   project, require an explicit confirmation (TUI prompt / `--yes` in CLI) that shows
   the runner commands. A project you just cloned cannot silently run code.
3. **Never automatic** — TDOC-1 runs only on explicit `docs verify` / the chord. It is
   *not* wired into the idle background fact-check, project open, or `build`.
4. **`--dry-run`** prints the exact command per block without executing.
5. **Per-block timeout**; killed → `Fail`, not a hang.
6. Document clearly that runners execute with the user's privileges; recommend
   compile-only / lint-only runners (`rustc`, `py_compile`, `bash -n`) over test
   runners in the seed config.

## Phases

- **P0** — `DocsConfig` + `docs.verify` config block + `lang→ext` + `lang→runner`
  resolution. Serde-defaulted; `enabled` false.
- **P1** — `extract_verifiable` (pure) + `para:code` gating. Tests.
- **P2** — `run_block` (temp file, `{file}` substitution, `sh -c`, timeout, capture).
  Tests with a trivial always-pass / always-fail runner (`true` / `false`).
- **P3** — `kinds::DOC_VERIFY` + `message_source` "docs" group + glyph + emit +
  per-paragraph clear + status summary.
- **P4** — CLI `docs verify` (+ `--dry-run`, `--book`, `--paragraph`, non-zero exit).
- **P5** — TUI chord + scope (book / current paragraph), mirroring fact-check.
- **P6** — Safety: `enabled` gate, first-run consent, dry-run, timeout, no auto-run.
- **P7** — Tests: extractor (info-string flags, multiple blocks, no-verify), command
  substitution, a golden pass + fail through a stub runner, config round-trip.
- **P8** — Docs: extend the *Developing a story with Inkhaven* technical chapter
  (a "Hands-on: verify your examples" procedure) + a short note wherever the
  `para:code` subtype is documented.

## Non-goals (this RFC)

- Running examples across a *matrix* of toolchain versions (a later track).
- Capturing/asserting example **output** (golden-output testing) — verify is
  compile/run-success only here; output-assertion is TDOC-1.5 if wanted.
- Highlighting / HTML rendering of code (TDOC-4).
- Auto-fixing a stale example (out of scope — Inkhaven flags, the author fixes).

## Open decisions

1. **`sh -c` vs argv split** — `sh -c` allows pipelines/redirection in a runner but is
   less portable and slightly riskier. Lean `sh -c` (documented), since runners are
   author-declared anyway.
2. **The `verify` marker location** — fence info string (` ```rust verify `,
   travels with the code, portable) vs a per-paragraph tag. Recommend the info string.
3. **Severity of a failure** — `Warning` (⚠) vs `Contradiction` (⊗). Recommend
   `Warning`; a failed example is a defect but not a self-contradiction.
