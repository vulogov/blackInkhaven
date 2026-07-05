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

To use it, add a `dem` block to your geology block. At its simplest it is just a
`path` to the image:

#hjson[```
geology: {
  dem: { path: "maps/my-continent.png" }
}
```]

The `dem` block also carries the `scale_km_per_pixel` that turns pixels into real
ground — so that later, when a rider crosses your map, the distance is true — and
the `sea_level_pixel_value`, the pixel brightness (0–255) Inkhaven reads as the
shoreline: anything at or below it is sea.

#hjson[```
geology: {
  dem: { path: "heightmap.png", scale_km_per_pixel: 5.0, sea_level_pixel_value: 40 }
}
```]

#note[
  The `dem` path is relative to your project, not to wherever you happen to be
  standing in the terminal — `maps/my-continent.png` means the `maps` folder
  beside your `world.hjson`. Keep the image in the project and the world stays
  portable: the definition and the map travel together.
]

#subsection("Producing a heightmap")

You do not need special software or any artistic skill to make a DEM — it is an
ordinary greyscale image, and a free image editor is enough. Before the recipe,
three facts about how Inkhaven reads the file, so nothing about it is a guess:

#list(
  [*Any common image format works* — PNG, JPEG, TIFF, BMP. Prefer *PNG*: it is
   lossless, so it will not smear your coastlines the way JPEG can.],
  [*The size does not matter.* Inkhaven resamples your image onto its own grid, so
   a 512×512 or 1024×512 picture is plenty; you do not need to match any exact
   dimension.],
  [*Brightness is height, and it is read relatively.* Inkhaven finds the darkest
   and brightest pixels in your image and stretches that range to fit — your
   darkest pixel becomes the deepest sea floor, your brightest the highest peak.
   So use the *whole* range from black to white; the absolute grey values do not
   matter, only which areas are darker than which.],
)

#subsection("Drawing one by hand, step by step")

This is the most direct road — you get exactly the continents you imagined. Using
*GIMP* (free, from `gimp.org`; the same steps work almost unchanged in Krita or
Photoshop):

#list(
  [*Make a new greyscale image.* `File → New`, set the size to `1024 × 512`
   pixels, then `Image → Mode → Grayscale`.],
  [*Fill it with black* — this is your open ocean. `Edit → Fill with FG Color`
   with the foreground set to black.],
  [*Paint your land in white.* Take a soft-edged brush, set the colour to white
   and the brush opacity low (about 20%), and paint where you want land. Build
   the brightness up in passes: one pass for coastal lowland (dim grey), more
   passes stacked in the interior for hills, brightest of all along the spines
   where you want mountains. Think of the brush as *raising* the ground each time
   you stroke.],
  [*Smooth the slopes.* `Filters → Blur → Gaussian Blur` with a radius of about
   `20` pixels. This is the important step (see the Pitfall): it turns your
   painted patches into gentle grades that rivers can run down.],
  [*Export it.* `File → Export As…`, name it `my-continent.png`, and save it into a
   `maps` folder next to your `world.hjson`.],
)

Then point the world at it, exactly as shown above:

#hjson[```
geology: {
  dem: { path: "maps/my-continent.png" }
}
```]

Compile, and Inkhaven runs the entire climate-and-rivers machine over *your* land.

#subsection("Where the coastline falls")

The simplest way to set the sea is to *not* set it: leave `sea_level_pixel_value`
out, and Inkhaven puts the shoreline at 40% up your height range — the lowest 40%
of the land becomes ocean. To get more sea, paint more of your image dark; for
more land, paint more of it bright. That trial-and-error is usually all you need.
(If you want to place the coast at an exact brightness, `sea_level_pixel_value`
takes a number from `0` to `65535`, where `0` is black and `65535` is white;
everything at or below it is sea.)

#subsection("Two shortcuts")

If drawing is not your strength, two roads hand you a heightmap ready-made:

#list(
  [*Generate one.* Free terrain tools — *Wilbur*, or Blender's built-in *A.N.T.
   Landscape* generator (and the paid *World Machine* and *Gaea*) — grow realistic
   erosion, ranges, and valleys from noise and export a greyscale heightmap
   directly. Good when you want plausible terrain without placing every ridge.],
  [*Borrow the real Earth.* The website `tangrams.github.io/heightmapper` lets you
   pan to any real region and download its terrain as a greyscale PNG; *QGIS* (free)
   can do the same over public SRTM elevation data. A quiet way to give a fantasy
   map the bones of a real coastline.],
)

#pitfall[
  A crisp, high-contrast image with hard edges makes for cliff-walled, unnatural
  terrain and confused rivers. Real land is smooth: always blur your heightmap
  (the Gaussian Blur step above), keep the transitions gradual, and let the sea
  meet the land along a soft grey coast, not a black-to-white wall. If your rivers
  come out strange, the usual cure is *more blur*.
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
  Change the `seed` in your `world.hjson` by a single digit and run `realworld compile --layer geology` again. The continents are not nudged — they are
  *different continents*, a whole new arrangement of land and sea from one
  altered number. Try three or four seeds and keep the world whose shape you like
  best. This is worldbuilding by audition: you are not drawing the land, you are
  choosing which grown land to keep.

  #hjson[```
  seed: 0x5eed2
  ```]
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
