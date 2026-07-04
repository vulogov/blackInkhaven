#import "../design.typ": *

#chapter(number: 12, title: "What You Declare")

Everything so far has come to you by *emergence*. You set a star, a planet, a
seed, and the world worked the rest out: the seasons fell out of the orbit, the
mountains out of the seed, the rivers out of the mountains, the cities out of the
rivers. You did not place any of it. That is the great economy of building a
world as a system — most of it arrives on its own, already consistent, because
each layer is a consequence of the ones before it.

But a world is not only its physics. A river the compiler carves is real, yet it
has no name until you give it one. A fertile crescent the climate produces is a
place, yet it is not *the Sunmark Vale* until you say so. Some of a world is
emergent, and some of it you *declare* — by intention, in your own hand — because
no equation was ever going to invent the word your characters use for home.

#two_hands()

#term("Declared")[
  A fact you state outright in `world.hjson`, rather than one the compiler derives.
  Emergent facts answer *what the physics produces*; declared facts answer *what
  you have decided to call it and how you mean it to work*. A world is both hands
  at once — the physics that falls out on its own, and the names and rules you set
  by intention — and the compiler honours them together.
]

#section("The declared blocks")

Three optional blocks in `world.hjson` exist for your intentions. None of them is
computed; each is read exactly as you wrote it.

`geography` is where you name the land. You declare *regions* — a named stretch of
the map — and *landmarks* — a single notable point. These are not decoration.
Their names feed the *gazetteer* (below) and the fact-checker, so that when your
prose mentions the Sunmark Vale, the world knows the place exists and roughly
where it lies. A region you never declare is, to the fact-checker, a place that is
not in the world.

`hydrology` you have already met as an emergent layer — the compiler runs the
rivers downhill on its own. But the descriptive `hydrology` block lets you *name*
the waters it found: this river is the Aldermere, that inland sea is the Bitter
Gulf. The water flows by physics; the names are yours.

`economy` is pure declaration — the compiler has no opinion on trade. Here you set
`tech_level`, the `currency` people count in, the `trade_goods` that move between
cities, and the `resources` the land is known for. These give the fact-checker a
handle on economic sense (a bronze-age realm minting steel coin is worth a flag),
and they give your scenes their texture.

#note[
  All three blocks are optional. Leave them out and the world still compiles — you
  simply have an unnamed, un-priced world. Add them a line at a time, naming only
  what your story actually reaches for, exactly as Chapter 1 counselled.
]

#section("The authority discipline")

Here is the rule that governs everything the world produces, and it is worth
stating as plainly as it can be stated: *the world proposes; the author decides*.
Nothing the compiler generates is ever written into your manuscript on its own.
The world's job is to offer; yours is to accept or refuse.

The clearest place to see this is settlements. The demographics layer produces
dozens of them — but they are not yet Places in your book. `realworld propose`
turns them into *proposals*: candidate Place entries, waiting. `realworld
proposals` lets you walk the list and accept the ones you want, rejecting the
rest. Only an accepted settlement becomes an entry in the Places book. The forty
villages the compiler found but you never accept simply stay in the simulation,
available if you need them, silent if you do not.

#propose_accept()

#term("Proposal")[
  A fact the world *offers* for your manuscript — most often a settlement put
  forward as a candidate Place entry — that becomes canon only if you accept it.
  Proposals are how the world stays generous without ever being presumptuous: it
  shows you everything it found, and writes down only what you chose.
]

#term("Authority")[
  The principle that the *author always wins*. Where a declared fact and a
  generated one disagree — you named a river the Aldermere, the generator would
  have called it something else — the declared name stands. The world is an
  advisor with an excellent memory, never a co-author with a vote.
]

#insight[
  The world proposes; you dispose. A declared name always beats a generated one,
  and a generated fact enters your book only when you accept it. This is not a
  limitation of the tool — it is the whole point of it. A world that wrote itself
  into your manuscript would be a collaborator you did not hire. This one is a
  consultant you can always overrule.
]

#section("The gazetteer")

Once you have named the land and priced the economy, you will want it all in one
place. `realworld gazetteer --output <file>` exports a *gazetteer* — a single
consolidated Markdown reference gathering the calendar, the sky, the named regions
and landmarks, the waters, the settlements, the economy, and the magic ledger into
one document you can keep as an appendix to your manuscript.

#term("Gazetteer")[
  A consolidated, human-readable reference to the whole world — every declared
  name and every headline fact in one file. Where the compiled World book is the
  world's inner workings, the gazetteer is its *index*: the page you hand a
  co-writer, or keep at your elbow, when you just need to know what everything is
  called.
]

#pitfall[
  Do not lean on the generator to keep a name you care about. If you love the name
  the compiler happened to give a river, and you do not *declare* it in
  `hydrology`, a later recompile with a changed seed or layer can quietly replace
  it — and your prose now points at a place the world no longer calls that. The
  cure is one line: declare the name, and authority makes it permanent. Anything
  you have written into your story, you should have declared into your world.
]

#question[
  Which names in your story are *load-bearing* — the ones a reader would notice
  changing? Those are exactly the ones to declare, not to leave to the generator.
  List them, and you have written most of your `geography` and `hydrology` blocks
  already.
]

#tryit[
  Add a `geography` block declaring a single region — a name and a rough location.
  Run `realworld compile`, then `realworld gazetteer --output world.md`, and open
  the file. Find your region sitting among the emergent ones. You have just put
  your own hand on the map, and the world has folded it in beside its own work.
]

#recap((
  [A world has two hands: what *emerges* from the physics you set, and what you
   *declare* by intention. The `geography`, descriptive `hydrology`, and `economy`
   blocks are where your intentions live — names, waters, and trade the compiler
   would never invent.],
  [Declared facts feed the gazetteer and the fact-checker: a region you name is a
   place the world knows; a river you name keeps its name.],
  [*The world proposes; the author decides.* Settlements become Places only through
   `realworld propose` and `realworld proposals` — accept what you want, refuse the
   rest. Nothing is written into your manuscript without your say-so.],
  [Where declared and generated disagree, the *author always wins* — so declare any
   name your story leans on rather than trusting the generator to keep it. Export
   the lot with `realworld gazetteer` as a manuscript appendix.],
))
