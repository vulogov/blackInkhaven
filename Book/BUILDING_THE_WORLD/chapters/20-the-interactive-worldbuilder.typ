#import "../design.typ": *

#chapter(number: 20, title: "The Interactive Worldbuilder")

Everything so far in this book has passed through one file. You opened `world.hjson`
in an editor, typed a block, saved, and ran a command to see what the compiler made
of it. That loop is honest and it never lies to you — but it asks you to hold the
whole world in your head between edits. This chapter introduces a second way in: a
full-screen companion that keeps the world, its score, its facts, and its map all in
front of you at once, and lets you shape the world by *asking* rather than by
hand-editing HJSON.

It is called the *worldbuilder*, and you start it from your project like this:

```
inkhaven worldbuilder
```

#note[
  The worldbuilder is a *front-end*, not a replacement. Every change it makes lands
  in the same `world.hjson` the rest of this book has taught you to read, and the
  same compiler turns it into a world. Nothing here is magic you cannot also do by
  hand — the worldbuilder simply keeps the loop tight and shows you the consequences
  as you go. Your existing `world.hjson` opens unchanged.
]

#section("The shape of the screen")

The worldbuilder fills the terminal with four regions. Down the left run two trees,
one above the other: your *Facts* book on top and the *World* book below. To their
right is a single wide pane that cycles between four views. Along the bottom is the
*Query* prompt — one line where everything you type goes — and beneath it a status
bar carrying the world's name and, once there is a world to score, its plausibility.

#screen(caption: "The worldbuilder, at rest")[```
┌ Facts ──────────────┐┌ Chat ───── Ctrl+R cycles ┐
│ ▾ ◎ Tides at equinox ││ [You]                    │
│   · Founding date    ││ how long is the year?    │
│ ▸   Old calendar     ││                          │
└──────────────────────┘│ [World Builder]          │
┌ World ───────────────┐│ The compiled year runs   │
│ ▾ ⊙ Astronomy        ││ 384 planet-days …        │
│   ⊙ Geology          ││                          │
│   ⊙ Climate          ││                          │
└──────────────────────┘└──────────────────────────┘
  /interview · ask · /wfact · /research · /compile …
┌ Query ───────────────────────────────────────────┐
│ how long is the year?                             │
└───────────────────────────────────────────────────┘
 worldbuilder · Aldoria · ★ 88 ▲3 · s:default
```]

Each glyph in the trees means something. In the *Facts* tree, `◎` marks a paragraph
you have tied to the world (a #emph[world fact]); a plain `·` is any other fact. In
the *World* tree, `⊙` marks a chapter the compiler owns — you do not edit those by
hand; they are re-derived from `world.hjson` on every compile. A fold arrow (`▾`
open, `▸` closed) sits before anything with children, and the row under the cursor is
highlighted. A small pin marker before a row means you have pinned it into the AI's
context.

#term("The Query prompt")[
  The single point of entry. Plain words are a question to the worldbuilder; a line
  that begins with `/` is a #emph[command]. You never leave this prompt to work — you
  ask, you shape, you record, all from the one line. `Esc` clears the line (and, in
  an interview, steps out of it).
]

Move between the panes with `Tab`, and back with `Shift+Tab`; the cycle runs Facts →
World → Query → the right pane. The right pane cycles independently with `Ctrl+R`
through four views — *Chat*, *Research*, *Map*, and *Ledger*. Resize to taste: `{`
and `}` change how the two left trees share their height, `[` and `]` change how much
width the left column takes from the right pane. Press `?` to toggle the one-line hint
bar shown above the Query prompt; press `Ctrl+Q` to leave.

#table(
  columns: (auto, 1fr),
  stroke: none,
  column-gutter: 12pt,
  inset: (x: 0pt, y: 3pt),
  [`Tab` / `Shift+Tab`], [cycle panes (Facts → World → Query → Right)],
  [`Ctrl+R`], [cycle the right pane (Chat / Research / Map / Ledger)],
  [`j` `k` `g` `G`], [move within a tree (up / down / top / bottom)],
  [`h` `l` · `Enter`], [fold / unfold / step into a tree node],
  [`Ctrl+P`], [pin the selected node into the AI context],
  [`Ctrl+T`], [toggle the `◎` world-fact tag on a Facts paragraph],
  [`Shift+F`], [filter the Facts tree to world facts only],
  [`z`], [zoom the focused left tree to fill the column],
  [`{` `}` · `[` `]`], [resize the tree split · the column ratio],
  [`?` · `Ctrl+Q`], [toggle hints · quit],
)

#section("Start by being interviewed")

If you are beginning from nothing, do not reach for commands. Type `/interview` (or
launch with `inkhaven worldbuilder --interview`) and answer plain questions. The
worldbuilder walks five short stages — *Sky*, *Land*, *People*, *Rules*, and a
closing review — asking one thing at a time.

#screen(caption: "Mid-interview — the Sky stage")[```
┌ Chat ─────────────────────── Ctrl+R cycles ┐
│ [World Builder]                             │
│ Interview — I'll ask about the sky, land,   │
│ people, and rules. Answer in your own words │
│ (blank to skip, Esc to leave).              │
│                                             │
│ [World Builder]                             │
│ [Sky · 1/9] What kind of star? (G Sun-like  │
│ · K orange · M red dwarf)                   │
│                                             │
│ [You]  K                                    │
│ [World Builder]  recorded · star → K  (★ ▲2)│
│                                             │
│ [World Builder]                             │
│ [Sky · 2/9] Axial tilt in degrees? (Earth   │
│ 23.4 — higher means harsher seasons)        │
└─────────────────────────────────────────────┘
 interview — answer in the Query prompt · Esc to leave
```]

Every answer becomes a *pending edit* — a proposed change to `world.hjson` that has
not been committed yet — and you watch each one recorded in the conversation. A blank
line skips a question; `Esc` leaves the interview at any point without losing what you
have already answered.

#insight[
  The plausibility score at the bottom of the screen moves as you answer, so you feel
  the shape of your world firming up before you have written a single line to disk.
  When the interview ends, review everything at once with `/diff`, then commit with
  `/write`.
]

#section("Shaping by command")

Once you know your way around, the shaping commands are faster than the interview.
Each one proposes a precise edit and shows it to you for confirmation before it joins
the pending delta.

#table(
  columns: (auto, 1fr),
  stroke: none,
  column-gutter: 12pt,
  inset: (x: 0pt, y: 3.5pt),
  [`/set <path> <value>`], [set any dotted key, e.g. `/set geology.generated.sea_level 0.6`],
  [`/star <class>`], [the star's spectral class — `G`, `K`, `M`],
  [`/tilt <degrees>`], [axial tilt; higher means harsher seasons],
  [`/moon <name> [days]`], [add a moon, optionally with its period],
  [`/nation <name> [era] [kind] [traits…]`], [add a nation],
  [`/magic on|off`], [enable or disable the magic ledger],
  [`/rule <kind> <cat,cat> [description]`], [declare a magic rule (enables the ledger)],
)

Type a shaping command and the worldbuilder opens a small confirmation over the
screen, showing exactly what will change:

#screen(caption: "A shaping delta, awaiting confirmation")[```
   ┌ Confirm delta → world.hjson ──────────────┐
   │ moon Lunara                                │
   │                                            │
   │   astronomy.moons[] += {"name":"Lunara",   │
   │     "period":29.5}                         │
   │                                            │
   │ y accept (into pending) · n/Esc discard    │
   │ · then /write to commit                    │
   └────────────────────────────────────────────┘
```]

Press `y` to fold the edit into the pending delta, or `n` to discard it. Nothing
touches `world.hjson` until you say so.

#term("The pending delta")[
  The stack of accepted-but-uncommitted edits. `/diff` lists it, `/undo` drops the
  last edit, `/reset` clears it, and `/write` folds all of it into `world.hjson` at
  once — atomically. The pending delta is #emph[saved with your session], so if you
  quit mid-thought, your uncommitted edits are waiting when you return.
]

#section("Seeing what your choices imply")

Declaring a world is only half of it; the point is the *consequences*. Two commands
bring them to the surface. `/compile` runs the whole deterministic chain — astronomy,
geology, climate, hydrology, demographics — over your world as it stands (disk plus
pending edits) and reports the compiled state. From that moment the worldbuilder
reasons over the #emph[simulated] world, not merely what you declared.

`/validate` runs the plausibility lints and reports the score with every warning,
graded high, medium, or low. This is the same score that rides in the status bar; the
command spells out what is costing you points.

#screen(caption: "/compile and /validate, reported into Chat")[```
┌ Chat ─────────────────────── Ctrl+R cycles ┐
│ [You]  /compile                             │
│ [World Builder]                             │
│ Compiled world state —                      │
│ World: Aldoria                              │
│ Astronomy: 0.82 solar-mass star · year 384  │
│   planet-days · axial tilt 23.4° · 1 moon   │
│ Geology: 96×64 · 3 continent(s) · 61% sea   │
│ Climate: mean land 12.4°C · 7 biome zone(s) │
│ Demographics: 4.1M people · 12 cities …     │
│                                             │
│ [You]  /validate                            │
│ [World Builder]  Plausibility 88/100 —      │
│   1 warning(s):                             │
│   [MED ] nations: capital is landlocked     │
└─────────────────────────────────────────────┘
```]

#tryit[
  Cycle the right pane to *Map* with `Ctrl+R` and run `/compile`. The pane fills with
  an ASCII minimap drawn straight from the compiled biomes — sea, forest, desert, ice
  — with rivers and settlements stamped over it. It needs no external tool and works
  on any terminal, so you always have a picture of the world your numbers describe.
]

#screen(caption: "The Map pane, after /compile")[```
┌ Map ────────────────────────── Ctrl+R cycles ┐
│ ~~~~~~~TTTTT~~~~~~~~~~~~~~~~~~**~~~~~~~~~~~~ │
│ ~~~~TTTT###TTT≈~~~~~~~:::~~~~~~~~~~~~~~~~~~~ │
│ ~~~TTT##◉####≈TT~~~~::;;;::~~~~~~~~~~~~~~~~~ │
│ ~~~~TTTT###T≈TT~~~~::;•;;;::~~~~~~~~"""""~~~ │
│ ~~~~~~~TTTTT≈~~~~~~~::;;;::~~~~~~~~"""""""~~ │
│ ~~~~~~~~~~~~~~~~~~~~~~:::~~~~~~~~~~~"""""~~~ │
│ grid 96×64 → 44×6 · 214 river cells · 12     │
│ settlements                                  │
│ ~ sea  ≈ river  #T forest  : desert  •◉ …    │
└──────────────────────────────────────────────┘
```]

#section("Recording what is true")

Not everything about a world is physics. The name of a harbour, the reason two houses
feud, the festival that moves with the second moon — these are *facts* you decide, and
the worldbuilder records them without ever inventing them for you.

```
/wfact The tides run backwards at the autumn equinox
```

writes that statement into your Facts book, tagged so it shows with a `◎` in the Facts
tree and feeds back into the worldbuilder's own context. To find related material you
have already recorded, `/research <query>` retrieves matching Facts into the Research
pane, where a `◎` marks the ones already tied to the world.

#screen(caption: "The Research pane after /research tides")[```
┌ Research ─────────────────── Ctrl+R cycles ┐
│ query: tides                                │
│                                             │
│ ◎ Facts / Hydrology / Tides at equinox 0.71 │
│   The tides run backwards at the autumn     │
│   equinox, when both moons align.           │
│ · Facts / Calendar / Founding date     0.44 │
│   The realm was founded in the Year of …    │
└─────────────────────────────────────────────┘
```]

#pitfall[
  The worldbuilder never edits your prose and never writes fiction for you. `/wfact`
  records #emph[your] words verbatim; the AI in the Chat pane answers questions and
  points out contradictions, but the decisions — and the sentences — are always yours.
]

#section("The magic ledger")

If your world breaks physics on purpose, say so, and the fact-checker will stop
flagging it. The *Ledger* pane (cycle to it with `Ctrl+R`) shows the world's declared
exceptions and lints them live — a rule that covers no category, a ledger left
disabled, a duplicate. Declare a rule from the Query prompt:

```
/rule messenger_birds travel_time Royal pelicans fly day and night
```

That enables the ledger and adds a rule covering the `travel_time` category, exactly
as Chapter 13 described — only now you watch it appear, and its lint, in the pane.

#screen(caption: "The Ledger pane")[```
┌ Ledger ───────────────────── Ctrl+R cycles ┐
│ Magic ledger · enabled  ·  2 rule(s)        │
│                                             │
│ 1. messenger_birds  covers: travel_time     │
│    Royal pelicans fly day and night         │
│    applies: roles royal_messenger           │
│ 2. extended_lifespan  covers: character_age │
│    Sun-priests live to 200                  │
│                                             │
│ lint:                                       │
│   ! rule `seer`: covers no category         │
└─────────────────────────────────────────────┘
```]

#section("The journey, and taking it with you")

Your worldbuilding is a *record*, not just a result. Every accepted edit, every
committed write, every recorded fact is a step in the session's timeline. See it with
`/journey`, which prints each step with its plausibility arc — how the score moved:

#screen(caption: "/journey — the session timeline")[```
┌ Chat ─────────────────────── Ctrl+R cycles ┐
│ [World Builder]                             │
│ Worldbuilding Journey · default · 4 step(s) │
│   1. 2026-07-29T09:30  interview: K →        │
│      star → K · ★100→100                     │
│   2. 2026-07-29T09:31  interview: 3 →        │
│      geology.generated.continents = 3       │
│   3. 2026-07-29T09:34  /write →              │
│      committed 6 delta(s) · ★95              │
│   4. 2026-07-29T09:36  /wfact Tides →        │
│      ◎ recorded fact:world · ◎1              │
└─────────────────────────────────────────────┘
```]

Sessions are named — `inkhaven worldbuilder --session aldoria-v2` — and `/sessions`
lists them. When you want the world out of the tool and onto paper, `/export`
assembles a single readable Markdown dossier — the compiled state, the plausibility
report, the magic ledger, your recorded facts, and the whole journey — and writes it
under `exports/` in your project. It is a record you can read, share, or drop into an
appendix.

#insight[
  The worldbuilder measures, validates, and records; you decide. That is the whole
  posture of this book, made interactive. The `world.hjson` it leaves behind is the
  same file the compiler, the materialiser, and the fact-checker have read all along —
  so everything you learned in the previous nineteen chapters is exactly what the
  worldbuilder is doing on your behalf, one question at a time.
]

#recap((
  [`inkhaven worldbuilder` is a four-pane front-end to the `realworld` pipeline: Facts
   and World trees, a cycling right pane (Chat · Research · Map · Ledger), a Query
   prompt, and a live plausibility score. Every change lands in `world.hjson`.],
  [Build by *asking* — `/interview` walks Sky · Land · People · Rules; or shape
   directly with `/set`, `/star`, `/tilt`, `/moon`, `/nation`, `/magic`, `/rule`.],
  [Edits accumulate as a *pending delta* — previewed, `/diff`-able, `/undo`-able,
   saved with the session — and commit atomically with `/write`.],
  [`/compile` makes the AI reason over the simulated world; `/validate` grades the
   plausibility warnings; the Map pane draws the compiled biomes on any terminal.],
  [Record world facts with `/wfact` (`◎`, retrievable via `/research`), keep a
   `/journey`, and `/export` a Markdown dossier — the worldbuilder never writes prose
   for you.],
))
