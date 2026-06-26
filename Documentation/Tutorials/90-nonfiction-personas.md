# Tutorial 90 — Nonfiction & Ideas Reader Personas

*Inkhaven 1.4.6–1.4.7*

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

## Three more for ideas-driven work (1.4.7)

Some work is neither plain fiction nor empirical nonfiction — a philosophical
treatise, a theological argument, a utopia. The empirical reviewer is the wrong
reader for these, and so is the fiction reader. Three more personas:

| Persona | For | Reads like |
|---|---|---|
| `philosophical-reader` | philosophy | The Dialectician — reads for the *argument's structure*: the unstated premise, the term that shifts meaning, the counterexample left unanswered, the valid-but-unsound step. |
| `theological-reader` | theology | Respects revelation and tradition as ground — probes internal coherence, fidelity to source, and the scope of each claim. **Never demands empirical proof.** |
| `utopian-architect` | utopia / dystopia | Reads the world as a *designed argument* — what it assumes about human nature, the alternative it forecloses, the cost it elides — while still reading the story as a story. |

Two design notes worth knowing:

- **The theological reader is deliberately non-empiricist.** Demanding "where's
  your evidence?" is the wrong question for claims grounded in revelation. This
  persona asks instead whether the argument *coheres* within its tradition and
  whether each claim *knows its own scope* (what's offered as revealed, what as
  reasoned). Its empirical "hedging / asserted-necessity" detectors are dialled
  down, not up — it won't read conviction as overclaiming.
- **The utopian architect is a hybrid.** A utopia is still fiction, so unlike the
  nonfiction and other ideas personas it does **not** mute the narrative
  categories — it reads the prose as a story *and* presses on the society it
  imagines. It's the one persona that sits in both camps at once.

## Two adversaries — the verdict personas (1.4.7)

Everything above still *asks questions* — that's Inner Socrates' whole nature: it
never praises, never prescribes. Two personas deliberately break that, for the
times you want a one-sided read:

| Persona | Says |
|---|---|
| `defender` | **only praise** — counsel for the defense: what works, and what to protect. |
| `prosecutor` | **only concern** — the prosecution: the weak line, the unearned beat, the soft claim. |

Use them as a steelman / devil's-advocate pair: run the Defender to see what the
passage is getting right (and shouldn't lose in revision), then the Prosecutor to
hear every charge against it.

Two things to know:

- **They're LLM-only.** A verdict can't come from the deterministic Fast track, so
  these two run only on the **Slow track** — `inkhaven inner-socrates check --slow`
  or, in the editor, **`Ctrl+B J → E`** (Engage), which runs the pass in the
  background and drops the verdict into the Output pane. With a verdict persona
  active, the Fast chord (`Ctrl+B J → F`) just points you to `E`.
- **They don't change anyone else.** `inner-socrates` and every other persona stay
  the neutral questioner they always were — only these two speak in verdicts. (You
  can give your *own* persona a one-sided voice with `stance: praise` / `concern`
  in its HJSON file.)

```sh
$ inkhaven inner-socrates persona activate prosecutor
$ inkhaven inner-socrates check --slow --path manuscript/03-rain/01-opening
◆ Concern [Framing] "The rain fell like tears" leans on a simile the paragraph hasn't earned.
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
genre: technical      // nonfiction: nonfiction | technical | documentation | academic | science | business
                      // ideas (1.4.7): utopian | philosophy | theology
```

With a nonfiction or ideas genre declared:

- **Inner Socrates** reframes from "a fiction manuscript" to, e.g., "a technical
  document — procedures must be complete, claims testable, prerequisites
  explicit", or "a theological work — claims rest on revelation and tradition …
  not empirical evidence." It asks the questions that form demands.
- **Inner Editor** (`Ctrl+V O`) reframes too: clarity and completeness for a
  manual; precision of term and the clean move from premise to claim for
  philosophy; register and argument for theology.

With **no** genre declared, both keep their original fiction framing — declaring
a non-fiction genre is the only thing that changes it (an unknown genre also
falls back to fiction).

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

And a treatise on, say, political philosophy:

```hjson
// inkhaven.hjson
genre: philosophy
inner_socrates_default_persona: philosophical-reader
```

Now the interrogator is **The Dialectician** — asking which premise you left
unstated, where a term shifted meaning between sections, which objection you
didn't answer — and the Editor watches for the definition that wobbles and the
transition that smuggles in a premise. Swap `genre: theology` /
`theological-reader` and the reader stops asking for evidence and starts asking
whether the argument coheres within its tradition — the right question for the
form.

---

**See also:** [INNER_SOCRATES.md](../INNER_SOCRATES.md) ·
[Tutorial 78 — Inner Socrates](78-inner-socrates.md) ·
[CONFIGURATION.md → `inner_socrates_default_persona`](../CONFIGURATION.md).
