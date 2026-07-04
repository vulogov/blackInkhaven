#import "../design.typ": *

#chapter(number: 6, title: "Weather and Water")

You have a sky and you have land. The star climbs and sets, the seasons turn,
the plates have shoved up mountains and drowned basins under a sea. What you do
not yet have is weather — and without weather, land is only a relief map. In
this chapter two more layers grow on top of the ones you have already built.
First *climate*: where the world is warm and where it is cold, where the rain
falls and where it never does. Then *hydrology*: what that rain does once it
lands — the rivers it gathers into, the lakes it fills, the valleys it makes
worth living in.

Neither layer is drawn. Both are consequences. That is the whole lesson of this
chapter, so hold it from the first line: you did not decide where the desert
goes. The sun and the mountains decided, and the compiler merely worked out what
they had already settled.

#section("Climate: sunlight, then shelter")

Climate is the marriage of two things you already have. From the astronomy comes
*sunlight* — how much of it reaches each band of latitude, which the sky layer
worked out for you as insolation. From the geology comes *shelter* — the
mountains that stand in the way of the wind and wring the water out of it before
it can travel further.

Start with the sun. Insolation is not spread evenly over a planet: the equator
faces the star almost head-on and bakes; the poles catch the same light at a
long, cold slant and it spreads thin. So temperature falls, in a smooth and
utterly predictable way, from a hot middle to two frozen ends. You did not paint
that gradient. It falls straight out of the geometry of a round world lit from
one side — the same geometry that gave you the seasons.

Now add the mountains, and the tidy picture of "warm middle, cold ends" breaks
into something far more interesting.

#term("Rain shadow")[
  The dry country on the far side of a mountain range. Wind carrying moist air
  is forced upward as it crosses the range; rising air cools, cannot hold its
  water, and drops it as rain or snow on the near, *windward* slope. By the time
  the air spills down the far, *leeward* side it is wrung dry — so the land in
  the mountains' lee is starved of rain. A great many of the world's deserts sit
  in exactly such a shadow, and yours will too.
]

This is why a desert can sit a short ride from a green and dripping forest: the
forest drinks the rain on the windward slope, and the desert lives in the
shadow behind the peaks. Temperature from the sun, moisture from the winds and
the mountains — put the two together across every cell of the world and you have
climate.

#subsection("The twelve biomes")

The compiler does not hand you back a fog of numbers. It reads the temperature
and the rainfall of each cell and names the *kind* of place they make.

#term("Biome")[
  A category of living landscape defined by its climate — its temperature and
  its rainfall together — and by the community of life that climate supports. A
  biome is not a place but a *type* of place: "hot desert" and "taiga" describe
  land you would recognise anywhere on the world, on any continent, wherever the
  same warmth and the same rain happen to meet.
]

Inkhaven's climate layer sorts the world into twelve of them. Eleven cover the
land, and one is the water that surrounds it:

#list(
  [*ice_cap* and *tundra* — the frozen and near-frozen ends of the world;],
  [*taiga* and *temperate_forest* — the cold and the mild woodlands;],
  [*temperate_grassland* and *mediterranean* — open plains, and the dry-summer
   coasts of the middle latitudes;],
  [*cold_desert* and *hot_desert* — the rain-starved lands, whether frozen or
   baking;],
  [*savanna*, *tropical_seasonal*, and *tropical_rainforest* — the warm belt, by
   rising order of rain, from grassland with scattered trees to unbroken jungle;],
  [and *ocean* — everything below the sea line, the biome that frames all the
   rest.],
)

The compiler groups the cells that share a biome into named *zones*, so you get
back not a pixel map but a legible geography: a rainforest belt here, a band of
cold desert there, the tundra fringing the ice. When you run the layer, you can
read the world's weather as a list of places, not a spreadsheet.

#note[
  `realworld compile --layer climate` runs just this layer and reports the
  temperature, rainfall, winds, and biome zones it derived. It needs the
  astronomy and the geology beneath it; if you have those, the climate follows
  with no further input from you.
]

#insight[
  Climate is not painted onto a world — it is *deduced* from it. The poles are
  cold because the light reaches them at a slant; the equator is warm because it
  faces the star; the deserts sit behind mountains because the rain fell on the
  windward slope. Every band of biome is the physics of sunlight and shelter,
  made visible. When your climate surprises you, it is not being arbitrary — it
  is telling you something true about the sky and the land you chose.
]

#tryit[
  Open `world.hjson` and nudge `star.luminosity_solar` upward — say from `1.0`
  to `1.15` — then run `realworld compile --layer climate`. A brighter star
  pours more energy into the same latitudes: the warm belt widens, the ice
  retreats toward the poles, and more water evaporates to fall as rain. You have
  just made a warmer, wetter world without touching a single biome by hand.
]

#section("Hydrology: what the rain does next")

The rain has fallen. Now follow it downhill.

Water does one honest thing: it flows to the lowest ground it can reach. The
hydrology layer takes the elevation from your geology and the rainfall from your
climate and simply lets gravity finish the job. The rule it uses is deliberately
simple, and it is worth knowing by name.

#note[
  The compiler routes water by the *D8* rule: each cell on the grid looks at its
  eight neighbours and sends its water to the single lowest one. Follow those
  little arrows downhill from every cell and they braid together — a thousand
  trickles joining into streams, streams into rivers — until the water reaches
  the sea or a basin it cannot climb out of.
]

Trace those flows and a river network draws itself. Where many cells drain into
one, you get a river; where a river has nowhere lower to go — a hollow with no
outlet — the water pools into a lake. And every river gathers its water from a
definite patch of land.

#term("Watershed")[
  All the land that drains into one river or lake — every slope whose rain, by
  the downhill rule, ends up in the same water. Watersheds are the world's
  natural divisions: a ridge line is not just scenery, it is the boundary
  between two of them, sending rain one way to one river and the other way to
  another. Realms and roads and rivalries tend to follow these divides, because
  water does.
]

Once the water has found its paths, some places along them are plainly better to
live in than others — and the compiler marks them. It flags the good sites as
*settlement priors*: a *river_mouth*, where a river meets the sea and trade and
fresh water and fish all come together; a *confluence*, where two rivers join
and the traffic of both passes; a *fertile_valley*, where a river has laid down
good soil across a sheltered floor. It does not build anything there yet. It
only notices that people would.

#note[
  `realworld compile --layer hydrology` runs this layer on top of the geology
  and climate beneath it, and reports the rivers, lakes, watersheds, and the
  settlement sites it found. Those flagged sites are the seed the next chapter
  grows cities from.
]

#pitfall[
  The commonest way a fantasy map betrays itself is a desert lying right against
  a rainforest with nothing to explain the seam. Real climate does not switch
  like a light. If two wildly different biomes share a border, the world owes you
  a *reason* standing between them — a mountain range casting a rain shadow, or a
  cold ocean current chilling one coast while the other bakes. Let the compiler
  derive your climate and this never happens by accident; if you later override a
  biome by hand, ask what physical thing draws the line, or a careful reader will
  ask it for you.
]

#question[
  Look at the climate and rivers your world grew, and ask what its weather *does*
  to the stories you mean to tell there. Do seasonal monsoons rule the calendar of
  a farming people? Does one realm suffer a winter that never quite ends? Has a
  rising sea drowned a coast, leaving a city half in the water? Weather is not
  backdrop — it is pressure. The most memorable settings are the ones whose
  climate the characters cannot ignore.
]

#section("The physical picture so far")

Step back and look at the chain. The star set the temperatures; the plates raised
the mountains; the mountains and the sun together made the climate; and the
climate's rain, running down the geology's slopes, made the rivers and lakes and
marked the places worth settling.

#layer_chain()

Every layer in that chain is a pure consequence of the ones before it. You have
now built four of the five, and the fifth is already half-decided: the hydrology
has quietly pointed at every river mouth and fertile valley where a town would
want to stand. In the next chapter you let people arrive and take them up.

#recap((
  [*Climate* is derived, not drawn: sunlight by latitude sets temperature, and
   mountains casting *rain shadows* set where the rain falls — together they sort
   the world into *biomes*.],
  [The twelve biomes run from *ice_cap* and *tundra* through the temperate and
   dry lands to *tropical_rainforest*, with *ocean* framing them all; the compiler
   groups them into named zones you can read.],
  [*Hydrology* lets the rain run downhill by the *D8* rule into rivers and lakes,
   divides the land into *watersheds*, and flags the good settlement sites —
   *river_mouth*, *confluence*, *fertile_valley*.],
  [Run them with `realworld compile --layer climate` and `--layer hydrology`;
   each grows on the layers beneath it with no further input from you.],
  [A desert beside a rainforest needs a *reason* between them — a rain shadow or a
   current — or the world has quietly contradicted itself.],
))
