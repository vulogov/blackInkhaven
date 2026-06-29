# CHAR-1 — Character Arc Tracking and Completeness

| | |
|---|---|
| **RFC** | CHAR-1 |
| **Title** | Chapter-ordered character state chains; deterministic agency score; declared-arc completeness checks; Planning-Board linkage; CLI + Bund |
| **Status** | In progress — 1.4.16 |
| **Author** | Vladimir Ulogov |
| **Depends on** | NARR-1 (`ProseLanguage`, `detect_passive`, `prose_profiles.interiority_ratio`), DIALOG-1 (`dialogue_spans`, `character_dialogue_fingerprints`), the LLM engage + cost + bg infra |
| **New dependency** | none |

A character-centric arc view: per character, the chapter-ordered observable
state (LLM-extracted), a deterministic agency score, declared-vs-observed arc
completeness checks, stall detection, and Planning-Board scene linkage.
Read-only / advisory; caps inform, never block.

## Audit corrections (RFC vs reality — verified)

- **Target 1.4.15 → 1.4.16** (1.4.15 shipped). Baseline "2,174" → **2069**.
  Dep versions are off by one: DIALOG-1 shipped 1.4.14, WORLD-6 1.4.15.
- **No continuity "character mention index"** — fabricated. Character-relevant
  paragraphs are found by scanning paragraph text against the Characters-book
  roster (the `character_names` / `facts_entities` pattern from DIALOG-1 /
  WORLD-6), not a pre-built index.
- **NARR-1 store is `prose.duckdb` / `prose_profiles`**, not
  `voice.duckdb`/`voice_profiles`. Interiority read = `prose_profiles
  .interiority_ratio` (per chapter scope). The **DIALOG-1 reads are correct** —
  `dialogue_spans.attribution_name`, `character_dialogue_fingerprints
  .hedge_density` are the real names.
- **`Ctrl+V A` is taken** (`a`=ViewAddLink, `Shift+a`=AiThreadAudit). New chord:
  **`Ctrl+V Shift+N`** (N = character Narrative arc) — a free view-layer slot.
- **The story bible has no tabs** (flat `BibleRow` list). The "Arc tab" becomes
  a dedicated `Modal::CharacterArc` (like the DialogueFingerprint modal).
- **No "deep-refresh category selector."** The deterministic agency scores fold
  into the `Ctrl+B Shift+C` review pass (like dialogue/utopia); state extraction
  + arc checks stay **CLI-explicit** (the RFC agrees they're opt-in).
- **Hashing/schema:** `DefaultHasher` (not xxHash); **TEXT/INTEGER** (not
  `TIMESTAMPTZ`/`UBIGINT`/`BOOLEAN`).
- **No `inkhaven character` command** — a new top-level `Character(CharacterCommand)`
  group (mirrors `dialogue`/`prose`).
- **NARR-1 passive** (`prose::passive::detect_passive`) exposed via a re-export
  seam, like the `modal_unigrams`/`mattr` accessors DIALOG-1 added.
- **Planning Board** `Scene` struct (`planning.rs`) gains a `characters` field
  on the HJSON scene cards; `arc_function` is display-only.
- **Arc taxonomy** = 5 values (positive_change / flat / corruption / fall /
  disillusionment) + open-string fallback to a generic probe.

## Phases

| Phase | Content |
|---|---|
| C-P0 | `src/character/` scaffold: ArcType (5 + open), Claim/state/check types, action-verb lists (5 langs) |
| C-P1 | `char.duckdb` + `CharStore` (5 tables, house pattern) |
| C-P2 | `character_arc` HJSON block reader (Characters book) + Planning `Scene.characters` |
| C-P3 | Deterministic agency score (active/passive heuristic; NARR-1 `detect_passive` seam) |
| C-P4 | Character-relevant-paragraph scan + state extraction LLM pipeline (sliding window, incremental, DIALOG-1/NARR-1 enrichment) |
| C-P5 | Stall detection (deterministic) + arc-completeness checks (LLM, arc-type-specific) |
| C-P6 | Planning-Board gap detection (deterministic) |
| C-P7 | CLI `inkhaven character arc <name> \| check \| refresh \| plan` |
| C-P8 | Output `char` category in the review pass + suppression |
| C-P9 | Story-bible Arc modal + `Ctrl+V Shift+N` |
| C-P10 | Bund `ink.char.*` (5 words) + `char:` config + docs |

LLM stages are integration-only; the deterministic surface (verb lists, agency,
stall, planning gaps, store, prompt construction, JSON parsing, enrichment
joins) is unit-tested.

## Future hooks (schema-reserved; no code)

`emotion_label` (CHAR-2) · `chapter_ord` join for DIALOG-2 voice consistency ·
`book_slug` PK (SERIES-1) · `ink.char.*` readable by a future Inner Theologian.

## Target

+72 tests (2069 → ~2141). No new runtime crates; no new system books. One new
`.inkhaven/char.duckdb`; one `character_arc` HJSON block; one `char:` config
block; one new chord; one new top-level CLI group.
