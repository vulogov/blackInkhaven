# Plot Threads (THREADS)

*(1.2.14+ — `inkhaven thread …`; see
[`PROPOSALS/1.2.14_PLAN.md`](PROPOSALS/1.2.14_PLAN.md) for the full design)*

A novel is a weave of arcs — the inheritance subplot, the redemption arc, the
secret-society reveal — and the failure modes are structural: a thread set up and
never paid off, a "payoff" that nothing in the prose actually fires, an arc that
goes quiet for two hundred pages.

> **THREADS gives each narrative arc a home: a named record under the `Threads`
> system book that captures its status, weight, arc shape, and links into your
> Characters / Places / artefacts, plus a `doctor` that reads the whole set against
> the manuscript's paragraph links and reports the structural blind spots.**

Each thread is an HJSON-fronted Paragraph under the `Threads` system book — the same
content-type pattern as Language dictionary entries. Manuscript paragraphs attach to
a thread through the **existing paragraph-link mechanism** (`Ctrl+V A` outgoing,
`Ctrl+V I` incoming) — no new linking primitive — and those links are exactly what
the doctor counts.

---

## A thread and its lifecycle

`inkhaven thread add` seeds a fully-commented HJSON template you open and fill in:
identity (`title`), arc shape (`opening` / `midpoint` / `payoff`), connections
(`characters` / `places` / `artefacts` / `related_threads`), and metadata
(`tension` 0–10, `register`, `notes`).

Two fields drive the doctor:

- **`status`** — the arc's stage: `setup` → `develop` → `payoff` → `resolved` (plus
  `abandoned`). `add` defaults it to `setup`.
- **`weight`** — `major` | `subplot` | `runner` | `bridge`. `add` defaults it to
  `major`.

```
inkhaven thread add "inheritance subplot"                    # setup · major
inkhaven thread add "redemption" --status develop --weight subplot
inkhaven thread add "the seal" --title "The Broken Seal"     # display title ≠ slug
```

`add` rejects a duplicate title case-insensitively, writes the seed body to disk
first (so the editor's on-disk `.typ` matches), and tells you which fields to open
and fill.

```
inkhaven thread list [--status S] [--weight W]   # table: status/weight/tension + link counts
inkhaven thread export [--format json|csv|markdown] [--output FILE]
```

`list` prints a summary table (name, status, weight, tension, and character / place
/ artefact link counts), optionally filtered by status or weight.
`export` (`-f`/`--format`, `-o`/`--output`, default JSON to stdout) emits the full
record set as JSON, a flat CSV table, or a printable Markdown inventory.

---

## The doctor

```
inkhaven thread doctor [--json]
```

The doctor reads every thread's HJSON, tallies a **project-wide reverse-link count**
per thread (how many manuscript paragraphs link *to* it), and reports the status and
weight distributions, the average tension, and three **blind-spot** passes that
cross a thread's declared status against the evidence of the prose:

| Check | Fires when | Reads |
| ----- | ---------- | ----- |
| `zero_links` | a thread's status is **past `setup`** yet **no** paragraph links to it | the arc claims to be underway but the manuscript shows no trace |
| `payoff_unfired` | status is **`payoff`** yet **zero** paragraph links | the arc is marked as landing but nothing fires it |
| `dormant` | status is **`develop`** yet **0–1** links project-wide | the arc is supposedly developing but has all but gone quiet |

It always **exits 0** — it informs, it never gates. `--json` emits
`{ thread_count, status_distribution, weight_distribution, tension_avg, blind_spots:
{ zero_links, payoff_unfired, dormant } }` for CI or a dashboard.

---

## The Bund surface

One read-only word (classified `STORE_READ`) exposes the thread set to scripts and
hooks:

```
ink.thread.list  ( -- list )   every thread as a dict { id, title, slug, waypoint_count }
```

`waypoint_count` is the number of paragraph waypoints under a thread. When the
`Threads` system book is absent (it only auto-spawns on 1.2.14+ projects) the word
returns an empty list rather than erroring. Writing threads is not exposed to Bund —
scripts read the weave, they don't author it.

---

## What it is not

- Not a new linking primitive — threads reuse the paragraph-link mechanism, and the
  doctor's evidence *is* those links.
- Not an AI reader — `add` / `list` / `doctor` / `export` are deterministic over the
  `Threads` book's HJSON and the project's link graph (the LLM thread audit is a
  separate editor surface).
- Not a gate — the doctor always exits 0; it points at blind spots, it doesn't fail
  your build.
