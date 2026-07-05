# WORLD-10 · Scene world-context — implementation proposal

| | |
|---|---|
| **Status** | **Shipped** (S-P1..S-P4) · closes out WORLD-10 |
| **Builds on** | `realworld scene` (the composition), `world/weather.rs`, `world/timeline_context.rs` (WORLD-5), the polities/culture layers, `resolve_place_link` |
| **Theme** | Make the world *present while you write*: for the scene under the cursor, show its season/weather, place, and people — ambiently, without leaving the editor. |

## Goal

The whole WORLD stack compiles a place, a past, and its peoples — but today an author must run a `realworld` command to see any of it. This brings the scene's world-context **to the cursor**: a glanceable footer chip that updates as you move between scenes, plus the full brief on demand in the overview you already open.

## Grounding (verified against the tree)

Every input already exists and is tested; this is wiring, not new modelling.

- **The composition** — `realworld scene` already computes the brief (place → latitude via `row_to_latitude`, `weather_at`, nearest realm's culture). It lives inside the CLI `fn scene`; it needs extracting into a reusable pure function.
- **The WHEN** — `world/timeline_context.rs` (WORLD-5): `gather_events(&hierarchy)` + `build_context(...)` returns a `TimelineContext { effective_date: Option<i64>, … }` for a paragraph from its linked Timeline events (the same code the world fact-checker uses). `events_for_place(events, place_uuid)` links a scene's event to a Place.
- **The WHERE** — **paragraph links** (there are no wiki-links in inkhaven). A node carries `outgoing_links` (`add_paragraph_link`, `resolve_open_links`, backlink pickers — `src/tui/app.rs:10285-10360`); a **Place** is any node whose owning book has `system_tag == SYSTEM_TAG_PLACES` (`"places"`). A Place's coordinates/biome/climate come from `resolve_place_link` → `PlaceLink { x, y, biome, climate_zone, … }`.
- **The surfaces** — footer chips are an established pattern (`terms_hit_chip`, `editor_goal_footer_text` in `src/tui/app/editor_impl.rs`); the `Ctrl+B W` overview is `build_world_overview_rows`.

## Design

### No new right pane
The right side already cycles **Output / Ai / Thoughts**; a fourth pane would add Tab-cycling and steal space from a brief that is mostly glanceable. Instead, reuse two surfaces already in the codebase:

1. **Ambient glance → a footer chip.** A one-line **Scene chip**, alongside the terms/goal chips:
   `scene · Karthage · late autumn, cool & wet · Karon (mercantile)`
   Recomputed on paragraph change via the existing ambient debounce (fingerprint the open paragraph; skip while a modal is open). Costs no pane space — this is the "at the desk" surface.

2. **On-demand depth → the `Ctrl+B W` overview, made scene-aware.** When the cursor resolves a scene, prepend a **"This scene"** section (place · when · weather · people · nearby water/terrain) above the world layers. `Ctrl+B W` becomes "the world, focused on where I am." No new modal.

### Place detection (the WHERE) — a fallback chain
1. **Place-linked Timeline event** (preferred): if the paragraph's anchoring event (`timeline_context`) is tied to a Place via `events_for_place`, use it — this also yields the date for free.
2. **Paragraph link**: else walk the paragraph's `outgoing_links`; take the first whose target node sits under the Places book (`SYSTEM_TAG_PLACES`).
3. **None**: no place → the chip shows only the date/season (still useful), or hides.

### Date detection (the WHEN)
`build_context(...)` → `effective_date` (calendar ticks) → **day-of-year** via the project `Calendar` (the calendar bridge already models ticks↔units). No date → skip the weather line.

### The composition — one refactor
Extract the CLI logic into a pure, testable:

```rust
pub struct SceneBrief {
    pub place: Option<String>,        // name
    pub biome: Option<String>,
    pub climate_zone: Option<String>,
    pub day_of_year: Option<f64>,
    pub season: Option<String>,       // from weather_at
    pub conditions: Option<String>,   // weather descriptor
    pub realm: Option<String>,        // nearest polity
    pub ethos: Option<String>,        // its culture
    pub nearby: Vec<String>,          // named waters / ranges near the place
}
pub fn scene_brief(world: &WorldInputs, place: Option<&PlaceLink>, day: Option<f64>) -> SceneBrief;
```

Both the CLI `scene` and the editor call it — one source of truth, unit-tested on fixed inputs.

## Phases

| Phase | Content |
|---|---|
| **S-P1** | Extract `scene_brief() -> SceneBrief` (pure) from the CLI `scene`; point the CLI at it; unit tests. |
| **S-P2** | `place_for_paragraph(&self, id) -> Option<PlaceLink>` (event-link → paragraph-link) + ticks→day-of-year from `Calendar`. |
| **S-P3** | `scene_chip(&self) -> Option<String>` (mirrors `terms_hit_chip`); wire into the editor footer with the ambient debounce (opt-in toggle). |
| **S-P4** | "This scene" header in `build_world_overview_rows` when a scene resolves. |

## Decisions (recommended defaults)

- **Place source** — event-link first, then paragraph-link. *(Reuses shipping code; the event path also gives the date.)*
- **Surfaces** — footer chip (ambient) + scene section in the `Ctrl+B W` overview. **No new `RightPane`, no new modal.**
- **On by default?** — **opt-in**, like the other ambients (a config flag / toggle chord), so it never surprises an existing project.
- **Cost** — zero AI, deterministic; recompute is cheap (a bounded compile chain already used by the overview), debounced per paragraph version.

## Effort / risk

**Medium, low-risk.** Inputs all exist and are tested; the composition is a refactor; both surfaces copy established patterns (footer chip, overview rows). The only judgement call — place detection — resolves to code that already ships (`events_for_place`, `outgoing_links`, `resolve_place_link`). No new crates.

## Out of scope (follow-ups)

- Nearest **rivers/ranges** proximity ("River Vael, 12 cells") beyond the place's own biome — needs a small spatial query over the hydrology/geology grids; can land in S-P4 or after.
- A dedicated always-open pane — explicitly rejected in favour of the chip + overview.
- Auto-inserting world facts into prose — out of scope; this only *surfaces* context.
