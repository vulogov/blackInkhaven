# NF-CITE — The Sourcing pass

| | |
|---|---|
| **RFC** | NF-CITE |
| **Title** | Uncited-claim detection — "every claim answers to a source" |
| **Status** | Proposed — targets a 1.6.x point release |
| **Author** | Vladimir Ulogov |
| **New dependency** | none |
| **Audience** | nonfiction authors |

## The idea

Fiction's fact-checker measures prose against the world; the technical track's `docs
verify` measures examples against the system. Nonfiction's discipline is that **every
checkable claim carries a source**, and nothing in Inkhaven checks it. `inkhaven
sources check` only verifies that the keys you *did* cite are defined — it says nothing
about the assertions you cited *nothing* for.

NF-CITE closes that gap. It scans the manuscript for sentences that make a checkable
factual claim — a statistic, a date, a quotation, an attributed finding — and carry
**no `@key` citation**, and reports each one, so the author can source it (or
deliberately mark it common knowledge) before publishing.

## Grounded current-state (reuse, don't rebuild)

- **`@key` extraction** — `crate::sources::extract_cite_keys(prose) -> Vec<String>`
  (`src/sources/mod.rs:391`), Typst `@key` tokens, email/unicode-safe.
- **Sentence splitting** — `crate::continuity::split_sentences(text) -> Vec<String>`
  (`src/continuity.rs:295`), `.!?` boundaries with abbreviation suppression.
- **The `sources check` pattern** — `src/cli/sources.rs::check`: open project, resolve
  user books, walk `NodeKind::Paragraph` under each, read body, report + JSON + CI exit.
  NF-CITE mirrors it exactly.
- **Sources / Facts** already exist as the corpus a claim *should* answer to.

## Design

### What counts as a claim needing a citation

High-precision signals only — a noisy checker gets ignored. A sentence is flagged when
it carries **no `@key`** *and* matches at least one signal:

- **Statistic** — a percentage (`57%`), a grouped number (`1,200`, `3.4 million`),
  `N percent`, a currency amount. (Bare small integers like "three steps" are ignored.)
- **Date** — a 4-digit year (1000–2099), optionally with a month.
- **Quotation** — a `"…"` / `“…”` span of ≥ 6 words (a quote wants attribution).
- **Attribution** — "according to", "a/the study", "research(ers) (found|show|suggest)",
  "data (show|indicate)", "survey", "estimated", "statistics", "reported that",
  "scientists/experts (say|found)". These announce a sourced claim.

### What suppresses a flag

- The sentence already contains an `@key` (it's sourced).
- The paragraph carries the tag **`no-cite`** — the author's "this section is common
  knowledge / deliberately unsourced" override. (A lightweight stand-in for the intent
  ledger; ledger integration is a later refinement.)

### Surfacing

`inkhaven sources coverage [--book-name <name>] [--json]` — mirrors `sources check`:

```
sources coverage — The Long Peace

  ch03 · The Numbers
    "Deaths fell by 42% between 1946 and 1991."   [statistic, date]
    "According to Pinker, violence has declined."  [attribution]

sources coverage: 2 uncited claim(s) across 1 book
```

Exits non-zero when any uncited claim is found, so it drops into the same pre-release /
CI gate as `sources check`, `docs verify`, and `terms check`. `--json` for tooling.

## Phases

- **P0** — `src/sources/coverage.rs`: the pure detector. `UncitedClaim { sentence,
  signals: Vec<&str> }`; `scan(body) -> Vec<UncitedClaim>` (split sentences, skip those
  with `@key`, match the signal set). Fully unit-tested (each signal, the `@key` skip,
  the small-integer non-trigger).
- **P1** — CLI `SourcesCommand::Coverage { book_name, json }` + dispatch; walk manuscript
  paragraphs, honour the `no-cite` tag, report + `--json` + non-zero exit.
- **P2** — Docs: the *Developing a story with Inkhaven* nonfiction chapter gains a short
  "Source your claims" procedure.

## Extensions built (2026-07-09/10)

- **AI track** — `sources coverage --ai [--provider]`: per chapter, RAG-retrieves the
  relevant Facts (`facts_scan::relevant_facts`), asks the model (via
  `facts_scan::run_blocking`) to list uncited checkable claims *and* judge each against
  the retrieved Facts, then reports them split into "unsupported — research or cite" vs
  "backed by your Facts — add the citation." Combines the AI-claim-finder and the
  Facts-support check in one pass. Degrades to claim-finding when no Facts book exists.
- **TUI chord** — `Ctrl+V Shift+C` (`SourcingCheckParagraph`), beside the `Ctrl+V @`
  cite picker: the deterministic pass on the open paragraph → Output-pane findings
  (`kinds::SOURCING`, `sourcing` source group, `❝` glyph). Respects `no-cite`.

## Non-goals (this RFC)

- Checking a claim against external truth (only the Facts-book *support* is checked).
- In-text `@key` → bibliography *rendering* (separate).
- Full intent-ledger integration (the `no-cite` tag remains the override).

## Why this is the right first nonfiction feature

It reuses everything already present (Sources, `@key`, the sentence splitter, the
`sources check` shape), it is deterministic and zero-AI, it fits Inkhaven's checker /
CI-gate pattern, and it addresses the single deepest nonfiction obligation. It is the
nonfiction sibling of `docs verify` and the fiction fact-check: _your claims answer to
your sources._
