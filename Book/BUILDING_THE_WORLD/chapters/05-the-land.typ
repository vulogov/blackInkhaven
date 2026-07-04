#import "../design.typ": *

#chapter(number: 5, title: "The Land")

Under the sky lies the land, and the land is the layer where most people expect
to do their worldbuilding — hunched over a map, drawing coastlines, penciling in
a mountain range because the story wants a hard place to cross. Inkhaven asks you
to work the other way. The *geology* layer does not ask you to draw. It asks you
for a seed, and from that seed it grows a whole planet's crust: the plates that
carry the continents, the ranges thrown up where those plates collide, the sea
level that decides how much of it stands above water, the minerals in the rock,
and the elevation of every point on the surface. You do not place the mountains.
You set the conditions, and you *read* the mountains that emerged.

#insight[
  You are not drawing coastlines. You set the seed and read what emerged — the
  same discipline as the sky, one layer down. Your job is to choose good starting
  conditions and then to *listen* to the land they produced, not to overrule it
  line by line. And when you genuinely must control the shape — a specific
  continent your story cannot do without — you do not reach for a pencil. You
  hand the layer a map.
]

#section("Two ways to make a world's land")

The geology layer will take its shape from one of two sources. By default it
generates the land from your world's `seed` — the single number that fixes every
"random" choice. The same seed always raises the same continents, so your world
is reproducible: change the seed and you get a different planet entirely; keep it
and the land is fixed forever.

#term("Tectonic plate")[
  A large rigid slab of a planet's outer shell that moves, slowly, over the hotter
  material beneath. Where plates pull apart, oceans open; where they collide, crust
  crumples upward into mountain ranges. Inkhaven models plates because they are the
  *cause* beneath the scenery — the reason a range runs where it does rather than a
  line you drew because it looked good.
]

#term("Continent")[
  A large connected mass of land standing above the sea. In Inkhaven a continent is
  not something you outline — it is what remains above `sea_level` once the plates
  have moved, the ranges have risen, and the water has found its level. You choose
  the conditions; the coastline is the consequence.
]

The second source is for when the emergent land is not what your story needs.
Instead of the seed, you can hand the layer a real heightmap — a *DEM* — and it
will build the continents from your image rather than from noise. This is the
bring-your-own-map door: the whole of Inkhaven's downstream machinery — climate,
rivers, cities — running on *your* chosen shape of land.

#term("DEM (heightmap)")[
  A *digital elevation model* — an image in which the brightness of each pixel
  encodes the height of the ground there: black for the deepest sea floor, white
  for the highest peak, greys for everything between. It is the standard way real
  geographers store terrain, and the standard way to hand Inkhaven a shape you
  drew, scanned, or exported from another tool. Point `geology.dem` at one and the
  layer reads your land instead of inventing it.
]

To use it, add the field to your geology block:

```
geology: {
  dem: "maps/my-continent.png"
  sea_level: 0.42
}
```

#note[
  The `dem` path is relative to your project, not to wherever you happen to be
  standing in the terminal — `maps/my-continent.png` means the `maps` folder
  beside your `world.hjson`. Keep the image in the project and the world stays
  portable: the definition and the map travel together.
]

#section("Compiling the land")

Grow just this layer and read what came up:

```
realworld compile --layer geology
```

You will see the plates it settled on, the continents that stood above the sea,
the mountain ranges where plates met, the sea level, the minerals distributed
through the crust, and the elevation statistics of the whole surface. Whether the
land came from your seed or from a DEM, the report reads the same — the rest of
the world neither knows nor cares which door you came in by.

#note[
  When you *materialize* the world — the full `realworld compile --materialize`
  from the introduction — the geology layer writes an actual heightmap image into
  the World system book alongside the prose. Even a seed-grown world hands you a
  picture of its terrain to keep, look at, and hand to a mapmaker.
]

#section("Why the land is a root, not a decoration")

Geology is the second physical layer, and like the sky it is a *root*: the
climate layer reads its elevation to decide where the air cools and the rain
falls, and the hydrology layer reads its slopes to run rivers downhill. A
mountain range is not scenery. It is a rain shadow, a watershed, a wall that a
climate will pile weather against on one side and starve of it on the other. When
you change the land — a new seed, a different DEM — you are not repainting a
backdrop; you are re-deciding where it rains and where the rivers run, two
chapters from now.

#question[
  Does your world's *shape* matter to your story, or only its *climate*? For many
  tales the answer is climate: you need a frozen north and a burning south and a
  temperate middle, but the exact silhouette of the coast is the story's to
  discover — so let the seed decide it. For others the map itself is a character:
  a particular strait, a sacred mountain, an island that must sit just so. Know
  which you are writing, because it tells you whether to trust the seed or reach
  for a DEM.
]

#tryit[
  Change the `seed` in your `world.hjson` by a single digit and run `realworld
  compile --layer geology` again. The continents are not nudged — they are
  *different continents*, a whole new arrangement of land and sea from one
  altered number. Try three or four seeds and keep the world whose shape you like
  best. This is worldbuilding by audition: you are not drawing the land, you are
  choosing which grown land to keep.
]

#recap((
  [The *geology* layer grows the land from your `seed` — plates, continents,
   mountain ranges, sea level, minerals, and elevation — the same land every time
   for a given seed.],
  [You do not draw coastlines; you set conditions and *read* what emerged. To
   control the shape directly, hand the layer a *DEM* via `geology.dem`.],
  [A *DEM (heightmap)* is an image encoding terrain height — the bring-your-own-map
   door — and its path is relative to your *project*.],
  [`realworld compile --layer geology` reports the land; a full `--materialize`
   writes an actual *heightmap image* into the World book.],
  [Land is a *root*, not decoration: climate and rivers are grown from its
   elevation and slopes, so changing the land re-decides the weather and the
   waters downstream.],
))
