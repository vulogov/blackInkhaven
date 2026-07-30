#import "../design.typ": *

#chapter(number: 20, title: "The Interactive Worldbuilder")

Everything so far in this book has passed through one file. You opened `world.hjson`
in an editor, typed a block, saved, and ran a command to see what the compiler made
of it. That loop is honest and it never lies to you — but it asks you to hold the
whole world in your head between edits. This chapter introduces a second way in: a
full-screen companion that keeps the world, its score, its facts, and its map all
in front of you at once, and lets you shape the world by *asking* rather than by
hand-editing HJSON.

It is called the *worldbuilder*, and you start it from your project like this:

```
inkhaven worldbuilder
```

#note[
  The worldbuilder is a *front-end*, not a replacement. Every change it makes lands
  in the same `world.hjson` the rest of this book has taught you to read, and the
  same compiler turns it into a world. Nothing here is magic you cannot also do by
  hand — the worldbuilder simply keeps the loop tight and shows you the
  consequences as you go. Your existing `world.hjson` opens unchanged.
]

#section("The four panes")

The screen is divided into four regions. Down the left run two trees, one above the
other: your *Facts* book on top and the *World* book below. To their right is a
single wide pane that cycles between four views. Along the bottom is the *Query*
prompt — one line where everything you type goes — and beneath it a status bar that
carries the world's name and, once there is a world to score, its plausibility.

#term("The Query prompt")[
  The single point of entry. Plain words are a question to the worldbuilder; a line
  that begins with `/` is a *command*. You never leave this prompt to work — you ask,
  you shape, you record, all from the one line.
]

Move between the panes with `Tab` (and back with `Shift+Tab`). The right pane cycles
independently with `Ctrl+R` through four views: *Chat*, *Research*, *Map*, and
*Ledger*. Resize as you like — `{` and `}` change how the two left trees share their
height, `[` and `]` change how much width the left column takes from the right pane.
Press `?` to toggle the one-line hint bar; press `Ctrl+Q` to leave.

#section("The interview")

If you are starting from nothing, do not reach for commands. Type:

```
/interview
```

The worldbuilder walks you through five short stages — *Sky*, *Land*, *People*,
*Rules*, and a closing *Review* — asking one plain question at a time. What kind of
star? How many continents? Is there magic? You answer in your own words in the Query
prompt; a blank line skips a question, and `Esc` leaves the interview at any point
without losing what you have already answered. You can also open straight into it
with `inkhaven worldbuilder --interview`.

#insight[
  Every interview answer becomes a *pending edit* — a proposed change to
  `world.hjson` that has not been committed yet. The score at the bottom of the
  screen moves as you answer, so you feel the shape of your world firming up before
  you have written a single line to disk. When the interview ends, review everything
  at once with `/diff`, then commit with `/write`.
]

#section("Shaping by command")

Once you know your way around, the shaping commands are faster than the interview.
Each one proposes a precise edit and shows it to you for confirmation before it joins
the pending delta:

#table(
  columns: (auto, 1fr),
  stroke: none,
  inset: (x: 0pt, y: 4pt),
  [`/set <path> <value>`], [Set any dotted key, e.g. `/set geology.generated.sea_level 0.6`],
  [`/star <class>`], [The star's spectral class — `G`, `K`, `M`],
  [`/tilt <degrees>`], [Axial tilt; higher means harsher seasons],
  [`/moon <name> [days]`], [Add a moon, optionally with its period],
  [`/nation <name> [era] [kind] [traits…]`], [Add a nation],
  [`/magic on|off`], [Enable or disable the magic ledger],
  [`/rule <kind> <cat,cat> [description]`], [Declare a magic rule (enables the ledger)],
)

Each shaping command opens a small confirmation: press `y` to fold the edit into the
pending delta, or `n` to discard it. Nothing touches `world.hjson` until you say so.

#term("The pending delta")[
  The stack of accepted-but-uncommitted edits. `/diff` lists it, `/undo` drops the
  last edit, `/reset` clears it, and `/write` folds all of it into `world.hjson` at
  once — atomically. The pending delta is *saved with your session*, so if you quit
  mid-thought, your uncommitted edits are waiting when you return.
]

#section("Seeing what your choices imply")

Declaring a world is only half of it; the point is the *consequences*. Two commands
bring them to the surface. `/compile` runs the whole deterministic chain —
astronomy, geology, climate, hydrology, demographics — over your world as it stands
(disk plus pending edits) and reports the compiled state: the real year length, the
sea coverage, the mean climate, the rivers, the population. From that moment the
worldbuilder reasons over the *simulated* world, not merely what you declared.

`/validate` runs the plausibility lints and reports the score with every warning,
graded high, medium, or low. This is the same score that rides in the status bar; the
command spells out what is costing you points.

#tryit[
  Cycle the right pane to *Map* with `Ctrl+R` and run `/compile`. The pane fills with
  an ASCII minimap drawn straight from the compiled biomes — sea, forest, desert, ice
  — with rivers and settlements stamped over it. It needs no external tool and works
  on any terminal, so you always have a picture of the world your numbers describe.
]

#section("Recording what is true")

Not everything about a world is physics. The name of a harbour, the reason two
houses feud, the festival that moves with the second moon — these are *facts* you
decide, and the worldbuilder records them without ever inventing them for you.

```
/wfact The tides run backwards at the autumn equinox
```

writes that statement into your Facts book, tagged so it shows with a `◎` in the
Facts tree and feeds back into the worldbuilder's own context. To find related
material you have already recorded, `/research <query>` retrieves matching Facts into
the Research pane, where a `◎` marks the ones already tied to the world.

#pitfall[
  The worldbuilder never edits your prose and never writes fiction for you. `/wfact`
  records *your* words verbatim; the AI in the Chat pane answers questions and points
  out contradictions, but the decisions — and the sentences — are always yours.
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

#section("The journey, and taking it with you")

Your worldbuilding is a *record*, not just a result. Every accepted edit, every
committed write, every recorded fact is a step in the session's timeline. See it with:

```
/journey
```

which prints each step with its plausibility arc — how the score moved — in the Chat
pane. Sessions are named (`inkhaven worldbuilder --session aldoria-v2`); list them
with `/sessions`.

When you want the world out of the tool and onto paper, `/export` assembles a single
readable Markdown dossier — the compiled state, the plausibility report, the magic
ledger, your recorded facts, and the whole journey — and writes it under `exports/`
in your project. It is a record you can read, share, or drop into an appendix.

#insight[
  The worldbuilder measures, validates, and records; you decide. That is the whole
  posture of this book, made interactive. The `world.hjson` it leaves behind is the
  same file the compiler, the materialiser, and the fact-checker have read all along
  — so everything you learned in the previous nineteen chapters is exactly what the
  worldbuilder is doing on your behalf, one question at a time.
]
