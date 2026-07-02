#import "../design.typ": *

#chapter(number: 4, title: "Structured Facts and Real Places")

In Part I you kept a fact that began life as a model's guess — the bottom rung.
It worked, but its provenance admitted the truth: *this is only a guess.* This
part is about starting higher. Instead of asking a model to recall a fact, you
ask an authoritative source to *hand you one*, already carrying a citation.

There are several such sources, and the rest of this part introduces them one at
a time. Here is the family:

#sources_fan()

We begin at the top of that fan — the *structured* sources — because they are the
firmest of the ones you look up, and because they show most clearly what
"authoritative" buys you.

#section("Prose versus a datum")

Most of what you find when you research is *prose*: a paragraph that mentions the
fact you want, surrounded by other words, written by someone with a point of
view. You have to read it, extract the fact, and trust the writer.

A structured source gives you something different: a *datum*. Not "an article
that discusses the founding of Rome," but the discrete, identified statement
*Rome — inception — 753 BCE*, attached to a stable identifier that a reader can
resolve independently.

#term("Structured data")[
  *Structured data* is information stored as discrete, labelled facts rather than
  free-flowing prose — subject, property, value — each with a stable identifier.
  Because a structured fact is already broken into its parts and carries an id,
  there is nothing to misread and everything to cite.
]

This is why the structured rung sits so high on the ladder. There is no paragraph
to misinterpret, no author's slant to see past. The fact arrives pre-checked and
pre-cited.

#section("Wikidata: facts with an id")

The first structured source is *Wikidata* — a vast, curated database of facts
about people, places, works, events, and things, each identified by a code. You
reach it with a command:

```
/wikidata Roman aqueduct
```

The Assistant looks the entity up and shows you a compact card: the thing's name,
a short description, and its key properties as clean subject–property–value lines
— cited by the entity's id.

#term("Q-id")[
  Every Wikidata entity has a *Q-id* — a stable code like `Q220` (the city of
  Rome). It never changes, so a fact cited by Q-id can be re-checked years later.
  When you keep a `/wikidata` fact, that Q-id becomes its provenance.
]

Take a fact from that card — `/fact`, exactly as before — and something new
happens at the gate: because the source is structured and authoritative, the fact
*skips the check*. There is nothing to fact-check; a Q-id-backed datum is already
a verifiable fact, not a guess to be second-guessed. It lands in your Facts with
its provenance reading `wikidata`, several rungs above where the same fact would
have started from the model.

#callout(label: "Why not just use an encyclopedia?")[
  Inkhaven deliberately grounds on Wikidata's *per-statement data*, not on
  encyclopedia prose. Narrative articles carry well-documented editorial slant —
  in politics, economics, history — that discrete triples do not. The tool
  grounds on the fact, not the story told around it. It is a small design choice
  that keeps the low-slant sources doing the load-bearing work.
]

#section("GeoNames: the world's places")

The second structured source is a *gazetteer* — a dictionary of real places.
Fiction and non-fiction alike lean constantly on geography: where a town is, what
region it sits in, how large it was, what kind of place it is. `/geonames`
answers all of that from the GeoNames database:

```
/geonames Rome
```

You get a card — the place's name, its region and country, its type (*capital of
a political entity*, *populated place*, *stream*), its coordinates, its population
— cited by a GeoNames id. Like Wikidata, it is structured, so a `/fact` from it
skips the check and records `geonames` provenance.

#term("Gazetteer")[
  A *gazetteer* is a geographical dictionary: a database of place names with their
  locations, administrative regions, feature types, and populations. It answers
  "where is this, and what kind of place is it?" with real coordinates rather than
  a guess.
]

Coordinates are quietly powerful. Once a place is a fact with real latitude and
longitude, later chapters can *compute* with it — the distance between two towns,
whether a day's travel between them is plausible — turning a looked-up place into
a rung-topping computed fact.

#two_track(
  [Ground your invented map in a real one. Give your fictional city the
   coordinates and climate-band of a real analogue, and its distances and seasons
   will quietly behave — the reader feels a world that holds together, without
   ever seeing a source.],
  [Every place-name in your text becomes citable — region, type, population, all
   pinned to a GeoNames id a reader can resolve. Your geography stops being
   assertion and becomes evidence.],
)

#callout(label: "One-time setup for GeoNames")[
  GeoNames asks for a *free username* (a one-time registration, not a paid key).
  Until you set `research.geonames.username`, `/geonames` politely reports that it
  is unavailable — nothing breaks, the command simply waits for its username.
  Wikidata needs no setup at all.
]

#section("Starting higher")

Notice what changed between Part I and this chapter. The *gesture* is identical —
ask, `/fact`, confirm — but the fact now begins its life several rungs up, cited,
and gate-skipped, because the source vouched for it. You did less second-guessing,
and you got a citation you never had to write.

The structured sources cover an enormous amount of what a book needs: dates,
identities, relationships, places, populations. But not everything is a datum in
a database. For claims that live in the *literature* — findings, studies,
arguments — you want the scholarly rung. That is the next chapter.

#recap((
  [Authoritative sources *hand you a cited fact* instead of a paragraph to
   interpret — starting it high on the ladder.],
  [*Structured data* is discrete labelled facts with stable ids; there is nothing
   to misread and everything to cite.],
  [`/wikidata` returns facts by *Q-id*; a `/fact` from it skips the check and
   records `wikidata` provenance.],
  [`/geonames` returns real places from a *gazetteer* (region, type, population,
   coordinates), cited by id; coordinates enable later computation.],
  [The workflow gesture is unchanged — *ask, `/fact`, confirm* — but the fact
   starts cited and gate-skipped.],
))
