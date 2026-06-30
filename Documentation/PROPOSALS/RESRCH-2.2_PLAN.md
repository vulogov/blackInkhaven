# RESRCH-2.2 — Document import (RESRCH-2 / R2-B)

| | |
|---|---|
| **Track** | RESRCH-2 (Grounded Research) — R2-B |
| **Status** | Complete — bundled into 1.5.1 |
| **Builds on** | RESRCH-2.1 (provenance) |
| **New runtime crates** | **1** — `pdf-extract` (PDF text extraction; MD/text needs none) |
| **Scope decision** | Markdown / text **+ PDF**; ships **bundled into 1.5.1** with the trust cut |

The first step of *grounded research*: ground the assistant on the author's **own documents**, not just
the model's closed-world knowledge. `/import` ingests a Markdown / text / PDF file as a **research
source**; its chunks are retrieved alongside Facts and cited, and a `/fact` taken from a source-grounded
answer records provenance `origin=document`.

## Grounding (verified)

- `DocumentStorage::add_document(metadata, content)` embeds arbitrary content into the shared HNSW with
  arbitrary JSON metadata; `Store::raw()` exposes it; `delete_document(id)` removes a chunk.
- `Store::search_text(query, n)` returns hits as JSON with `id` / `metadata` / `document` / `score` —
  so research-source chunks filter by `metadata.kind == "research_source"`, cleanly separate from the
  tree-scoped Facts retrieval (book_rag scopes by node ids; standalone docs aren't tree nodes).
- `pdf-extract 0.9` provides `extract_text(path) -> String`; MD/text is read directly.

## Phases (built)

| Phase | Content |
|---|---|
| DB-P1 | `research/imports.rs`: `read_source` (md/txt/pdf), `chunk_text` (paragraph-packed, hard-split), and the `.inkhaven/research-sources.json` sidecar (`Imports`). + `pdf-extract` dep. |
| DB-P2 | `/import <path>` embeds chunks tagged `kind:research_source` (+ source / name / thread / chunk metadata) and records the sidecar; `/import` (bare) lists; `/forget <name>` deletes a source's chunks. Re-import drops the previous chunks. |
| DB-P3 | Retrieval: `rag::build_context` now also pulls research-source chunks (`search_text` filtered by kind, gated off only in *Full only* mode), cites them `[source: name]`, and returns the contributing source names. |
| DB-P4 | Provenance: a `/fact` from a source-grounded answer records `origin=document` + the source names (`ChatTurn.sources` threads the grounding through). |
| DB-P5 | CLI `inkhaven research --import <path>` (non-interactive, thread-global); `research.import_chunk_chars` (1500) config; docs (tutorial 103 §6½, CONFIGURATION, KEYBINDING, Ctrl+B h); tests. |

## Notes & limits
- Imported sources are **project-global** (recorded under thread `""` / their thread but retrieved for
  all threads) — a PDF is a project resource, not a per-session one.
- **No OCR**: scanned / image-only PDFs yield little text.
- Web fetch (R2-C) is the next external-retrieval step; the source-retrieval + provenance plumbing here
  is what it slots into.

## Out of scope (later)
- Web search & fetch (R2-C). Per-source credibility. Re-chunking strategy tuning / overlap.
