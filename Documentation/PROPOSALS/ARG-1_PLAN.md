# ARG-1 — The argument outline (first slice)

| | |
|---|---|
| **RFC** | ARG-1 |
| **Title** | A read-only claim → support outline, with two structural gaps |
| **Status** | Proposed — the first, deliberately modest slice of the argument-map idea |
| **New dependency** | none |
| **Audience** | nonfiction authors |

## The idea

The full argument-map — claims, support, objections, rebuttals, and the links between
them, as a navigable graph — is a large, AI-heavy, UX-heavy build. This RFC is its
**smallest honest first step**: an AI pass that, for a chapter, produces a **claim →
support outline** and flags the **two cheapest structural gaps**, surfaced as findings.
No graph, no objection tracking, no cross-chapter reasoning. The goal is to prove the
extraction is *reliable* before investing in the map view.

It is the nonfiction cousin of the fiction story-graph (`Ctrl+V W`), but this slice
delivers a list, not a graph.

## The two gaps (only these, for now)

1. **Unsupported central claim** — a load-bearing assertion in the chapter that the
   text backs with no evidence, reasoning, or citation.
2. **Orphan citation** — a `@key` cited in the chapter that supports no identified
   claim (a citation that reads as decorative).

Both are cheap because they need only the chapter's own prose and its own `@keys` — no
external truth, no cross-chapter graph.

## Grounded reuse (nothing new to invent)

Exactly the pipeline NF-CITE's `--ai` track already uses:

- `AiClient::from_config` / `resolve_provider` / `facts_scan::run_blocking` (the
  blocking LLM call).
- `book_walk::chapter_raw_prose` + `audiobook::typst_to_plain` — the chapter text.
- `sources::extract_cite_keys(prose)` — the `@keys` cited in the chapter.
- `sources::BibEntry` (resolved from the Sources book) — to describe each cited key to
  the model so it can judge orphans.
- The Output-pane finding machinery (`kinds`, `emit`, `with_source_paragraph`) —
  already used by NF-CITE / fact-check / docs verify.

## Design

### Command

`inkhaven argue [--book-name <name>] [--provider <p>] [--json]`. Per chapter it:

1. Builds the chapter's plain prose and the list of `@keys` it cites, each paired with
   its Sources description (`Author (Year) — Title`).
2. Asks the model to extract the argument. **The model must quote claim text from the
   chapter** (no inventing), and mark each claim's support as a cited `@key`, a
   reasoning phrase found in the text, or `NONE`.
3. Parses a parse-stable, line-oriented format:

```
CLAIM ||| <claim quoted from the chapter> ||| <@key / reasoning / NONE>
ORPHAN ||| <@key> ||| <one line: cited but attached to no claim>
```

### Output

- **The outline** — for each chapter, its claims and their support (printed, or `--json`
  for tooling):

  ```
  argue — chapter "The Numbers"
    • Deaths from war fell across the period.        ← @pinker2011
    • The decline is not an artefact of population.  ← reasoning
    • Peace is self-reinforcing.                     ← (no support)   ⚠ unsupported
    orphan citation: @keegan1993 supports no claim
  ```

- **The two gaps as findings** — each unsupported claim and each orphan citation becomes
  an Output-pane finding (`kinds::ARGUMENT`, a `argument` source group), anchored to the
  chapter, so a later TUI chord / background pass can surface them in place. The CLI also
  prints them and exits non-zero when any gap is found (a pre-review gate).

  **Chord (when added):** `Ctrl+V Shift+A` (A = Argument) — a free slot in the Ctrl+V
  family, sitting with the other nonfiction chords: `Ctrl+V @` (cite picker) and
  `Ctrl+V Shift+C` (the Sourcing pass). All nonfiction reader/check chords live under
  `Ctrl+V`.

### Keeping it honest (the reliability bar this slice must clear)

- **No claim without a quote.** The prompt forbids paraphrase — a `CLAIM` line that does
  not appear (fuzzily) in the chapter is dropped at parse time. This is the guard against
  the model hallucinating an argument the author never made.
- **Conservative on "central."** The prompt asks only for *load-bearing* claims (a
  handful per chapter), not every sentence — noise kills a checker.
- **Orphans judged only against cited keys**, which are extracted deterministically, so
  the model can't invent a citation.
- Ships behind `--ai` semantics (opt-in, costs tokens); zero-AI paths are unaffected.

## Phases

- **P0** — `kinds::ARGUMENT` + an `argument` Output-pane source group + glyph.
- **P1** — `src/argue.rs` (pure): the finding model (`ArgClaim { claim, support,
  supported }`, `OrphanCite`), the parser (`parse_argument(raw) -> (Vec<ArgClaim>,
  Vec<OrphanCite>)`), and the quote-verification drop. Tested.
- **P2** — `src/cli/argue.rs`: per-chapter prose + cited-source context, the prompt, the
  LLM call, the outline render (text/JSON), the gap findings + non-zero exit.
- **P3** — Docs: a short note in the nonfiction chapter.

## Non-goals (this slice)

- The **graph / map UI** — this is a list. (The map view is the eventual ARG-2+.)
- **Objections & rebuttals**, and their tracking.
- **Cross-chapter** argument structure (a claim supported in a later chapter).
- Any **auto-fix** — Inkhaven flags; the author reasons.
- A deterministic tier — argument extraction is inherently semantic, so this slice is
  AI-only (unlike NF-CITE, which has a deterministic fast track).

## Why this is the right first step

It reuses the entire NF-CITE `--ai` pipeline, adds only a prompt + a parser + two gap
rules, and produces something an author can act on today — while the quote-verification
guard forces the model to stay tied to the text. If it proves reliable, the map view
(and objections, and cross-chapter links) become worth building on top; if it does not,
we've spent a prompt and a parser, not a graph engine.
