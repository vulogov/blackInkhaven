#import "../design.typ": *

#chapter(number: 9, title: "Writing a poem at the desk")

Every chapter so far measured verse from the command line — a line here, a stanza there,
handed to `inkhaven poetry` and read back. That is the right way to *learn* the tools, and
the right way to interrogate a poem that already exists. But it is not how you *write* one.
Writing happens at the desk, in the editor, with the poem in front of you and the measuring
running quietly alongside — and everything the earlier chapters did on the command line,
the editor does without your ever leaving the page. This chapter is the whole loop, in one
sitting: from an empty paragraph to a measured, form-checked poem.

Everything lives under one door. Press `Ctrl+B J` to open the family of Inner readers, then
`P` for the Poet:

#screen(caption: "Ctrl+B J → P — the Inner Poet")[```
┌─ ♪ Inner Poet ───────────────────────────────────┐
│                                                   │
│   F   fast-scan the stanza (metre + rhyme)        │
│   E   engage the slow track (an AI reading)       │
│   D   declare a form                              │
│   T   two-column translation view                 │
│   A   ambient — scan as you open each stanza      │
│                                                   │
│   Esc close                                       │
└───────────────────────────────────────────────────┘
```]

Six keys, and the rest of this chapter is what they do.

#section("Make a stanza")

A poem is paragraphs of the verse family (Chapter 1). In the Tree pane, add a paragraph and
give it a verse type — press `i` for the structural-type picker and choose *verse stanza*,
or cycle an existing paragraph's type with `t` until the `♩` appears. The glyph is your
confirmation that Inkhaven now holds this block as verse, not prose:

#screen(caption: "The Tree pane — a verse stanza, marked ♩")[```
  Tree ─────────────────────────
  ▾ Sonnets
    ▾ 1 — First frost
      ♩ First frost   ← your stanza (para:verse-stanza)
      ¶ (a note)
```]

Type your lines into it. Inside a verse paragraph the line breaks you type are the line
breaks that print — Inkhaven never reflows verse.

#section("Declare the form")

A stanza on its own is just lines; what lets the Poet *measure* it is a declared form. From
the Inner Poet (`Ctrl+B J → P`), press `D` for the form picker:

#screen(caption: "Ctrl+B J → P → D — the form picker")[```
┌─ ♪ Declare a form · D ───────────────────────────────────┐
│                                                          │
│  › sonnet                14 lines of iambic pentameter   │
│    petrarchan_sonnet     Italian: octave + sestet        │
│    villanelle            19 lines, two refrains          │
│    haiku                 3 lines, 5-7-5 syllables         │
│    …                                                     │
│                                                          │
│  writes a poem: block beside the stanza — no verse       │
│  is generated · ↑↓ select · Enter attach · Esc cancel    │
└──────────────────────────────────────────────────────────┘
```]

Choose a form and press Enter. Inkhaven writes its `poem:` block — localised to your
project's language — as a small sidecar beside the stanza, exactly where the measuring tools
look for it. This is the one act the command-line workflow could not do for you at the desk,
and it is what turns the passive ruler into an active second reader. It writes a *form
declaration*, never a line of verse; the poem stays entirely yours.

#callout(label: "Localised to your language")[
  Declare a sonnet in a Russian project and the block comes back with the Russian iambic
  conventions set (`allow_pyrrhic`, `require_final_stress`); in an English one, the plain
  English scheme. The same picker, the right defaults — the multilingual promise, at the desk.
]

#section("Write, and watch the count")

Now write, and look at the status bar. While a verse paragraph is open, Inkhaven shows a
live readout of the line your cursor is on:

#screen(caption: "The status bar — a live syllable count")[```
 [Editor]  ♩ 8 syl · l2/4                    │  First frost
```]

`♩ 8 syl` is the syllable count of the current line, updated as you type; `l2/4` is your
position — line 2 of a four-line stanza. It answers, without a command, the question a
metrical poet asks a hundred times an hour: *is this line at its ten? is this haiku line at
its five?* It counts in your project's language — exact for Russian, honestly approximate
for English (Chapter 2's limit, carried to the desk).

#subsection("The next stanza, in one keystroke")

When a stanza is done and you want the next, press `Ctrl+B Shift+Y`. Inkhaven creates a
sibling verse paragraph of the *same* type, right after this one, and opens it for editing —
so a sonnet grows stanza by stanza, or a villanelle tercet by tercet, without your hands
leaving the flow. (Structure only, of course — the new stanza is empty; the lines are yours
to write.)

#section("Read the poem back")

With a form declared, ask the Poet what it sees. Press `F` for the fast track — a
deterministic, offline scan of metre and rhyme. Its findings land in the Output pane:

#screen(caption: "Ctrl+B J → P → F — findings in the Output pane")[```
  Output ───────────────────────────────────────────
  ♪ Praise   Line 1 scans cleanly as iambic pentameter.
  ♪ Note     Line 2 scans short: 8 of 10 syllables.
  ♪ Concern  Lines 2↔4 (B–B): "temperate" / "boom" —
             do not rhyme.
```]

Three severities, and the whole philosophy in them (Chapter 5): *Praise* for a line that
keeps its promise, *Note* for a departure worth seeing, *Concern* for a promise plainly
broken. The harshest word is Concern, and even that is a flag, never a fix. What you do about
it is yours: mend the rhyme, or keep it and mean it.

For a reading a ruler cannot give — where an image strains, whether the sonnet's argument
turns at its volta — press `E` to engage the slow track. It sends the stanza to a language
model under a strict observer's brief and returns *prose to weigh*, in the poem's own
language, headed `♪ Inner Poet`. It never rewrites your poem; there is nothing to accept,
only something to consider.

#section("Let it read as you go — ambient")

Pressing `F` on every stanza gets old. Press `A` instead and turn on *ambient*: now every
verse paragraph with a declared form is scanned the moment you open it, its findings posted
without a keystroke. Because the fast track uses no model, ambient is free — there is no cost
to it and no cap on it, only a quiet guard so identical lines aren't re-scanned. Move through
a manuscript of poems and each one greets you with its own reading.

#subsection("Silence what you're keeping on purpose")

A poet breaks rules deliberately, and a reader that keeps flagging a rule you have *chosen*
to break is a reader you stop trusting. So a finding can be *suppressed*: silence its key
(its severity, kind, and line), and the fast track and ambient mode stop reporting it. The
scan still counts it — nothing is hidden from the totals — but the Output pane goes quiet
about it, and the status line notes how many you have silenced (`3 findings (1 suppressed)`).
Your decisions persist across sessions, kept in a small `inner_poet.db` beside your project;
Inkhaven remembers what you have already answered.

#section("The whole book at a glance")

Open the Outline (`Ctrl+2`, or `Ctrl+B Shift+O`) and a book of verse shows you its progress
without your opening a single poem:

#screen(caption: "Ctrl+2 — the Outline, with completion chips")[```
  Outline ──────────────────────────────────────
  ▾ Sonnets
    ▾ 1 — First frost
      ♩ First frost              8/14
    ▾ 2 — The thaw
      ♩ The thaw                 14/14 ✓
    ▾ 3 — (untitled)
      ♩ a beginning              3/14
```]

Each verse row carries its glyph and a completion chip — `8/14` while a sonnet drafts,
`14/14 ✓` when it is whole. Land on one and the detail panel names its form, its
`written / expected` count, and any structural issue the form checker found — a villanelle's
missing refrain, a sonnet without its turn. The whole manuscript's state, read off one screen.

#section("Source beside translation")

If the stanza is a translation — a `para:verse-translation` paragraph holding a source and
its rendering, split by a `---` line — press `T` for the two-column view:

#screen(caption: "Ctrl+B J → P → T — the translation view")[```
┌─ ⇄ Translation ──────────────────────────────────────────┐
│ source (ru)                 │ → translation (en)          │
│ Мой дядя самых честных…     │ → My uncle, of the purest…  │
│ ── trilemma (ru → en) ────────                            │
│ Form    ██████░░░░  62%   1/1 lines keep the foot         │
│ Meaning ░░░░░░░░░░        the AI axis — engage (E)         │
│ Sound   ████████░░  80%   alliteration 0.17 → 0.14        │
└───────────────────────────────────────────────────────────┘
```]

Source and translation side by side, line-aligned, with the trilemma of Chapter 7 measured
beneath: *Form* and *Sound* counted, *Meaning* left blank because it is judgement, not a
number — and the pane points you to the Poet's slow track for it. The source language is
detected for you; the whole thing is read-only, a review, never a rewrite.

#section("The loop, entire")

That is the desk. You made a stanza and declared its form; you wrote it with the count in
view and grew it a keystroke at a time; you read it back — instantly with `F`, reflectively
with `E`, continuously with ambient — and silenced what you meant to keep; you saw the whole
book's progress in the Outline and weighed a translation side by side. At no point did
Inkhaven write a word of your poem, judge a line good or bad, or cross from measuring into
making. The ruler stayed a ruler. The poem stayed yours. That is the only promise this book
has, and it holds all the way to the last keystroke.

#recap((
  [The whole editor loop lives under `Ctrl+B J → P` — `F` fast-scan, `E` engage, `D` declare
   a form, `T` translation view, `A` ambient.],
  [Make a verse paragraph (`i` / `t` in the Tree → `♩`), then *declare its form* with `D` —
   the one act the desk adds over the CLI, writing a language-localised `poem:` sidecar so the
   tools have a target.],
  [Write with the live status readout (`♩ N syl · l L/M`); grow the poem a stanza at a time
   with `Ctrl+B Shift+Y`.],
  [Read back with `F` (Praise / Note / Concern to Output) or `E` (an AI reading); turn on
   *ambient* (`A`) to scan as you navigate; *suppress* a finding you are keeping on purpose —
   the decision persists in `inner_poet.db`.],
  [See every poem's completion in the Outline (`♩ 8/14`, `14/14 ✓`); review a translation
   side by side with `T`. Throughout, Inkhaven measures and reports — it never writes the poem.],
))
