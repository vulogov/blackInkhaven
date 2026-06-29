# Tutorial 101 — Inner Theologian

*Inkhaven 1.4.18*

Fiction carries moral weight. Every narrative makes claims — about what suffering means, whether
goodness is possible, what people owe each other, what transformation costs — and most of them are
implicit: the author never chose a theodicy, but the novel enacts one.

Inkhaven's Inner family has two members already. **Inner Socrates** interrogates logical structure;
**Inner Editor** observes craft. **Inner Theologian** is the third axis — *moral and theological
seriousness*: whether the work engages honestly with the weight of what it depicts.

It is a **tradition-neutral comparativist**. It reads any manuscript through the lenses of eleven moral
and theological traditions — Catholic, Protestant, Orthodox, Gnostic, LDS, Islam, Judaism, Hinduism,
Buddhism, Confucianism, and secular moral philosophy — **not to judge the work by any of them, but to
ask what each of them sees.** It belongs to no tradition, advocates none, and never delivers a verdict.
Everything it produces is a question. It never edits your prose.

## Two tracks

**Fast track — deterministic, zero-AI.** Three `info` signals over your prose, in the Output
`theologian` category (the `⚖` glyph), folded into the `Ctrl+B Shift+C` review pass:

- **moral invisibility** — a named character harms another with no acknowledgment in the next few
  paragraphs;
- **consequence gap** — lethal or severe violence with no depicted consequence;
- **sacred levity** — sacred/ritual vocabulary sharing a paragraph with comic markers (the most
  cautious detector; comedy with sacred content is a legitimate style, so this only flags for
  attention).

All five languages (EN/RU/DE/FR/ES), tradition-neutral word lists, no tradition weighted more heavily.
None ever rises above `info`.

**Slow track — the LLM session.** Press **`Ctrl+B J → T`** to engage the open paragraph: the persona
poses two or three moral/theological questions through the lenses most illuminating for that passage,
**in the book's language**, always naming which tradition raises which — and always inviting you to
say a lens is irrelevant ("that's useful information too"). Questions land in the Output pane.

## From the terminal

```sh
inkhaven theologian scan                       # the fast-track signals (exit 1 if any)
inkhaven theologian session --chapter 7        # a slow-track session over chapter 7
inkhaven theologian session --category 6       # the book's implicit theology (the broadest)
inkhaven theologian session --lens gnostic     # restrict to one tradition lens
inkhaven theologian suppress --para <id> --reason "the banality of harm is the point"
```

`session` defaults to **Category 6** (the author's implicit theology) over the whole book; `--chapter`
narrows the scope, `--category 1-6` picks the question family, `--lens <code>` restricts to one
tradition. The six categories: **1** moral weight & consequence, **2** theodicy & innocent suffering,
**3** redemption & transformation, **4** sacred & transcendent, **5** duty & obligation, **6** the
work's implicit theology.

## Grounding

Before a session, Inner Theologian reads what's already known — WORLD-6 findings flagged with a
theological dimension, CHAR-1 character arcs that involve a change of belief, and any fast-track
signals open in scope — and opens there. When nothing grounds it, it starts from Category 6. Each
source degrades cleanly; none is required.

## Scripting & config

Bund: `ink.theologian.signals` (the fast-track signals) and `ink.theologian.suppress` (mute one). The
`theologian:` config block tunes the windows, the sub-budget, the disabled lenses, and the language;
`enabled: false` turns the whole feature off. Findings live in `.inkhaven/inner_theologian.db`.

It belongs to no tradition. It advocates for none. It asks what each of them sees — and leaves the
deciding to you.
