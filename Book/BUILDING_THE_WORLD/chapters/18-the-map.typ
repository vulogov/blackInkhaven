#import "../design.typ": *

#chapter(number: 18, title: "Drawing the Map with plakat")

You have built a world that knows where its mountains rose, where its rivers reach
the sea, and where its people settled. All of that is *shape* — coordinates and
elevations and biomes held in the compiler. This chapter turns that shape into a
picture: a labelled, painted map you can pin to the wall, drop into an appendix,
or keep at your elbow while you write. Inkhaven does not draw the map itself. It
hands the work to a companion tool built for exactly this — *plakat* — and the two
pass a world back and forth. By the end of this chapter you will have rendered your
own world three different ways, and you will understand every step in between.

If you met plakat back in the chapter on the land, where you used it to *grow* a
heightmap, this is the other half of the partnership: there you brought terrain
*in*; here you send a finished world *out* to be drawn.

#term("MapSpec")[
  The bridge between the two tools — a single JSON file describing your world's
  cartography: its coastline and mountains, its rivers, its regions, its labelled
  places, and the roads between them. Inkhaven *emits* a MapSpec from your compiled
  layers; plakat *reads* it and draws. Because the spec is a pure function of your
  world and its seed, the same world always yields the same map, and you can keep
  the spec as a small, portable description of your map that renders identically on
  any machine.
]

#section("Getting plakat")

plakat is a separate program — a Rust binary you install once. If you have the
Rust toolchain, the simplest route is:

```
cargo install plakat
```

That puts a `plakat` command on your `PATH`. Check it answers:

```
plakat --version
```

Inkhaven runs plakat *for* you when it needs to, and it always checks for it first
with exactly that command — so if the map commands ever tell you plakat is missing,
this is the line to run. Everything else in this chapter assumes `plakat` is
installed and on your `PATH`.

#note[
  plakat is *optional*. Nothing in the rest of the book needs it — your world
  compiles, materialises, and fact-checks with Inkhaven alone. The map is a
  bonus at the end of the road, not a dependency along it. If you never install
  plakat, you simply never render a picture; the world is no less complete.
]

#section("One command, one map")

The fastest path from a compiled world to a drawn map is a single command. From a
project with a `world.hjson`, run:

```
inkhaven realworld map
```

Inkhaven compiles every layer, assembles a MapSpec from the geology, climate,
rivers, and settlements — together with the Places you have accepted and any
realm capitals and trade routes — writes it to `assets/maps/world.mapspec.json`,
and then calls plakat to draw it. Out come a rendered image and a GeoJSON of the
features, all under your project. Here is a world called *Aeloria*, rendered
exactly this way:

#align(center, image("../images/plakat-parchment.png", width: 52%))
#figure_note[Aeloria, rendered by `realworld map` — coastline, mountains, rivers, and every accepted settlement, drawn from the compiled world. The compass, scale bar, and legend are plakat's; everything they point at is yours.]

Every mark on that map came from a layer you have already met. The coastline is
where geology's sea level cut the heightmap; the shaded relief is the heightmap
itself; the blue threads are hydrology's rivers, running downhill to the sea; the
labelled dots are the settlements demographics placed, sized by population. Nothing
was invented for the picture — the picture is the world, seen from above.

#section("Choosing a look")

`realworld map` draws with plakat's default *parchment* style and sensible
defaults. When you want to choose the look yourself — a different palette, a
season, a printed edition — ask Inkhaven for the *spec alone* and drive plakat by
hand:

```
inkhaven realworld map --spec-only
```

That writes `assets/maps/world.mapspec.json` and stops, without calling plakat.
Then you render the spec yourself, choosing a style:

```
plakat map --map-spec assets/maps/world.mapspec.json --map-render aeloria.png --map-style inked --seed 7
```

plakat ships three cartographic styles. The *same* Aeloria, the *same* spec and
seed, drawn three ways:

#grid(
  columns: (1fr, 1fr, 1fr),
  gutter: 6pt,
  image("../images/plakat-parchment.png"),
  image("../images/plakat-inked.png"),
  image("../images/plakat-blueprint.png"),
)
#figure_note[`--map-style parchment` · `inked` · `blueprint`. One world, three atmospheres: the aged chart, the clean line drawing, the surveyor's plan.]

The `--seed` is plakat's own drawing seed — it fixes the little cartographic
choices (how a coastline wobbles, where a label sits) so the same command always
draws the same picture. Change it and the terrain shifts slightly; keep it and
your map is reproducible. A *season* recolours the land — the same Aeloria under
`--map-season winter` trades its summer greens for snow:

#align(center, image("../images/plakat-winter.png", width: 46%))
#figure_note[`--map-season winter`. The palette follows the season; the geography does not move.]

#note[
  A few more knobs worth knowing: `--map-render-sd <PATH>` paints the map with a
  diffusion model instead of drawing it flat; `--map-grid N` overlays an N×N
  tabletop coordinate grid (A1/B2…); `--map-tiles CxR` with `--map-render-tiles`
  slices the world into a grid of seamless tiles that stitch back together, for a
  large printed or virtual-tabletop map. `--map-export-svg` writes a vector
  version. Run `plakat map --help` for the full set.
]

#section("What the map shows, and where it came from")

It is worth naming every kind of mark, because each is a layer of your world made
visible — and each is something you can *change* by changing the world:

#list(
  [*Relief and coast* — the geology heightmap and its sea level. Grow a different
   DEM, or change `geology.generated.sea_level`, and the land itself redraws.],
  [*Rivers* — hydrology's flow model, running from source to sea. Declare a named
   course under `hydrology.rivers` and it is labelled.],
  [*Settlements* — the towns and cities demographics placed, and the Places you
   accepted, sized by population. A coastal city is drawn as a *port*.],
  [*Realm capitals and trade roads* — each realm's seat is a hub, and the routes
   from `realworld trade` are drawn as roads (land) or sea lanes between them.],
  [*Declared landmarks* — any `geography.landmarks` entry you gave a `lat`/`lon`
   (or `x`/`y`) is drawn where you placed it, labelled by name.],
)

So the map is not a separate artefact you maintain by hand — it is a *view*. Every
time you recompile, the map can be redrawn to match, and it will always agree with
the world your prose is checked against, because it is drawn from the same numbers.

#section("The other direction: growing terrain from a heightmap")

plakat can also hand a world *to* Inkhaven. Its most useful gift is a *heightmap* —
a grayscale image where brightness is elevation, bright peaks down to black sea.
plakat will dump one from any spec:

```
plakat map --map-spec assets/maps/world.mapspec.json --map-dump-heightmap heightmap.png --seed 7
```

#align(center, image("../images/plakat-heightmap.png", width: 34%))
#figure_note[A plakat heightmap (DEM): brightness is elevation. This is not a picture *of* a world — it is the *shape* a world grows from.]

To build your world's land *from* that image rather than from a seed, drop it into
your project and point the `geology.dem` block at it:

#hjson[```
geology: {
  dem: {
    path: "maps/heightmap.png"
    scale_km_per_pixel: 5.0
    sea_level_pixel_value: 40
  }
}
```]

Now `realworld compile --layer geology` reads the heightmap as your terrain, and
every layer above it — climate, rivers, cities — grows over *that* shape instead of
a generated one. This is how a coastline you drew, or generated in plakat, or
lifted from real elevation data, becomes the ground your whole world stands on. The
chapter on the land walks this path in full; here it completes the loop — a shape
that left as a spec can come back as terrain.

#pitfall[
  A heightmap is only a shape — it has no names. Bringing one in gives you a
  coastline and mountains, but the rivers, regions, and cities are still grown by
  the layers above, and the *names* are still yours to declare or accept. Do not
  expect a plakat-drawn map's labels to survive the round-trip into `geology.dem`;
  the elevation survives, the lettering does not.
]

#section("The round-trip, closed")

There is one quiet exchange worth understanding, because it keeps your two
pictures of the world in agreement. When `realworld map` renders, plakat resolves
each labelled place to a precise position on the drawn map. Inkhaven reads those
resolved positions back and uses them to *refine the coordinates* of your accepted
Places — so the place on the map and the Place in your book sit at exactly the same
spot. It happens automatically; pass `--no-ingest` if you would rather the map not
touch your Places' coordinates.

#insight[
  Hold the whole loop in your head, because it is the point of the partnership.
  *Inkhaven → plakat*: your compiled world becomes a MapSpec, and plakat draws it.
  *plakat → Inkhaven*: a heightmap becomes your `geology.dem`, and named features
  become declared regions, rivers, and landmarks. Each tool does what the other
  cannot — Inkhaven runs the physics and grows the life; plakat draws the shapes
  and paints the picture — and the world passes between them without ever losing
  its truth.
]

#tryit[
  In a project with a compiled world, run `inkhaven realworld map` and open the
  image it wrote under `assets/maps/`. Then run `realworld map --spec-only` and
  render the spec yourself three times, once each with `--map-style parchment`,
  `inked`, and `blueprint`. Pin the one you like. You have just turned a definition
  a few lines long into a map of a whole world — and you can redraw it, exactly,
  any time the world changes.
]

#recap((
  [Your compiled world can become a *drawn map*. Inkhaven emits a *MapSpec* (a pure
   function of the world + seed); the companion tool *plakat* reads it and draws.
   Install plakat with `cargo install plakat`; it is optional.],
  [`realworld map` does it in one command — compile, emit the spec, call plakat,
   write the image + GeoJSON under `assets/maps/`. `--spec-only` writes just the
   spec so you can drive plakat yourself.],
  [`plakat map --map-spec … --map-render … --map-style parchment|inked|blueprint`
   renders the spec; `--map-season`, `--map-grid`, `--map-tiles`, and
   `--map-render-sd` shape the look. A `--seed` keeps the drawing reproducible.],
  [Every mark on the map is a layer made visible — relief, rivers, settlements,
   trade roads, declared landmarks — so the map always agrees with the world your
   prose is checked against.],
  [The reverse direction: `--map-dump-heightmap` writes a DEM you feed back through
   `geology.dem` to grow terrain from a shape. And the round-trip refines your
   accepted Places' coordinates from the drawn map (`--no-ingest` to skip).],
))
