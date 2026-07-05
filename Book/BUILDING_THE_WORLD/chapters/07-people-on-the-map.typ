#import "../design.typ": *

#chapter(number: 7, title: "Where People Settle")

Here is the layer that turns a landscape into a world someone lives in. You have
a sky, land, a climate, and a network of rivers running down to the sea. Now
people arrive — and the question this chapter answers is not *where do you want
your cities* but *where would people actually build them*. The two answers are
rarely the same, and the difference is the whole point.

This is the *demographics* layer, and it is the last of the five physical layers.
When it finishes, the physical world is complete: everything from the star's
brightness to the population of a river-mouth port has followed, step by step,
from the handful of conditions you set at the beginning.

#section("People go where the land lets them")

No one founds a city out of nothing. A settlement needs water to drink and to
carry goods, soil that will grow more food than its people eat, and a position
worth defending or trading from. Every one of those needs is something your
earlier layers already worked out. The climate said where crops can grow. The
hydrology marked exactly the sites where water and soil and traffic meet — the
river mouths, the confluences, the fertile valleys. The demographics layer does
not go looking for good ground. It was handed the good ground already.

#term("Demographics")[
  The distribution of a world's people across its land — how many live where, in
  settlements of what size, doing what kinds of work. In Inkhaven, demographics
  is not a thing you author city by city; it is *derived* from the climate and
  hydrology beneath it, so that population follows the land's ability to feed and
  water it, the way real population always has.
]

The governing idea is that land can only support so many people, and the layer
respects that ceiling everywhere.

#term("Carrying capacity")[
  The number of people a given stretch of land can sustain, set by its climate
  and water — how much food it can grow, how reliably the rain comes, how easily
  goods can move across it. A rainforest delta and a cold desert do not have the
  same carrying capacity, and so they do not grow the same cities. The compiler
  reads capacity from the layers beneath it and lets settlements grow only as
  large as their ground allows.
]

So a fertile river valley in the warm belt can carry a great city; a patch of
tundra can carry a hunting village and no more. You did not assign those sizes.
The climate and the rivers assigned them, and the demographics layer merely
counted.

#section("A hierarchy, not a scatter")

Real settlement is not a random sprinkle of dots. A country has a few great
cities, more middling towns, and a great many small villages — and the sizes
follow a startlingly regular pattern.

#term("Rank-size hierarchy")[
  The empirical regularity that, across a region, a settlement's population tends
  to fall off in step with its rank: the second city is roughly half the largest,
  the third roughly a third, and so on down to the smallest hamlet. It means a
  believable world has *one* or two dominant centres, a modest tier of towns, and
  a broad base of villages — never a dozen equal metropolises, and never a
  capital with nothing between it and the wilderness.
]

Inkhaven's demographics layer builds exactly this shape. It ranks the settlement
sites the hydrology flagged, seats the largest population where the land can best
support it, and steps the rest down through *city*, *town*, and *village* in the
familiar falling curve. The result reads like a real country: a handful of names
you would know, a scattering you might, and a long tail of places too small to
mark. To each settlement it also attaches *role archetypes* — the kinds of people
a place of that size and setting would hold, a river port trading differently
from an upland market town — so the settlement list is already peopled, not just
counted.

#note[
  `realworld compile --layer demographics` runs this layer on top of the climate
  and hydrology beneath it, and reports the settlements it grew: each one's class
  (city, town, or village), its population, its site, and its role archetypes. It
  invents no new ground — every settlement stands on a site the hydrology already
  marked.
]

#insight[
  Cities are not *placed* — they *grow*. By the time this layer runs, the choosing
  is already done: the hydrology pointed at every river mouth and fertile valley,
  and the climate said how many mouths could feed a city. Demographics only takes
  those sites, ranks them, and lets population settle onto them in the shape real
  populations take. When you see where your largest city landed, you are not
  seeing a decision you made here — you are seeing the sum of every layer beneath
  it, come to its natural conclusion.
]

#tryit[
  Run `realworld compile --layer demographics` and read the settlement list from
  the top. Notice the shape of it: the sharp drop from the first name to the
  second, the widening tier of towns, the long tail of villages. Trace the largest
  few back to their sites — you will find them sitting on the river mouths and
  fertile valleys the hydrology flagged a chapter ago. Nothing here was placed by
  hand, and yet it reads like a country.
]

#question[
  Look at where your civilization clustered — and, just as hard, at where it did
  *not*. Every world has empty quarters: a desert interior no city will ever hold,
  a frozen north with a single lonely outpost, a mountain wall with people crowded
  on one side and nobody on the other. Those blanks are not failures of the map.
  They are where the frontier stories live, the crossings that cost something, the
  places a character disappears into. Where does your world leave room to be lost?
]

#section("The physical world is complete")

This is a threshold, so mark it. With demographics grown, all five physical
layers are in place, and each one fell out of the last:

#layer_chain()

The star set the seasons; the plates raised the land; the sun and the mountains
made the climate; the climate's rain carved the rivers; and the rivers decided
where the people stand. From a seed and a few choices you now have a *place* — a
whole physical world, warm where it should be warm, wet where it should be wet,
peopled where the land invites people and empty where it forbids them, and
consistent in every corner because none of it was guessed.

What that place does not yet have is *time* and a *people* in the fuller sense —
a past that predates page one, and nations and cultures and tongues who carry it.
Those are the work of the parts ahead: Part III gives your world a history, and
Part IV gives it civilizations. The land is built. Now you fill it with lives.

#recap((
  [*Demographics* is the last physical layer, derived from climate and hydrology:
   people settle where the land can feed and water them, never in a random
   scatter.],
  [*Carrying capacity* caps how large a settlement can grow from its climate and
   water; a fertile delta carries a city, a tundra a village.],
  [Settlements fall into a *rank-size hierarchy* — a few cities, more towns, many
   villages, with role archetypes — grown from the sites the hydrology already
   flagged.],
  [Cities are not placed but *grown*: run `realworld compile --layer demographics`
   and the largest centres land on the river mouths and valleys chosen a layer
   earlier.],
  [With this layer done the *physical* world is complete; Parts III and IV give
   it time and people.],
))
