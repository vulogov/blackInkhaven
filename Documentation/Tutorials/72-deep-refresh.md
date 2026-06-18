# Tutorial 72 — The background deep refresh

*Inkhaven 1.3.12+*

The world-consistency checks (facts, drift, continuity) are AI passes — they
can take minutes on a long book. Until now you ran them from the CLI
(`inkhaven world --deep`) and waited. 1.3.12 brings the whole batch *inside*
the editor, on a background thread, so you never leave your prose and never
freeze the UI.

## One chord

Press **`Ctrl+V Shift+F`** (F for re**F**resh). It kicks off, in the
background:

- `facts check` — contradictions *within* the Facts book,
- `facts scan` — prose-vs-fact contradictions,
- `drift scan` — divergent descriptions of the same entity,
- `continuity extract` — tracked character attributes,

the same set as `inkhaven world --deep`, **in your manuscript's language**.
A **`⟳ deep refresh`** chip appears in the status bar and tracks progress
(`⟳ deep refresh: drift [3/4] …`); the editor stays fully responsive — keep
writing while it runs. One refresh at a time; it needs an LLM provider.

When it finishes, if the story bible (`Ctrl+V Shift+L`) or the Editorial Pass
cockpit (`Ctrl+V Shift+R`) is open, it **rebuilds itself** from the fresh
findings, and the status shows the new world-consistency summary — e.g.
`deep refresh done — World: 2 issue(s) — 1 fact conflict · 1 drift`.

## It's advisory — it never edits your text

This is the important guarantee: the deep refresh **reads** your manuscript
and **writes findings** (to `.inkhaven/*.json` sidecars). It does **not**
change a single word of your prose — no paragraphs created, edited, or
deleted. It surfaces what might be inconsistent; *you* decide what to do.

The only thing that ever rewrites prose is the Editorial Pass's `f` / `F` —
and that is always something *you* press, on one finding at a time, snapshotted
(the original is one `F6` away) and shown to you as a diff to accept or reject
before anything lands. The background refresh never touches that path.

## Stale findings announce themselves

Findings live in their sidecars until you re-run a scan — so after you edit, a
finding can predate your fix. Inkhaven notices: each scan stamps a cheap
fingerprint of the manuscript, and when the prose has moved since, the world
report, the `Ctrl+V Shift+L` bible banner, and the Editorial Pass all show
**`⚠ may be stale`** with a nudge to re-run `Ctrl+V Shift+F`. It only *warns* —
it never hides or deletes a finding; you decide when to refresh.

## Why it can run while you write

DuckDB is single-writer, so a second process can't open the project the TUI
holds. Instead the refresh runs in-process on a thread that **shares** the
editor's database connection pool (a clone of the open store) — no second
open, no lock. The pool is 4 connections by default, so the background read
and your foreground edits never contend; tune it with `embeddings.pool_size`
if you script heavy concurrent access.

If you quit mid-refresh, the run is simply abandoned: sidecars it already
finished are valid (writes are atomic), an unfinished one is just stale — never
corrupt. Re-run the chord any time.

## Where to go next

- What the findings mean: [Tutorial 71](71-world-report.md) (the world
  report) · [70](70-semantic-drift.md) (drift) · [69](69-world-consistency.md)
  (facts / anachronisms).
- Acting on them: the Editorial Pass, [Tutorial 68](68-editorial-pass.md).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [DEEP-1 plan](../PROPOSALS/DEEP-1_PLAN.md).
