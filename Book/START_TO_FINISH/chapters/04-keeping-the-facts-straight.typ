#import "../design.typ": *

#chapter(number: 4, title: "Keeping the Facts Straight")

By the third chapter of *The Ninth Lantern* there were already more small
truths than one head holds. Nine lanterns — not eight, not ten. The ninth, the
cold one, alone at the far end of the Long Mole, not among the ring along the
quays. The fret coming ashore at *dawn*, not at dusk. None of these is hard to
remember on the morning you invent it. Every one of them is easy to contradict
on a tired afternoon four chapters later, when the rhythm of a sentence wants a
different number and your hand writes "the eighth lantern" without your noticing.

These are not mistakes of craft. The sentence that says "the eighth lantern was
cold" is a perfectly good sentence; it is only wrong about the book it lives in.
And it is exactly the kind of wrong you cannot catch by rereading, because you
know what you *meant*, and your eye supplies it. Inkhaven's answer is to stop
trusting your memory: write the settled truths down in a place the tool can
read, and then let two different readers hold your prose to them. This chapter
meets both — the one you invoke when you want a paragraph checked, and the one
that reads over your shoulder as you type.

#callout(label: "Two different readers")[
  The *fact-check* (`Ctrl+B Shift+X`) is a reader you *ask*: it weighs one
  paragraph against the truths you wrote in the Facts book, and streams back a
  verdict. The *continuity watch* is a reader that never sleeps: turn it on and
  it re-checks continuity on every save, deterministically, for free. The first
  is about *what is true*; the second is about *what stays consistent*. You want
  both, and they do not overlap.
]

#section("Writing down what's true")

In the last chapter you sketched the world in `world.hjson` — the harbour, the
coast, the physics of the place. That file records what the world *is*. The
truths that a simulation could never derive — that there are nine lanterns and
not eight, that the ninth is the one that went dark, that the keepers' creed is
*no lantern goes dark* — live somewhere else: the *Facts book*.

#term("The Facts book")[
  A system book of the settled truths of your story, written as ordinary
  paragraphs in plain prose. It sits beside your manuscript, never inside it.
  Distinct from `world.hjson`: the world file *derives* a world from physics; the
  Facts book *records* the decisions you have made and mean to keep — the ones no
  simulation could produce.
]

You write into it exactly as you write anywhere else: open the Facts book,
create a paragraph, type the truth. Keep each one short and flat — one settled
fact per paragraph reads best, both for you and for the checker that will parse
it. Here are the three facts *The Ninth Lantern* cannot do without.

#screen(caption: "Three entries in the Facts book")[```
Facts / The lanterns
──────────────────────────────────────────────
There are nine lanterns. They ring the harbour
on stone pillars; the ninth stands alone at the
end of the Long Mole.

No lantern has gone dark in three hundred years.
The keepers' creed is: no lantern goes dark.

The sea-fret comes off the water at dawn. The
light is what holds it back from coming ashore.
```]

That is the whole ritual. There is no schema to satisfy and no field to fill —
the Facts book is prose, and it stays readable as prose. What it buys you is
that from this point on, every claim your manuscript makes about lanterns, about
the age of the light, about the fret, has something concrete to be measured
against. Two facilities read it. One you meet now.

#callout(label: "The Facts AI scope")[
  The same entries also ground the assistant. `F9` cycles the AI scope, and
  *Facts* is one of the sticky scopes: select it once and every follow-up is
  answered from your written truths rather than freshly invented ones — so the
  AI never quietly retcons the harbour mid-conversation. For a large Facts book,
  `Ctrl+B Shift+S` opens a semantic search over it, so you ground in the handful
  of relevant facts instead of the whole book.
]

#section("The fact-check — Ctrl+B Shift+X")

A few pages into a new scene, Mira is out on the quay in the grey before
sunrise, and you write the line that names what is wrong:

#screen(caption: "The line you just wrote")[```
She had counted them twice on the way out. Eight
lanterns burning down the harbour, and the eighth
one — the one at the end of the Mole — stone cold.
```]

Everything about that sentence sounds right. It has the count, the place, the
cold. It is also wrong twice over: the lantern on the Mole is the *ninth*, and
if the ninth is dark then *eight* are burning, not counting a cold one among
them. You will not see it. You wrote it. So you ask.

With the cursor in that paragraph, `Ctrl+B Shift+X` runs the fact-check. It
locks the AI scope to this one paragraph, grounds the check against every entry
in the Facts book — the lanterns, the creed, the fret — and streams a verdict
into the AI pane, flagging any claim that contradicts what you wrote down. Its
mnemonic is *X* for fact e#[*X*]amination.

#screen(caption: "The verdict streams into the AI pane")[```
┌─ AI · fact-check · this paragraph ──────────────────┐
│ ⊗ Contradiction                                     │
│   You wrote: "the eighth one — the one at the end   │
│   of the Mole."                                     │
│   Recorded fact: the lantern at the end of the Long │
│   Mole is the NINTH. The one that went dark is the  │
│   ninth, not the eighth.                            │
│                                                     │
│ ⚠ Also check: "Eight lanterns burning." If the cold │
│   lantern is the ninth, eight burn and one is dark. │
├─────────────────────────────────────────────────────┤
│  Ctrl+B Shift+J  jump to the next flagged claim     │
└─────────────────────────────────────────────────────┘
```]

The check found what your eye slid over. Now you fix it — and here the second
chord earns its place. After a fact-check flags contradictions,
`Ctrl+B Shift+J` cycles the editor cursor through them one at a time: each press
jumps to the next flagged claim in the paragraph and shows the violated fact,
with its explanation, on the status bar. Its mnemonic is *J* for *jump*. You
walk the findings, you correct "eighth" to "ninth" and "eight lanterns" to
"nine," and the paragraph now agrees with the book it lives in.

#callout(label: "When the Facts book is empty")[
  The fact-check never fails for want of facts. With an empty Facts book it
  degrades gracefully to a generic local fact-check of the paragraph on its own
  terms, rather than refusing. And it works in all five of Inkhaven's languages
  — English, Russian, German, French, Spanish — detecting the paragraph's
  language and rendering the verdict in it. The honest question, *does this work
  in Russian?*, answers yes.
]

There is a command-line form for when you would rather sweep the whole book than
check one paragraph: `inkhaven facts scan` walks every chapter, semantically
matches each against the Facts book, and reports the contradictions — the
scriptable, CI-friendly twin of the in-editor check (add `--json` for a
machine-readable report). In the editor, though, `Ctrl+B Shift+X` on the open
paragraph is the motion you will reach for, because it lands where you are
already looking.

#callout(label: "Two commands, easy to confuse")[
  `inkhaven facts scan` — the one here — weighs your prose against the *Facts
  book*. Do not mistake it for `inkhaven fact-check`, which weighs prose against
  the *simulated world* from `world.hjson` (travel times, the gazetteer, the
  `magic:` ledger). Same instinct, different canon: one guards the truths you
  wrote down, the other the physics you declared.
]

#section("The book that watches as you write")

The fact-check is a reader you summon. The second reader you switch on once and
then forget, because it does its work in the same breath you make the mistake.
Inkhaven's continuity intelligence — the layer that watches for characters in
two places at once, for numbers that drift, for facts that quietly change across
chapters — can be told to re-check continuity on *every save*. It is off by
default, because not every author wants it; you turn it on in your config.

#screen(caption: "Turning the watch on — the continuity block")[```
continuity: {
  enabled: true             // the review-pass ledger
  ambient: true             // re-check on every save  ←
  ambient_cooldown_secs: 30 // throttle floor (seconds)
  co_location: true         // per-detector toggles
  numeric: true
  char_facts: true
  introduce: true
}
```]

The single line that matters is `ambient: true`. With it set, every time you
save, the watch reads the paragraph you just touched — its entities, its chapter
— re-runs the deterministic continuity detectors against just that scope, and
surfaces anything new in the Output pane. Because the core is *deterministic and
free* — it computes its answer from structure, with no model in the loop — this
happens inline, with no background job and no pause you would notice, however
long the manuscript has grown.

Suppose that, a chapter on, you describe Aldous's walk out to the cold lantern
and write the Mole as *half a mile* of wet stone — forgetting that in Chapter 1
you called it *a quarter-mile*. Neither number is in the Facts book; this is not
a truth you declared, just a measurement you let drift. The fact-check would say
nothing. The watch does not miss it.

#screen(caption: "The watch flags the slip on save")[```
┌─ Output · continuity watch ─────────────────────────┐
│ ⚠ [continuity · numeric]                            │
│   The Long Mole is "half a mile" here, but "a       │
│   quarter-mile" in ch. 1 — a distance that          │
│   conflicts across chapters.                        │
├─────────────────────────────────────────────────────┤
│  continuity watch: 1 finding(s) touching this edit  │
└─────────────────────────────────────────────────────┘
```]

The status line tells you the edit touched something; the Output pane names the
break, the detector that raised it (`numeric`), and both readings so you can see
which to keep. Its sibling detector, `co_location`, would have fired the same
way had you put Mira on the quay and out on the Mole in the same grey morning —
a character in two places at once. Neither costs anything, and neither waited to
be asked.

#callout(label: "A throttle, not a queue")[
  A burst of rapid saves does not re-check on every keystroke-flush. The watch
  throttles itself with `ambient_cooldown_secs` (default 30) — a floor that
  collapses a flurry of saves into one re-check, rather than running the check
  that many times. Re-checking a paragraph replaces its own prior findings, so
  the Output pane never accretes stale warnings.
]

The watch is the *ambient* face of a larger machinery. When you want the whole
book swept at once rather than the last edit's neighbourhood, `Ctrl+B Shift+I`
opens the full continuity ledger — every deterministic finding, ranked and
grouped, `Enter` to jump to each — and `inkhaven continuity check` runs the same
sweep from the command line, exiting non-zero on any hard contradiction so it
can gate a commit hook. The ambient watch is that same reader, narrowed to what
your fingers just changed, running while you write.

#section("Two checks, two questions")

It is worth keeping the pair distinct, because they answer different questions
and you will reach for them at different moments. The fact-check asks *is this
true?* — it weighs a claim against the truths you deliberately wrote into the
Facts book, and it is a language-aware model reading for meaning, which is why
it caught "the eighth lantern" as a matter of *fact*. The continuity watch asks
*does this stay consistent?* — it is a deterministic structural reader, no model
involved, catching the Mole that shrinks and the character who bilocates,
whether or not you ever declared a fact about either. One guards the world you
decided on; the other guards the book from itself.

#two_track(
  [In *The Ninth Lantern* the Facts book is the series bible in miniature: nine
  lanterns, the ninth on the Mole, the fret at dawn. Write the truths the story
  turns on, and the fact-check holds every scene to them while the watch keeps
  the geography and the numbers from drifting underneath you.],
  [In *non-fiction* the Facts book is the ledger of claims you have verified —
  dates, figures, definitions, the settled results of your research. The same
  `Ctrl+B Shift+X` holds each paragraph to your verified record, keeping the
  manuscript honest to your own evidence rather than to a half-remembered draft.],
)

Between them, the two readers close the gap that memory cannot: one you ask when
a paragraph feels load-bearing, one that answers unasked the moment a save lands.
The lanterns will stay nine. In the next chapter the manuscript grows a secret —
and a third reader that guards not what is *true*, but who is allowed to *know*
it.

#recap((
  [The *Facts book* records the settled truths of your story as ordinary
  paragraphs — the decisions no `world.hjson` simulation could derive (nine
  lanterns, the ninth on the Long Mole, the fret at dawn). It also grounds the
  AI through the sticky `F9` *Facts* scope.],
  [`Ctrl+B Shift+X` *fact-checks* the open paragraph against the Facts book and
  streams a verdict into the AI pane — it caught "the eighth lantern" as a
  contradiction of the recorded ninth. `Ctrl+B Shift+J` walks the flagged claims
  one at a time. It degrades gracefully with an empty Facts book and works in
  five languages.],
  [The *continuity watch* — `continuity.ambient: true` — re-checks continuity on
  every save over just the edit's scope, deterministically and for free, and
  surfaces breaks (`numeric` drift, `co_location`) in the Output pane inline. A
  `ambient_cooldown_secs` floor throttles it.],
  [The two are complementary: the fact-check asks *is this true?* against truths
  you declared; the watch asks *does this stay consistent?* structurally. Sweep
  the whole book with `Ctrl+B Shift+I` or `inkhaven continuity check`.],
))
