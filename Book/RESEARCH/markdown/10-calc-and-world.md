# 10 — Facts You Compute

Not every fact is looked up. Some you **work out**. The distance between two towns is not something to find in a database — it follows from their coordinates. A day's ride is not a citation — it is a speed times a time. The compound growth of a population, the length of a year on an invented planet, the water a channel of a given size can carry: these are **derived** facts, and they sit at the very top of the trust ladder for a simple reason.

**Deterministic** — A fact is **deterministic** when it follows from its inputs by a fixed rule, so that anyone who runs the same computation gets the same answer. A looked-up fact asks you to trust a source; a deterministic fact asks you to trust **arithmetic** — which is why it is the firmest rung of all. Nothing to misread, nothing to go stale, nothing to cite but the calculation itself.

## The calculator that speaks your domains: `/calc`

`/calc` evaluates an expression and can turn the result straight into a fact. It is not just a four-function calculator — it knows the domains a book actually needs: unit conversions, geography, astronomy, climate, economics.

```
/calc 100 mi to km
```

Convert Roman miles to kilometres so your distances are consistent. Compute the great-circle distance between two sets of coordinates — the **haversine** — so a journey's length is real rather than guessed. Grow a figure by a rate over years with compound interest. Reduce a list of numbers to a sum or an average. In each case the result comes back as a value you can keep with `/fact`, and its provenance reads `computed` — the top rung — because the answer is not a claim anyone vouched for but a calculation anyone can repeat.

Notice how the sources compose. In Chapter 4 you learned two places from GeoNames, each with real coordinates. Here you feed those coordinates to `/calc` and get the distance between them — a **new** fact, firmer than either input, that you never had to look up. Looked-up facts become the inputs to computed ones, and the computed ones sit higher on the ladder than the sources they came from.

> **Let the palette hold the spellings:** `/calc` understands a broad vocabulary of operations — conversions, distances, growth curves, list reductions, domain formulas for climate and geography and economy. You do not need to memorise them. As with every command in this book, the way you rediscover an operation is to reach for it when you need it; the point here is the **idea** — that a fact can be derived, and that a derived fact is the firmest kind.

**For fiction —** Keep your invented world internally consistent by **computing** its constants, not guessing them. Fix your planet's year, its seasons, the distances on your map — once, deterministically — and every later scene that depends on them stays true, because they all trace back to the same arithmetic.

**For non-fiction —** Do the quantitative work of your argument **in** your corpus, cited to the computation. A growth rate, an aggregate, a distance — each becomes a `computed` fact a reader can re-derive, which is the strongest form a quantitative claim can take.

## A world that produces its own facts

For writers building an invented world — a planet, a continent, an imagined society — Inkhaven can go one step further than a calculator. It can **simulate** the world and materialise the results as facts you can research like any other.

**The World book** — The **World** book holds a project's own deterministic simulation, compiled from a small definition: a star, a planet, its moons. From that, Inkhaven derives an astronomy (the length of the year in planet-days, axial tilt, seasons, lunar cycles, tides), a climate, a geography, a rough demography — and files them as facts. Because they are computed from a seed, they are consistent with each other and reproducible: the same world definition always yields the same world.

Once a world is compiled, `/world` lets you browse those simulated facts the way you browse any part of your corpus, and `/calc` can **read** them — computing new answers from your world's own constants. A `/fact` taken from the World book records `simulation` provenance: another top-of-the-ladder rung, deterministic for exactly the same reason `computed` is — re-run the simulation and you get the same world.

> **Invented, but not arbitrary:** A simulated world fact is **invented** in the sense that the world is yours — but it is not **arbitrary**. Because it is computed from a seed by fixed rules, your invented planet's seasons and distances hold together as rigorously as real ones. This is the bridge between the two halves of your book: the facts you make up and the facts you borrow, both made trustworthy by the same discipline.

## The top of the ladder

You have now met every rung. At the bottom, a model's guess; above it the web, your documents, the scholarly record, the structured databases; and at the top, the facts you **compute** and the facts your **simulation** produces — deterministic, reproducible, citing nothing but the calculation. A book grounded from top to bottom of this ladder, with every fact recording its rung, is about as honest as a book can be about what it knows.

Which raises the question the next part answers. Everything so far has been about **borrowed** facts — the ones a reader could check. But a work of fiction is built on facts the author **invented**, which must not be fact-checked at all. How does a tool this insistent on grounding make room for the things you simply decreed to be true? That is the subject of Part V.

**Recap**

- Some facts are **derived**, not looked up — distances, travel times, growth, aggregates — and a **deterministic** fact is the firmest rung because anyone can re-run it.
- `/calc` computes across real domains (conversions, geography, astronomy, climate, economy); a `/fact` from it records `computed` provenance.
- Looked-up facts **compose**: GeoNames coordinates feed `/calc` to produce a distance firmer than either input.
- The **World** book compiles a project's own deterministic simulation into facts; `/world` browses them, `/calc` reads them, and a `/fact` from it records `simulation` provenance.
- Computed and simulated facts sit at the **top** of the ladder — invented worlds made rigorous by the same discipline as borrowed facts.
