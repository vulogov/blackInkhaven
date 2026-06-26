# Tutorial 90 — Nonfiction Reader Personas

*Inkhaven 1.4.6*

Inner Socrates reads your prose as a careful interlocutor and asks **questions**
— never corrections. Through 1.4.5 its five bundled personas all read for
*fiction*: prose rhythm, narrative tension, dramatization, scene-time density.
Run it on a technical manual or a research paper and every question is coherent
but miscalibrated — it interrogates the framing of an argument as if framing
were a rhetorical flourish, or hunts for temporal density in a procedure that
has no scenes. The interrogator is a fiction reader who wandered into a research
library.

AUDIENCE-1 adds four nonfiction personas and makes the Socratic framing
genre-aware. **None of it changes anything for fiction authors** — with no genre
declared and no default persona set, you get exactly the reader you had before.

## The four nonfiction readers

| Persona | For | Reads like |
|---|---|---|
| `skeptical-practitioner` | IT / technical / science | An engineer attempting to reproduce every procedure — asks what's omitted, what's untestable, what's silently assumed. |
| `domain-newcomer` | general nonfiction | A motivated first-time reader — every undefined term is a closed door; finds where you'd lose them. |
| `expert-reviewer` | academic / peer review | A domain reviewer — evidence must support the claim, scope must be stated, causal language must earn itself. |
| `end-user` | documentation | A user following steps to *do* a thing — what's next, what if it fails, how do I know I'm done. |

Each one mutes the narrative-only categories (dramatization gap, temporal
density, unattributed dialogue) and leans on **assumption-surfacing**,
**framing**, and **significance** — so the questions land on what a step assumes,
where a claim outruns its evidence, and whether a procedure earns its place.

List and inspect them:

```sh
$ inkhaven inner-socrates persona list
  …
  skeptical-practitioner Every procedure is a reproduction attempt; what did you leave out?
  domain-newcomer    Every undefined term is a door that won't open for me.
  expert-reviewer    Does the evidence support the claim, and is the scope stated?
  end-user           What do I do next, and how will I know when I'm done?

$ inkhaven inner-socrates persona show expert-reviewer   # voice + emphasis weights
```

## Picking one

Per session, activate one — in the terminal or by cycling in the TUI:

```sh
$ inkhaven inner-socrates persona activate skeptical-practitioner
```

In the editor, **`Ctrl+B J → S`** cycles the active persona through all nine.

For a project that is *always* nonfiction, set a **default** so you don't
re-activate every session:

```hjson
// inkhaven.hjson — top level
inner_socrates_default_persona: skeptical-practitioner
```

This is consulted only when you haven't explicitly chosen a persona; an explicit
`persona activate` (or a `Ctrl+B J → S` cycle) always wins. The Inner Socrates
overview marks the active persona `(project default from config)` while it comes
from this key. Leave the key unset and the default stays the fiction
`inner-socrates`.

## Tell both companions the genre

Set `genre` and the calibration deepens — it reaches the *system prompt* of both
examined-authorship companions, not just the persona weights:

```hjson
genre: technical      // nonfiction | technical | documentation | academic | science | business
```

With a nonfiction genre declared:

- **Inner Socrates** reframes from "a fiction manuscript" to, e.g., "a technical
  document — procedures must be complete, claims testable, prerequisites
  explicit." It asks the questions that form demands.
- **Inner Editor** (`Ctrl+V O`) reframes too: clarity and completeness become the
  craft it observes, noting where ambiguity or omission would stop a
  practitioner.

With **no** genre declared, both keep their original fiction framing — declaring
a nonfiction genre is the only thing that changes it.

## Putting it together — a docs project

```hjson
// inkhaven.hjson
genre: documentation
inner_socrates_default_persona: end-user
```

Now Inner Socrates opens as **The End User** reading your documentation to *do*
the task — "where's the next step?", "what if this fails?", "will I know when I'm
done?" — and the Inner Editor watches for the ambiguous step and the missing
success criterion. Pair it with **`inkhaven sources check`** (Tutorial 89) if
your nonfiction cites references, and the examined-authorship companions are
calibrated for the book you're actually writing.

---

**See also:** [INNER_SOCRATES.md](../INNER_SOCRATES.md) ·
[Tutorial 78 — Inner Socrates](78-inner-socrates.md) ·
[CONFIGURATION.md → `inner_socrates_default_persona`](../CONFIGURATION.md).
