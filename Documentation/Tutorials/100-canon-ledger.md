# Tutorial 100 — The Canon Ledger

*Inkhaven 3.11.0*

Your manuscript is the text; git versions the text. But the **decisions** behind
the story — *the city floats on a leviathan; the sea gate is the only way out; the
harbour-master knows and hasn't told anyone* — live only in your head and scattered
across chapters. The **Canon Ledger** records them, with what each rests on and how
settled it is, so you can ask the one question revision keeps raising: *if I cut
this, what breaks?*

It never edits your prose. It records decisions; you decide.

## Where decisions come from

You don't hand-maintain a bible. Decisions enter three ways:

1. **Tags you already write.** A `rel:mentor:Vasa:Iri` tag on a paragraph becomes a
   `character-trait` decision the moment you save. No cost, no model.
2. **An opt-in model pass.** Ask a model to read a scene and *propose* decisions:

   ```
   inkhaven canon harvest my-book/chapter-1
   ```

   Nothing enters the ledger yet — the proposals are staged. Review them:

   ```
   inkhaven canon staged
   ```
   ```
   4 staged proposal(s) (not yet in the ledger):
     [world-fact]  The city of Vell floats on the back of a sleeping leviathan.  (my-book/chapter-1/scene-1)
     [reveal]      Mara has never told anyone that the tremors are the beast waking.  (…)
     …
   ```

   When you're happy, confirm — this is the only thing that writes to the ledger:

   ```
   inkhaven canon accept
   ```

## Reading the ledger

```
inkhaven canon list
```
```
4 canon decision(s):
  b3:lypx…  [world-fact]  The city of Vell floats on the back of a sleeping leviathan.  (…/scene-1)
  b3:pgdf…  [reveal]      Mara has never told anyone that the tremors are the beast waking.  (…)
  …
```

In the editor, open the **reader hub** (`Ctrl+B *`) and choose **Canon** — the same
list, scrollable, and `Enter` jumps to the paragraph a decision came from.

## The question that matters

Mark how settled a decision is, then see what depends on it:

```
inkhaven canon commit b3:lypx --level canonical
inkhaven canon impact b3:lypx
```
```
If cut:  b3:lypx  [world-fact]  The city of Vell floats on the back of a sleeping leviathan.
  2 decision(s) would dangle:
    b3:x3k7  [plot-point]  The escape uses the sea gate in the leviathan's flank.
    …
```

That's the payoff: cut the leviathan and the ledger tells you the escape route
dangles — *before* you discover it three chapters later.

Two more you'll reach for:

- `inkhaven canon why b3:x3k7` — the chain a decision rests on.
- `inkhaven canon check` — anything you've marked `canonical` that rests on something
  only `floated` (a scene built on sand).

## What it will not do

It won't rewrite your prose, and the model never adds a decision you didn't accept.
The deterministic parts (`list`, `impact`, `why`, `check`) cost nothing and run
instantly. The ledger is rebuildable from your manuscript — losing it costs a
re-harvest, never a word of the book.

See [`../CANON.md`](../CANON.md) for the full reference.
