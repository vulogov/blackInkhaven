#import "../design.typ": *

#chapter(number: 19, title: "Speech communities and ecology")

A language is spoken by *someone*, *somewhere*. The varieties of Chapter 17 and
the contact of Chapter 18 only matter because a particular town speaks the
lowland dialect and a particular character grew up there. This last chapter of the
pillar ties your languages to your world — to its places and its people — and
then renders a character's speech in their own voice.

#section("Linking languages to your world")

Inkhaven records who speaks what in a small sidecar file, separate from your prose
— your manuscript is never touched. You attach a language to a place, and record a
character's command of it:

```sh
inkhaven language link-place Tirion Eldar
inkhaven language link-character Erendil Eldar native
inkhaven language speakers Eldar
```

The first says the city of Tirion speaks Eldar; the second records that the
character Erendil is a *native* speaker (other levels mark fluent or learner
command). `speakers` then lists everyone and everywhere tied to a language. A
place may have a secondary language (`--secondary`), and a character may command
several — the picture of a multilingual world.

#term("Speech community")[
  The group of people who share a language or variety — the speakers of a town, a
  class, a guild. A *place* and a *character* are how Inkhaven anchors speech
  communities: a community is the set of speakers a language reaches.
]

#section("Which variety, exactly")

A town does not speak \"the language\" in the abstract — it speaks a *variety* of
it. And a person's speech is built on a *home* variety, the one they grew up in.
The links carry both:

```sh
inkhaven language link-place Tirion Eldar --variety lowland
inkhaven language link-character Tost Eldar native --native-variety lowland
```

Now Tirion does not merely speak Eldar; it speaks Eldar's *lowland* dialect. And
Tost's speech is grounded in that same dialect — the base of their idiolect.

#term("Native variety")[
  The variety a speaker acquired first and speaks by default — the dialect or
  register at the base of their personal *idiolect* (Chapter 17). A marsh-born
  character's native variety is the marsh dialect, however far they later travel.
]

#section("The ecology view")

With places and people tied to languages and varieties, Inkhaven can draw the
whole picture — its *language ecology*:

```sh
inkhaven language ecology                  # who speaks what, and which variety, where
inkhaven language ecology --svg atlas.svg  # a node-link atlas of the world's tongues
```

#term("Language ecology")[
  The full pattern of languages and varieties in a world — which are spoken
  where, by whom, in contact with what. The term (borrowed from biology) frames
  languages as living in an environment of speakers and neighbours, not in
  isolation.
]

The text report lays out every place with its language and variety, every
character with the languages they command and their native variety, and the
contact areas from Chapter 18. The `--svg` form writes a labelled *atlas* — a
node-link diagram of the world's languages and the places and people joined to
them — a map you can drop straight into your worldbuilding notes.

#section("Speaking as a character")

The payoff of all this wiring: render a line *as a particular character would say
it*, automatically in their native variety.

```sh
inkhaven language idiolect Tost --word kata          # → kada
inkhaven language idiolect Tost --text "kata tira"   # a whole line, in Tost's dialect
```

`idiolect` looks up the character's primary language and native variety, then runs
the text through the variety engine of Chapter 17. Because Tost's native variety
is the lowland dialect — where /t/ softens between vowels — *kata* comes out of
Tost's mouth as *kada*, with no further instruction from you. A marsh-dweller's
speech simply *arrives* in the marsh dialect. This is dialogue that knows who is
talking.

#callout(label: "It reaches the grammar book too")[
  When a language declares varieties or contact, its reference grammar (Chapter
  23) grows two new sections automatically: a *Variation* section listing the
  dialects and registers with the dialectology comparison table, and a *Contact*
  section describing its linguistic area, shared areal features, and how it
  adapts loanwords. The sociolinguistics you build here becomes part of the
  printed, published description of your language.
]

#recap((
  [Inkhaven ties languages to your world in a sidecar (your prose is untouched):
   `link-place` and `link-character`, listed by `speakers`.],
  [Places carry a `--variety` and characters a `--native-variety`, so a *speech
   community* speaks a specific dialect, not the language in the abstract.],
  [`ecology` reports the whole *language ecology* — who speaks what variety where,
   plus contact areas; `--svg` writes a node-link atlas.],
  [`idiolect <character>` renders a form or line in that character's native
   variety automatically — dialogue in the speaker's own voice.],
  [Declared varieties and contact also appear as *Variation* and *Contact*
   sections in the reference grammar (Chapter 23).],
))
