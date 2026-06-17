# Tutorial 68 — The Editorial Pass: one revision worklist

*Inkhaven 1.3.6+*

By the time a draft is done, Inkhaven has a dozen detectors looking at it —
echo repetition, pacing collapse, continuity drift, numeric contradictions,
naming near-misses, dropped characters, stalled threads, the Planning
Board's structural findings, the Facts-scan contradictions. The problem was
never detection. It was that you had to run each one separately
(`doctor --scan`, `plan check`, `facts scan`, …) and hold the results in your
head.

The **Editorial Pass** collapses all of it into **one ranked, walkable
worklist**: every finding, errors first, each one you can jump to, defer, or
skip. It's the pre-submission sweep.

## The worklist

```sh
inkhaven edit
```

```
EDITORIAL PASS · 14 finding(s)  (2 error · 9 warn · 3 info)

  ✗ continuity  ch.7         Mara's eyes change green → brown across chapters
  ⚠ pacing      ch.3         chapter runs 3.1× the local average length
  ⚠ structure   —            drift: `Midpoint` lands at 64% (target 50%, +14%)
  ⚠ tension     —            tension: `Midpoint` is flat — actual 33% vs expected 65%
  ⚠ fact        ch.9         "a three-day ride" contradicts: the capital is 200 miles off
  ⚠ scene       ch.4         scene `The locked study` states a goal but never turns
  · echo        ch.3         `suddenly` appears 4× within ¶2–4
  ...
```

Every finding carries a **category**, a **severity** (error → warn → info),
a **location**, and the message. They come from four places, unified:

- the editorial **`doctor`** classes (echo, pacing, continuity, naming,
  dropped-character, stalled-thread, numeric-contradiction, …),
- the Planning Board's **`plan check`** structural findings (gaps, drift,
  pacing, flat tension, weak scenes / sequels),
- the **Facts-scan** sidecar (contradictions against your world facts),
- the **show-don't-tell** detector (telling phrases, per paragraph).

The pass is **deterministic** — it reads what's already computed, runs no
AI, and so works offline and in CI:

```sh
inkhaven edit --json | jq '.errors'      # gate a build on zero errors
inkhaven edit --only structure,tension   # one category at a time
```

`edit` vs `doctor`: **`doctor`** is *project integrity* (zero-byte files,
orphan rows, storage drift). **`edit`** is *manuscript readiness*. They share
the author-judgment classes; they're not the same surface.

## The cockpit

In the editor, **`Ctrl+V Shift+R`** (R for **R**evision) opens the same
worklist as a walkable modal:

- `↑↓` move; the selected finding's full message + hint expand below.
- `[` / `]` cycle the category filter (all → echo → pacing → …).
- **`Enter`** jumps straight to the finding's location — the chapter /
  paragraph opens in the editor, cursor placed. A `→` marks the jumpable
  ones. (Book-level structural findings have no single location.)

Most editorial findings are *author judgment* — there's no auto-rewrite for
"the midpoint sags"; the fix is you editing at the jumped-to spot.

### Let the AI fix it: `f`

The prose-level findings *can* be fixed for you. A **`✎`** marks the
rewritable ones — **echo** (vary the over-repeated word), **pacing** (tighten
an overlong paragraph), and **show-don't-tell** (rewrite a telling passage to
show). Press **`f`** on one:

```
EDITORIAL · ch.2 · show-don't-tell
finding: "she was furious" — telling

  f → AI rewrite (streams into the AI pane):
  "Her knuckles whitened on the rail; she didn't
   trust her own voice."

  [a]ccept (snapshots first) · [r]eject
```

`f` opens the finding's paragraph, streams a category-specific rewrite, and
pops the **diff review** — the same one the sentence-rhythm rewrite uses.
**`a`** snapshots the paragraph (so the original is one `F6` away) then
replaces it; **`r`** discards. The prompt resolves through the usual three
tiers (a `editorial-fix-<category>` entry in your Prompts book →
`prompts.hjson` → built-in), so the rewriting voice is yours to tune. This is
the loop: see it, `f`, accept — or jump and write it yourself.

### Clearing the list: skip and defer

- **`s`** skips a finding for this session (it's back next time).
- **`d`** **defers** it — persisted. A deferred finding stays hidden until
  its prose *changes*: the fingerprint is the finding's text, so editing the
  passage renews it and it resurfaces, but a deliberate choice you've
  accepted stays gone.
- **`D`** clears every deferral and re-scans.

From the CLI, deferred findings are hidden too (with a count); `inkhaven edit
--show-deferred` includes them.

## The deep tier

The default pass reads the AI sidecars but never *runs* the AI. To refresh
them first — re-run the Facts scan, the tension scan, and the continuity
extract, then aggregate — use:

```sh
inkhaven edit --deep                 # needs an LLM provider
inkhaven edit --deep --provider ollama
```

Each scan prints its progress; one that can't run (no provider, an empty
Facts book) is skipped and the pass falls back to deterministic-only rather
than aborting. `--deep` can't combine with `--json` (the scans print); for a
CI gate, run the scans separately and then `inkhaven edit --json`.

## Where to go next

- The structure findings come from [Tutorial 67](67-planning-board.md).
- The Facts contradictions: `inkhaven facts scan` (Tutorial 63).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [EDITORIAL-1 plan](../PROPOSALS/EDITORIAL-1_PLAN.md).
