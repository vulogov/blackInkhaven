# Tutorial 108 — The Map Editor

*Inkhaven 1.10 (MAPED)*

A fantasy or historical map isn't a picture — it's data your prose has to agree
with. The map editor, inside the worldbuilder (Tutorial 107), lets you place and
edit a world's spatial features directly on a rendered map, and checks them
against the physics the `realworld` compiler derives.

## The feature layers

Everything you place is a declaration in `world.hjson` — the map is a spatial
front-end to it. The layers, roughly in order of how the 1.10 "Cartographer"
release built them up:

- **Towns & landmarks** — settlements and named places (already `world.hjson`
  declarations; the map gives them coordinates).
- **Rivers** — traced watercourses that must run downhill.
- **Regions** — named areas with their own climate and terrain.
- **Roads** — routes between settlements (a schema the map editor introduced).
- **Terrain** — a DEM (digital-elevation) heightmap: the hard layer, where
  rivers, coastlines, and travel times get their physical grounding.

A cursor moves between features; the mouse works too. As you place a river or a
road, the editor knows the terrain beneath it.

## Check the map against the world

```
/mapcheck
```

`/mapcheck` validates the map against the derived world: a river running uphill,
a town in the sea, a road crossing an impassable range. It's the map's answer to
the prose fact-checker — spatial contradictions surfaced before they reach the
page.

## Explore, then commit

- **`/roll`** — seed-explore: generate candidate terrain from a seed so you can
  audition several worlds before committing to one.
- **`/switch`** — swap between candidates; **promote** the one you want.
- A **plakat** raster map renders the result; **`--pdf`** exports it print-ready.

Because the map is `world.hjson`, the same map feeds the worldbuilder's
plausibility scoring, the World book, and the fact-checker — so "the caravan
reached the capital by nightfall" is checkable against the actual road and
terrain you drew.

---

**See also:** [WORLDBUILDING.md](../WORLDBUILDING.md) · Tutorial 107 (the
worldbuilder) · Tutorial 77 (world maps & fact-checking).
