# Tutorial 107 — The Worldbuilder

*Inkhaven 1.9 (WBLD-1)*

Tutorials 76–77 built a world from `world.hjson` with the `realworld` compiler
and asked the AI to critique it. The **worldbuilder** is the interactive
front-end to all of that — a full-screen TUI companion, a third window alongside
the editor and the research assistant, where you construct a world by interview
instead of hand-editing HJSON. Every change still lands in `world.hjson` and
compiles through `realworld` unchanged.

## Launch it

```sh
inkhaven worldbuilder
```

You get a dedicated TUI with the world's derived facts on one side and the tools
to shape them on the other. What lives here:

- **The interview** — a guided set of questions (climate, geography, seasons,
  scale) that seeds `world.hjson` without you memorising the schema.
- **AI-assisted construction** — propose regions, settlements, and history, each
  scored for **plausibility** against the deterministic physics the `realworld`
  compiler derives (astronomy → climate → hydrology → demographics).
- **World-fact research** — gather grounded facts about your world into the Facts
  system book, tagged `fact:world`, so the fact-checker holds your prose to them.
- **The magic-ledger editor** — declare a magic system's costs and constraints as
  first-class world facts.
- **The map workflow** — an interactive map feature editor (see Tutorial 108).
- **A session journal** and **world export**.

## The pipeline underneath

The worldbuilder is a front-end; the engine is the same `realworld` pipeline you
can also drive from the CLI:

```sh
inkhaven realworld compile        # derive the World book from world.hjson
inkhaven realworld propose        # AI proposals for gaps
inkhaven realworld coherence <node>   # an LLM pass over a container for contradictions
inkhaven realworld map            # render the world map
```

Because every worldbuilder change writes `world.hjson`, you can move freely
between the interactive TUI and the CLI — and version-control the world as a plain
text file.

## World facts hold your prose accountable

Facts you establish in the worldbuilder (a region's climate, a distance, a
season) become `fact:world` entries in the Facts book. The live fact-checker
(`Ctrl+B W`) and the `Ctrl+B Shift+X` fact-check then flag any prose that
contradicts them — snow in a tropical region, a three-day ride that should be
three weeks.

---

**See also:** [WORLDBUILDING.md](../WORLDBUILDING.md) · Tutorial 108 (the map
editor) · Tutorial 69 (world consistency) · `inkhaven worldbuilder --help`.
