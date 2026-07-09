# TDOC — Technical Documentation roadmap (1.6 branch)

A phased program of point-release tracks that make Inkhaven a genuinely strong tool
for **technical documentation** authors, grounded in what already ships. Each track
is bounded to one 1.6.x release, additive, and (except where noted) needs no new
runtime crates.

## The through-line

Fiction's enemy is *self-contradiction*; technical documentation's enemy is
**staleness** — prose that was true of the last release and is now a quiet lie about
this one. Inkhaven's soul is checking prose against a ground truth (the fact-checker
against the compiled world). The whole point of TDOC is to give the documentation
author the *same superpower against the system they describe*: make the docs answer
to the code, the API, the spec — and stay findable and consistent while they do.

## What already ships (build on, don't rebuild)

- **Semantic building blocks** — STRUCT‑2 structural subtypes: admonitions
  (note / warning / tip / caution), code listings, math (`para:*`, morph via
  `Ctrl+B m`). *Admonitions and code blocks exist.*
- **Single-sourcing primitives** — the **Snippets** book (`#include`, REUSE‑1) and
  STRUCT‑1 **Jinja** paragraphs (`content_type: "jinja"`, rendered with `minijinja`
  against a per-book manifest context at assembly).
- **Terminology governance** — the **Glossary** book + `terms suggest` / `terms
  check` (+ the `Ctrl+V z` overlay). *A synonym is a bug; already enforced.*
- **Structure & findability** — the tree, Outline (`Ctrl+2`), links/backlinks
  (`Ctrl+V a/i/l/k`), the **concordance** (`Ctrl+B Shift+L`), status marks + tags.
- **The reader's eyes** — the AUDIENCE personas (`domain-newcomer`, `end-user`,
  `expert-reviewer`) on Inner Socrates; comments (`Ctrl+V Shift+C`); the `technical`
  init template.
- **Production** — export to Typst / PDF / Markdown / LaTeX / EPUB / DOCX; the `pdf`
  finishing workshop; tree-sitter highlighting in the editor.

The gaps below are what a technical writer still can't do well.

---

## TDOC‑1 · Fidelity — code that can't go stale  *(flagship; recommended first)*

The single most on-brand feature: **verified code blocks.** Mark a code listing
`verify` and name a per-language runner in config; `inkhaven docs verify` (and a
`Ctrl+B` chord) extracts every marked block, runs it, and surfaces pass/fail in the
**Output pane** exactly like a fact-check finding — a red ⊗ on the listing whose
snippet no longer compiles against the current release.

- **Config**: `tdoc.verify.<lang> = "<command>"` (e.g. `cargo check`, `pytest`,
  `bash -n`). The block's text is written to a temp file and the command run against
  it; nonzero exit → a finding.
- **Why it fits**: this is fiction's fact-checker for docs — a deterministic check
  against ground truth, zero AI, zero network. Failures land on the exact
  paragraph, like every other Inkhaven finding.
- **Effort/risk**: low. No new crate (shell out to a configured command). The
  scariest part is sandboxing — scope to opt-in, project-configured commands only.

## TDOC‑2 · Fidelity — links that resolve

Extend the existing dead-link machinery (`src/research/deadlinks.rs`, today scoped
to research sources) to the **whole manuscript**: `inkhaven docs links` checks (a)
**internal cross-references** — a link to a node that was renamed or deleted is
flagged — and (b) **external URLs embedded in prose** for link-rot. Findings land in
the Output pane.

- **Why it fits**: a broken cross-reference is the reference-side face of staleness;
  a dead external link looks like authority and delivers nothing. Reuses `reqwest`
  (already present) and the deadlinks pattern.
- **Effort/risk**: low–medium. Internal xref check is pure/deterministic; external
  reuses deadlinks (timeouts already added in 1.6.6).

## TDOC‑3 · Single-sourcing — write once, render many

Two additive pieces so one source serves several outputs:

- **Conditional / profiled content.** Tag any node with a profile (`audience:expert`,
  `edition:enterprise` — reusing the existing free-form tag system); `export
  --profile edition=enterprise` renders only the matching + unconditional content.
  DITA-style profiling on tags Inkhaven already has.
- **Prose variables.** A lightweight `{{product}}` / `{{version}}` substitution pass
  over *ordinary* prose at assembly (STRUCT‑1's Jinja covers whole paragraphs; this
  covers inline values), resolved from a small `variables:` config block. One place
  to change the product name across the whole book.
- **Effort/risk**: low. Profiling is an export-scoping filter (siblings of the
  `--status` scope); variables are a substitution step in `assemble.rs`.

## TDOC‑4 · Production — the published site  *(the big production win)*

Technical docs live as **websites**, and Inkhaven has no HTML output. Add `inkhaven
export html`: a multi-page static site with

- a **sidebar navigation** built from the tree,
- **syntax-highlighted** code (reuse `tree-sitter-highlight`),
- resolved **variables + profiles** (TDOC‑3),
- **client-side reader search** over the content (a prebuilt JSON index + a few KB
  of vanilla JS), and
- admonitions rendered as styled callouts.

- **Effort/risk**: high. Now a **sub-program** of five point releases — full spec in
  [`TDOC-4_PLAN.md`](TDOC-4_PLAN.md).

**Decision (2026-07-08): build our own exporter — do NOT use Typst's HTML target.**
It is document-oriented (not a site) and heavy/unstable; it would give us none of
what matters (nav, per-page split, search, profiles, semantic admonitions).

**Scope expanded (2026-07-08, Vladimir).** TDOC-4 is no longer just "manuscript →
HTML" — it publishes the whole shelf. Ten requirements: (1) **multilingual** by the
book's `Config.language` (chrome via `Labels::for_language`, en/ru/fr/de/es);
(2) **templateable** (reuse `minijinja`); (3) **HJSON as jinja variables** (a `site`
vars file + the existing linked-HJSON path); (4) **images + clean Typst→HTML**
(reuse `typst_to_markdown`, copy assets); (5) **Sources** with proper formatting
(`BibEntry` + `@key` resolution); (6) a **reliable TOC** (from the tree + heading
scan); (7) **separate "visual" from "functional"** in the templates (two overridable
namespaces — `functional/` machinery vs `theme/` look); (8) **enable/disable system
books** (People/Places/Glossary/Notes/Mythology as appendix sections); (9) the
**Language** book as a full HTML **dictionary/grammar/study** (reuse
`conlang::output`); (10) the **World** book as a formatted world guide (materialised
world HJSON). Research confirmed heavy reuse — conlang/sources renderers, the jinja
HJSON context, and the assembler already exist; system books are already excluded
from normal assembly, so folding them in is opt-in. Phasing: **4.1** the site
skeleton (engine + templating + i18n + TOC + images), **4.2** search + Sources,
**4.3** companion books, **4.4** the living language, **4.5** the world guide.

*Architecture — structure from the tree/metadata, prose via the existing converter:*
- The exporter is a **third consumer of the same node tree** the PDF/DOCX/Markdown
  exporters walk. Page structure (and the sidebar nav) comes from the
  book→chapter→subchapter hierarchy.
- **Structural subtypes render from metadata** (`para:*` / `content_type`), not by
  re-parsing: prose nodes go through `export::markdown::typst_to_markdown` (already
  tested) → a small md→HTML step; admonitions → `<aside class="note|warning|…">`;
  code → `<pre><code class="language-x">`; math → MathML.
- **TDOC‑3 applied inline**: skip non-matching profiled nodes; substitute `{{var}}`.
- **Self-contained shell**: inlined CSS + a few KB of vanilla JS (client-side search
  over a build-time `search-index.json`, code copy, theme, collapsible nav). No CDN,
  no server — drop on any static host or open from disk.
- **Crates**: effectively **zero new** for the MVP (reuse `typst_to_markdown`,
  hand-roll the small md→HTML over its known subset). Optional later: `pulldown-cmark`
  (robust markdown) and per-language tree-sitter grammars (code highlighting).

*Three up-front decisions:* (1) **code highlighting** — MVP ships unhighlighted
`language-x` classes; grammars are a fast-follow; (2) **math** — prefer MathML (no
JS) or defer; (3) **coverage** — degrade gracefully on unsupported Typst functions;
the `technical` template steers authors to the HTML-safe subset (`typst_to_markdown`
coverage is the fidelity ceiling — extend as gaps surface).

*Phasing:* **P1** multi-page site + nav + admonitions + images + plain code (a
shippable release); **P2** client-side search; **P3** profiles/variables + theme +
highlighting + math.

## TDOC‑5 · Workflow — the staleness dashboard

A **review/currency view** keyed on a release marker (a tag or a named snapshot
point): "pages whose content changed since `v2.0`" and "pages not *reviewed* since
`v2.0`." Leverages the status marks, tags, and snapshot history already in place, so
a writer preparing a release sees exactly which pages need a re-read against it.

- **Effort/risk**: low–medium. Pure bookkeeping over existing data; a panel +
  a `docs review --since <marker>` CLI.

## TDOC‑6 · Reference generation from a spec  *(optional, larger — hold or scope narrowly)*

The endgame of anti-staleness: import an **OpenAPI / JSON-schema** and
generate/refresh reference pages (an endpoint or type per node), re-runnable so the
reference *tracks the spec* — the docs regenerate when the source of truth changes.

- **Effort/risk**: high; needs a spec parser (a new crate) and a merge strategy that
  preserves hand-authored prose across regenerations. Recommend holding until
  TDOC‑1..5 land, or scoping to a single narrow spec format.

---

## Recommended order & shape

1. **TDOC‑1 (verify code)** — flagship, most on-brand, lowest risk, no new crate.
2. **TDOC‑2 (links)** — completes the fidelity story; reuses deadlinks.
3. **TDOC‑3 (single-sourcing)** — high leverage, low effort.
4. **TDOC‑4 (HTML site)** — the production release; the largest, most visible win.
5. **TDOC‑5 (review dashboard)** — workflow polish over existing data.
6. **TDOC‑6 (spec-gen)** — optional endgame; hold unless there's clear demand.

TDOC‑1 through TDOC‑3 form a coherent "**docs that answer to the system**" arc that
could ship across three point releases; TDOC‑4 is a headline production release on
its own. None requires an AI call except where the author opts into the existing
audience personas, and none touches the fiction tracks.
