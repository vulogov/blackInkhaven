#import "../design.typ": *

#chapter(number: 4, title: "Interrogating the corpus with SCHOLAR")

We have the sources on disk and a corpus of provenance-tagged claims. Now the work
proper begins: finding out what the corpus actually _says_ when you hold its pieces
against one another. This is where a study of two traditions lives or dies — in the
places they agree, the places they strain, and the places they flatly contradict —
and it is exactly the work retrieval alone cannot do, because retrieval finds what
is _similar_, and an argument turns on what is _opposed_. SCHOLAR is the family of
commands built to close that gap.

#term("SCHOLAR")[
  Inkhaven's relation engine — the set of Research Assistant commands that judge how
  claims and sources _stand to one another_, reporting not just contradiction but
  its confirmation-counterpart too. Where a fact-checker asks "is this claim true?",
  SCHOLAR asks "how does this claim relate to that source, and how do these sources
  relate to each other?" — and grades the answer, from flat contradiction through
  tension and qualification to agreement.
]

#section("Relate a claim to the corpus")

The workhorse is `/relate`. Give it a claim, and it retrieves the nearest facts and
the nearest ingested source passages, then judges _each_ against the claim on a
graded scale — contradicts, tension, qualifies, agrees, or silent — splitting the
verdict into what stands _against_ the claim and what _supports_ it. Point it at the
first joint of our thesis, the asymptotic self:

#transcript("/relate Kant and eternal progression both make perfection an endless approach rather than a state reached", [
  Relating your claim to the corpus — 2 against · 3 supporting · 1 silent. \
  ✓ *supporting* — `@kant-cpr` (Ak. 5:122): the highest good requires an
  endless progress toward complete fitness of the will; immortality is postulated
  for it. \
  ✓ *supporting* — `@bible` (Matthew 5:48): the command "be perfect" set as a
  standard, not a report of arrival. \
  ⚠ *tension* — `@book-of-mormon` (Moroni 10:32): "perfected in Christ" reads as a
  reachable completion, not an endless approach. \
  ⚠ *against* — Kant's holy will is a limit the finite being never becomes; the
  approach is permanent, which strains "progression toward parity."
])

Read what that did. It did not tell us whether our claim is _true_ — that is not a
question this kind of writing can be held to. It told us where in our own corpus the
claim is _supported_ and where it is _strained_, and it named the passage for each.
The "tension" on Moroni 10:32 is the essay's second section arriving unbidden: the
sources themselves are telling us that "endless approach" holds for Kant and the
Sermon on the Mount but strains against the Book of Mormon's language of arrival.

#insight[
  `/relate` is the tool that makes a resemblance _precise_. "Kant and eternal
  progression rhyme" is a hunch; "they agree at Matthew 5:48 and the second
  _Critique_, and pull apart at Moroni 10:32" is a thesis with joints you can name
  and a critic can check. The graded scale is the whole point — a binary
  "agree/disagree" would have flattened the most interesting finding, which is that
  the sources _mostly_ align and strain at one identifiable seam.
]

#section("Find where the sources contradict")

`/relate` judges a claim against the corpus. `/contradict` turns the engine on the
corpus itself: it scans the Facts book for pairs that cannot both stand, and — reading
the provenance from Chapter 3 — reports each as _cross-source_ (two sources disagreeing)
or _within-source_ (one source, or one tradition's reading, contradicting itself):

#transcript("/contradict", [
  Found 2 contradiction(s) — 1 between sources, 1 within a source. \
  *[cross-source]* · "the creature never becomes holy; the gap is permanent"
  ⟨`@kant-cpr`⟩ ⇄ "man may become as God now is" ⟨`@book-of-mormon`⟩ — asymptote
  versus arrival: the gap Kant fixes, exaltation collapses. \
  *[within-source]* · "postulates are not knowledge claims" ⟨`@kant-cpr`⟩ ⇄ "we may
  hold immortality with certainty of moral faith" ⟨`@kant-cpr`⟩ — a tension internal
  to Kant's own account of how firmly a postulate may be held.
])

The cross-source contradiction is the spine of our essay's third movement — the one
place the two traditions do not merely strain but genuinely oppose. The within-source
one is a gift: it is a tension _inside Kant_, the kind of thing that makes a study
more than a comparison, and we would not have gone looking for it. The engine found
it because it read Kant's own claims against each other, not against the Saints.

#section("Find where they converge")

Contradiction has a confirmation-counterpart, and SCHOLAR reports it too.
`/converge` scans for pairs that _agree_, and marks each convergence as
_triangulated_ (independent sources arriving at the same claim — the strong kind)
or same-source:

#transcript("/converge", [
  Found 1 convergence(s) — 1 triangulated. \
  *[triangulated]* · "perfection is set as a standard, not reported as reached"
  ⟨`@bible`⟩ ≈ "the moral law commands a holiness no finite will attains at a
  moment" ⟨`@kant-cpr`⟩ — the Sermon on the Mount and the second _Critique_ arrive
  independently at perfection-as-trajectory.
])

A triangulated convergence is worth more than either source alone: when Kant and the
Sermon on the Mount reach the same shape of claim by different roads, "perfection is
a trajectory" stops being a reading you imposed and becomes one the sources
volunteer. That is the first section of the essay, earned.

#section("Question the whole with the Dialectician")

We met `/socrates` in Chapter 1, sharpening the bare thesis. Run it again now, over
the _gathered_ corpus, and it questions what you have actually assembled — the
questions the facts must answer to stand:

#transcript("/socrates the relation between the two accounts", [
  _The Dialectician asks:_ You have a convergence on "trajectory" and a
  contradiction on "arrival." Is arrival a _further_ claim the two traditions add to
  a shared core, or does it retroactively change what "trajectory" meant for each?
  If the Saints' trajectory _ends_ and Kant's does not, were they ever the same
  trajectory — or only the same word?
])

That is the essay's hardest paragraph, handed to us as a question. SCHOLAR detects;
the Dialectician interrogates. Used together, they map the corpus and then ask
whether the map holds.

#section("The persistent report")

Every one of those passes rendered its findings into the chat and, by default, into
a _persistent report_ on disk. `/report` renders the accumulated picture — every
contradiction, convergence, and relation you have found — clustered by topic, so the
whole interrogation is one document you return to across sessions:

#transcript("/report", [
  *SCHOLAR REPORT — contradictions, convergences, and relations* (updated 2026-07-13) \
  *Contradictions (2)* — ▸ arrival: asymptote vs. exaltation … ▸ postulates:
  Kant's internal tension … \
  *Convergences (1)* — ▸ trajectory: Bible ≈ Kant, triangulated … \
  *Relations to claims (1)* — ▸ "endless approach": 3 supporting · 2 against …
])

Because the report is keyed to the corpus, it knows when it has gone stale. Ingest a
new chapter or edit a fact, and the next `/report` prepends a warning that the facts
have changed since the findings were gathered — a prompt to re-run the passes before
you trust an old conclusion:

#note[
  The staleness warning is small and load-bearing. A study is written over weeks; the
  corpus grows the whole time. A conclusion drawn from `/contradict` on Tuesday may no
  longer hold after Thursday's new source, and nothing is easier to forget than to
  re-run the pass. The report carries an order-independent hash of the fact texts, so
  it can tell you — without your having to remember — that the ground moved under a
  finding you were about to cite.
]

#recap((
  [*SCHOLAR* grades how claims and sources stand to one another — contradiction *and* its confirmation-counterpart — where retrieval alone only finds the similar.],
  [`/relate` judges a claim against the corpus on a graded scale (contradicts · tension · qualifies · agrees · silent), naming the passage for each — turning a resemblance into a thesis with joints you can name.],
  [`/contradict` finds pairs that cannot both stand (cross-source vs. within-source); `/converge` finds pairs that agree (triangulated vs. same-source) — the spine and the shared core of the argument, read off the corpus.],
  [`/socrates` questions the *assembled* corpus (the Dialectician); `/report` renders the accumulated findings clustered by topic and *flags staleness* when the facts move under a conclusion.],
))
