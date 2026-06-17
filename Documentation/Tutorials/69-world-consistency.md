# Tutorial 69 — World consistency: facts, anachronisms, the story bible

*Inkhaven 1.3.6+ Facts / continuity tools; this chapter is 1.3.8+*

A long book builds a world — a climate, a geography, a cast with fixed
attributes, a setting in time. The tools to keep that world *consistent*
were scattered: `facts scan` checks the prose against the Facts book,
`continuity-drift` catches a character's eyes changing colour. 1.3.8 fills
the four remaining gaps: does the world contradict *itself*, does the prose
fit its *era*, can you *see* the whole world at once, and can a *series*
share one canon.

## 1. Does the Facts book contradict itself?

`facts scan` checks chapters against your facts. But the facts can disagree
with *each other* — "winters are mild" under Climate, "the harbor freezes
each January" under Geography. The internal-consistency check finds those:

```sh
inkhaven facts check            # or --json for a CI gate
```

```
facts check: 1 internal contradiction(s):
  ⚠ winters are mild  ⟷  the harbor freezes each January
      ↳ a mild winter can't freeze a harbor
```

It's an AI pass over the whole Facts book (cached in
`.inkhaven/facts_check.json`), and its findings also surface in the
**Editorial Pass** (`inkhaven edit`, category `world`) alongside everything
else.

## 2. Does the prose fit its era? (anachronisms)

Set your manuscript's year, and Inkhaven flags terms that postdate it — a
"wristwatch" in an 1840 novel, a "telephone" before 1876:

```hjson
editor: {
  style_warnings: {
    anachronism: {
      year: 1840
      // extend the built-in lexicon with your own:
      terms: [ { term: "spyglass-cam", earliest: 1990 } ]
    }
  }
}
```

A ~35-term built-in lexicon ships (each term with the earliest year it
plausibly appears); your `terms` add to it. The findings appear in
`inkhaven edit` (category `anachronism`), jumpable to the exact word. Off
until you set a `year` — a contemporary novel sees nothing.

## 3. See the whole world: the story bible

Press **`Ctrl+V Shift+L`** (L for **L**ore) for a consolidated, navigable
view of everything you've built:

- **CHARACTERS** — each one, with the attributes `continuity extract` has
  tracked across chapters beneath it (`eye_color: brown (ch.3)`,
  `hometown: Selhaven (ch.1)`).
- **PLACES · ARTEFACTS · FACTS** — every entry of each book.

`↑↓` navigate, **`Enter`** jumps to an entry's source paragraph (a `→`
marks the jumpable rows). Run `inkhaven continuity extract` first to
populate the character attributes; the books alone still list the cast.

## 4. A series: share one canon

Writing Book II of a trilogy? Keep the shared facts in one directory —
one plain-text file per fact (the filename is the title, the contents the
body) — and point every book at it:

```hjson
facts: { shared_path: "../series-bible/facts" }
```

Now `facts check` layers the shared canon with each book's local facts
(local wins on a name clash), so a contradiction between Book II's prose
and the series bible is caught. Or copy a snapshot in:

```sh
inkhaven facts import            # from facts.shared_path; --yes to write
inkhaven facts import --from ../series-bible/facts --yes
```

`import` adds the shared facts to *this* book's Facts book as paragraphs
(idempotent — it skips ones already present), after which `facts scan` and
the fact-check chord see them as local facts too.

## Where to go next

- The Facts book itself: [Tutorial 63](63-facts.md).
- The Editorial Pass that surfaces these findings:
  [Tutorial 68](68-editorial-pass.md).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [WORLD-1 plan](../PROPOSALS/WORLD-1_PLAN.md).
