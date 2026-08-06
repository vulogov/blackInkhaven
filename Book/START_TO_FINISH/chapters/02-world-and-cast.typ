#import "../design.typ": *

#chapter(number: 2, title: "The World and the Cast")

We have an empty project with a book in it and nothing written. It is tempting
to start with a sentence. Resist it for one more chapter. Before *The Ninth
Lantern* has a first line, it needs two things a mystery cannot do without: a
*place* for the ninth lantern to go dark in, and *people* to notice. We are not
going to build them exhaustively — the story is a scaffold, and the bible's
first instruction is to keep it light — but we are going to write down just
enough that Inkhaven can start watching our world for us. Ten minutes of typing
now buys the overlay, the grounded assistant, and — chapters from here — the
continuity reader, the voice reader, and the who-knows-what reader, all at once.

This is the payoff of doing it first. A name you record is a name every reading
intelligence in the tool can see; a name you never record is a name none of them
can. So we spend this chapter filling two small books that were seeded for us at
`init`, and then we watch the editor light up.

#section("Two books were already waiting")

Look back at the Tree pane from the last chapter. Below our own book and above
Help sits a small block of *system books* — Notes, Research, Prompts, and two
that this chapter is about: *Places* and *Characters*. Inkhaven created them at
`init`. They are ordinary books — chapters, paragraphs, Typst prose — but
Inkhaven reserves them, keeps them out of the finished manuscript, and wires
behaviour to what you type into them.

#screen(caption: "The Tree — our book above, the seeded system block below")[```
  The Ninth Lantern        (our book)
  ─────────────────────────────────────
  Notes
  Research
  Prompts
  Places          ← cyan overlay · Ctrl+B P
  Characters      ← yellow overlay · Ctrl+B C
  Help
```]

The two are structural twins. Everything below about a Place is true of a
Character with three swaps: the book, the overlay colour (cyan for Places,
yellow for Characters), and the RAG chord (`Ctrl+B P` versus `Ctrl+B C`). The
data model is the whole of one sentence: *an entry is a paragraph, and the
paragraph's title is the entity's name.* There is no "register" step — saving a
paragraph is registering it.

#term("System book")[
  A book Inkhaven seeds and reserves — `places`, `characters`, `facts`, and a
  few more. You freely add, edit, and delete the paragraphs #emph[inside] it, but
  you cannot delete the book itself. Its contents drive a feature instead of
  appearing in your PDF, and the prose tools leave it out of the corpus.
]

#section("Sketching Saltmarch")

Move the tree cursor to *Places*, press `→` to expand it, and press `+` to add a
paragraph. The Add modal asks for a title; that title *is* the place name. Press
Enter, and the new paragraph opens in the editor ready for whatever you want to
remember — description, history, why it matters. It is ordinary Typst: headings
with `=`, `*bold*`, lists, all of it.

Saltmarch is small, so its map is small. We give it the town, the harbour it
rings, the mole that reaches into the water, and the two lanterns that matter —
the nine as a set, and the ninth that stands alone at the end of the mole. Five
flat paragraphs, no grouping; with a cast of one town we do not need chapters
yet. (When a Places book grows to dozens of entries, you would add chapters —
`Cities`, `Regions`, `Buildings` — and nest under them. Not here.)

#screen(caption: "The Places book for The Ninth Lantern")[```
  Places
    Saltmarch                (paragraph)
    The harbour              (paragraph)
    The Long Mole            (paragraph)
    The nine lanterns        (paragraph)
    The ninth lantern        (paragraph)
```]

Each body stays a few lines — the bible says keep it light, and a short entry is
enough for the overlay and plenty for the assistant to ground on. Saltmarch's
looks like this:

#screen(caption: "Editor · Places / Saltmarch — the entry body")[```
  = Saltmarch

  A poor fishing town on a cold northern coast. Stone
  quays, a working harbour, sea-fret off the water most
  mornings. Nine lanterns ring the harbour; the creed
  is that no lantern goes dark.
```]

#callout(label: "One entry per paragraph")[
  Keep entries atomic. The overlay, the grounded lookup, and later readers all
  key off the paragraph *title*, so a paragraph that describes two places is only
  ever found under one name. If you catch yourself writing "the harbour and the
  mole" as one entry, split it — that is exactly why *The Long Mole* is its own
  paragraph here.
]

#section("Adding the cast")

Characters works identically. Move to the *Characters* row, `→` to expand, `+`
to add, and title each paragraph with the character's *canonical name*. Our cast
is five, so again we stay flat — most authors group a larger cast by role
(Protagonists, Supporting) or by the family they belong to, but five names read
fine as a list.

#screen(caption: "The Characters book — the cast of The Ninth Lantern")[```
  Characters
    Mira Fenn                (paragraph)
    Aldous Crane             (paragraph)
    Sella Vale               (paragraph)
    Toft                     (paragraph)
    Bryn Crane               (paragraph)
```]

The bodies are, again, deliberately thin — a line of who-they-are, and the
relationships the plot turns on. This is where voice notes, secrets, and the
chapters a character enters and leaves will eventually live, but on day one a
sentence each is right:

#screen(caption: "Editor · Characters / Mira Fenn — the entry body")[```
  = Mira Fenn

  Protagonist and POV. Young under-keeper, apprenticed
  to the harbourmaster; practical, stubborn. The one
  who finds the cold lantern and goes looking.
```]

Note one thing we are *not* writing yet: nowhere in Aldous Crane's entry does it
say he put the ninth lantern out himself. That is the book's central secret, and
who is allowed to know it — and when — is a whole later chapter's concern. For
now the roster just needs to exist. Recording a name is not the same as spending
its secrets.

#two_track(
  [Your Characters book is the cast list and your Places book is the map's index.
  The five names here are what will, in later chapters, switch on continuity,
  voice, and the who-knows-what reader — all from these paragraphs.],
  [For a non-fiction book the same two books hold recurring *entities*: the
  people your argument keeps naming and the institutions or sites it returns to.
  The overlay then doubles as a spell-check — a name written two ways stops
  lighting up on the variant.],
)

#section("A light touch of world.hjson")

Places and Characters record *what things are called*. There is a third,
optional file that records *how the world behaves* — its physics — and Saltmarch
needs a whisper of it: the coast is cold, and the fret comes off the water. That
file is `world.hjson` at the project root. It is opt-in; without it nothing
changes, and drop one in and the world layer wakes up.

We are going to keep this to the bone. Only two blocks are load-bearing — the
world's `name` and its `astronomy` (everything about seasons and tides derives
from the sky) — and then one small `geography` landmark to pin Saltmarch's cold.
That is all the story wants.

#screen(caption: "world.hjson — Saltmarch, kept small")[```
{
  name: "Saltmarch coast"
  seed: 0x5A17
  primary_language: "en"

  astronomy: {                 // the only required block
    star:   { class: "G2V", luminosity_solar: 1.0 }
    planet: { axial_tilt_deg: 23.4, day_length_hours: 24.0 }
    orbit:  { semi_major_axis_au: 1.05, year_length_days: 365 }
    moons:  [ { name: "the Watch", period_days: 29.5 } ]
    calendar: { months: 12, month_length_days: 30, weekdays: 7 }
  }

  geography: {
    landmarks: [
      { name: "Saltmarch", kind: "town",
        climate_zone: "subarctic", population: 1400 }
    ]
  }
}
```]

Two small faithfulness points earn a mention. First, the *HJSON quoting rule*:
an unquoted string runs to the end of the line, so an inline enum must be quoted
— `class: "G2V"`, `kind: "town"`, never bare. When a value comes out looking
wrong, this is nearly always why. Second, the landmark's `climate_zone` turns
Saltmarch into a *gazetteer* entry: the fact-checker now knows the town's weather
by name, so a chapter that puts a warm afternoon on this cold coast has something
to be measured against. We push the orbit out to `1.05` AU and give the world a
subarctic town precisely so "cold" is a declared fact, not a vibe.

#callout(label: "We are not compiling a world here")[
  `world.hjson` can derive whole continents, rivers, and cities by simulation —
  that is a large feature with a companion book of its own. *The Ninth Lantern*
  needs none of it. We declare the sky and one cold town so the coast is honest,
  and stop. Reach for the compiler when a story's geography has to hold together
  across a map; a single harbour town does not.
]

#section("Watching it light up")

Now the reason we did all this before drafting. Open any paragraph of the
manuscript and Inkhaven compiles a lexicon from the two books' titles and lays it
over your prose: every word that matches a Place name renders *cyan and bold*,
every word that matches a Character name renders *yellow and bold*. No tagging,
no markup — your world becomes visible in the sentence as you write it.

A monospace page cannot show colour, so picture the overlay as a highlight over
the matching words. Here is the opening line we will actually write next chapter,
seen through it:

#screen(caption: "A draft line, mentions lit by the overlay")[```
  Editor · The Ninth Lantern / ch.01 / 01-cold-morning

  Mira ran the length of the Long Mole to where the
  ninth lantern stood dark, and behind her all
  Saltmarch still slept.

  lit cyan   → the Long Mole · the ninth lantern
               · Saltmarch          (Places)
  lit yellow → Mira                 (Character)
```]

Everything lit up on the first line we ever showed the tool. Three things worth
naming happened there:

- *Stemming.* We recorded `Mira Fenn`, but the prose here says just `Mira`, and
  a possessive `Mira's` two paragraphs later will light too — the matcher
  compares *stems*, keyed to the project `language` (English by default), not
  letters. This is the same machinery that makes a Russian `Москве` light an
  entry titled `Москва`.
- *Multi-word titles* match as a *run* of stems. `The Long Mole` lights the whole
  phrase, not a stray `Long`, and only where the run appears together.
- *A collision rule.* If a name were both a Place and a Character, *Place wins by
  design*. Watch for it with surnames that are also place names; `Crane` is a
  family here, not a location, so we are safe.

The overlay refreshes live as you type, again on every `Ctrl+S`, and once more
from scratch on project open — so it is always current. Add `Bryn Crane` to the
Characters book at noon and every `Bryn` in the manuscript is yellow by the time
you save.

#callout(label: "The overlay is passive")[
  It shows what you recorded; it never talks back, and it never edits your prose.
  A name that *should* light but does not is the fastest continuity check there
  is — it means the entry is missing or spelled differently. Trust the dark word.
]

#subsection("Asking the assistant, grounded in our canon")

The overlay is the passive half. The active half is the same two chords, used to
*ask*. Put the cursor in `Saltmarch` in your prose and press `Ctrl+B P`.
Inkhaven sweeps the Places book for every paragraph whose title contains your
term, builds a context block from their bodies, and — because our AI prompt bar
is empty — *arms* it and jumps focus to the prompt. The status line reads:

#screen(caption: "Status line after Ctrl+B P on `Saltmarch`")[```
  Place RAG armed for `Saltmarch` — type your question
  and Enter
```]

Type a question — "what does the town believe the lanterns are for?" — press
Enter, and the model answers from *our* text, not from whatever it half-knows
about real salt marshes. The block it receives is exactly our entry, wrapped:

#screen(caption: "The context block prepended to the question")[```
  ── Place context for `Saltmarch` (1 match(es)) ──

  ── Place: Saltmarch ──
  = Saltmarch
  A poor fishing town on a cold northern coast. Stone
  quays, a working harbour, sea-fret off the water most
  mornings. Nine lanterns ring the harbour; the creed
  is that no lantern goes dark.
  ── end place ──
```]

`Ctrl+B C` does the identical thing against the Characters book — cursor in
`Aldous`, ask "what does he know that the others do not?", and the model reasons
from Aldous's recorded entry. (Had the prompt bar already held text, the chord
would fire immediately instead of arming.) This is how you keep an assistant
inside a canon it is otherwise happy to invent around: give it the entry as
ground, and ask.

#two_track(
  [`Ctrl+B P` / `Ctrl+B C` keep the AI answering from your bible in miniature —
  the town's belief, a keeper's secret — instead of a plausible-sounding
  invention that quietly contradicts chapter nine.],
  [The same chords ground the assistant in a non-fiction *entity's* recorded
  facts — an institution's founding date, a figure's real title — so an answer
  cites the ledger you verified rather than the model's training-time guess.],
)

#recap((
  [The *Places* and *Characters* system books are seeded at `init`; you record an
  entity as a paragraph whose *title is its name*, and saving is registering — no
  separate step.],
  [We gave *The Ninth Lantern* a small map (Saltmarch, the harbour, the Long Mole,
  the nine and the ninth lanterns) and a five-name cast (Mira, Aldous, Sella,
  Toft, Bryn), each body kept deliberately thin — and left the central secret
  unwritten.],
  [A light `world.hjson` — `name` + `astronomy` + one `geography` landmark —
  declares the cold coast so "cold" is a checkable fact, without compiling a
  whole simulated world.],
  [Mentions light up live in the editor — *cyan* Places, *yellow* Characters —
  with *stemming* keyed to the project `language`, multi-word titles matched as a
  run, and Place winning any collision.],
  [`Ctrl+B P` / `Ctrl+B C` sweep the matching book and *arm or fire* a question
  grounded in your own canon; the same act of filling these books switches on the
  continuity, voice, and knowledge readers we meet later.],
))
