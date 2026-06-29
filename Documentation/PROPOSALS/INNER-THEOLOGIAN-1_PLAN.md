# INNER-THEOLOGIAN-1 — Inner Theologian

| | |
|---|---|
| **RFC** | INNER-THEOLOGIAN-1 |
| **Title** | Tradition-neutral comparativist reader: 11 moral/theological lenses, fast-track ethical signals + slow-track interrogation corpus |
| **Status** | In progress — 1.4.18 |
| **Author** | Vladimir Ulogov |
| **New dependency** | none |

The third Inner-family reader after Inner Socrates (logical structure) and Inner Editor (craft):
**moral and theological seriousness**. A tradition-neutral comparativist that reads any manuscript
through 11 tradition lenses — never to judge by any, but to ask what each *sees*. Belongs to no
tradition, advocates none, never exceeds `info` severity. Like every advisory feature, it **never
edits prose**.

## Audit corrections (the RFC was written against a fabricated surface)

- **Target "unversioned" → 1.4.18.** 1.4.17 (HAIKU-1) shipped.
- **`inner_socrates.duckdb` + `sessions` table + `kind` discriminator is FICTION.** The real store is
  `inner_socrates.db` with `socratic_findings` / `intent_entries` / `active_persona` /
  `inner_socrates_llm_usage` — **no `sessions` table, no session-kind column** (the `kind` column is on
  `intent_entries`, the ledger). Inner Editor does **not** store `kind='editor'` rows there — it has
  its **own** `inner_editor` store. Each Inner-family member owns a separate DuckDB store.
  → **Decision: own `inner_theologian.db`** (mirrors inner_socrates/inner_editor/char), with a
  `theologian_findings` table modelled on `socratic_findings`. No migration, clean isolation.
- **"Conversation mode / session resume" has no backing.** Inner Socrates "conversation" =
  `socratic_open_conversation` seeding the AI chat pane from findings; there is nothing persisted to
  resume. → Inner Theologian conversation = seed the AI pane the same way. `theologian resume` and a
  persisted session log are **out of scope** (would have to be built from scratch for Socrates too).
- **No shared "paragraph-idle modal" with `◇`/`✦` participants.** Inner Socrates Fast-track and Inner
  Editor emit **Output-pane findings**, not a combined modal; `SOCRATIC_INQUIRY` has no kind-glyph and
  `◇` only appears in a status string. → Inner Theologian fast-track signals are **Output-pane
  findings** in a new `theologian` category. Auto-fire on idle = an Output finding / optional LLM
  question, not a modal participant.
- **Provenance glyph `✦` is TAKEN** — assigned to `kinds::HAIKU` (1.4.17). → Use **`⚖`** (U+2696,
  scales): tradition-neutral, fits "moral weight." (`✎` = Editor, `⊘`/`⧉` = timeline.)
- **`Ctrl+B J` is the Socrates family** (overview subkeys `S`/`L`/`F` + `E` engage); `Ctrl+V O` is the
  separate Editor family. Adding a `J→T` subkey to the Socrates overview handler is feasible.
- **What's REAL (grounding):** WORLD-6 `FindingDomain::Theological` is explicitly reserved "for a
  future Inner Theologian (never written here)" — Source 1 legit. CHAR-1 `character_arc_declarations`
  in `char.duckdb` — Source 2 legit. Both degrade gracefully when absent.

## Phases

| Phase | Content |
|---|---|
| IT-P0 | Module scaffold `src/inner_theologian/` + types (SignalType, TheologianFinding, TraditionLens enum ×11, QuestionCategory 1–6); `mod` gate |
| IT-P1 | Per-language vocab lists (violence / consequence / sacred), 5 langs, sorted `&[&str]` binary-search — pure data |
| IT-P2 | `inner_theologian.db` store (`theologian_findings`, mirrors socratic_findings) — open/upsert/by-chapter/suppress/clear |
| IT-P3 | Fast-track detector: 3 signals (moral-invisibility, consequence-gap, sacred-levity), window logic, lang dispatch; Output `theologian` category (⚖) |
| IT-P4 | Slow-track corpus: 6 categories × 5 langs as localised constants (tradition terms glossed inline) |
| IT-P5 | Tradition-lens selection + LLM prompt construction (lens-marker scan, explicit lens labelling) + `theologian_llm_call` |
| IT-P6 | Grounding pipeline: WORLD-6 theological findings + CHAR-1 belief-arc + in-scope fast-track signals, Category-6 fallback; feature-availability guards |
| IT-P7 | `Ctrl+B J→T` subkey (Socrates overview handler) → slow-track session at current F9 scope; AI-pane seed |
| IT-P8 | Output integration + intent-ledger suppression (reuse inner_socrates intent store) + review-pass fold |
| IT-P9 | CLI `inkhaven theologian scan|session|suppress` (exit codes) |
| IT-P10 | Bund `ink.theologian.{signals,suppress}` + policy |
| IT-P11 | `theologian:` config block + thread config + remove dead-code gate + docs (KEYBINDING/CONFIGURATION/tutorial 101) |

## Out of scope (RFC features dropped or deferred)

- Persisted conversation sessions + `theologian resume` (no backing exists; Socrates doesn't either).
- Writing into WORLD-6's `utopia.duckdb` theological domain (read-only grounding only).
- Per-tradition deep-dive sessions, author-tradition declaration, MYTH-1 integration (future).
