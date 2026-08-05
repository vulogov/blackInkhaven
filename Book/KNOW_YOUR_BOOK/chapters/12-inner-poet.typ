#import "../design.typ": *

#chapter(number: 12, title: "The Inner Poet")

Verse is the one place in a book where the *form* is a promise. Call a stanza a
sonnet and you have sworn to fourteen lines and a turn; call it iambic and every foot
is a vow. The Inner Poet is the reader who holds you to those vows — measuring what you
wrote against what you declared — and it does so under an iron rule that sets it apart
from every generative tool: *it never writes a line of verse.* It observes, measures,
and reports. The poem stays entirely yours.

#term("The Inner Poet")[
  A reader for verse. You declare a stanza's intended form in a `poem:` block; the Poet
  measures the actual lines against it — metre, rhyme, syllable count, completeness —
  and reports the fit. It *observes*, never generates: it will tell you a line runs
  long, never rewrite it short.
]

#section("Measured against what you declared")

Point the Poet at a stanza with a declared form, and it scans each line — marking
stressed and unstressed syllables, checking the rhyme scheme, counting syllables
against the metre's demand — and shows you where the verse keeps its promise and where
it breaks it.

#screen(caption: "inkhaven poetry — the form, measured")[```
Sonnet · iambic pentameter · fit 0.86
  L1  / × / × / × / × / ×   the winter came, and with it came the cold
       × / × / × / × / × /   (declared: iambic — L1 opens with a trochee)
  L9  ✗ ten syllables wanted, eleven found — "remember" runs long
  rhyme  abab cdcd efef gg  ✓ intact
```]

The glyphs are a poet's shorthand: `/` a stressed syllable, `×` an unstressed one, `·`
a syllable the language leaves free. Where the fit is honest, the Poet says so; where a
line strains the form, it names the syllable that strained it — and leaves the fixing,
as always, to you.

#section("The translator's trilemma")

Its most quietly profound tool is for translated verse. A verse translation cannot
keep everything — *form*, *sound*, and *meaning* pull against each other, and every
translator sacrifices one to save the others. The Poet does not resolve the trilemma
(no tool can); it *measures* it, scoring a translation on how much of each it kept, so
you can see the trade you made with your eyes open.

#callout(label: "It will never generate verse")[
  This is the Inner Poet's whole ethic, and the reason it is a *reader* and not a
  co-author. A poem is the most personal thing a writer makes; a machine that wrote it
  for you would not be helping. So the Poet measures, scans, and reports — and the
  words on the line are always, only, yours.
]

`Ctrl+B J → P` opens it — `F` for the fast scan, `E` for the deeper reading, `D` to
declare a form, `T` for the two-column translation view, `A` for an ambient reading as
you write. Like the rest of the inner family, it informs and never prescribes; unlike
any of them, it holds a line it could never have written.

#recap((
  [The *Inner Poet* measures verse against a declared `poem:` form — metre, rhyme,
  syllables, completeness — and reports the fit (`/` stressed, `×` unstressed, `·`
  free).],
  [It *never generates verse* — it observes and measures, and the words stay yours.],
  [It scores the *translator's trilemma* (form vs. sound vs. meaning) so you see the
  trade you made; `Ctrl+B J → P` engages it.],
))
