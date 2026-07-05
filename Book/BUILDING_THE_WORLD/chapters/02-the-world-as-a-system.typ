#import "../design.typ": *

#chapter(number: 2, title: "A World Is a System")

The last chapter promised that most of a world can *follow* from a few choices
rather than being invented fact by fact. This chapter is about how — and the idea
underneath it is the most important one in the book. A world, built well, is not a
collection. It is a *system*: a small set of starting conditions, and a chain of
consequences that fall out of them on their own.

#section("Emergence: you set the physics, the world falls out")

Think about how a real coastline comes to be shaped the way it is. Nobody chose it.
It is the result of rock and rain and sea acting on each other over ages — an
outcome, not a decision. The same is true of almost everything in a convincing
setting: where deserts lie, which rivers are navigable, why the great cities grew
where they did. These are not facts to be authored one by one. They are
*consequences*.

#term("Emergence")[
  When complex, specific, believable detail arises on its own from a few simple
  rules and starting conditions — rather than being placed by hand. A coastline,
  a climate, a pattern of cities: each *emerges* from the conditions beneath it.
  Emergence is what lets a small definition grow into a large, coherent world.
]

This is the exact opposite of drawing a world by hand. When you draw, every
feature costs you a decision, and every decision is a chance to contradict another.
When you let the world emerge, you make only the handful of decisions that truly
belong to you — the size of the star, the tilt of the planet — and the rest arrives
already consistent, because it was *computed* from those decisions rather than
guessed alongside them.

#insight[
  Drawing a world places every detail by hand and hopes the pieces agree.
  Emergence sets a few conditions and lets the details *derive* from them, so they
  cannot help but agree. This is the whole reason a built world stays consistent
  where a binder drifts: consistency is not maintained, it is *produced* — a
  property of how the world is made, not a discipline you must keep.
]

#section("The chain of layers")

The physics of a world does not emerge all at once. It comes in *layers*, each one
computed from the layers before it. This is the spine of the physical world, and
you will spend all of Part II inside it:

#layer_chain()

#term("Layer")[
  One stage of the world's physical derivation — astronomy, geology, climate,
  hydrology, or demographics. Each layer takes the finished layers before it as
  its input and produces the next set of facts. You will meet them one at a time;
  for now, what matters is the *order*, because the order is the causation.
]

Read the chain as a story of cause. *Astronomy* comes first: a star, a planet, an
orbit, a tilt — and from these, the length of a year and the swing of the seasons.
*Geology* raises the land: plates collide, mountains rise, a sea level settles.
*Climate* is then forced by the two above it — the sun deciding how much heat
each latitude receives, the mountains deciding where the rain falls and where it
never does — and out of that come temperature, wind, and biomes. *Hydrology*
follows the climate: rain has to go somewhere, so it runs downhill into rivers,
pools into lakes, and drains whole watersheds. And *demographics* comes last,
because people settle where the earlier layers made it possible to live — at a
river's mouth, in a fertile valley, where water and land and climate agree.

Nothing in this chain is arbitrary. Each layer is a *pure consequence* of the ones
before it: change the tilt of the planet and the seasons shift, which moves the
climate bands, which redraws the rivers, which relocates the cities. That is
emergence made concrete — and it is why a world built this way is grounded all the
way down, exactly as the last chapter promised.

#section("The same seed, the same world")

For this to be trustworthy, it must be *repeatable*. If the same starting
conditions gave you a different world each time you asked, none of the consistency
would be worth anything. They do not. The system is deterministic, anchored by the
seed you met in the introduction.

#term("Deterministic")[
  Producing exactly the same result every time, given the same input. The world
  compiler is deterministic: the same definition, with the same *seed*, always
  grows precisely the same world — the same coastlines, the same rivers, the same
  cities in the same valleys. Nothing is left to chance that you did not choose.
]

Determinism is what turns a world from a happening into an object. Because the same
seed always yields the same world, your entire setting is captured by a short,
shareable definition: hand someone your seed and your choices, and they get your
world back, whole. And when you deliberately *change* the seed, you are not
patching one detail — you are asking the whole system for a different world grown
by the same laws. Every world it can give you is internally consistent; the seed
simply chooses which of them you get.

#section("The two hands")

Emergence is powerful, but it is not the whole of worldbuilding — and it would be a
poor tool if it were. Some things should never be left to physics. The name of a
city is not a consequence of rainfall. Your world's economy, and the rules of its
magic, are *yours* to declare. So a built world is made with two hands:

#two_hands()

With one hand you set the *physics* and let the world emerge — climate, rivers,
where people settle. With the other you *declare* what physics has no opinion
about — the names on the map, the shape of the economy, the exceptions your magic
makes to the ordinary rules. Neither hand overrules the other. The emerged world
gives your declarations somewhere true to stand; your declarations give the
emerged world meaning and intent. Later chapters return to this line often — where
exactly it falls, and how Inkhaven keeps the two from ever contradicting each
other — because knowing what to let emerge and what to declare is much of the craft.

#question[
  Look at your own world and sort it into the two hands. Which parts do you *care*
  to control by name — a particular ruling dynasty, a sacred mountain, the tongue a
  people speaks? And which parts would you be content to let *emerge*, and perhaps
  be surprised by — where the deserts fall, which river the capital sits on? There
  is no wrong answer, but there is a wasteful one: controlling by hand what you
  could have let the world grow for you, consistently and for free.
]

#recap((
  [A world is a *system*: a few starting conditions and a chain of consequences
   that *emerge* from them on their own — the opposite of drawing every detail by
   hand.],
  [The physical world derives in *layers* — astronomy, geology, climate, hydrology,
   demographics — each a pure consequence of the ones before it, so consistency is
   produced rather than maintained.],
  [The compiler is *deterministic*: the same definition and *seed* always grow the
   same world, which makes your whole setting a short, shareable, reproducible
   object.],
  [A world is built with *two hands* — what you let *emerge* from physics, and what
   you *declare* by intention (names, economy, magic). Much of the craft is knowing
   which is which.],
))
