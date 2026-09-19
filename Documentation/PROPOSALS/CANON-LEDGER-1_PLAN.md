# Inkhaven CANON-LEDGER-1 — the story's development history

Status: **proposal**, written 2026-09-19. Depends on **smysl 1.7.0** (published on crates.io) — see
[`SMYSL-1_RFC.md`](SMYSL-1_RFC.md) for the substrate and its verification. Greenlit for *planning*;
the build is still gated on the ~1-user test in §9.

## 0. Why

inkhaven understands the **text** (git) and the **current canon** (Facts / SEMNET / KEN / SENTINEL),
but not the **structure of decisions** behind the fiction: what was decided, revised, retracted, and
what each decision rests on. CHRONICLE records *findings* per milestone — the symptom, not the
decision. There is no ledger of the canon's own development. This plan builds one, on smysl.

The one capability that pays for the whole feature: **"what breaks if I cut this?"** — dependency-aware
revision impact, which inkhaven has nothing like today.

## 1. What it is

A **Canon Ledger**: a per-project, append-only, content-addressed record of the story's load-bearing
decisions — world-facts, character traits, plot commitments, reveals — each with:

- **grounds** — what it depends on ("this reveal needs that setup"),
- **commitment** — how settled it is (`Floated → Drafted → Committed → Canonical → Retconned`),
- **provenance** — `human:` (the author) vs `model:` (a reader), with a logical clock,
- a **`SourceKind::Node`** back-reference to the inkhaven paragraph it came from (`inkhaven:<uuid>#<breadcrumb>`).

It is **not** a second Facts store. Facts/SEMNET are current-state; the ledger is *history + dependency
+ supersession*. It sits beside CHRONICLE (finding-history) and can share CHRONICLE's milestone boundary
for its own canon-`diff`.

## 2. Architecture

### 2.1 Dependency
`smysl = { version = "1.7", default-features = false, features = ["render-typst"] }`. Pure profile:
`blake3`(pure) + `unicode-normalization` + `bm25` + `fxhash` — no async/HTTP/C++/arg-parser, warning-free,
publishable, MPL-2.0 (a dependency, does not reach inkhaven's Apache/MIT source). **Never** enable
`semantic` (model2vec/C++) — inkhaven brings its own embeddings (§5).

### 2.2 Owned wrapper (mirror the vector-store pattern)
`src/canon/` with a `CanonLedger` over smysl's `Store`, built exactly like `VectorEngine`
([[owned-vector-store]]): lazy open, one store per project at **`<project>/canon.cbor`** beside
`chronicle.db` / `metadata.db`, **atomic writes via `crate::io_atomic`**, a dirty flag, and background
flush like `VectorEngine::sync_in_background` so a save never blocks the render thread. smysl's store is
append-only + content-addressed, so re-deriving the same decision is idempotent (same `Uid`).

### 2.3 Unit model + node bridge
- Decisions are smysl units under an **`x.narrative/…` extension schema** (`world-fact`, `character-trait`,
  `plot-point`, `reveal`, `setup`) — retrievable since smysl 1.7 indexes extension-schema units.
- Every unit carries `SourceKind::Node = inkhaven:<node-uuid>#<chapter/scene breadcrumb>`. Reverse lookup
  via `Store::units_with_source_prefix("inkhaven:<uuid>")` answers "which decisions came from this paragraph."
- inkhaven's DuckDB stays system-of-record; the ledger is the content-addressed history over it.

### 2.4 Harvest-on-save (deterministic part)
Hook the existing save path — `Store::update_paragraph_content` → `DocumentStorage::reembed_document`
(`src/store/mod.rs:2014` / `src/storage/document.rs:223`) — to also refresh the paragraph's canon units,
**backgrounded** like the vector sync. Only **deterministic** producers run here (SENTINEL continuity
detectors `src/continuity_intel/`, `src/world/fact_check.rs`), emitting/superseding units at
`Drafted` with `model:`/`tool:` provenance. Model-based extraction is **not** on this path (§6).

## 3. The queries (deterministic, ~$0 — the value core)

All pure smysl operations, no model call:

- **`inkhaven canon impact <node|unit>`** — `retract` blast-radius: every downstream reveal/setup/decision
  that dangles if this is cut. The headline feature. TUI view + CLI.
- **`inkhaven canon why <unit>`** — `trace` the grounds chain ("why is this true in my world").
- **`inkhaven canon diff [<milestone>]`** — what changed in canon since a CHRONICLE mark (reuse its
  milestone boundary; smysl `diff` over two store snapshots).
- **`inkhaven canon list --scope <book|character|chapter>`** — units filtered by `SourceRef` prefix.

## 4. Commitment, CommitmentFork, and the editorial loop

- **Author sets commitment** (`Floated…Canonical…Retconned`) on a decision — TUI marker + CLI
  `inkhaven canon commit <unit> --level …`, provenance `human:`. This is the axis smysl added for this use.
- **Commitment-support check** (smysl pass 11, `SMY-W057`): "a `Canonical` scene resting on a `Floated`
  premise" — the canonical-scene-built-on-sand warning. Surfaced as an advisory finding.
- **CommitmentFork** (smysl `SMY-W058`): when a `model:` harvest and the `human:` author (or two sessions)
  concurrently disagree on how settled a unit is, the fork is **recorded**, not silently won. Surface it in
  the **Editorial Pass** as a contention to resolve — it slots into the existing findings→editorial flow,
  and (unlike today's per-reader `.inkhaven/*.json` sidecars) it is one editable, provenance-carrying artifact.

## 5. Grounded context via `pack` (optional, improves existing AI)

Book chat / F1 / research can pull canon context through smysl `pack` — budget-fit, closure-complete
(a decision drags in its grounds + rebuttals), `--reserve` for prompt+answer — instead of best-hits
truncation. Seed relevance with inkhaven's HNSW hits via smysl `SalienceRequest::role_weights` (or supply
inkhaven's fastembed as a smysl `Retriever` impl through the `Hybrid` seam) — **no smysl embedding dep**.
Context cost stays constant in canon size. Optional; land after the core.

## 6. LLM harvest (opt-in / milestone only)

Model-based extraction ("what decision does this paragraph *establish*") is expensive and **must not** run
on save. It runs on demand (`inkhaven canon harvest`) or at a CHRONICLE milestone, **cost-capped**, and —
per the AI-advisory rule — **staged** (smysl ingest staging → author confirms) rather than written straight
into the ledger. Model-authored units are capped in commitment (a reader may propose, the author commits).

## 7. Surfaces

- **TUI:** a Canon Ledger dashboard (a free chord — assign from the live keymap at build time) with the
  impact / why / diff views and inline commitment marking; CommitmentFork items routed to the Editorial Pass.
- **CLI:** `inkhaven canon {impact,why,diff,list,commit,harvest}`.
- **Bund:** `ink.canon.*` read words (impact/why/list/commitment_of) + policy classification (reads
  `STORE_READ`, harvest/commit `STORE_WRITE`), matching [[bund-coverage-phases]].
- **Config:** a `canon:` block (harvest cadence, commitment defaults, milestone binding).
- **Docs:** RELEASE_NOTES + MANUAL chapter + a tutorial + FEATURE_INDEX entry.

## 8. Phase map

| Phase | Delivers | Core? |
|---|---|---|
| **CL-P0** | `smysl` dep + `src/canon/` `CanonLedger` wrapper (lazy open, `canon.cbor`, atomic + background flush) | ★ |
| **CL-P1** | unit model (`x.narrative/…`) + `SourceKind::Node` bridge + reverse lookup | ★ |
| **CL-P2** | deterministic harvest-on-save (SENTINEL / fact_check → units), backgrounded | ★ |
| **CL-P3** | queries: `impact` (retract blast-radius), `why` (trace), `diff` (milestone), `list` (scope) | ★ |
| **CL-P4** | author commitment marking + `SMY-W057` support-check advisory | |
| **CL-P5** | CommitmentFork (`SMY-W058`) → Editorial Pass contention | |
| **CL-P6** | `pack`-based grounded context for Book chat / F1 (HNSW-seeded) | |
| **CL-P7** | opt-in / milestone LLM harvest (staged, cost-capped, confirm-to-merge) | |
| **CL-P8** | surfaces — TUI dashboard + `ink.canon.*` Bund + `canon:` config + docs | |

**Value core = CL-P0…P3**: the ledger persists, harvests deterministically on save, and answers "what
breaks / why / what changed" for **$0**. That is a shippable, self-contained release on its own.

## 9. Constraints, risks, and the gate

- **Stability (1.2.15 bar):** `canon.cbor` written via `io_atomic`; the ledger is a *derived* artifact
  (rebuildable by re-harvest), so a crash mid-flush is recoverable — same posture as the vector index.
  No new panic surfaces; background flush uses the same single-flight + backoff as `VectorEngine`.
- **No hot-path cost:** deterministic harvest is backgrounded; LLM harvest is opt-in/milestone (§6).
  Deterministic queries are sub-ms at book scale (§7 of the RFC).
- **Multilingual:** unit text and any rendering key off project language; inkhaven renders via its own
  Typst pipeline (smysl's human-facing strings are English and out of scope — smysl multilingual deferred).
  The "does it work in Russian?" gate applies to any prompt in CL-P7.
- **No duplication:** the ledger does **not** replace Facts/SEMNET (current state) or CHRONICLE
  (finding-history). It cross-references them (SourceKind::Node → node; milestone boundary shared) and
  owns only decision-history. Guard against a second source of truth for *current* facts.
- **The ~1-user gate (decisive):** build only if the honest answer is yes — *would the author keep a canon
  ledger while writing?* The harvest-on-save design exists precisely so the answer can be yes at **zero
  extra author effort**; the author-facing payoff is `impact`/`why`/`diff`. If in practice it goes unused,
  CL-P0…P3 is the natural stopping point and the rest is dropped, per the consolidate/stop/go-deep principle.

## 10. Tests target

Follow the repo bar (warning-free, deterministic). New coverage: `CanonLedger` open/append/save roundtrip
+ atomic-write + background-flush (mirror the `hnsw_store` tests); the node↔unit bridge (SourceRef prefix
round-trip); each query (`impact`/`why`/`diff`) on a fixture ledger with known grounds/supersession; the
commitment-support and CommitmentFork paths surfaced as findings. Integration: a save emits/supersedes the
right units; a cut reports the right blast radius.
