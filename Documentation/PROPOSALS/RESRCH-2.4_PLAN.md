# RESRCH-2.4 — Trust & hygiene (RESRCH-2 / R2-E)

| | |
|---|---|
| **Track** | RESRCH-2 (Grounded Research) — R2-E |
| **Status** | Shipped 1.5.3 (RE-P1..P5) |
| **Target** | 1.5.3 |
| **Builds on** | RESRCH-2.1–2.3 (provenance, import, web) |
| **New runtime crates** | **none** |

Pay the RESRCH-1 review's debt before widening source surface (the "pay debt before breadth" step of the
[RESRCH-4 RFC](RESRCH-4_RFC.md) cross-track path). Four self-contained hygiene fixes, each grounded
against the current code.

## Grounding (verified)

- **Cost is a flat char-heuristic.** `llm::estimate_cost` (`src/research/llm.rs:18`) = `(prompt+response
  chars / 4) / 1000 · $0.003`; `EST_USD_PER_1K_TOKENS` is the *entire* pricing model. The model name is
  known at the call site but never reaches cost; genai's real usage (`StreamEnd.captured_usage`) is
  dropped in the `End(_)` arm (`src/ai/stream.rs:88`). genai 0.6 exposes `prompt_tokens` /
  `completion_tokens` (`Usage`, opt-in via `ChatOptions::with_capture_usage(true)`).
- **`/factcheck` consistency is one unbounded call.** Truth is chunked (`TRUTH_CHUNK=8`,
  `factcheck.rs:24`; `factcheck_next_call` `app.rs:1013`) but `consistency_user(&fc.facts)`
  (`factcheck.rs:80`) puts the **whole corpus** in one prompt — O(1) calls, unbounded context. Facts
  already carry their tree `location` (`gather_facts` `factcheck.rs:27`), so branch clustering is free.
- **Extraction / factcheck stream but don't show it.** Both already run via `spawn_chat_stream` (the safe
  TUI path — `collect_blocking` is CLI-only). Extraction buffers into `ExtractState.buf`
  (`poll_extraction` `app.rs:1710`) and shows only a status line; the chat path already renders live dim
  tokens (`poll_stream` `app.rs:1356`). Reuse that rendering.
- **Tab is free to repurpose contextually.** The prompt is a `tui_textarea::TextArea` (`app.rs:185`); Tab
  is currently swallowed for focus-cycle at the top (`app.rs:373`). `Hierarchy::find_by_path`
  (`hierarchy.rs:261`) + `children_of` resolve/enumerate slug segments; the OS-shell completer
  (`src/tui/shell/mod.rs:310`, splice recipe `src/tui/app.rs:18816`) is the model.

## Phases

| Phase | Content |
|---|---|
| **RE-P1 — Real cost model** | `StreamMsg::Done(Option<TokenUsage>)` carries genai's captured usage (enable `with_capture_usage`); poll/collect match sites updated. `cost.pricing` config: per-model `{input_per_1m, output_per_1m}` table + `default_*` fallback, seeded for the known providers. `chat::ChatTurn` gains `model`; `finish_stream`/`poll_chain` compute cost via `llm::cost_for(cfg, model, usage, prompt, response)` — real tokens when present, else the char heuristic split input/output. Status bar shows `$` (exact) vs `~$` (estimated). |
| **RE-P2 — Chunked `/factcheck` consistency** | Cluster facts by **tree branch** (top-level Facts chapter, via the `location` slug head). Run the consistency pass **within each branch**, then a **cross-branch** pass over one representative-per-branch (or a bounded sample) so cross-cutting contradictions still surface. Report groups the contradictions by cluster. Bounded call count instead of one unbounded prompt. |
| **RE-P3 — Streamed extraction & factcheck output** | Surface tokens live (dim) while they arrive, reusing the chat render path: extraction appends to a transient streaming `ChatTurn` (or a status pane) until Done, then opens the confirmation overlay as today; `/factcheck` shows the in-flight chunk's tokens instead of a silent status. No new async machinery — same `try_recv` poll, just rendered. |
| **RE-P4 — Tab-completion** | In `Focus::QueryPrompt`, when the cursor sits in a completable path token — after `/goto `, or after `→` / `->` in `/fact`/`/note` — Tab completes the next slug segment against the Facts tree (`find_by_path` parent + `children_of` candidates; longest-common-prefix splice; list on ambiguity). Otherwise Tab keeps cycling focus. Mirrors the shell completer's 0/1/N handling. |
| **RE-P5 — `/factcheck` verdict glyphs + `/whatswrong`** | The truth pass already grades each fact `ACCURATE \| DUBIOUS \| INACCURATE — reason`; capture that per Facts node into a `.inkhaven/fact-verdicts.json` sidecar and **glyph the Facts tree** (✓ green / ? yellow / ✗ red) so problem paragraphs are visible at a glance. New **`/whatswrong [facts/path]`** (bare → the selected fact) streams an AI explanation of *what* is inaccurate/questionable, seeded with the recorded verdict reason, in the project language. The audit report + status line gain a `✓N ?N ✗N` tally. *(Feature requested after RE-P1..4; folds into the same cut.)* |

## Tests
- `cost_for`: table hit (input/output split), default fallback, real-tokens vs heuristic parity shape.
- factcheck clustering: branch grouping by `location` head; large-branch split; cross-branch pass; bounds.
- completion: token extraction after `/goto` and after `→`; LCP of candidates; non-path commands.
- verdicts: truth-report parsing (levels + reasons, out-of-range/garbage ignored); glyph mapping.
- (Streaming + tree glyph rendering are integration-tested; the parse/format helpers stay unit-tested.)

## Out of scope (later)
- Cache-token / reasoning-token pricing detail (only prompt+completion priced).
- Embedding-neighbourhood clustering for `/factcheck` (tree-branch is the first cut; neighbourhood is a
  follow-on if branches prove too coarse).
- Completion on `/promote` paths and config keys.
