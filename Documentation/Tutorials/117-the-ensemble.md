# Tutorial 117 — The Ensemble: Your Cast, Connected

*Inkhaven 3.2 (ENSEMBLE)*

You've declared who knows what (KEN), who's bonded to whom (BONDS), and where each
arc bends (CHAR-1). ENSEMBLE stops treating those as separate findings and lets
you *see the people* — the cast, their relationships, and their arcs — as one
connected picture. Nothing new to declare; it's built entirely from what you've
already tagged.

## Put your relationships on the graph

Inkhaven keeps a typed graph of your book (paragraphs, events, facts, senses).
As of 3.2, your declared bonds join it. Rebuild once:

```sh
inkhaven graph rebuild
```

Every `rel:<kind>:<A>:<B>` tag becomes a **`relates`** edge between the two
characters. Now the relationship network is traversable with the graph tools you
already have — ask who's connected to a character:

```sh
inkhaven graph neighbors <character-node-id>
```

```
◆ Mara
├─ relates (2)
│    ⇄ Kell — ally (ch. 1)
│    ⇄ Sella — enemy (ch. 3)
```

A transition — allies who become enemies — shows as **two** edges, so both
states of the relationship are on the graph. In the editor, the **`Ctrl+B z`**
graph hub → `n` shows the same one-hop view, and the F9 **Graph** chat can now
answer *"how are Mara and Sella connected?"* in prose over the graph.

## See the whole cast at once

The **Dramatis Personae** joins your cast with their bonds and their arc state —
one view of who's in the book and how they stand:

```sh
inkhaven cast
```

```
Dramatis Personae — The Ninth Lantern (4 character(s))

Mara  [corruption · broken (ch. 5) · ✦1]
  ⇄ Kell — ally (ch. 1)
  ⇄ Sella — enemy (ch. 3)
Kell  [flat · steady (ch. 6) · ✦0]
  ⇄ Mara — ally (ch. 1)
```

Each character shows their declared arc shape, their latest observed state (with
the chapter and a ✦ change count), and their bonds. Anyone the book tracks
appears — even a character who only ever shows up in a `rel:` tag (they'll be
listed without an arc). `--json` gives you the whole structure for scripting.

## Open it in the editor

Press **`Ctrl+B z`** for the graph hub, then **`c`** — the Dramatis Personae opens
as a scrollable dashboard. `↑↓` to move, **Enter** to jump straight to a
character's bible entry, `Esc` to close. It's the fastest way to answer "wait,
who is this again, and who are they to everyone else?" three hundred pages in.

## What it costs

Nothing. The relationship graph is derived edges; the Dramatis Personae is a pure
join over data already on disk. No model, ≈ $0, at any book size.

---

ENSEMBLE turns your scattered character declarations into one connected cast you
can read and traverse. The full reference is [`ENSEMBLE.md`](../ENSEMBLE.md); the
relationship findings themselves still live in [`BONDS.md`](../BONDS.md).
