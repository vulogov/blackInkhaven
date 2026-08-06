#import "../design.typ": *

#chapter(number: 5, title: "The Secret and Who Knows It")

*The Ninth Lantern* has a secret at its heart, and the whole book is built to
protect it. On the cold morning the story opens, the ninth lantern is dark and
its keeper is gone, and for four chapters every character — and every reader —
is allowed to believe the same wrong things: that the oil ran out, that Toft the
merchant failed a delivery, that old Aldous wandered off into the fret and was
lost. The truth is worse and quieter. Aldous *put the lantern out himself*, and
walked out along the Long Mole on purpose, because he had learned what the light
was really holding back. That truth is the hinge of the book. It lands in
Chapter 6, in the one scene built to carry it, and it must land *there* — not a
page sooner, in nobody's mouth, in no one's thoughts.

This is a different kind of mistake from the ones the last chapter guarded
against. A character standing in two places at once, a river that shrinks
between chapters, a distance that does not add up — those are fractures in
*where and when*, and a reader half-feels them. A secret spoken too early is a
fracture in *who knows what*, and a reader feels it exactly: the small, fatal
click of a mystery giving itself away. No sentence is wrong. Every line is clean
prose. The break lives in the gap between a scene in Chapter 5 and the reveal in
Chapter 6 — a gap no single reading holds in view, which is precisely why you
will read straight past it and a first reader will not.

Inkhaven has a reader that holds exactly that gap in view. It is called *KEN*,
and this chapter is the one moment in the whole book where the tool does
something no general-purpose assistant can: it reconstructs, across the entire
manuscript, *who could know what, and by when*, and it flags anyone who outruns
their own knowledge.

#section("The one axis that breaks a mystery")

KEN is the sibling of the continuity watch you met last chapter. That watch —
SENTINEL — has one cardinal move: it flags a thing *named before it exists*, an
entity referenced in Chapter 2 that the book does not introduce until Chapter 5.
KEN takes that move and carries it one step further, from *does this thing exist
yet* to *could this person know this yet*. Same forward walk through the book,
same Unicode-aware matching of names and phrases in the prose, a new axis:
knowledge.

#term("Ken")[
  A character's *ken* is the range of what they know — the set of facts they
  have been let in on, up to a given point in the book. KEN the reader is named
  for it. The whole check is a forward walk that builds each character's ken
  chapter by chapter and flags any line that reaches past it.
]

What makes this the book's signature "only Inkhaven can do this" moment is that
KEN never *guesses* what a character knows. It *derives* it, from four things
Inkhaven already holds and a chat model never will: the timeline (who was
present when), the events' participant lists, the cast in your Characters book,
and the point of view each scene is written from. A generic AI asked "does
anyone know the secret too early?" can only re-read the prose and form an
opinion. KEN reads the *structure* — the ground truth you declared — and
computes an answer that is the same every run and costs almost nothing. It
reasons only over what it can ground, and where it cannot ground a thing it
stays silent rather than inventing a break.

#two_track(
  [For a mystery this reader is close to the whole reason the timeline and the
  cast exist. Keep the participant lists honest and tag your secrets, and KEN
  catches the leaked confidence and the too-early deduction a whodunit lives or
  dies by — the errors beta readers find and you cannot.],
  [The same guard serves an *argument*. A conclusion used before it is
  established — a proof that leans on the very thing it has not yet shown — is
  premature knowledge in a suit and tie. Declare the claim, mark where you
  establish it, and the guard flags the paragraph that uses it early.],
)

#section("Declaring the secret")

KEN starts from nothing. A project with no secrets and no timeline gives it
nothing to check, so it does nothing and costs nothing — you pay for this reader
only in proportion to the epistemic structure you actually declare. The first
thing you declare is the secret itself.

You mark it with a tag on the paragraph that carries the truth — the sentence,
in the manuscript, that states plainly what happened. In *The Ninth Lantern*
that is one line in the Chapter 6 reveal, and the tag names the secret in the
words the rest of the book will use to refer to it. The words after the colon
are the *topic*: a short, plain handle KEN will hunt for in the prose.

#screen(caption: "The tag that names the secret")[```
  secret:Aldous put the lantern out
```]

That is the whole declaration. The tag does two things at once. It tells KEN the
topic *exists* — that "Aldous put the lantern out" is a thing a character might
refer to — and it tells KEN this particular topic is a *secret*, which raises
the stakes on anyone who reaches for it before they are allowed to. A secret is
not merely a fact that arrives late; it is a fact whose early appearance is a
leak. We will see the difference bite in a moment.

#callout(label: "Where the tag lives")[
  KEN reads the tags on your *manuscript* paragraphs — the same place your
  `pov:` marks live — not the Facts book you built last chapter. The secret is
  also a fact about your world, and it is fine for it to sit in your Facts book
  too; but the tag KEN acts on rides the sentence in the prose that states it.
  Put `secret:` where the truth is told.
]

#subsection("Setting, viewing, and finding a tag")

You have seen *what* to write; here is *where* you write it. A tag is not typed
into the prose — it lives in a small picker attached to each paragraph, so the
sentence in your manuscript stays clean. Open the paragraph that states the
truth and press `Ctrl+B ]`. The picker lists every tag this paragraph already
carries, and every tag anywhere in the project: press `A` to add a new one, type
`secret:Aldous put the lantern out` at the prompt, and `T` applies it. The same
picker is how you *read a paragraph's tags back* later — open it, `Ctrl+B ]`,
and there they are.

#screen(caption: "Ctrl+B ] — the per-paragraph tag picker")[```
  tags · ¶ "…and the ninth had never once gone cold."

  [x] secret:Aldous put the lantern out
  [ ] pov:Mira
  [ ] reveals:the end of the Mole

  Space select · T apply · A add new · D delete (project-wide)
```]

Finding them again is the other half. `Ctrl+B }` opens the search-by-tag
picker: choose a tag and it lists every paragraph that carries it, with a filter
box to narrow the list, and Enter on a paragraph opens it in the editor. This is
how you audit your own scaffolding — "show me every scene I marked `know:`" —
before you trust KEN to reason over it.

#screen(caption: "Ctrl+B } — search by tag")[```
  search tags:  know:▊

    know:Aldous put the lantern out@Aldous   ← Enter
    know:Aldous put the lantern out@Sella

  ── paragraphs carrying know:…@Aldous ───────────────
    ch. 1 · The keeper's round            Enter → open
```]

#callout(label: "One picker, every tag")[
  `Ctrl+B ]` and `Ctrl+B }` are not KEN-specific — they set and find *any*
  paragraph tag: the `secret:` and `know:` of this chapter, the `pov:` of the
  next, a `reveals:` on a scene, or a private tag of your own. Learn the two
  chords once and every tagged feature in the book uses them.
]

#section("Granting the knowledge — two honest ways")

A secret nobody may know is no use; the point is that *some* people know it, and
the reader is watching them not say so. So the second thing you declare is the
*grant*: the earliest point at which a given character could know a given topic.
KEN derives a grant two deterministic ways, and never any other.

#term("Grant")[
  A *grant* is the earliest place in the book a character could know a topic —
  the moment their ken widens to include it. It comes from *presence* (they were
  in the room) or from a *declaration* (you tagged it). A use of the topic
  before the character's earliest grant is a break; a use at or after it is
  clean.
]

#subsection("Presence — they were in the room")

The first source is free, and it comes straight from the timeline. A character
in an event's participant list *knows what happened at that event*, from the
moment of the scene that depicts it onward. The reveal in Chapter 6 is an event
on your timeline; its participants are the people present when Mira learns the
truth — Mira herself, and Bryn, who followed her onto the Mole. Listing them as
present is all it takes: from that scene forward, both are granted knowledge of
the secret, with no tag at all.

The reveal's own event title is often terse — "The end of the Mole" — too vague
for KEN to match against the prose. A `reveals:` tag on the reveal scene binds
that terse event to the real topic, so presence grants the thing the book
actually talks about rather than a chapter heading.

#screen(caption: "The reveal scene — presence + a bound topic")[```
  event   The end of the Mole   ch. 6
          present: Mira, Bryn

  tag     reveals:Aldous put the lantern out
```]

#subsection("Declaration — you say so, by name")

The second source is a tag, for the knowledge that no event conveniently
records. Three forms cover it:

#screen(caption: "The know: tags")[```
  know:<topic>          grants the scene's POV character
  know:<topic>@<name>   grants a named character
  secret:<topic>        marks a topic secret (and, like
                        know:, makes it a thing KEN tracks)
```]

Aldous is the one who did the thing, so he has known it from the first page. A
`know:Aldous put the lantern out@Aldous` tag on an early scene grants him at
Chapter 1 — now his own later remembering of it, in a flashback or a found
letter, reads clean instead of tripping the guard. And Sella, the harbourmaster,
learns it only when Mira brings the truth back to her — a Chapter 6 scene. A
`know:Aldous put the lantern out@Sella` there grants Sella at Chapter 6, and not
before. Sella's ken, for this one topic, opens in Chapter 6. Hold that thought.

#callout(label: "The precision ladder")[
  When more than one source could name a topic, KEN takes the most precise: an
  explicit `know:` or `reveals:` tag beats a bare event title. That is why the
  `reveals:` tag on the Mole scene matters — it hands KEN the exact words to
  match, instead of leaving it to guess from a terse event name.
]

#section("The slip — a chapter too early")

Now the reason for all of it. Somewhere in the drafting of Chapter 5 — a chapter
*before* the reveal — you wrote a scene on the quay where Sella, cornered and
angry, says more than she should. It is a good line. It is also impossible:

#screen(caption: "Chapter 5, on the quay — one line too knowing")[```
  "Don't look for a spill or a dry wick," Sella
  said. "Aldous put the lantern out himself. He
  chose the dark, and he chose it for us."
```]

Sella cannot say this. Her grant for the topic opens in Chapter 6, when Mira
tells her. Here she is naming it in Chapter 5, in her own mouth, a full chapter
ahead of the only moment that could have told her. You will not catch it —
you know the ending, so every rehearsal of it sounds natural to you. Run the
reader that does not know the ending:

#screen(caption: "inkhaven knowledge — the deterministic check")[```
$ inkhaven knowledge
⊗ [premature_knowledge] Sella speaks of "Aldous
  put the lantern out" in ch. 5 — before learning
  it in ch. 6.

1 finding(s): 1 break(s), 0 other.
```]

There it is: the click of the mystery giving itself away, caught before a reader
ever sees it. KEN attributed the line to Sella — the dialogue-attribution layer
knows *"Sella said"* names the speaker — matched the secret's topic inside her
speech, looked up her earliest grant, found it a chapter *later* than the line,
and raised a `premature_knowledge` break: a character acting on a fact before
they could have learned it. The glyph `⊗` is a hard break; the kind is in
brackets; the message names the character, the topic, the chapter of the slip,
and the chapter where the knowledge actually arrives.

#callout(label: "It is a gate, not just a report")[
  Like the continuity check, `inkhaven knowledge` *exits non-zero* whenever a
  hard break survives — `premature_knowledge` or `leaked_secret`. That single
  rule lets it sit in a pre-commit hook or a CI pipeline: the build fails on a
  leaked secret the way it fails on a compile error. Add `--json` and the same
  findings come back as an array for a script to read.
]

#section("When a fact is a secret")

Watch what the `secret:` tag was quietly doing. Suppose, in an earlier draft,
you had granted the knowledge and tracked the topic but *not* yet told KEN it was
secret. The slip above is what you would get: `premature_knowledge`, a character
who knows a fact too early. Serious, but ordinary — the same finding KEN raises
when a detective names the murder a chapter before the body is found.

Now add the one tag that says *this fact is a secret*, and re-run. The break
does not change chapters, and the message reads the same — but its *kind* does:

#screen(caption: "The same slip, once the fact is a secret")[```
$ inkhaven knowledge
⊗ [leaked_secret] Sella speaks of "Aldous put the
  lantern out" in ch. 5 — before learning it in
  ch. 6.

1 finding(s): 1 break(s), 0 other.
```]

`leaked_secret`. KEN has raised the stakes on the identical line, because you
told it the topic is not just late but *guarded*. This is the escalation worth
carrying out of the chapter: a too-early use of an ordinary tracked topic is
`premature_knowledge`; a too-early use of a `secret:` topic is a
`leaked_secret`. Both are hard breaks, both fail the gate, but the second says
something sharper — *your book let its secret out a chapter early* — and it is a
thing only a tool that knows which facts you meant to keep could possibly say.

#section("The dashboard — Ctrl+B Shift+Z")

You do not have to leave the editor to see this. `Ctrl+B Shift+Z` opens the
*knowledge dashboard* — every finding grouped by kind and ranked, breaks at the
top. `↑↓` scrolls it, `Enter` jumps straight to the offending paragraph so you
see the slip in its own context, and `Esc` closes it. It is the fastest way to
walk a book's secrets without losing your place in the prose.

#screen(caption: "Ctrl+B Shift+Z — who knows what, when")[```
┌─ Knowledge · who knows what, when · 2 ──────────────┐
│ BREAKS                                              │
│ ▌⊗ leaked_secret  Sella: "Aldous put the lantern   │
│    out"  ch. 5 · grant ch. 6 · on-the-quay          │
│                                                     │
│ NOTICES                                             │
│  ● dropped_reveal  Bryn told "the family's old      │
│    debt" ch. 2 — it never surfaces again            │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll · Enter jump · Esc close                  │
└─────────────────────────────────────────────────────┘
```]

#subsection("dropped_reveal — the epistemic unpaid setup")

That second line is a softer finding, and it repays a look. Bryn "knows more of
the family's past than he says," so early on you tagged a scene
`know:the family's old debt@Bryn` — granting him a piece of buried history. But
in the drafting, that thread got away from you: the old debt is granted and then
never spoken of, never acted on, never paid off. KEN notices. A declared reveal
whose topic never surfaces again is a `dropped_reveal` — an epistemic *unpaid
setup*, dangling knowledge you either forgot to use or a tag you can retire. It
is a `●` notice, not a break; it does not fail the gate. It only asks the
question you would want asked: *you told me Bryn knows this — did you mean to do
anything with it?*

#callout(label: "Only your declarations count here")[
  `dropped_reveal` fires only for knowledge you *declared* with a `know:` tag —
  the places you deliberately opted in. Knowledge granted by mere presence at an
  event does not have to resurface: a character can witness a storm and never
  mention it again without that being a loose thread. The dropped-reveal notice
  watches the promises you made on purpose.
]

#section("The fix — move the line, or grant the knowledge")

Here is the part that matters most, and it is the part KEN refuses to do for
you. It flags; it never rewrites. A knowledge break is not a phrasing to
correct — it is a *choice about the story*, and there are two honest answers,
each of which clears the finding:

- *Move the line.* Sella should not know yet, and the scene is stronger for her
  not knowing — so the revelation belongs later. Cut her too-knowing line from
  Chapter 5, or move the beat to after her Chapter 6 grant, and her ken no
  longer outruns her lips. The gate goes green.

- *Grant the knowledge.* On reflection Sella *should* have known all along —
  she is the keeper of the town's story, and perhaps she has known since before
  the book began. Then the fix is not to silence her but to make her knowledge
  true: give her an earlier grant, with a `know:Aldous put the lantern out@Sella`
  tag on an early scene, or by adding her to the participant list of an earlier
  event she was present at. Now the line in Chapter 5 sits *after* her grant,
  and it is no longer a leak — it is foreshadowing you meant.

Which of these is right is a decision no tool can make, because it is a decision
about what your book is. That is exactly how Inkhaven routes it. When these
findings reach the *Editorial Pass* — the revision worklist of a later chapter —
a knowledge break arrives not as a mechanical rewrite but as a *decision*: *fix
the leak, move the reveal, or add a grant?* You choose; the tool holds the
consequences. And when you have chosen, re-run the check and watch the book go
quiet:

#screen(caption: "After the fix")[```
$ inkhaven knowledge
✓ no epistemic breaks — nobody knows what they
  shouldn't.
```]

#section("The deep pass — for the slip with no words")

Everything so far is deterministic: a forward walk, set membership, and matching
a topic's *name* in the prose. It costs essentially nothing and gives the same
answer every run, whatever the book's length. But it has one blind spot by
construction. A character can betray knowledge *without ever naming it* — acting
on the secret, or acting conspicuously ignorant of a thing they plainly know —
and no name-match can see that, because there is no name to match.

For exactly that case there is an opt-in pass, and only that case:

#screen(caption: "The command and its flags")[```
inkhaven knowledge          # the deterministic check
inkhaven knowledge --json   # findings as JSON
inkhaven knowledge --deep   # + the implied-irony pass
    [--max-cost 8000]       #   soft budget for --deep
    [--book-name SLUG]      #   restrict to one book
```]

`--deep` hands each scene and a ledger of who-learns-what-when to a model and
asks for the *implied* breaks — Sella pouring two cups before Mira has said a
word about a second visitor, a character who steps around the dark lantern too
carefully for someone who believes it an accident. These land as soft
`implied_irony` notices, never hard breaks; the deterministic layer owns the
hard calls. The pass is *explicit, cost-capped, and off by default*, on the same
rail as the world fact-checker — you pay for it only when you ask, and its cost
is previewed against your daily cap before it runs. There is, by deliberate
design, no *have the AI judge what everyone knows* mode. That would be guessing,
and KEN does not guess.

#callout(label: "Does it work in Russian?")[
  Yes. KEN inherits its reach from the layers beneath it: the topic matcher is
  Unicode-aware, and the dialogue attribution that decides *who said this line*
  ships English, Russian, German, French, and Spanish conventions. A secret
  named in Cyrillic dialogue is caught exactly as one named in English. The tags
  — `secret:`, `know:`, `reveals:` — are the same in every project language;
  only the topic between the colon and the end is yours to write.
]

#recap((
  [*KEN* watches one axis nothing else does — *who knows what, and when*. It is
  SENTINEL's *referenced-before-introduced* move carried into knowledge: a
  character *acting on a fact before they could learn it*. It *derives* each
  character's knowledge from structure and never guesses.],
  [Declare a secret with a `secret:<topic>` tag on the sentence that states it;
  the words after the colon are the *topic* KEN hunts for in the prose. Tags are
  not typed into the prose: `Ctrl+B ]` sets and reads back a paragraph's tags,
  `Ctrl+B }` finds every paragraph carrying a given tag — the same two chords for
  every tagged feature.],
  [Grant knowledge two deterministic ways: *presence* (a character in a timeline
  event's participant list knows it, free) — with a `reveals:<topic>` tag to
  bind a terse event to the real topic — and *declaration*
  (`know:<topic>` grants the scene's POV, `know:<topic>@<name>` a named
  character).],
  [A use before the earliest grant is a hard break: `premature_knowledge` for an
  ordinary tracked topic, escalating to `leaked_secret` once the topic is marked
  `secret:`. A declared reveal that never surfaces again is a soft
  `dropped_reveal` notice.],
  [Run it with `inkhaven knowledge` (`--json`, `--book-name`, `--deep` for the
  opt-in `implied_irony` pass); it *exits non-zero on any break*, so it gates
  CI. `Ctrl+B Shift+Z` opens the dashboard — `↑↓` scroll, `Enter` jump.],
  [KEN flags; it never rewrites. Every break is a *decision* — move the line or
  add a grant — that you resolve in the Editorial Pass, not a correction the
  tool imposes.],
))
