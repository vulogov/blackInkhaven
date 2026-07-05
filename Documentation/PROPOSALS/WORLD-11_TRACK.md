# WORLD-11 — Declare & Verify (the author's hand over the emergent layers)

| | |
|---|---|
| **Status** | Proposed (track) |
| **Builds on** | WORLD-4 (layers), WORLD-8 (history), WORLD-9 (polities/culture), the declared blocks (geography/economy/magic), the world fact-checker |
| **Theme** | Today the physical-and-derived layers are split cleanly in two: `astronomy · geology · geography · hydrology · economy · magic` are *declared*; `history · nations · rivers-courses · culture · ecology` are *purely generated*, with no `world.hjson` block to touch. WORLD-11 removes that hard split. Every generated layer becomes **declare-or-generate**: you may pin your own facts, the sim fills the rest, and `realworld` **verifies your declarations for plausibility** — informing you when a declaration fights the physics, never overriding you. |

## The unifying pattern

One shape, applied to each layer:

1. **Declare-or-generate.** An optional `world.hjson` block lets you pin entries. Absent → the layer generates exactly as it does today (fully backward-compatible).
2. **Merge, author-wins.** Declared entries override or augment the generated ones; the remainder generates around them. A declared name always beats a generated one.
3. **Verify, don't block.** `realworld` checks each declaration against the physics and reports what looks implausible (a river that climbs, a polar keystone in the tropics), as a warning at `validate`/command time. It never rejects a declaration — the author always wins; the sim is a second pair of eyes.

This is the same discipline the rest of WORLD already uses (the propose/accept flow, the magic-ledger fact-check, "inform, never block"). WORLD-11 just makes *every* layer answer to the author's hand.

## The four features

### W11-P1 · Declared history — pinned to an epoch, mapped to the Timeline

A `history:` block of author events, merged into the generated chronology.

```
history: {
  events: [
    { year: -1200, title: "The First Landing", epoch: "Founding Age",
      places: ["Karthage"], description: "The seafarers make landfall." }
    { year: -80,   title: "The Sundering War" }
  ]
}
```

- `year` (required) is placed on the world's own calendar; `epoch` is optional and **inferred from the year** when omitted (which epoch span contains it). Declared events sort into the printed/materialised chronology alongside foundings and generated events.
- `places` pins an event to accepted Places, so when it is **adopted into the Timeline** (`realworld history` already emits `inkhaven event add …` lines), it carries its place link — closing the loop to the story Timeline.
- **Verify:** a `year` outside the world's recorded span, or an `epoch` that does not contain the `year`, is flagged.

### W11-P2 · Declared nations — where they sit

A `nations:` block; declared realms take precedence, the rest cluster as today.

```
nations: [
  { name: "Karon", capital: "Karthage", seats: ["Karthage", "Aldermouth"],
    relations: { "Serai": "rival" } }
]
```

- `capital` resolves to a settlement (a Place name, or `[x, y]`); `seats` are member settlements; unclaimed settlements cluster into generated realms around the declared ones. Declared names + relations override the generated pairing.
- **Verify:** a `capital` that resolves to open ocean or has no settlement near it, or a `seat` implausibly far from its capital, is flagged.

### W11-P3 · Declared river courses — propose source and mouth

Extend `hydrology.rivers` entries with an optional `from`/`to`; absent → the procedural river stands.

```
hydrology: {
  rivers: [
    { name: "The Aldermere", from: [12, 4], to: [40, 30] }
  ]
}
```

- With `from`/`to` (coordinates, or a named region/place), the layer traces a course from source to mouth, preferring the downhill path; without them, the generated river is simply named as today.
- **Verify (the key check):** a declared course that **runs uphill** between source and mouth, or a **mouth that reaches no sea or lake**, is flagged — "the Aldermere climbs 400 m between its source and mouth." Rivers run downhill to water; the sim says so when a hand-drawn one does not.

### W11-P4 · Declared culture & ecology — pin your own, checked for fit

Optional `cultures:` and `ecology:` blocks pin artefacts per region/biome; the rest generate.

```
cultures: [
  { region: "The Ashfall Reach", ethos: "austere, devout", belief: "the cult of ash",
    language: "SOV · agglutinative · guttural" }
]
ecology: {
  regions: [
    { biome: "hot_desert", flora: ["blood-thorn", "glass cactus"],
      fauna: ["sand-strider", "dune wyrm"], keystone: "dune wyrm" }
  ]
}
```

- Declared cultures/ecologies pin their region/biome; `realworld culture` / `ecology` generate the remainder around them.
- **Verify:** an ethos that fights its biome (a *seafaring* people declared in a landlocked desert), or a keystone/fauna implausible for its biome (a *polar* animal pinned to a tropical rainforest), is flagged by a biome-appropriateness heuristic.

## Shared mechanics

- **Schema:** all new blocks are optional and `#[serde(default)]` — existing `world.hjson` files are unaffected.
- **`realworld validate`** grows a verification pass over every declaration, printing plausibility warnings alongside the layer-compiles it already runs. Each command (`history`, `polities`, `culture`, `ecology`) also surfaces its own warnings.
- **Authority discipline preserved:** declared always wins; verification is advisory. Nothing is rejected, nothing is written into the manuscript without acceptance.
- **Deterministic:** declarations are fixed inputs; the generated remainder is still seeded, so `(world.hjson, seed)` still fully determines the world.

## Phases (suggested order)

| Phase | Content | Effort |
|---|---|---|
| **W11-P1** | Declared history + epoch pinning + place-linked Timeline adoption + verify | Med |
| **W11-P2** | Declared nations (capital/seats/relations) + verify | Med |
| **W11-P3** | Declared river source/mouth + the downhill/reaches-water plausibility check | Med |
| **W11-P4** | Declared culture & ecology pinning + biome-fit verify | Med |

## Book impact

This is not just a code change — it revises the book's spine, and *strengthens* it. The chapters that today say "history / nations / culture / ecology have no block, they only emerge" become **"generate, declare, or both — and the world checks your hand."** The ch12 `#insight` and the ch8/ch9 "no block" notes are rewritten; each emergent chapter gains a short "declaring your own" section with an `#hjson` example and a note on the plausibility check. The distinction the book draws sharpens from *emergent vs. declared* to **generated, declared, or verified** — a better lesson, and the reason WORLD-11 is worth doing.
