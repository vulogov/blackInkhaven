# Building the World with Inkhaven

*A Worldbuilder's Process — from a Single Seed to a Living World.*

A complete, beginner-friendly guide to building a believable, consistent world
and writing inside it — from a first compiled world through its history, peoples,
and rules to a world that is present at your desk while you write — using
Inkhaven's built-in World Simulation.

It assumes **no prior knowledge** of worldbuilding *or* of Inkhaven: every idea
is defined where it first appears, in a boxed **Term**. The focus throughout is
the **process** — what you do, in what order, and why — not a feature tour. It
serves anyone who tells stories in a setting: novelists, game-masters,
screenwriters, designers.

Asides come in five clearly-marked kinds so you always know what you are reading:
**Note** (how Inkhaven behaves), **Insight** (a worldbuilding principle),
**Ask Yourself** (a question about *your* world), **Pitfall** (a mistake to
avoid), and **Try It** (an exercise at the keyboard).

It teaches with **diagrams** (built with [fletcher](https://typst.app/universe/package/fletcher))
rather than screenshots, and it centres one idea: you do not draw a world — you
set its starting conditions and a **seed**, and a deterministic compiler grows
the rest, the same way every time. A built world is only worth it if it **touches
the page**, so the last part brings the world to your writing desk.

## Reading it

The compiled book is [`BUILDING_THE_WORLD.pdf`](BUILDING_THE_WORLD.pdf) (B5, ~130
pages). To rebuild it from source you need [Typst](https://typst.app):

```sh
typst compile Book/BUILDING_THE_WORLD/BUILDING_THE_WORLD.typ Book/BUILDING_THE_WORLD/BUILDING_THE_WORLD.pdf
```

Fonts are the ones Typst bundles (Libertinus + New Computer Modern + DejaVu Sans
Mono), so there is no font setup. The **first** compile fetches two packages from
the Typst universe (fletcher + cetz) once, then caches them.

## What it covers

- **Part I — What a World Is** · why build one, a world as a system, your first
  compiled world.
- **Part II — The Physical World** · the sky, the land, weather and water, and
  where people settle — each layer a consequence of the ones before it.
- **Part III — Giving the World a Past** · history, epochs, and chronology.
- **Part IV — Giving the World a People** · nations, cultures and tongues (the
  bridge to Inkhaven's ConLang suite), and life.
- **Part V — The Author's Hand** · what you declare vs. what emerges, and the
  magic ledger — with the discipline that the author always has the last word.
- **Part VI — The World at the Desk** · writing against the world (weather,
  travel, scene context), keeping your prose true (the fact-checker), and the
  bridges that flow the world into your manuscript.
- **Part VII — A Complete Walkthrough** · one world, end to end.
- **Appendices** · a command reference and a glossary.

Companion to *Grounding Your Book in Fact* (`Book/RESEARCH/`), in the same style.
