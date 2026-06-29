# WORLD-6 — Utopian/Dystopian Coherence Checker

| | |
|---|---|
| **RFC** | WORLD-6 |
| **Title** | Three-stage coherence checker for declared social/systemic premises; `para:utopia-*` tags; `utopian-architect` grounding; CLI + Bund |
| **Status** | In progress — 1.4.15 |
| **Author** | Vladimir Ulogov |
| **Depends on** | STRUCT-2 (`para:*` tags), WORLD-4 (magic ledger), the LLM engage + cost + bg-job infra |
| **New dependency** | none |

WORLD-6 checks the *logical coherence* of a declared social/systemic premise —
premise → mechanism → consequence, plus the elimination inventory — that
WORLD-4/5 (physical/temporal facts) don't. Three LLM stages read author-declared
`para:utopia-*` paragraphs from the World system book, check the chain, and scan
the manuscript for entailment violations. Findings ground the existing
`utopian-architect` Inner Socrates persona. **Read-only / advisory; caps inform,
never block.**

## Audit corrections (RFC vs reality — verified against the tree)

- **Target 1.4.14 → 1.4.15.** 1.4.14 (DIALOG-1) already shipped. Test baseline
  "2,107" is wrong; real = **2039**, so +67 → ~2106.
- **CLI: `inkhaven world` is a flat command** (`World { json, deep, provider,
  entity }`), not a subcommand group. Per user decision, restructure into a
  `WorldCommand` subcommand enum with the **current overview as the default
  (no-subcommand) path** so `inkhaven world` stays backward-compatible, plus
  `utopia-check / utopia-model / utopia-suppress / utopia-refresh`.
- **No "deep-refresh scheduler with a category selector."** `start_deep_refresh`
  runs a fixed scan set (`deep_refresh_shared`); the Output pane has a *filter*,
  not a per-category recompute trigger. Stage 1/3 hook into `deep_refresh_shared`
  or a dedicated `BgJobKind::UtopiaCheck` + idle pass; **Stage 2 is explicit-only**
  (CLI / opt-in), never silent — as the RFC itself specifies.
- **Hashing: `std::DefaultHasher` u64, not xxHash** (no xxhash dep in-tree).
  Schema follows the house pattern: **TEXT** for reals/hashes/timestamps,
  **INTEGER** for counts/bools (0/1) — not `TIMESTAMPTZ`/`UBIGINT`/`BOOLEAN`.
- **`voice` → `prose`.** §10.1/§13 repeat the DIALOG-1-RFC error; verified
  categories are `prose`/`dialogue`/`continuity`/… — no `voice` / `voice.duckdb`.
- **Inner Socrates grounding is new code**, not an existing injection slot: the
  slow track builds a system prompt via `slow_system(genre)`; §8's grounded
  opening gathers findings and prepends them to the slow session.
- **`para:utopia-*` glyphs need a parallel registry.** `STRUCTURAL_TYPES` is the
  *seeded-boilerplate* picker; utopia tags are declarations with no seed and
  `structural_glyph` wouldn't know them — a small `UTOPIA_TYPES` table + glyph
  lookup is added. Tags are freeform so `para:utopia-*` is valid; World-book-only
  scope is enforced by the indexer, not the tag parser.
- **Magic ledger** has `kind: String` + `covers: Vec<String>` (freeform), so
  `kind:"deliberate_tension", covers:["utopia_coherence"]` works; `applies(ctx)`
  keys on roles/regions/seasons that utopia findings lack, so suppression uses a
  wildcard-applicable lookup.

## Phases

| Phase | Content |
|---|---|
| W-P0 | `src/world/utopia/` scaffold: `UTOPIA_TYPES` tag/glyph registry, types (`ClaimType`/`FindingType`/`FindingDomain`/`UtopiaClaim`/`UtopiaFinding`/`ChapterScan`), premise-group detection over the World book |
| W-P1 | `utopia.duckdb` schema + `UtopiaStore` (house pattern, DefaultHasher) |
| W-P2 | Stage 1 extraction — prompt builder + JSON parse + claim upsert + hash cache; Facts cross-reference (deterministic, no LLM) |
| W-P3 | Stage 2 pairing — pair selection + compatibility prompt/parse + finding generation + cost warning + explicit-only guard |
| W-P4 | Stage 3 entailment scan — elimination inventory + per-chapter prompt/parse + bg pass + cache (incl. `research_corpus_version` hook) |
| W-P5 | `utopian-architect` grounding — 3-source resolution → grounded opening |
| W-P6 | CLI — restructure `world` into subcommands + `utopia-check/model/suppress/refresh` |
| W-P7 | Output `utopia` category + magic-ledger / intent-ledger suppression |
| W-P8 | Bund `ink.utopia.{model,findings,violations,suppress}` |
| W-P9 | `utopia:` config block + docs (CONFIGURATION, tutorial 98) |

LLM stages: only the deterministic parts are unit-tested (prompt construction,
JSON parse, upsert, caching, group detection, cross-reference, grounding text,
CLI/Bund); the live LLM call is integration-only, like Inner Editor's engage.

## Future hooks (schema-reserved; no code)

`FindingDomain::Theological` (Inner Theologian) · `grounded_by_research` +
`research_corpus_version` (`inkhaven research`) · `book_slug` PK (SERIES-1).

## Target

+67 tests (2039 → ~2106). No new runtime crates; no new system books. One new
`.inkhaven/utopia.duckdb`; 4 new `para:utopia-*` tag values; `world` CLI
restructured (backward-compatible default).
