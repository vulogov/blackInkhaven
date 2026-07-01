# RESRCH-UNDISPUTED — Authorial ("undisputed") facts (track proposal)

| | |
|---|---|
| **Status** | Proposed (track) |
| **Builds on** | RESRCH-1 (Facts corpus) · R2-E `/factcheck` · RE-P5 verdict glyphs · UX-P2 tier glyphs |
| **Theme** | Some Facts are **the author's creative invention**, not claims about the real world — an invented god's name, a fictional aqueduct's capacity, a magic system's rule. They *will* fail a real-world fact-check (which grades against the model's general knowledge), and they *should not* be checked against it at all. This track lets the author mark a fact **undisputed**: excluded from `/factcheck`, glyphed in the tree, and checked instead for **internal common sense** by a separate, non-destructive `/undisputed` pass. |

## The core idea (the user's spec, verbatim intent)

- A fact is **undisputed** when its Facts paragraph carries the tag **`fact:undisputed`**.
- An undisputed fact is **marked with a glyph** in the Facts tree.
- An undisputed fact is **excluded from `/factcheck`** — but `/factcheck` **reports the count** of undisputed
  facts it skipped.
- A **separate `/undisputed` command** runs a crafted LLM prompt that checks undisputed facts for
  **common sense / internal coherence** (not real-world accuracy) and **never changes them**.
- **Every check respects the project's configured language** (the standing multilingual requirement).

## Grounding (verified)

- **Tags already exist on every node.** `Node { … pub tags: Vec<String> … }` (`src/store/node.rs:179`),
  a free-form `Vec<String>` persisted in **DuckDB metadata** (`update_metadata` → `metadata.db`), *not*
  the `.typ` file. A Facts paragraph is undisputed iff `node.tags.iter().any(|t| t == UNDISPUTED_TAG)` —
  byte-for-byte the `is_structural_paragraph` idiom (`src/tui/app.rs:2416`). `fact:undisputed` has the
  same `prefix:value` shape as the existing `para:*` / `lang:*` tags.
- **The tag is already user-addable today** — open the Facts paragraph in the editor, `Ctrl+B ]` (the
  generic tag picker, `tag_impl.rs:23`) → `A` → type `fact:undisputed` → Enter (persists via
  `set_tags_on_node`). So a manuscript can carry undisputed marks before any of this code lands.
- **`gather_facts` already has what it needs.** `factcheck::gather_facts(store, h, book_id)`
  (`src/research/factcheck.rs`) walks the Facts subtree via `h.get(id)` — so it holds the `&Node` and can
  read `node.tags` to partition disputed vs undisputed with **no new store work**.
- **The Facts tree already renders per-node glyphs.** The research tree draws the `/factcheck` verdict
  glyph (`fact_verdicts`, RE-P5) and the provenance-tier glyph (`fact_provenance`, UX-P2) from per-node
  lookups — an undisputed glyph reads `node.tags` the same way and slots in beside them.
- **`/factcheck` is already chunked + language-aware** (`factcheck::truth_system(language)`,
  `resolve_prose_language`), so `/undisputed` reuses the same chunking + language plumbing with a
  different prompt.

## Design

### 1. The tag + a toggle
- Constant `UNDISPUTED_TAG = "fact:undisputed"` (config-overridable via `research.undisputed_tag`).
- **Toggle in the research Facts tree — the `u` key** adds/removes the tag on the selected fact and
  reloads. The research app has no tag-mutation method yet, so this adds a small one mirroring the
  editor's `App::set_tags_on_node` (`src/tui/app.rs:9965`): clone `hierarchy.get(id)`, push/remove
  `fact:undisputed`, `store.raw().update_metadata(id, node.to_json())`, then `reload_hierarchy`. The
  editor's generic `Ctrl+B ]` path also works, so manuscripts already tagged are honoured.

### 2. The glyph
- A distinct, neutral **authorial** glyph in the Facts tree — proposed **`※`** (a "reference/authorial"
  mark) in a calm colour (e.g. violet/blue), rendered before the verdict/tier glyphs. It reads
  "this is the author's invention; it lives outside the trust ladder." Distinct from the trust-tier
  glyphs (which say *where a real-world fact came from*).

### 3. `/factcheck` exclusion + count
- `gather_facts` gains a partition: **disputed** facts (checked as today) and a **count** of undisputed
  ones. The truth + consistency passes run **only over the disputed set**.
- The `/factcheck` report header gains a line: **`N undisputed fact(s) excluded (authorial)`**, and the
  status tally notes it. Undisputed facts never contribute a `/factcheck` verdict glyph.

### 4. `/undisputed` — the common-sense pass (separate track)
- Gathers **only** the undisputed facts and runs a **chunked** LLM check with a *different* posture from
  `/factcheck`. The prompt (in the **project language**) makes the frame explicit:
  > *"The following are statements from a **work of fiction** — the author's deliberate creative
  > invention. Do **not** check them against real-world knowledge; they are not meant to be real. Judge
  > only **internal common sense**: is each self-consistent, plausible *within its own fictional frame*,
  > and free of obvious contradiction or nonsense? For each, reply `PLAUSIBLE | ODD | INCOHERENT —
  > <reason>`. Never propose rewrites."*
- Reports into the chat like `/factcheck` (read-only; **never edits**). Optionally records a verdict per
  node so the tree can show an `※`-scoped mark (reusing the RE-P5 verdict store, kept separate from the
  real-world verdicts) — a follow-on, not required for the first cut.
- Language: `resolve_prose_language` + `extract::language_name`, exactly as `/factcheck`.

## Phases

| Phase | Content |
|---|---|
| **UD-P1 — Tag + glyph + toggle** | `UNDISPUTED_TAG` const + `research.undisputed_tag`; a Facts-tree key to toggle the tag (store write + reload); an `※` glyph in the research tree read from `node.tags`. |
| **UD-P2 — `/factcheck` exclusion + count** | `gather_facts` partitions disputed/undisputed; truth + consistency run over the disputed set only; the report + status show the excluded count. |
| **UD-P3 — `/undisputed` common-sense pass** | New `/undisputed` command + `CommandSpec` (UX-P1 palette/hints); gather-undisputed; a chunked, language-aware common-sense prompt (`PLAUSIBLE/ODD/INCOHERENT`); read-only chat report. |
| **UD-P4 *(stretch)*** | Persist `/undisputed` verdicts → an `※`-scoped tree mark; a config to auto-exclude on a per-branch basis. |

## Notes & constraints
- **No new store schema, no new crates.** Tags are an existing persisted `Vec<String>`; the checks reuse
  the `/factcheck` LLM plumbing.
- **Read-only + advisory** — consistent with the project's hard rule: `/undisputed` reports, never
  rewrites; it writes no prose. (The only write is the user-driven tag toggle.)
- **Language-respecting** everywhere (the standing multilingual requirement) — the common-sense prompt is
  built in the project language and asks for reasons in it.
- **Trust-ladder relationship:** undisputed facts sit **outside** the ladder — they're authorial axioms.
  The `※` glyph is deliberately *not* a tier glyph; it says "not a real-world claim." A `/fact` is never
  auto-marked undisputed — it's always a deliberate author act.

## Recommended first cut
**UD-P1 + UD-P2 + UD-P3** together — the whole loop the user described (mark → excluded-with-count →
separate common-sense check). Small and self-contained; all three reuse existing tag + `/factcheck` +
UX-P1 machinery with no new dependencies.
