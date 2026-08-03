# BUND-2.2 — Sync Bund exposure for the 2.x features (plan)

*Status: plan. The 2.0→2.1 flagships (SEMNET graph, GRAPHMIND, CHORUS, the Inner
Stylist) shipped with CLI + TUI surfaces but **no Bund stdlib words**. This plan
adds the deterministic ("sync") `ink.*` surface so scripts and hooks can read the
graph and the voice-at-scale findings, following the exact pattern the existing
modules already use.*

## Scope — "sync" means deterministic

Bund words are synchronous: they run inline on the VM thread and push a value.
So this plan covers the **deterministic reads** (and the deterministic writes —
edge triage, suppression). The **LLM-backed** operations (`graph ask`, `graph
link`, the Inner Stylist `--coach`) are *not* sync — they'd block the VM on a
network call. Those get **blocking** words only if wanted later (the precedent is
`ink.ai.send_blocking`); they are out of scope here and noted as a follow-up.

## The pattern (already established, verbatim)

Each feature is one file `src/scripting/stdlib/<feature>.rs` with:

```rust
pub fn register(vm: &mut VM) -> Result<()> {
    let words: &[(&str, fn(&mut VM) -> Result<&mut VM, BundError>)] = &[
        ("ink.<ns>.<word>", w_word), …
    ];
    for (name, f) in words { vm.register_inline(name.to_string(), *f)?; }
    for (name, _) in words { /* register the `ink.`-stripped short alias */ }
    Ok(())
}
```

- Each `w_word` is a thin wrapper over a `do_word(vm) -> anyhow::Result<&mut VM>`
  mapped through `to_bund_err`.
- Context comes from `super::helpers::{active_store, active_config, push}` — a
  `ctx(tag)` resolves `(store, cfg, hierarchy, single-user-book, feature-store)`
  exactly like `prose.rs::ctx`.
- Results push `Value::from_list` / `from_dict` / `from_int`; `Value::nodata()`
  for absent (never a fake `0.0`), mirroring `prose.rs`.
- Register the module: `pub mod <feature>;` + `<feature>::register(vm)?;` in
  `src/scripting/stdlib/mod.rs` (one line each).

`prose.rs` and `dialogue.rs` are the reference implementations to copy.

---

## `ink.graph.*` — the knowledge graph (SEMNET)

Reads over `edges.db`, plus the deterministic triage writes. Node/edge ids are
strings (UUIDs).

| Word | Stack | Notes |
| ---- | ----- | ----- |
| `ink.graph.stats` | ( -- dict ) | node/edge counts + per-kind breakdown (`store.graph_stats`). |
| `ink.graph.neighbors` | ( node -- list ) | one-hop edges as dicts (kind, dir, other-endpoint, reason) — `store.subgraph(id,1,&[])`. |
| `ink.graph.contradicting` | ( node -- list ) | stance clashes (`store.contradicting`). |
| `ink.graph.loci` | ( node -- list ) | cited primary-source loci (`edges_out(CitesLocus)`). |
| `ink.graph.paths` | ( from to -- list\|nodata ) | bounded Cites+LinksTo path (`store.paths(…,8)`). |
| `ink.graph.pending` | ( -- list ) | the advisory `judged` edge inbox (`store.pending_edges`). |
| `ink.graph.rebuild` | ( -- dict ) | re-derive structural edges; returns `{cleared, added}` (`graph_rebuild`). |
| `ink.graph.promote` | ( edge -- bool ) | judged → promoted (`promote_edge`). |
| `ink.graph.dismiss` | ( edge -- ) | delete a stance edge (`dismiss_edge`). |

Endpoint dict shape reuses the CLI's `endpoint_label` mapping (node title / extern
`kind ref`).

*GRAPHMIND note:* `graph ask` / `graph link` are LLM — out of sync scope. A later
`ink.graph.ask_blocking` ( question -- answer ) could wrap `graph_rag::ask::ask`
with `collect_blocking`, if the blocking cost is acceptable.

---

## `ink.chorus.*` — voice & style at book scale (CHORUS)

All deterministic. Each word runs the relevant pillar over the single user book
(the `ctx` book) and pushes structured results.

| Word | Stack | Notes |
| ---- | ----- | ----- |
| `ink.chorus.voices` | ( -- list ) | per-character voice fingerprints: `{name, cv, mattr, p50, modal_density, interiority_ratio, confidence, utterances}` (`voices::character_profiles`). |
| `ink.chorus.distinct` | ( -- dict ) | `{compared:[…], indistinguishable:[{a,b,distance}], closest, most_distinct}` (`distinct::matrix`). |
| `ink.chorus.drift` | ( -- list ) | per-character voice drift: `{character, violations:[{chapter,metric,delta}]}` (`drift::character_drift`). |
| `ink.chorus.headhops` | ( -- list ) | POV/head-hop findings: `{chapter, scene, pov, experiencer, count}` (`pov::scan_head_hops`). |
| `ink.chorus.tense` | ( -- dict ) | `{supported:bool, reason?, slips:[{chapter,scene,dominant,excerpt,tense}]}` (`tense::scan_tense`). |
| `ink.chorus.register` | ( -- dict ) | per-chapter register + drifts (`register::scan_register`). |

Language coverage carries through as the functions already report it (tense
`{supported:false, reason}` for Russian; language-sensitive metrics `nodata` on
unsupported languages).

---

## `ink.stylist.*` — the Inner Stylist synthesis

The synthesised Praise/Note/Concern findings + suppression management. (The LLM
`--coach` is out of sync scope.)

| Word | Stack | Notes |
| ---- | ----- | ----- |
| `ink.stylist.findings` | ( -- list ) | the synthesised findings minus suppressions: `{severity, kind, key, message}` (`inner_stylist::pipeline::gather` + drop `all_suppressions`). |
| `ink.stylist.suppress` | ( key -- ) | silence a finding (`InnerStylistStore::suppress`). |
| `ink.stylist.unsuppress` | ( key -- ) | restore it. |
| `ink.stylist.suppressions` | ( -- list ) | the silenced keys. |

---

## Cross-cutting

- **No new dependencies.** Every word wraps a function that already exists.
- **Deterministic + free** — no LLM, no cost; safe to call from an on-save hook
  or an ambient script (though `chorus`/`stylist` words run the full pillar
  sweep, so cache/rate-limit at the script layer for large books).
- **Errors** propagate as `BundError` (via `to_bund_err`), consistent with the
  other modules.
- **Tests** — each module gets the same smoke coverage the tree already applies:
  a Bund script exercising each word against a fixture project (the pattern in
  the existing stdlib tests), plus the pure functions are already unit-tested.

## Phases

- **B-1** `ink.graph.*` (SEMNET reads + triage) — mirrors `dialogue.rs`.
- **B-2** `ink.chorus.*` (the six pillars) — mirrors `prose.rs`.
- **B-3** `ink.stylist.*` (findings + suppression).
- **B-4** docs — the `ink.*` tables in GRAPH.md / CHORUS.md + a scripting tutorial
  section; update `INKHAVEN_CHEAT_SHEET.typ`.
- **(later, opt-in)** blocking LLM words — `ink.graph.ask_blocking`,
  `ink.stylist.coach_blocking` — only if the blocking-on-network cost is wanted.

This is a small, highly-regular cycle — three new files copying `prose.rs`, three
one-line registrations, one docs pass — and it closes the scripting-parity gap
the 2.x flagships opened.
