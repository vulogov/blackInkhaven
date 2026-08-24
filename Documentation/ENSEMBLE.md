# The Ensemble — the People Layer (ENSEMBLE)

*(3.2, ENSEMBLE)*

Inkhaven's readers watch many axes — continuity (SENTINEL), knowledge (KEN),
relationships (BONDS), voice (CHORUS), arc (CHAR-1). **ENSEMBLE doesn't add a new
reader; it deepens the *people* layer** the existing ones already feed, so the
cast, their bonds, and their arcs are things you can *see and traverse*, not just
findings you're told about.

Two pieces, both building straight on [BONDS](BONDS.md) and the
[knowledge graph](GRAPH.md) — deterministic, ≈ $0, no new subsystem.

---

## 1 · The relationship graph

Every declared bond becomes a first-class **graph edge**. On `inkhaven graph
rebuild`, each `rel:<kind>:<A>:<B>` tag is promoted into a symmetric **`relates`**
edge between the two characters' nodes — the bond kind (ally / enemy / …) and the
first chapter riding in the edge, deduped to one edge per (pair, kind) so a
transition (`ally` → `enemy`) shows as two edges: both relationship states, on
the graph.

That means the relationship network is traversable with the tools you already
have — no new surface to learn:

```
inkhaven graph rebuild                 # promotes rel: bonds into `relates` edges
inkhaven graph neighbors <char-node>   # one-hop: every relationship touching them
inkhaven graph paths <a> <b>           # is there a chain of relationships between two?
```

In the editor, the **`Ctrl+B z` graph hub** → `n` shows a character's one-hop
neighbourhood, where a bond reads legibly:

```
◆ Mara
├─ relates (2)
│    ⇄ Kell — ally (ch. 1)
│    ⇄ Sella — enemy (ch. 3)
```

And because relationships are now edges, the **F9 Graph chat** / `graph ask` can
reason over them: *"who is connected to Mara, and how?"* See [GRAPH.md](GRAPH.md).

---

## 2 · The Dramatis Personae

One book-wide view of the cast — **who is in this book, how they connect, and
where each arc stands** — joining three things inkhaven already tracks:

- the **cast** (the Characters roster),
- their **bonds** (BONDS `rel:` declarations), and
- their **arc state** (CHAR-1: the declared arc shape + the latest observed
  chapter state + the change count + agency).

```
inkhaven cast              # the Dramatis Personae (human)
inkhaven cast --json       # the whole structure
```

```
Dramatis Personae — The Ninth Lantern (4 character(s))

Mara  [corruption · broken (ch. 5) · ✦1]
  ⇄ Kell — ally (ch. 1)
  ⇄ Sella — enemy (ch. 3)
Kell  [flat · steady (ch. 6) · ✦0]
  ⇄ Mara — ally (ch. 1)
```

The cast is the union of everyone the book *tracks* — anyone with an arc
declaration, a recorded state, or a declared bond — so a character who only ever
appears in a `rel:` tag still shows (nodeless, no arc). In the editor, the
**`Ctrl+B z` graph hub → `c`** opens the same view as a scrollable dashboard;
`↑↓` scroll, **Enter** jumps to the highlighted character's bible node, `Esc`
closes.

---

## Cost — a design invariant

ENSEMBLE writes no new model call. The relationship graph is a set of derived
edges rebuilt with the rest of the graph; the Dramatis Personae is a pure join
over data already on disk (roster + BONDS + the CHAR-1 store). ≈ $0, deterministic,
independent of book length.

---

## Multilingual

The cast and the relationship graph inherit BONDS's and KEN's Unicode-aware name
matching (Cyrillic, accents, Latin match alike); character names join across the
roster, bonds, and arc store in the project's language.

---

## What it is not

- Not a new reader — it surfaces what BONDS and CHAR-1 already know; it adds no
  new findings. (The relationship *findings* still come from `inkhaven bonds`.)
- Not a relationship *guesser* — the graph holds only **declared** bonds; the
  Dramatis Personae reflects only what you tagged and what CHAR-1 observed.
- Not a rewriter — every surface here is read-only.
- Not alias-aware (yet) — a character is matched by their roster name; nicknames
  and titles are a documented non-goal for this release.
