# Tutorial 77 — World maps and the fact-checker

*Inkhaven 1.3.26+ (the fast checker); maps, the slow track, scope chords, and
coherence are 1.3.27+*

Tutorial 76 built a world. This one *renders* it and *checks your prose against
it*. Two halves of RFC WORLD-4 Branch B: a map you can hang on the wall, and a
fact-checker that flags the messenger who outran his horse, the blizzard in your
tropical capital, the city ten times too big for its valley — quietly, while you
keep typing.

## Rendering a map

Inkhaven doesn't draw the map itself; it hands the world it compiled to
[**plakat**](https://crates.io/crates/plakat), a deterministic cartographer.
Install it once:

```
$ cargo install plakat
```

Then render — Inkhaven emits a *MapSpec* from the compiled geology, climate,
hydrology, and demographics, and plakat draws it:

```
$ inkhaven realworld map
map · Aldoria (160×120 grid) · plakat 1.10.0
  spec:     assets/maps/world.mapspec.json
  features: assets/maps/world.features.png
  geojson:  assets/maps/world.geojson
  20 landmark(s) resolved, 20 Place coordinate(s) refined
```

You get three artifacts under `assets/maps/`: the **MapSpec** (the JSON Inkhaven
emitted), a **features PNG** (the rendered map), and a **GeoJSON** (machine-
readable coast, rivers, roads, and landmarks). The mountains come from clustering
the heightfield's high ground; rivers run their real downhill course; landmarks
are your largest settlements (coastal cities become ports).

The map is **deterministic** — same world, same seed, same map. Because Inkhaven
emits the spec directly, plakat skips its own AI entirely.

- **`--spec-only`** writes the MapSpec without invoking plakat (inspect it, or
  work without plakat installed).
- **`--no-ingest`** renders without writing the resolved landmark coordinates
  back onto your Places. (By default, plakat's land-validity nudging refines each
  accepted Place's position.)
- In the TUI: **`Ctrl+B W` → `M`**.

If plakat isn't on `PATH`, the command says so and does nothing destructive —
maps are an optional feature.

## The fact-checker, fast track

When a project has a `world.hjson`, the **fast track** runs automatically:
deterministic, instant, no LLM. Pause a few seconds on a paragraph and any
findings appear in the **Output pane** — no chord, no focus stolen. A re-check
replaces that paragraph's prior findings, so fixing a problem makes it disappear.

It checks five categories:

- **Travel time** — a distance and a duration in one sentence imply a pace; an
  impossible one is flagged (pure prose, no world data needed).
- **Climate** — weather at a known place that contradicts its climate zone.
- **Demographics** — a population that diverges sharply from the modeled figure.
- **Astronomy** — a moon count that disagrees with the world's sky.
- **Economy** — a metal mined or worked that the world's geology doesn't yield.

Climate, demographics, and economy resolve names through the **gazetteer** — your
accepted Places (Tutorial 76) plus any `geography.landmarks` you declared. Ask
explicitly any time:

```
$ inkhaven fact-check --text "Snow fell heavily on Cairo for three days."
⚠ [climate] Implausible: freezing weather at Cairo, whose climate zone is hot desert.

1 finding(s).
```

### Choosing what to check — scope chords *(1.3.27+)*

**`Ctrl+B W` → `F`** arms a scope picker, then one key chooses how much to check:

- **`P`** — the open **p**aragraph (the default).
- **`B`** — the whole enclosing **b**ook.
- **`R`** — the 12 most **r**ecently edited paragraphs.

Each pass replaces those paragraphs' prior findings and flips you to Output with
a per-scope summary.

### Languages — and graceful degradation *(1.3.27+)*

The checker works in English, Russian, Spanish, French, and German, detecting the
paragraph's language and rendering warnings in it. Place names resolve in their
**grammatical cases** too — a Russian city flagged in `в Москве` matches `Москва`.

Detection is a built-in heuristic that needs no external model. When it isn't
confident which language a paragraph is in, it **degrades** — rendering the
warning in English rather than guessing wrong. The fact-check footer tells you
which backend is active; point `INKHAVEN_LANG_MODEL` at an enhanced parser to
upgrade it, but nothing ever *requires* one.

## The magic ledger — declared exceptions

If your world allows what physics doesn't, declare it so the checker respects the
exception instead of nagging. Add a `magic:` block to `world.hjson`:

```hjson
magic: {
    enabled: true
    rules: [
        {
            kind: "messenger_birds"
            covers: ["travel_time"]
            description: "Royal pelicans fly day and night with relays"
            applicable_to: { roles: ["royal_messenger"] }
        }
    ]
}
```

Each rule names a `kind`, the categories it `covers`, and who/where it applies.
The checker consults the ledger **lazily** — only after a candidate warning — and
a covered, applicable rule suppresses the finding with a note rather than hiding
it. `inkhaven realworld magic` lists the ledger; it materializes into `World /
Magic Ledger`.

## The slow track — subtle contradictions *(1.3.27+)*

The fast track is pattern-based and instant; the **slow track** asks the
configured LLM to find what patterns miss — an assumption buried in dialogue, a
consequence two clauses deep. It's opt-in and cost-capped:

```
$ inkhaven fact-check --text "…" --slow
slow track · model: … · ~1,200 tokens · 3/200 calls today · checking…
```

Before each call it prints a **cost estimate** and the day's tally, refuses a
call whose estimate exceeds a per-call soft cap (`--max-cost <tokens>`, default
6000; `--force` overrides), enforces a daily ceiling, and retries transient
errors (rate limits, timeouts) with backoff. A missing provider or a reached cap
degrades to a notice — never a crash.

### Auto-running the slow track

In the TUI, **`Ctrl+B W` → `S`** toggles an opt-in idle trigger (off by default,
because it spends tokens). With it on, ~45 seconds of quiet on a changed
paragraph runs the slow track **in the background** — the editor stays
responsive, and findings land in Output when ready.

## Coherence — contradictions *between* paragraphs *(1.3.27+)*

Everything above checks one paragraph at a time. The **coherence pass** checks a
run of paragraphs *against each other* — a character in two places without the
travel to connect them, a fact asserted then quietly reversed, a season that
can't follow:

```
$ inkhaven realworld coherence <book-or-chapter-id>
coherence · 24 paragraph(s) under `…`
⊗ [continuity] ¶3 and ¶7 place the duke in two cities a day apart by noon.

1 cross-paragraph finding(s).
```

Give it a book or chapter node id; it gathers that node's paragraphs in document
order and runs one cost-capped call, citing the `¶` numbers involved. It honours
the same daily cap and `--max-cost` / `--force` as the slow track.

## The World overview hub

Everything has a home under **`Ctrl+B W`**:

| Key | Action |
| --- | ------ |
| `C` | Compile + materialize all layers, seed the proposal queue |
| `P` | Open the Place proposal queue |
| `F` | Fact-check → then `P` paragraph / `B` book / `R` recent |
| `M` | Render the world map with plakat |
| `S` | Toggle the idle auto slow-check |

## What you learned

- `inkhaven realworld map` renders a deterministic map (PNG + GeoJSON) via plakat
  and refines your Places' coordinates; `Ctrl+B W → M` in the TUI.
- The fast fact-checker runs automatically into the Output pane — five
  categories, five languages, with graceful degradation when language detection
  is unsure.
- `Ctrl+B W → F` then `P`/`B`/`R` scopes the check to a paragraph, a book, or
  recent edits.
- A `magic:` ledger declares your world's exceptions; the checker suppresses
  covered findings with a note.
- The **slow track** (`--slow`, or idle auto via `Ctrl+B W → S`) and the
  **coherence pass** (`realworld coherence <node>`) find what patterns can't —
  both cost-capped, with a cost preflight and a daily ceiling.

Back to: **[Tutorial 76 — Building a world](76-building-a-world-realworld.md)**.
Field-by-field reference: **[`../WORLDBUILDING.md`](../WORLDBUILDING.md)**.
