# TDOC-4 — The HTML site exporter

| | |
|---|---|
| **RFC** | TDOC-4 |
| **Title** | Own HTML static-site exporter — multilingual, templated, companion-aware |
| **Status** | Proposed — a sub-program across several 1.6.x point releases |
| **Author** | Vladimir Ulogov |
| **New dependency** | at most one (`pulldown-cmark` for md→HTML) — evaluate vs. hand-roll |
| **Program** | TDOC (see `TDOC_ROADMAP.md`) |

## The idea

Technical (and every other) docs live as **websites**. Inkhaven has no HTML output.
This builds our own static-site exporter — *not* Typst's HTML target (document-
oriented and unstable). It is a third consumer of the node tree, template-driven,
localised to the book's language, and able to fold in the companion books (Sources,
Language, World, People/Places) as first-class sections of the published site.

Because it is large, TDOC-4 is a **sub-program** of five bounded point releases
(TDOC-4.1 … 4.5). TDOC-4.1 is the engine; the rest layer capability onto it.

## What already exists (reuse, don't rebuild)

Grounded against the tree:

- **The tree + assembler** — `export::assemble_typst_source_profiled` walks user
  books; `src/export/markdown.rs::typst_to_markdown` converts Typst markup (incl.
  `#image("p", caption:…)` → `![alt](p)`, `src/export/markdown.rs:44`). Assembly
  **deliberately excludes system books** (`src/assemble.rs:67`), so companion books
  are opt-in side inputs — exactly the model we want.
- **Jinja, with HJSON variables already wired** — `build_jinja_environment`
  (`src/assemble.rs:286`) registers `.jinja` snippet templates; `jinja_context_for_node`
  (`src/assemble.rs:340`) already parses **linked HJSON paragraphs** into the context
  (`linked.<slug>`) and exposes `language`/`genre`/`book`/`chapter`
  (`src/assemble.rs:373`). Requirement "HJSON as jinja variables" is largely met.
- **Structural subtypes** — `para:code` / `para:admonition-*` / `para:math` /
  `para:table` (STRUCT-2, `src/tui/app.rs`), so semantic HTML comes from metadata.
- **Multilingual pattern** — `Labels::for_language(&cfg.language)` (en/ru/fr/de/es +
  English default) in `src/inner_grounding.rs:117`; book language is `Config.language`
  (`src/config.rs:157`), Typst lang tag `FontConfig.language` (`src/config.rs:974`).
- **Conlang renderers** — `src/conlang/output.rs` already emits
  `dictionary_markdown` (:71), `grammar_markdown` (:428) from `DictionaryEntry`
  (`src/language_entry.rs:35`) + `MetaOverview` (:143). Reuse for the HTML dictionary.
- **Sources** — `src/sources/mod.rs`: `BibEntry` (:22), `compile_bibtex`/`compile_csl_json`
  (:237/:246), `extract_cite_keys` (:391). Reuse for the bibliography + `@key` links.
- **World** — `world.hjson` → `WorldDefinition` (`src/world/types/world.rs:9`); the
  `world` system book holds **materialised HJSON paragraphs** written by
  `src/world/materialize.rs`. Render those leaves for the world guide.
- **TDOC-3** — profiles (`export --profile`) + `docs.variables` already filter/subst
  at assembly; the HTML exporter inherits both.

## Requirements → design

| # | Requirement | How |
|---|-------------|-----|
| 1 | Multilingual (book language) | `Labels::for_language(&cfg.language)` for all chrome; `<html lang="{FontConfig.language}">`; content is already authored in-language. First-class: en/ru/fr/de/es. |
| 2 | Templateable (jinja) | Site shell + partials are `minijinja` templates rendered per page; reuse the existing env so `{% include "snippets/…" %}` still works. |
| 3 | HJSON → jinja variables | A site vars file (`docs.html.variables_file`, default `html.hjson`) parsed via `serde_hjson` → exposed as `site` in every template context; plus existing linked-HJSON + `docs.variables`. |
| 4 | Images + clean formatting | `typst_to_markdown` → md→HTML; copy image files into `assets/`, rewrite `src`; structural subtypes → semantic HTML (`<aside>`, `<pre><code>`, `<figure>`). |
| 5 | Sources, properly formatted | A bibliography page from `BibEntry`; resolve in-text `@key` (via `extract_cite_keys`) → `<a href="sources.html#key">`. |
| 6 | Reliable TOC | Deterministic from the tree (book→chapter→subchapter→paragraph) + heading anchors scanned from bodies; drives the sidebar nav + a contents page; stable slugs. |
| 7 | Separate visual / functional | Two template namespaces: **`functional/`** (skeleton, nav, search, toc, citation wiring — the machinery) and **`theme/`** (CSS + visual partials — the look). Override each independently. |
| 8 | System-book toggles (People/Places/…) | `docs.html.include.{characters,places,glossary,notes,mythology,…}` booleans; each enabled book renders an appendix section from its paragraphs. |
| 9 | Language book → dictionary/grammar/study | `docs.html.include.language`: reuse `conlang::output::{dictionary_markdown,grammar_markdown}` + a searchable/sortable lexicon table; study companion optional. |
| 10 | World book → world description | `docs.html.include.world`: render the materialised world HJSON paragraphs as a formatted world guide. |

## The template split (the design centrepiece — requirement 7)

Two namespaces, both `minijinja`, both overridable per-file:

```
functional/            # the machinery — authors rarely touch
  page.html            # skeleton: <head>, injects {{ page.content }}, nav, search box
  nav.html             # renders the TOC tree (recursive)
  toc.html             # the per-book contents page
  search.html          # the search widget markup + <script> hook
  bibliography.html    # Sources list layout
  dictionary.html      # lexicon table (sort/filter hooks)
  entry.html           # one companion entry (character/place/world element)
theme/                 # the look — authors customise here
  theme.css            # ALL styling (light/dark, layout, type)
  header.html          # visual header/logo partial
  footer.html
  search.js            # client-side search behaviour (functional wiring lives in search.html)
```

- **Resolution order per file**: embedded default (`include_str!`) → project override
  at `<project>/html/functional/<f>` or `<project>/html/theme/<f>` (or
  `docs.html.template_dir`). A designer drops a new `theme/theme.css` and restyles the
  whole site without touching a line of `functional/`.
- **Context** handed to every template: `site` (HJSON vars), `book` (title/author/
  language), `page` (title/content/breadcrumb/anchors), `nav` (TOC tree), `labels`
  (localised chrome), `toc`, plus `docs.variables`.

## Config — `docs.html`

```hjson
docs: {
  html: {
    site_title: null            // defaults to the user book's title
    theme: "default"            // selects a bundled theme template set
    template_dir: "html/"       // optional project override root (functional/ + theme/)
    variables_file: "html.hjson" // HJSON → the `site` jinja context
    search: true                // build the client-side search index
    include: {                  // companion books to fold into the site
      sources: true
      glossary: true
      characters: false         // People
      places: false
      language: false           // full dictionary / grammar / study
      world: false              // world guide
      mythology: false
      notes: false
    }
    citation_style: "author-year" // author-year | numeric
  }
}
```

Off-menu companions default to `false`; a bare `export html` publishes the manuscript
plus Sources + Glossary.

## Module layout

```
src/export/html/
  mod.rs          # orchestrator: export_html(layout, h, cfg, opts, out_dir)
  render.rs       # typst→md→HTML fragment; structural subtypes → semantic HTML
  markdown_html.rs# md→HTML over the typst_to_markdown subset (or pulldown-cmark)
  toc.rs          # TOC/nav model from the tree + heading scan; stable slugs
  templates.rs    # load functional/ + theme/ (embedded defaults + project overlay)
  assets.rs       # copy images into assets/, rewrite paths
  search.rs       # build search-index.json
  labels.rs       # Labels::for_language — the chrome strings (en/ru/fr/de/es)
  sources.rs      # bibliography page + @key resolver (reuse crate::sources)
  conlang.rs      # dictionary/grammar/study (reuse crate::conlang::output)
  world.rs        # world guide from materialised world HJSON
  companions.rs   # generic system-book sections (People/Places/Glossary/Notes/Myth)
```

`ExportFormat::Html` is added and **special-cased** in `src/cli/export.rs` (like
`Pdf`) to require `--output <dir>` and dispatch to `export_html` rather than the
single-file artefact writer.

## Phased implementation (sub-program)

### TDOC-4.1 — The site skeleton  ✅ BUILT (1.6.10-dev)  *(the engine)*

Shipped: `src/export/html/` (mod/render/markdown_html/toc/templates/labels/assets),
`ExportFormat::Html` special-cased in `src/cli/export.rs` (dir output), config
`docs.html`, and the default template set at **`examples/html_templates/`** —
nicely designed after the *Building the World* palette, doubling as the binary's
embedded defaults (`include_str!`, one source of truth) so `export html` needs no
setup and the samples never drift. Delivered: multi-page site + sidebar TOC, prose
via `typst_to_markdown` → hand-rolled `markdown_to_html`, structural subtypes
(code/admonition/math/procedure), images copied + rewritten, the `functional/`↔`theme/`
override split, `site` HJSON vars, multilingual chrome (en/ru/fr/de/es) + `<html lang>`,
TDOC-3 profiles + variables. Verified end-to-end (en + fr, site vars, overrides);
zero attribution in generated output or samples. The spec below is what was built.


- `ExportFormat::Html` + `export html -o <dir>` (dir output, clear errors).
- Tree → multi-page site (page per chapter), sidebar **nav + TOC** (`toc.rs`), stable
  slugs/anchors.
- Per-node render: `typst_to_markdown` → md→HTML (`markdown_html.rs`); structural
  subtypes → `<aside>` / `<pre><code class=language-x>` / `<figure>` / `<table>`;
  jinja paragraphs via the existing env; **TDOC-3 profiles + variables** applied.
- **Images** — copy into `assets/`, rewrite paths (`assets.rs`).
- **Template split** — `functional/` + `theme/`, embedded defaults + project override
  (`templates.rs`), `site` HJSON vars (`variables_file`).
- **Multilingual** — `labels.rs` (en/ru/fr/de/es), `<html lang>`.
- Config `docs.html` (site_title/theme/template_dir/variables_file).
- *Ships a complete, styled, navigable manuscript site.*

### TDOC-4.2 — Search & Sources
- Client-side **search** — build `search-index.json` (page/heading/text) + a few KB of
  vanilla `theme/search.js`; `docs.html.search`.
- **Sources** — `bibliography.html` from `BibEntry`; resolve in-text `@key`
  (`extract_cite_keys`) → links; `citation_style` (author-year | numeric).

### TDOC-4.3 — The companion shelf  ✅ BUILT (1.6.10-dev)

Shipped `src/export/html/companions.rs`: `docs.html.include.{sources,glossary,characters,places,language,world,mythology,notes}` now wire real appendix pages into the *same* site (nav integrated, not a separate shelf). Sources → a formatted author-sorted bibliography (`crate::sources::BibEntry`); every other book → generic rendering (prose via `render_body`; HJSON entries → readable `<dl>` field lists). Empty/disabled books add nothing. Defaults: sources+glossary on (book back-matter), rest off (private workshop). Language/World render generically for now; their *rich* dedicated layouts (sortable lexicon, world guide) remain 4.4/4.5. Documented in `Book/WEBSITE` ch06. (Original plan for this phase below.)


- `docs.html.include.*` toggles; generic renderer (`companions.rs`) for
  Characters/Places/Glossary/Notes/Mythology — walk the book's paragraphs (prose or
  HJSON) → sectioned HTML, linked from the nav.

### TDOC-4.4 / 4.5 — The living language & the world guide  ✅ BUILT (1.6.10-dev)

`include.language` now renders a *formatted dictionary* per invented language,
reusing `conlang::output::dictionary_markdown` (entries read from disk →
`RenderEntry` → markdown → `markdown_to_html`, which gained hard-break support);
grammar/phonology/sample chapters render as prose. `include.world` (and Characters /
Places) render richly via an upgraded `value_to_html`: a list of objects becomes a
series of named *cards*, a list of scalars an inline comma list. Verified E2E
(a two-word Avesha dictionary rendered with bold headwords + glosses). Then upgraded: `include.language` now renders a **sortable/filterable lexicon table**
(inline vanilla JS, self-contained — click a header to sort, type to filter);
`include.world` renders a **bespoke `WorldDefinition`→narrative-prose guide**
(`src/export/html/world_html.rs`, loaded from `world.hjson`): the sky, the land, the
waters, the peoples, livelihood, magic, history — in sentences, degrading gracefully
on blank fields, falling back to the materialised paragraphs when no `world.hjson`.
Both verified E2E. (Original plan below.)


- `include.language`: reuse `conlang::output::dictionary_markdown` / `grammar_markdown`
  → HTML; a **sortable/filterable lexicon table** (headword/POS/gloss/etymology from
  `DictionaryEntry`), grammar pages, optional study companion. Its own sub-nav.

### TDOC-4.5 — The world guide
- `include.world`: render the materialised world HJSON paragraphs
  (`src/world/materialize.rs` leaves: astronomy/geology/hydrology/history/magic/…) as a
  formatted **world guide** section, each element a styled entry (`entry.html`).

## Decisions to make up front

1. **md→HTML: `pulldown-cmark` vs hand-roll.** We control the intermediate markdown
   (from our own converter), so a hand-rolled renderer over that known subset avoids a
   crate; `pulldown-cmark` (small, pure-Rust) is the robust alternative and the only
   candidate new dependency. Recommend hand-roll for 4.1, revisit if formatting gaps
   appear.
2. **`@key` citation styling** — Typst resolves citations natively in the PDF path;
   HTML needs its own. Start with two simple styles (author-year, numeric); a full CSL
   engine is out of scope.
3. **Math** — MathML (no JS) first, or defer with a graceful note; KaTeX only if
   demanded (must be inlined — CSP/self-contained).
4. **Theme count** — ship one clean default theme; the split makes more themes cheap
   later.
5. **Coverage / graceful degradation** — unsupported Typst functions degrade to a
   visible marker, never a crash; `typst_to_markdown` coverage is the fidelity ceiling.

## Non-goals

- Server-side anything, live reload, or a JS framework — it is a **static, self-
  contained** site (no CDN; opens from disk).
- A full CSL citation engine, RTL theming polish, or a search index with stemming
  (substring search first).
- Editing the site back into the store (export is one-way).
