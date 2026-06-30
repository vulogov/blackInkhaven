# RESRCH-2.1 — The "Trust" release (RESRCH-2 first cut)

| | |
|---|---|
| **Track** | RESRCH-2 (Grounded Research) — first cut |
| **Status** | Planned |
| **Target** | 1.5.1 |
| **Builds on** | RESRCH-1 (`inkhaven research`, 1.5.0) |
| **New runtime crates** | **none** |
| **Scope** | Provenance (R2-A) + `/promote` (R2-D) + dedup-on-insert guard. External retrieval (web/docs, R2-B/C) and the broader R2-E hygiene set are **out of scope** — separate cuts. |

A self-contained, zero-dependency release that makes the existing Facts corpus **trustworthy without
adding external sources yet**: every inserted fact records where it came from, you can promote a
speculative Note into a verified Fact, and you're warned before inserting a near-duplicate. This is the
keystone the external-retrieval cuts (web, documents) will stand on — once facts can come from the web,
provenance must already exist.

## Grounding (verified against the tree)

- **Sidecar JSON** is the established pattern (`research/thread.rs` over
  `.inkhaven/research-threads/`); provenance reuses it. `io_atomic::write` + serde.
- **Insertion** funnels through `research::insert::insert_paragraph` (used by `/fact`, `/note`, `n`),
  which returns the new node id — the hook point for recording provenance.
- **Dedup** reuses `book_rag::retrieval::retrieve(...)` (already used by `/diff`); `RetrievedPassage`
  carries a `score: f64` (0..1) to threshold on.
- **`/promote`** resolves a Notes paragraph with `Hierarchy::find_by_path` and reads its body with
  `Store::get_content`, then drives the existing extraction → confirmation → insertion pipeline with
  `TargetBook::Facts`.
- **Config** extends the existing `research:` block (`src/config.rs ResearchConfig`); no new block.
- The confirmation overlay (`ConfirmationState`) and the `/command` dispatcher are the existing
  extension points.

## Phases

| Phase | Content |
|---|---|
| **T-P1** | **Provenance store.** New `research/provenance.rs`: a `.inkhaven/fact-sources.json` sidecar mapping `node_id → { origin, detail, query, thread, created_at }`, where `origin ∈ { model, manual, promoted }` (web/document origins are reserved for R2-B/C). `record()` / `load()` / `for_node()`. Serde + `io_atomic::write`, mirroring `thread.rs`. |
| **T-P2** | **Record on every insert.** Thread the source through `insert_paragraph`'s callers: `/fact` → `{ model, query=<last research prompt> }`; manual `n` → `{ manual }`; `/promote` → `{ promoted, detail=<notes path> }`. Capture the originating research query from the thread's last `Query` turn at `/fact` time. |
| **T-P3** | **Surface provenance.** The confirmation overlay shows a `source:` line for the pending insert; a new **`/sources`** command lists each Facts node with its recorded origin + query (a chat report, like `/diff`); the Markdown export annotates each fact insertion with its source. |
| **T-P4** | **Dedup-on-insert guard.** Before `confirm_insertion` commits, run the Facts-scoped retriever on the body; if the top hit's `score ≥ research.dedup_warn_score` (default `0.92`) and it isn't the node under edit, hold the insert and show a warning in the overlay (`similar to facts/…  ·  Ctrl+S again to insert anyway`). A second confirm proceeds. Zero false-blocking — it only *warns*. |
| **T-P5** | **`/promote [notes/path] [→ facts/path]`.** Resolve the Notes paragraph (`find_by_path`, optional `notes/` prefix; default = the most recent `/note` insertion in this thread), read its body, run the extraction pipeline over it with `TargetBook::Facts`, open the confirmation overlay (provenance `origin=promoted`), insert on confirm. Non-destructive: the Note stays unless the author deletes it. |
| **T-P6** | **Config + docs + tests + cut.** Add `research.dedup_warn_score` (0.92) to `ResearchConfig` + defaults; update tutorial 103 (provenance, `/promote`, `/sources`, dedup) + CONFIGURATION + KEYBINDING + the `Ctrl+B h` overlay + the hint line. Tests: provenance round-trip, dedup threshold logic, `/promote` + `/sources` command parsing. Then `cut 1.5.1` on the user's word. |

## Provenance record (T-P1 shape)

```json
{
  "facts": {
    "<node-uuid>": {
      "origin": "model",                 // model | manual | promoted
      "detail": "",                      // notes path for `promoted`; URL/file later (R2-B/C)
      "query": "Why might the sky be green on an orange-dwarf world?",
      "thread": "rome",
      "created_at": "2026-07-01T10:00:00Z"
    }
  }
}
```

`origin` is an open string so R2-B/C can add `web` / `document` without a migration; the model/manual/
promoted set is all the first cut emits.

## Out of scope (later RESRCH-2 cuts)
- Web search & fetch (R2-C) — new HTTP dep + search API.
- Document / PDF import (R2-B / B′) — PDF needs a text-extraction crate.
- Real per-model cost table, chunked `/factcheck`, streamed extraction display, tab-completion (R2-E).
- Batch / headless research (R2-F).

## Definition of done
- Every `/fact` / `n` / `/promote` insert writes a provenance record; `/sources` and export show it.
- `/promote` turns a Note into a Fact through the same reviewed confirmation flow.
- A near-duplicate `/fact` warns once, then inserts on a second confirm.
- No new runtime crates; the `research:` block gains one field. Tests green; tutorial 103 updated.
