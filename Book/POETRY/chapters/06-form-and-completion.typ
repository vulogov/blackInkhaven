#import "../design.typ": *

#chapter(number: 6, title: "Form and completion")

A sonnet is not fourteen good lines; it is fourteen lines *in an order*, with a turn near
the end and a rhyme scheme that binds them. A villanelle is a machine of repetition — two
lines that come back, on a schedule, until the last stanza gathers them both. These forms
have architecture, and architecture can be half-built. This chapter is about `poetry
status`: the tool that tells you how much of a form's structure you have raised, and
whether what you have raised is sound.

#section("How far along are you?")

Where the last chapter measured lines, this one measures *the whole poem against its plan*.
Ask for a poem's status and Inkhaven counts what you have written against what the form
requires:

```
$ inkhaven poetry status --form villanelle --language en \
    --text "Do not go gentle into that good night
            Old age should burn and rave at close of day
            Rage rage against the dying of the light"
♩ villanelle · 3/19 · drafting
  no structural issues
```

Three lines of the nineteen a villanelle demands: `3/19`, and the state is `drafting`.
When the architecture is complete, the state flips:

```
$ inkhaven poetry status --form haiku --language ru \
    --text "Тихий старый пруд
            В воду прыгнула лягушка
            Всплеск в тишине"
♩ haiku · 3/3 · complete
  no structural issues
```

A Russian haiku, three lines of three, `complete` — the completion check counts syllabic
architecture in Russian exactly as it does in English, because a haiku is a haiku in any
language.

#subsection("What 'expected' means")

The line target is not one rule but several, because forms count differently:

- The *fixed forms* carry their number in their bones — a sonnet is 14, a villanelle 19, a
  haiku 3, a tanka and a limerick 5. Inkhaven knows these outright.
- The *stanzaic forms* compute it — a form declared as six stanzas of three lines expects
  eighteen, and the target moves with the `stanzas` and `lines_per_stanza` you declared.
- The *open forms* — ode, elegy, free verse — have no fixed length, and `status` says so,
  reporting the line count as information rather than measuring it against a target that
  does not exist.

#term("Completion")[
  The state of a poem relative to its form's required architecture: #emph[drafting] while
  lines are still missing, #emph[complete] once the full structure is present. It is a
  structural count, not a quality judgement — a complete poem is finished in *shape*,
  which is the only thing a machine can vouch for.
]

#section("Structural issues")

Counting lines is the easy half. The interesting half is checking that the lines you have
obey the form's internal *rules* — and the fixed forms have rules that go well beyond
their length. When `status` finds one broken, it lists it under the count rather than
saying \"no structural issues\":

- A *villanelle* runs on two refrains: line 1 and line 3 of the first tercet return, in
  strict alternation, as the last line of each following stanza, and both close the final
  quatrain. `status` checks that the refrains you owe have actually come back where the
  schedule places them.
- A *pantoum* interlocks: lines 2 and 4 of each quatrain become lines 1 and 3 of the next.
  The check follows the chain and flags a link that does not hold.
- A *ghazal* is built on a #emph[radif] — a word or phrase repeated at the end of both
  lines of the opening couplet and the second line of every couplet after — often with a
  *signature* (the poet's name) in the closing couplet, the #emph[maqta]. `status` looks
  for the radif on its schedule.
- A *sonnet* turns. Somewhere — classically at line 9 in the Italian, near line 13 in the
  English — the argument pivots: the #emph[volta]. The sonnet check looks for the turn the
  form promises.

#term("Volta")[
  The \"turn\" of a sonnet — the point where its argument shifts: a problem giving way to
  a resolution, a question to an answer, a scene to its meaning. In the Petrarchan sonnet
  it falls between octave and sestet (line 9); in the Shakespearean it often waits for the
  closing couplet. A sonnet without a volta is fourteen lines that never change their mind.
]

These checks are where `status` earns its place beside `scan`. The fast track of Chapter 5
reads line by line and would never notice that a villanelle's refrain failed to return —
that is a fact about the *whole poem's shape*, invisible from any single line. `status`
sees the architecture.

#callout(label: "Drafting is a first-class state, not a failure")[
  A poem at `3/19` is not broken — it is *in progress*, and Inkhaven treats that as a
  normal, honourable condition rather than a pile of missing-line errors. This is why a
  form can carry `suppress_while_drafting`: so the Inner Poet holds its structural
  complaints until the poem is complete enough for them to be fair. Measuring an unfinished
  poem against a finished poem's rules would be pedantry, and the workbench declines to be
  pedantic. It waits.
]

#section("Status in the workflow")

`status` is the bird's-eye view you check between sessions: *where is this poem, and is its
scaffolding sound?* In the editor the same completion count feeds the outline — a poem's
entry can show how far along it is at a glance — so a book of villanelles tells you which
are finished and which still owe refrains without your opening a single one. On the command
line, `status --json` (Chapter 8) turns the same reckoning into data a script can gate a
build on: a manuscript that reports every poem `complete`, or a CI job that will not pass
until they are.

#recap((
  [`poetry status` measures the *whole poem against its form's plan*: a line count
   (`written / expected`) and a state — *drafting* or *complete*.],
  [The expected count comes three ways: fixed in the form (sonnet 14, villanelle 19, haiku
   3), computed from declared `stanzas × lines_per_stanza`, or absent for open forms, which
   are counted but not judged.],
  [Beyond counting, `status` checks each fixed form's internal rules — a villanelle's refrain
   schedule, a pantoum's interlock, a ghazal's radif, a sonnet's volta — and lists any that
   are broken. These are whole-poem facts the line-by-line fast track cannot see.],
  [*Drafting* is a first-class state; `suppress_while_drafting` lets the Poet hold structural
   complaints until a poem is complete enough for them to be fair.],
  [The completion count feeds the outline and, via `status --json`, feeds scripts and CI.],
))
