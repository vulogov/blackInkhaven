#import "../design.typ": *

#chapter(number: 21, title: "The Map Editor")

Chapter 18 sent your world *out* to plakat to be drawn; Chapter 20 gave you the
worldbuilder, where the Map pane shows that drawing at your desk. This chapter joins
the two: it turns the Map pane into a place you *draw* — set a town where you want a
town, run a river to the sea, sculpt a coastline — and watch the world take your
edits. Nothing here leaves the model you already know. Every mark you make is an
ordinary `world.hjson` declaration; the map editor is a spatial front-end to the same
`geography` and `hydrology` blocks you could type by hand, and the same compiler turns
them into a world.

#note[
  The map editor lives inside `inkhaven worldbuilder`. Cycle the right pane to *Map*
  with `Ctrl+R`, then press `e` to edit. You need a compiled world first — run
  `/compile` (or `/map`) so there is terrain under the cursor. Every placement is a
  *pending edit*: review the batch with `/diff`, commit with `/write`.
]

#section("The cursor")

Press `e` and a crosshair appears on the map. Move it with `hjkl` or the arrow keys —
a coarse step for travel, `Shift` for a single cell — and a readout tells you exactly
where you are and what is under you: the grid coordinate, the biome, the elevation,
and the name of any feature on that cell.

#screen(caption: "Edit mode — the cursor over a placed town")[```
┌ Map ────────────────────────── Ctrl+R cycles ┐
│ ~~~~~~~TTTTT~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~TTTT###TTT≈~~~~~~~:::~~~~~~~~~~~~~~~~~~~ │
│ ~~~TTT##⌂####≈TT~~~~::;;;::~~~~~~~~~~~~~~~~~ │
│ ~~~~TTTT###T≈TT~~~~::;•;;;::~~~~~~~~"""""~~~ │
│ ~~~~~~~TTTTT≈~~~~~~~::;;;::~~~~~~~~"""""""~~ │
│ ~~~~~~~~~~~~~~~~~~~~~~:::~~~~~~~~~~~"""""~~~ │
│ ✎ (8,2) · forest · 0.61 · ⌂ Ashford          │
│ hjkl · t/n/g/r/o · +/- terrain · d · f · Esc │
└──────────────────────────────────────────────┘
```]

The glyphs read as a map: `~` sea, `T`/`#` forest, `:`/`;` desert, `"` grassland, `•`
a procedural settlement, and `⌂` a landmark you placed. The single-key tools along the
bottom are the whole editor; the rest of this chapter is those keys, one at a time.

#section("Towns and landmarks")

Put the cursor where you want a place and press `t` for a town or `n` for a named
landmark. A small prompt opens; type a name and press `Enter`.

#screen(caption: "Naming a town at the cursor")[```
      ┌ Town name (8,2) ──────────────────┐
      │ › Ashford▌                         │
      │ Enter place · Esc cancel           │
      └────────────────────────────────────┘
```]

The town joins your `geography.landmarks[]` at the cursor's cell, drawn `⌂`, and — once
you `/write` — appears on the plakat raster too. Press `d` to delete the feature under
the cursor. Because a landmark carries a name, it also becomes a gazetteer entry the
fact-checker can resolve when it reads your prose.

#section("Rivers")

A river is two points. Press `r`, move to the *source* and `Enter`, move to the *mouth*
and `Enter`, then name it. As you go, a provisional course follows the cursor from a
fixed `S`.

#screen(caption: "Drawing a river from source to mouth")[```
┌ Map ────────────────────────── Ctrl+R cycles ┐
│ ~~~~~~~~~~~T~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~~TTTTS·TTTTTTT~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~TTTTTT##····TTTTT~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~TTTTT########····TT~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~TTTTTT#####TTTTT·~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~~TTTTTTTTTTTTT~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ✎ (20,4) · coast · 0.48                      │
│ river: move to the MOUTH, Enter · Esc cancel │
└──────────────────────────────────────────────┘
```]

The river lands in `hydrology.rivers[]` with its source and mouth cells, and the
compiler *honours the declared course* — so after `/write` it carves its way to the sea
and draws as `≈`.

#insight[
  The river is *checked as you draw*. The worldbuilder runs the same `lint_rivers` the
  physics uses, so the moment you place a course that runs uphill, or ends its mouth
  above the shoreline, it tells you — "river runs uphill: its source sits below its
  mouth". You fix the map against the world, at the desk, before you commit.
]

#section("Regions and roads")

Two more tools round out the human layer. Press `g` to drop a *region* at the cursor;
its biome is filled in from the cell under you, so a region you place in the forest is a
forest region. It joins `geography.regions[]`, drawn `§`.

Press `o` to route a *road*. A road connects two of your named landmarks, so move to the
first and `Enter`, then to the second and `Enter`. It lands in `geography.roads[]` and,
because it references landmarks the map already knows, draws between them as `=` on the
worldbuilder map *and* on the plakat raster.

#section("Sculpting the land")

The tools so far annotate the world; this one *changes* it. Press `+` to raise the land
under a soft brush and `-` to lower it (`,` and `.` size the brush). The map re-shades
live as you sculpt — `~` sea, `.` lowland, `^` hills, `A` peaks — so you can raise a
mountain range or carve a bay and watch it appear.

#screen(caption: "Raising a mountain range with the brush")[```
┌ Map ────────────────────────── Ctrl+R cycles ┐
│ ~~~~~~~~~~~.~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~~~.....^.....~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~...^^^^A^^^^...~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~...^^^AAAAA^^^...~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~...^^^^A^^^^...~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~~~.....^.....~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ✎ (11,3) · land · 0.82 · terrain r2          │
│ + raise · - lower · ,/. brush · /terrain     │
└──────────────────────────────────────────────┘
```]

When the shape is right, `/terrain` writes the sculpted heightmap as a grayscale *DEM*
under `assets/maps/` and points `geology.dem` at it (a pending edit). The `realworld`
compiler reads DEMs, so a `realworld compile` rebuilds the *whole* world — climate,
rivers, settlements — from your terrain.

#pitfall[
  Terrain is the one edit the worldbuilder's own `/compile` does not yet re-simulate:
  it previews your sculpt and writes the DEM, but its live climate/biomes stay
  procedural until you compile with `realworld` (which is DEM-aware). Sculpt, `/terrain`,
  `/write`, then `inkhaven realworld compile` to see the land reshape everything.
]

#section("Checking the map")

Placing things by eye, you will sometimes drop a harbour on the wrong side of a
coastline. `/mapcheck` reads the whole map layer against the compiled world and flags
the mistakes — a town in open ocean, a coordinate off the map, a region in the sea —
right on the cells where they sit. Press `f` to jump the cursor from one to the next.

#screen(caption: "A town in the sea, flagged by /mapcheck")[```
┌ Map ────────────────────────── Ctrl+R cycles ┐
│ ~~~~~~~TTTTT~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~ │
│ ~~~~TTTT###TTT≈~~~~~~~:::~~~~~!~~~~~~~~~~~~~ │
│ ~~~TTT#######≈TT~~~~::;;;::~~~~~~~~~~~~~~~~~ │
│ ~~~~TTTT###T≈TT~~~~::;;;;;::~~~~~~~~"""""~~~ │
│ ~~~~~~~TTTTT≈~~~~~~~::;;;::~~~~~~~~"""""""~~ │
│ ~~~~~~~~~~~~~~~~~~~~~~:::~~~~~~~~~~~"""""~~~ │
│ ! settlement 'Sunkport' in open ocean (30,1) │
│ f: jump to next issue                        │
└──────────────────────────────────────────────┘
```]

#section("The mouse")

If your terminal supports it, the map takes the mouse: a left-click in the pane focuses
it, enters edit mode, and drops the cursor on the cell you clicked. Placement still
happens with the single keys — click to point, then `t`, `r`, `+`, and the rest — so a
world can be laid out as fast as you can aim.

#recap((
  [In the worldbuilder's Map pane, `e` enters edit mode: a cursor with a readout of
   coordinate, biome, and elevation. Every tool writes an ordinary `world.hjson` edit
   into the pending delta — `/diff`, then `/write`.],
  [`t`/`n` place towns and landmarks (`geography.landmarks[]`, `⌂`); `r` draws a river
   (`hydrology.rivers[]`, `≈`) checked as you go; `g` a region (`§`); `o` a road between
   landmarks (`=`); `d` deletes.],
  [`+`/`-` sculpt terrain under a brush; `/terrain` writes a DEM and points
   `geology.dem` at it, so `realworld compile` rebuilds the world from your land.],
  [`/mapcheck` flags map-layer mistakes — a town in the sea, an off-map coordinate — on
   the cells where they sit; `f` jumps between them. A left-click positions the cursor.],
))
