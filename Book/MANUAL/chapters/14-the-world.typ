#import "../design.typ": *

#chapter(number: 14, title: "The World")

A long story happens somewhere. The somewhere has a sky with a certain number of
moons, a climate that makes some weeks bitter and others sweet, distances that
take real days to cross, and cities of a certain size in certain places. Get any
of it wrong and a reader who is paying attention feels the floor tilt: snow in
the desert, a moon that was full last night and full again tonight, a courier
who rides four hundred miles between breakfast and supper. Inkhaven's answer is
to let you *declare* the physics of your world once, in a single file, and then
do two things with that declaration — build the rest of the world out from it by
deterministic simulation, and watch your prose for the moment it forgets what
you declared.

This chapter is the operator's tour of that machinery: how you write the
definition, what the compiler derives from it, how the map is drawn, the
interactive worldbuilder you can shape a world in, the coherence checker for
imagined societies, and the live fact-checker that reads over your shoulder in
five languages. It is deliberately breadth-first. The full treatment — every
field, the physics behind each layer, worked worlds from scratch — lives in the
companion book *Building the World*; this chapter tells you what each piece is,
how to reach it, and how to run it.

#section("The two halves")

The whole subsystem turns on one file at the root of your project: `world.hjson`.
It is opt-in. A project without it behaves exactly as before; drop one in and the
world layer wakes up. Everything then divides into two halves that share that one
declaration.

#term("world.hjson")[
  The single file, at the project root, where you declare a world's *physics* —
  its star and planet and moons, and optionally its geology, geography,
  hydrology, economy, and any magic that breaks the rules on purpose. Only the
  world's `name` and its `astronomy` are required; every other block is optional
  with sensible defaults. Scaffold one with `inkhaven realworld new <name>`.
]

The first half is *the compiler*. Give it the definition and a seed, and it
derives a coherent world — astronomy, geology, climate, hydrology, demographics —
in five layers, then materializes the result into a *World* system book, one
chapter per layer, plus a rendered map. The second half is *the fact-checker*.
As you write, Inkhaven reads each paragraph against the world you built —
against its climate zones, its distances, its populations, its sky — and flags
the sentences that contradict it, in whatever of five languages you wrote them
in.

#callout(label: "Determinism")[
  Everything the compiler produces is a pure function of `(definition, seed)`.
  The same inputs give the same continents, the same rivers, the same cities,
  every run — driven by an in-tree SplitMix64 keyed to your seed, never the
  `rand` crate. The astronomy layer has no randomness at all; it is closed-form
  physics, re-derived each run. Keep the seed and your world — and its map — are
  reproducible forever; change it and the whole world regenerates.
]

#section("Declaring the physics")

You write the definition in HJSON — JSON with the ceremony removed: no quotes
required on keys, comments allowed, trailing commas forgiven. There is one catch
worth stating first, because it bites everyone once.

#callout(label: "The HJSON quoting rule")[
  In HJSON an unquoted string runs to the *end of the line*. So an inline enum
  value must be quoted — write `class: "G2V"` and `kind: "city"`, not
  `class: G2V`. Multi-line blocks are fine; it is only inline scalar strings
  that need the quotes. When a value comes out looking wrong, this is almost
  always why.
]

Only two blocks are load-bearing: the world's `name`, and its `astronomy`, which
is required because everything about seasons, tides, and the calendar is derived
from it. The rest are optional, and each one you add makes the world richer and
gives the fact-checker more to check against.

#screen(caption: "world.hjson — the shape of a definition")[```
{
  name: "Aldoria"
  seed: 0x1A2B3C          // int or "0x…" hex; drives every layer
  primary_language: "en"  // sets the fact-checker's language

  astronomy: {            // the only required block
    star:   { class: "G2V", luminosity_solar: 1.0, mass_solar: 1.0 }
    planet: { axial_tilt_deg: 23.4, day_length_hours: 24.0 }
    orbit:  { semi_major_axis_au: 1.0, year_length_days: 365 }
    moons:  [ { name: "Luna", period_days: 27.32 } ]
    calendar: { months: 12, month_length_days: 30, weekdays: 7 }
  }

  geology:   { generated: { plates: 7, continents: 4,
               sea_level: 0.40 } }
  geography: { landmarks: [ { name: "Caer Dath", kind: "city",
               climate_zone: "tundra", population: 27000 } ] }
  economy:   { tech_level: "medieval", currency: "silver mark" }
  magic:     { enabled: false }
}
```]

Read the blocks as a table of what each one drives:

#chord_table((
  chord_row("name / seed", "Identity; the seed drives every procedural layer."),
  chord_row("astronomy", "Required. Seasons, tides, calendar — closed-form."),
  chord_row("geology", "generated (procedural) or dem (a real heightmap)."),
  chord_row("geography", "Named regions + landmarks → Setting + gazetteer."),
  chord_row("hydrology", "Named waters + rainfall → Setting chapter."),
  chord_row("economy", "Tech / currency / trade / resources → known goods."),
  chord_row("magic", "Declared exceptions the fact-checker respects."),
))

A few blocks earn a word here because they change what the checker will accept.
A `geography.landmark` that carries a `climate_zone` becomes a *gazetteer* entry
the checker resolves by name — so it knows Caer Dath's weather even before you
compile and accept the procedural cities. The `sea_level` under `geology.
generated` is a *threshold on the generated heightmap, not a literal ocean
fraction*; the terrain is not uniform, so a value near `0.40` gives an
Earth-like, ocean-dominant world — raise it for a wetter one, lower it for more
land. And `economy.resources` (like `geology.notable_minerals`) extend the
checker's list of known goods, so trading your world's own metals is never
flagged as an anachronism.

The `magic` block is the pressure valve. When your world breaks physics on
purpose, you declare the exception so the checker respects it instead of flagging
it in every chapter forever.

#screen(caption: "A magic rule — a declared exception to the physics")[```
magic: {
  enabled: true
  rules: [
    {
      kind: "messenger_birds"
      covers: ["travel_time"]     // categories this rule excuses
      description: "Royal pelicans fly day and night with relays"
      applicable_to: { roles: ["royal_messenger"] }
    }
  ]
}
```]

Each rule names a `kind`, the categories it `covers`, and an optional
`applicable_to` scope (`roles` / `regions` / `seasons`). The checker consults the
ledger *lazily* — only after it already has a candidate warning — and a covered,
applicable rule suppresses the finding with a note rather than hiding it
silently. The rule materializes into the World book's *Magic Ledger* chapter, and
`inkhaven realworld magic` lists what is on the books.

#callout(label: "Start from Earth")[
  The repository ships a complete, heavily-commented real-Earth definition at
  `examples/realworld/Earth.hjson` — the exact Sun, Earth, and Moon, the
  Gregorian calendar, Earth-tuned geology, and a populated geography. Copy it in
  as `world.hjson` and compile to begin in a world you already recognise, then
  edit outward toward your own.
]

#section("The compiler and what it derives")

`inkhaven realworld compile` runs five layers in dependency order, each a pure
function of the ones before it. You can run one layer or all of them, print a
human summary or the full structured JSON, and — with `--materialize` — write the
result into the World system book.

#chord_table((
  chord_row("Astronomy", "Kepler year, insolation by latitude, lunar periods, tides."),
  chord_row("Geology", "seed → plates → heightmap → continents, ranges, minerals."),
  chord_row("Climate", "Zonal model → Köppen biomes, rain shadows, winds."),
  chord_row("Hydrology", "D8 flow over the terrain → rivers, lakes, watersheds."),
  chord_row("Demographics", "Carrying capacity → a rank-size hierarchy of towns."),
))

#screen(caption: "Compiling the layers from the CLI")[```
$ inkhaven realworld compile --layer demographics
$ inkhaven realworld compile --layer climate --json
$ inkhaven realworld compile --layer geology --materialize

  Geology (seed 0x1A2B3C)
    plates 7 · continents 4 · sea level 0.40
    highest cell 4210 m · 3 mountain ranges clustered
    materialized → World / Geology
    wrote assets/world/heightmap.png
```]

`--layer` is `all` (the default — the whole world) or one of `astronomy` /
`geology` / `climate` / `hydrology` / `demographics`. The astronomy layer also does one check worth
knowing about: if you *declare* an `orbit.year_length_days`, the compiler
computes Kepler's value and warns when the two disagree by more than a day —
your calendar and your orbit are allowed to drift, but you will be told they
have.

When you materialize, the writes are *idempotent* — re-running never duplicates,
it updates in place — and they land in the World system book as one chapter per
layer, plus the author-declared `Setting` chapter, the `Magic Ledger`, and the
normalized heightmap under `assets/world/`. Nothing touches your manuscript.

#subsection("Proposals and Places")

The compiler models settlements, but it never writes places into your book
behind your back. Instead its cities become *proposals* you accept or reject one
at a time. Only an accepted proposal becomes a real *Place* record, and each one
records a Place-to-World cross-reference.

#screen(caption: "The proposal queue closes the loop")[```
$ inkhaven realworld propose            # seed the queue
$ inkhaven realworld proposals list
$ inkhaven realworld proposals accept-all
$ inkhaven realworld places             # the accepted places
```]

Re-running `propose` never re-offers a site you already resolved. These accepted
Places are exactly what the fact-checker resolves place names against, which is
what closes the loop: *compile → accept cities → write → check*. In the TUI the
whole cycle is under one chord, which the next section names.

#section("The World hub — Ctrl+B W")

Every CLI operation has a home in the TUI under `Ctrl+B W`, the World hub. It
opens a read-only, scrollable overview of the world — the definition, the
compiled astronomy layer, and whether it has been materialized yet — and from
there a handful of second keys drive the whole subsystem without leaving the
editor.

#chord_table((
  chord_row("Ctrl+B W", "Open the World overview (definition + astronomy + status)."),
  chord_row("→ C", "Compile + materialize all five layers, seed the proposal queue."),
  chord_row("→ P", "Open the Place proposal queue (Enter accept · r reject)."),
  chord_row("→ F → P/B/R", "Fact-check the paragraph / book / recent edits."),
  chord_row("→ M", "Render the world map with plakat."),
  chord_row("→ S", "Toggle the idle auto slow-check (off by default)."),
))

`Ctrl+B W → C` is the one-key path through the whole compiler: it compiles and
materializes all five layers *and* seeds the proposal queue in a single step, so
a fresh world is one chord from existing. The other keys open the queue, arm the
fact-checker's scope picker, draw the map, and toggle the background slow check —
each covered in its own place below.

#section("Maps — the plakat cartographer")

`inkhaven realworld map` (or `Ctrl+B W → M`) turns the compiled layers into a
*MapSpec* — a structured description of the map — and hands it to *plakat*, a
separate cartographer you install once with `cargo install plakat`. plakat loads
the spec and skips its own map-generation AI entirely, so the drawn map stays a
pure function of the world and its seed, exactly like everything else.

#screen(caption: "Rendering the world map")[```
$ inkhaven realworld map
  features: assets/maps/world.features.png   # the rendered map
  geojson:  assets/maps/world.geojson        # coast/rivers/roads
  spec:     assets/maps/world.mapspec.json   # the emitted MapSpec
```]

Mountains come from clustering the heightfield's high cells; rivers run their
real D8 watercourse; landmarks are your largest settlements, with coastal cities
promoted to ports. plakat's resolved landmark positions are read back to *refine
each accepted Place's coordinates*, so the map and the gazetteer agree. Two flags
tune the run — `--spec-only` writes the MapSpec without invoking plakat, and
`--no-ingest` renders without touching Place coordinates.

#callout(label: "plakat is optional")[
  These are called "plakat" maps after the cartographer that draws them. The
  binary is a genuinely external dependency, and it is the *only* one Inkhaven
  reaches for. A missing plakat never fails the run — it degrades to a notice,
  and everything else about the world still works. The rendered map is a
  convenience, not a load-bearing part of the pipeline.
]

#section("The interactive worldbuilder — at a glance")

Everything above is reachable from the CLI and the `Ctrl+B W` hub, editing
`world.hjson` by hand. When you would rather *build* a world interactively —
shaping the sky, adding nations, drawing the map, watching a plausibility score
move as you go — there is a whole separate application for it: `inkhaven
worldbuilder`, a full-screen TUI that is a third sibling beside `inkhaven
research` and the linguistic workbench.

#term("The worldbuilder")[
  A companion TUI (introduced in 1.9.0) that is a *front-end* to everything in
  this chapter. Every change it makes lands in the same `world.hjson`, compiled
  by the same chain, checked by the same checker. It never generates your prose —
  the author decides, and the worldbuilder measures, validates, and records.
]

Its window is four regions: a *Facts* tree over a *World* tree down the left; a
wide right pane that cycles *Chat / Research / Map / Ledger*; a full-width *Query*
prompt; and a status bar carrying the world's name and its live *plausibility
score* (`★ NN`, with `▲` / `▼` deltas). Plain text in the Query prompt is a
question to the AI; a line that opens with a slash is a command.

#screen(caption: "inkhaven worldbuilder — the four regions")[```
┌─ Facts ◎ ─────────┬─ Map ─────────────────────────────┐
│ ▾ fact:world      │   . . ~ ~ ^ ^ ^ . .   biome minimap│
│   ◎ two moons     │   ~ ~ = = ^ M ^ ~ .   after /compile│
│ ▾ World           │   ~ . = ⌂ . . ~ ~ .                │
│   ▸ astronomy     │   . . . . § § . . .                │
│   ▸ geography     ├───────────────────────────────────┤
├───────────────────┤  Ledger · Chat · Research (Ctrl+R) │
│ Query ▸ /compile▏                                       │
├─────────────────────────────────────────────────────────┤
│ Aldoria      ★ 74 ▲2      pending: 3 edits    ? hints   │
└─────────────────────────────────────────────────────────┘
```]

The commands typed at that prompt are the shape of the whole session. A few of
the load-bearing ones: `/interview` walks a guided five-stage world interview;
`/set <dot.path> <value>` sets any `world.hjson` key, with shortcuts like
`/star`, `/tilt`, `/moon`, and `/nation` for the common ones; `/wfact` records an
author fact into the Facts book; `/compile`, `/validate`, and `/map` run the same
chain, score, and cartographer you already met; and `/roll` compares candidate
worlds on derived seeds so you can `/adopt` the one you like.

Crucially, shaping commands do not touch the file as you type. They accumulate
into a *pending delta* — accepted-but-uncommitted edits you preview with `/diff`,
undo with `/undo`, and only fold into `world.hjson` atomically with `/write`. The
plausibility score, though, moves the moment an edit is *accepted*, before it is
committed, so you can feel a change's effect before you keep it.

#subsection("The map editor")

Cycle the right pane to *Map*, press `e`, and you can draw the world directly —
and every mark is another pending `world.hjson` edit, reviewed with `/diff` and
written with `/write` like any other. A brief tool list:

#chord_table((
  chord_row("t / n", "Place a town / named landmark → geography.landmarks (⌂)."),
  chord_row("r", "Draw a river source → mouth → hydrology.rivers (≈)."),
  chord_row("g / o", "Region from the cell's biome (§) / road between towns (=)."),
  chord_row("+ / - , . ", "Raise / lower terrain under a brush · brush size."),
  chord_row("d / f", "Delete the feature under the cursor · jump to each flaw."),
))

Drawing the terrain and running `/terrain` writes a sculpted grayscale heightmap
under `assets/maps/` and points `geology.dem` at it, so the next compile rebuilds
the whole world from the shape you drew. This is only the glance; the
worldbuilder and its map editor get a chapter of their own in *Building the
World*.

#two_track(
  [For a *secondary world* — the invented planet of a fantasy or SF novel — the
  worldbuilder is where the whole setting is born: shape a plausible sky and
  geology, draw the map, and let the checker hold your chapters to it.],
  [For *history, travel, or reportage* set on Earth, copy in
  `examples/realworld/Earth.hjson` and lean on the fact-checker's real climate,
  distance, and demographics knowledge to catch a wrong season or an impossible
  journey.],
)

#section("Coherence for imagined societies — utopia")

There is a second, narrower checker for a particular kind of world: the imagined
*society* — a utopia or dystopia whose premises are supposed to hang together as
an argument. You declare its premises as tagged paragraphs in the World book —
`para:utopia-premise`, `-mechanism`, `-consequence`, `-elimination` (glyphs ⊢ ⚙
⇒ ∅) — and run the checker over them.

#screen(caption: "The utopia coherence checker")[```
$ inkhaven world utopia-check --stage 1        # chain logic (free)
$ inkhaven world utopia-check --stage 2        # pairing (explicit)
$ inkhaven world utopia-check --stage all
$ inkhaven world utopia-model                  # the extracted claims
```]

Stage 1 (chain logic) is deterministic and free; Stage 2 (pairing premises for
tension) is explicit-only because it spends LLM calls per pair; Stage 3 scans the
prose for entailment. It reasons about *logical and systemic* structure only —
does the mechanism actually produce the claimed consequence, does one premise
quietly contradict another — and leaves moral and theological coherence to the
Inner Theologian. Findings are advisory, cached in `.inkhaven/utopia.duckdb`,
surface in the `Ctrl+B Shift+C` review pass, and ground the `utopian-architect`
Socratic persona. `utopia-check` exits `1` on a chain-logic finding and `2` on an
entailment violation, so it can gate a pre-submission script.

#section("The live fact-checker")

Now the second half of the whole subsystem: the reader that watches your prose.
When a project has a `world.hjson`, a *fast track* runs automatically — pause a
few seconds on a paragraph and any findings appear in the Output pane, with no
chord and no focus stolen. Re-checking a paragraph replaces its own prior
findings, so the board never accretes stale warnings.

#chord_table((
  chord_row("travel_time", "A distance + duration implying an impossible pace."),
  chord_row("climate", "Weather at a known place against its climate zone."),
  chord_row("demographics", "A population diverging sharply from the model."),
  chord_row("astronomy", "A moon count disagreeing with the world's sky."),
  chord_row("economy", "A metal worked that the geology does not yield."),
))

Climate, demographics, and economy resolve place names through the *gazetteer* —
your accepted Places plus any `geography.landmarks` you declared — so the checker
knows which Cairo you mean and what its weather should be.

#screen(caption: "A fact-check finding in the Output pane")[```
┌─ Output · 1/1 · fact-check ─────────────────────────┐
│ ⊗ climate                                           │
│   ▌"Snow fell on Cairo for three days."             │
│    Implausible: freezing weather at Cairo, whose    │
│    climate zone is hot desert.                      │
├─────────────────────────────────────────────────────┤
│ ↑↓ select   Enter jump   a ask-AI   d dismiss       │
└─────────────────────────────────────────────────────┘
```]

From the CLI the same check runs on demand: `inkhaven fact-check --text "…"`
checks a literal string, and `--paragraph <id>` checks a stored paragraph. In the
TUI, `Ctrl+B W → F` arms a scope picker — `P` for the open paragraph, `B` for the
enclosing book, `R` for the twelve most recently edited paragraphs.

#subsection("Five languages, and graceful degradation")

The checker works in *English, Russian, Spanish, French, and German*. It detects
the paragraph's language, renders its warnings in that language, and resolves
place names in their grammatical cases — Russian `в Москве` matches `Москва`, a
German genitive matches its nominative. The detector is a built-in heuristic that
needs no model, and when it is not confident it *degrades* — rendering in English
rather than guessing wrong — and never panics. An optional enhanced parser can be
pointed at with `INKHAVEN_LANG_MODEL`, but nothing ever requires one.

#subsection("The slow track and coherence")

The fast track is pattern-based and free. For the subtle contradictions patterns
miss, `fact-check --slow` adds an LLM pass, and it is scrupulously
cost-controlled: a preflight prints the estimated tokens and the day's call
tally; a per-call soft cap (`--max-cost`, default 6000; `--force` overrides)
refuses an oversized call; a daily ceiling holds; and a missing provider or a
reached cap degrades to a notice. An opt-in *idle auto* variant runs it in the
background after about 45 seconds of quiet — toggled with `Ctrl+B W → S`, off by
default because it spends tokens.

Where a single-paragraph check asks "is this sentence possible," `inkhaven
realworld coherence <node>` asks whether a *run* of paragraphs holds together —
a character in two places without the travel between, a fact asserted then
reversed, a timeline that cannot follow. Give it a book or chapter node; it
gathers the paragraphs in document order and runs one cost-capped call, citing
the paragraph numbers.

#subsection("Timeline-aware checking")

When your project also has a *timeline*, the fast checker learns *when* a
paragraph happens, and its guesses become grounded facts. A paragraph tied to a
dated event gains a *calendar-grounded season* — snow in a paragraph the timeline
places in summer becomes a flat contradiction, not a guess — plus event-derived
travel time (a prose "three days" against a 35-day gap between the traveller's
events), a `date_coherence` check on seasonal hints like a midsummer feast, and
`co_location` for a character whose events put them in two places at once. These
run automatically, in all five languages, and respect the magic ledger exactly as
the world checks do; timeline-derived findings carry a small calendar marker in
the Output pane. Check the whole timeline at once with `inkhaven realworld
co-location`.

#section("Facts, and grounding the AI")

There is one more sense of "the world" in Inkhaven, and it is worth keeping
distinct from the simulated one. Alongside your manuscript lives a *Facts* system
book — a place to write down the settled truths of your world in plain prose,
whether or not you ever declare a `world.hjson`. These facts do two jobs: they
give the AI ground to stand on, and they give a second fact-checker something to
check against.

#term("The Facts book")[
  A system book of established truths about your world, written as ordinary
  paragraphs. Distinct from the `world.hjson` simulation: the simulation *derives*
  a world from physics; the Facts book *records* the ones you have decided,
  including the ones no simulation could produce ("the queen has no heir," "iron
  is taboo in the north").
]

`Ctrl+B Shift+X` fact-checks the open paragraph against that Facts book. It locks
the AI scope to the local paragraph, grounds the check against every established
fact — climate, geography, seasons, distances, chronology — and streams a verdict
into the AI pane, flagging any claim that contradicts the world you have written
down. (With an empty Facts book it degrades to a generic local fact-check.) When
it flags contradictions, `Ctrl+B Shift+J` cycles the editor cursor through them
one at a time, showing the violated fact on the status bar. Its mnemonic is X for
fact e#[*X*]amination.

The same facts ground the assistant proactively through the *Facts* AI scope. As
Chapter 3 described, `F9` cycles the AI scope; `Facts` is one of the *sticky*
conversation scopes, so once selected it stays selected across follow-ups. With
it active, your prompts are answered against the Facts book — the assistant
reasons from your world's established truths instead of inventing fresh ones. For
a large Facts book, `Ctrl+B Shift+S` opens a semantic search over it: type a
query, mark the handful of relevant facts, and send just those into a targeted
Facts chat, so you ground in the passage that matters rather than the whole book.

#two_track(
  [In *fiction*, the Facts book is the series bible in miniature — the settled
  truths the AI must honour and the checker must guard, so the assistant never
  quietly retcons your world mid-conversation.],
  [In *non-fiction*, it is the ledger of claims you have verified — dates,
  figures, definitions — that the assistant answers from and the checker holds
  your prose to, keeping the manuscript honest to your own research.],
)

#section("The ink.world Bund surface")

Scripts written in Inkhaven's embedded Bund language (Chapter 34) can read the
timeline-aware checker's view of the world, read-only, through the
`ink.world.fact_check.timeline.*` family. It exposes exactly what the checker
sees — the events near a point in world-time, the events for a character or a
place, the season of a point, and a paragraph's effective date — so a script can
reason about *when* things happen without re-deriving the calendar.

#chord_table((
  chord_row("events_near", "Timeline events near a point in world-time."),
  chord_row("events_for_character", "The events that place a character in time."),
  chord_row("events_for_place", "The events tied to a named place."),
  chord_row("season_for", "The calendar season at a given point."),
  chord_row("effective_date", "A paragraph's effective world-date."),
))

These are all `store_read` words — safe, side-effect-free, allowed by default in
the Bund sandbox. The utopia checker has its own small read-only surface,
`ink.utopia.*` (`model`, `findings`, `violations`, and a `suppress` action), for
scripting the coherence pass. Between them a script can fold the world's own
knowledge into any automation you build.

#recap((
  [The world layer is opt-in on a single root file, `world.hjson`, and has *two
  halves*: a deterministic *compiler* that derives a world from declared physics,
  and a *fact-checker* that reads your prose against it.],
  [Only `name` and `astronomy` are required; `geology`, `geography`, `hydrology`,
  `economy`, and `magic` are optional and each gives the checker more to check.
  Everything the compiler makes is a pure function of `(definition, seed)`.],
  [`inkhaven realworld compile` derives five layers (astronomy, geology, climate,
  hydrology, demographics); `--materialize` writes them into the *World* book, and
  settlements become *proposals* you accept into *Place* records.],
  [`Ctrl+B W` is the hub — `C` compiles all layers, `P` triages proposals, `F`
  arms the fact-check scope, `M` draws the *plakat* map, `S` toggles the idle slow
  check.],
  [`inkhaven worldbuilder` is a whole companion TUI for building a world
  interactively — pending deltas, a live plausibility score, a map editor — all
  writing the same `world.hjson`. `inkhaven world utopia-check` grades an imagined
  society's *logical* coherence.],
  [The live fact-checker flags travel, climate, demographics, astronomy, and
  economy contradictions automatically in *five languages*, degrades gracefully,
  and grows timeline-aware when a timeline exists; the *Facts* book and the `F9`
  *Facts* scope ground the AI in your world's settled truths.],
  [Bund scripts read the world through `ink.world.fact_check.timeline.*` and
  `ink.utopia.*` — read-only, sandbox-safe. The deep dive lives in the companion
  book *Building the World*.],
))
