# Tutorial 102 — Mythological & Symbolic Pattern Library

*Inkhaven 1.4.19*

A serious novel runs a symbolic layer beneath its plot: an object that returns charged with new
meaning, a motif that rises and pays off, a character who inhabits an archetypal role. Most tools see
none of it. Inkhaven's **Mythology** library gives that layer a home — but on one strict condition: it
tracks only what **you declare**. It never discovers symbols you didn't name, never interprets them,
and never edits your prose. Every finding is advisory (`info`).

## Declaring your mythology

The **Mythology** system book holds three kinds of declaration paragraph, each by its tag:

- **`para:myth-symbol`** (`⊛`) — a symbol: its vocabulary (the words/phrases that invoke it), its
  declared meaning, a valence (`positive` / `negative` / `ambiguous`), and any traditions it draws on.
- **`para:myth-motif`** (`∿`) — a recurring motif: a name, a description, a valence.
- **`para:myth-archetype`** (`⍟`) — an archetypal role mapped to a character: the role (Vogler's eight
  — hero, mentor, threshold guardian, herald, shapeshifter, shadow, ally, trickster — or your own
  custom role), the character's name, and the function it performs.

Each declaration body is HJSON. The declared symbol vocabulary is also compiled into the editor's
highlight lexicon, so your symbols glow (lavender, `myth_symbol_fg`) as you write.

## Two kinds of check

**Deterministic — zero-AI**, folded into the `Ctrl+B Shift+C` review pass and shown in the Output
`myth` category (the `⊛` glyph):

- **archetype role vacant** — a declared role with no character, or a character not in your Characters
  book;
- **archetype character absent** — a mapped character who barely appears, or who is missing from the
  structural zone the role expects (a herald absent from the opening, a shadow absent from the middle);
- **motif absent from the final act** — a motif that never recurs in the closing chapters.

**LLM — explicit only**, on `inkhaven myth check`:

- **symbol consistency** — is a symbol ever used against its declared meaning or valence?
- **motif completeness** — does a declared motif form a complete arc (introduced, developed, paid off)?
- **archetype role fulfilment** — does the mapped character actually *perform* the declared function?

The model is given only your declarations plus concrete prose excerpts — never the whole book — and is
asked to answer in the project language. Costs inform, never block.

## The heatmap

Press **`Ctrl+V Shift+M`** to refresh the inventory, run the deterministic scans, render the
symbol-density / motif-presence / archetype-presence heatmap into the **Thoughts** pane, and jump to
the nearest declared symbol. It shows, across chapter buckets, where each symbol concentrates, where
each motif appears, and where each archetypal character is present — the shape of your mythic layer at
a glance.

## From the terminal

```sh
inkhaven myth scan                       # refresh + heatmap + deterministic findings (zero-AI)
inkhaven myth check                      # the LLM checks too (exit 1 on any finding)
inkhaven myth check --kind symbol        # just the symbol-consistency pass
inkhaven myth profile                    # print the declared inventory
inkhaven myth refresh                    # force-recompute the deterministic caches
inkhaven myth suppress --finding <id>    # mute a finding
```

## The Myth-Reader

A sixteenth Inner Socrates persona, **The Myth-Reader**, reads for symbolic and archetypal resonance:
when an image returns, what has it gathered since last time? Is a symbol's weight earned or asserted? Is
a character inhabiting the archetype the scene invokes, or wearing it as a label? It interprets nothing
and prescribes nothing — it only asks where the resonance lives.

Your declared symbol traditions also ground **Inner Theologian** (it reads with the traditions you
already invoke, rather than imposing another), and your declared motifs ground the **utopian-architect**
persona (whether your world's logic serves the patterns you care about).

## Scripting & config

Bund: `ink.myth.symbols`, `ink.myth.motifs`, `ink.myth.archetypes`, `ink.myth.density`,
`ink.myth.findings` (all `store_read`), and `ink.myth.suppress` (`store_write`). The `myth:` config
block tunes `heatmap_buckets` (8), `consistency_min_chapters` (5), `motif_min_occurrences` (3),
`final_act_pct` (25), and `check_cost_warn` (0.08); `enabled: false` turns the deterministic findings
and the heatmap chord off. Declarations and findings live in `.inkhaven/myth.duckdb`.

It tracks only what you declare. It interprets nothing. It never edits your prose — it only shows you
the pattern you already made.
