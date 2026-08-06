#import "../design.typ": *

#chapter(number: 17, title: "Continuity and Knowledge")

A short book can be held in one head. A long one cannot, and it is the
things you cannot hold that break it: a character who is standing in two
cities on the same afternoon, a river that runs six days wide in chapter
three and a morning's walk in chapter nine, a ferryman named twice before
he is ever introduced, a detective who names the murder a chapter before he
could possibly have learned of it. None of these are mistakes of prose.
Every sentence is clean; the fracture is between the sentences, across a
span no single reading holds in view at once. They are the errors a
manuscript accumulates precisely because it is long — and they are the ones
a careful reader will catch and you will not.

Inkhaven answers with two intelligences that read the whole book at once and
watch for exactly these cross-page fractures. *SENTINEL* watches
*continuity* — where and when, how many, what was established and then
quietly changed. *KEN* watches *knowledge* — who could know what, and by
when. They are siblings by design: KEN is SENTINEL's cardinal invariant,
*referenced before introduced*, carried one step further from *does this
thing exist yet* to *could this person know this yet*. Both flag and never
rewrite; both are deterministic and free at the core, spending nothing and
running in a blink whatever the book's length; both feed the same review
pass and the same revision worklist you will meet in the next chapters.

This chapter is the operator's tour: what each detects, how to run it from
the command line and from the editor, and how to read what comes back. It is
deliberately breadth-first. The full treatment — the theory of each
detector, the design of the grant model, the worked examples — lives in the
companion book *Know Your Book*, which this chapter points you toward
wherever the depth runs past what an operator needs to get moving.

#callout(label: "Advisory, always")[
  Neither intelligence ever touches your prose. They read the manuscript,
  the timeline, the world books, and the knowledge graph, and they write
  *findings* — anchored notices you can jump to and act on. Every actual
  edit stays yours, routed through the Editorial Pass of Chapter 19 with its
  snapshot-first, diff-reviewed contract. A finding is a pointed finger, not
  a red pen.
]

#section("SENTINEL — the book watches itself")

Inkhaven could always *check* a manuscript's continuity — but for years that
meant knowing which of six separate commands to run, each with its own
sidecar file and its own mental model. SENTINEL is the layer that folds them
into *one always-watching concern*. One engine runs every deterministic
continuity detector already in the tree, normalises each into a single
finding shape, dedupes them, and ranks the result — contradictions first,
then warnings, then information, and within a severity the earlier chapters
first. What you get back is a single ordered ledger, not six piles to
reconcile by hand.

#subsection("What it detects")

Five detectors feed the ledger. Four were scattered across the tool before
SENTINEL unified them; the fifth is the invariant nobody had.

#screen(caption: "The five deterministic detectors")[```
 detector      break it catches            reads
 -----------   -------------------------   -----------------
 co_location   a character in two places   the timeline
               at overlapping times        (magic suppressed)
 timeline      orphaned events · fuzzy-     the timeline
               precision overlaps          critique
 numeric       a direction reversed · a    prose quantities
               duration that conflicts     (EN / FR / ES)
 char_facts    an established fact changed  continuity.json
               across chapters             (the bible)
 introduce     an entity referenced        the graph +
               before it is introduced     prose mentions
```]

The first four are recognitions of a kind you would make yourself if you
could hold the whole book in mind: two scenes that put the same person in
two places at once, a timeline event with nothing anchoring it, a distance
that shrinks between chapters with no reason given, an eye colour or a
hometown that drifts. The `char_facts` detector leans on the *continuity
bible* — the per-character, per-attribute record that `inkhaven continuity
extract` builds with an AI pass and `inkhaven continuity list` dumps; once
extracted, the drift check over it is pure comparison, deterministic and
free.

#subsection("The invariant nobody had — referenced before introduced")

The `introduce` detector is the one SENTINEL adds, and it is worth a moment.
An entity's *introduction* is its first scene — the earliest paragraph any
of its timeline events touch. A *reference* is any mention of it in the
prose. When the first reference lands in a chapter earlier than the
introduction, by more than a configured tolerance, SENTINEL flags it:

#screen(caption: "The referenced-before-introduced finding")[```
⊗ [introduce] "Aldous the ferryman" is referenced in
  ch. 2 but not introduced until ch. 5.
```]

This is the fracture a first-time reader feels as a small stumble — *who?* —
and that you, who know Aldous intimately, read straight past every time. It
is language-safe everywhere, because the names come from your own Characters
and Places books and the mention match is Unicode-aware; a Cyrillic or an
accented name is matched exactly as a Latin one is.

#term("Deterministic")[
  A check is *deterministic* when it computes its answer from structure —
  the timeline, the graph, the tagged prose — with no model in the loop, so
  it is free, instant, and gives the same answer every run. SENTINEL's five
  detectors are all deterministic. The one fuzzy check it can invoke, the
  coherence pass, is kept explicitly out of the sweep for exactly that
  reason.
]

#subsection("Running it — inkhaven continuity check")

The whole ledger is one command, and it is built to drop into a script or a
CI gate as readily as into your own terminal.

#screen(caption: "The command and its flags")[```
inkhaven continuity check
    [--only DETECTOR]...     # run only these (repeatable)
    [--skip DETECTOR]...     # skip these (after --only)
    [--json]                 # machine-readable array
    [--coherence             # + the slow LLM pass
       [--max-cost 8000]     #   soft token budget
       [--force]]            #   ignore the cache
```]

The detector names are exactly the five from the table — `co_location`,
`timeline`, `numeric`, `char_facts`, `introduce`. Name one wrong and the
command tells you so and lists the known ones, rather than silently checking
nothing. A plain run prints the ranked findings with a severity glyph on
each — `⊗` a contradiction, `⚠` a warning, `●` information — the detector
that raised it in brackets, and the chapter it sits in.

#screen(caption: "A run with two findings")[```
$ inkhaven continuity check
⊗ [co_location] Mara is at the quay and at Rillmark
  on the third evening. (ch. 4)
● [introduce] "Aldous the ferryman" is referenced in
  ch. 2 but not introduced until ch. 5.

2 finding(s): 1 contradiction(s), 1 other.
```]

The one behaviour to build a habit around: *the command exits non-zero when
any contradiction survives.* A clean book, or one whose only findings are
warnings and information, exits zero. That single rule is what lets
`inkhaven continuity check` sit in a pre-commit hook or a CI pipeline as a
gate — the build fails on a hard continuity break the way it fails on a
compile error. Add `--json` and the same findings come back as an array of
objects (`kind`, `severity`, `chapter`, `anchor`, `entities`, `message`,
`source`) for a script to read.

#callout(label: "--only and --skip")[
  The two selectors compose in one order: `--only` narrows to the named
  detectors, then `--skip` removes from whatever remains. `--only introduce`
  runs the invariant alone; `--skip numeric` runs the other four. Reach for
  them when you are chasing one class of break, or when a detector's
  language coverage does not fit the book — `numeric` reads English, French,
  and Spanish quantities and skips cleanly for other languages, so on a
  German or Russian manuscript you lose nothing by skipping it explicitly.
]

#subsection("The ledger dashboard — Ctrl+B Shift+I")

Inside the editor, the same ranked ledger is a keystroke away. `Ctrl+B
Shift+I` opens the *continuity ledger* — a scrollable modal of every
deterministic finding, grouped by kind and ranked as on the command line.
`↑↓` scroll it; `Enter` jumps straight to the offending paragraph so you can
see the break in context; `Esc` closes it. It is the fastest way to walk a
book's continuity without leaving your place in it.

#screen(caption: "Ctrl+B Shift+I — the continuity ledger")[```
┌─ Continuity ledger · 3 findings ────────────────────┐
│ CONTRADICTIONS                                      │
│ ▌⊗ co_location  Mara: the quay & Rillmark, 3rd eve  │
│    ch. 4 · the-crossing                             │
│                                                     │
│ WARNINGS                                            │
│  ⚠ numeric  "six days' ride" → "a morning" ch. 9   │
│                                                     │
│ INFO                                                │
│  ● introduce  "Aldous the ferryman" ref. ch. 2      │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll · Enter jump · k coherence pass · Esc     │
└─────────────────────────────────────────────────────┘
```]

The `k` key in this dashboard is the one that costs — it runs the slow
coherence pass, described just below, over the open book. Everything else in
the ledger is deterministic and free, which is why the dashboard opens
instantly no matter how long the manuscript is.

#subsection("The slow coherence pass")

There is one fuzzy check SENTINEL will *invoke* but will never run on its
own. An LLM reads a run of paragraphs and flags the cross-paragraph
contradictions the deterministic detectors cannot see — a fact asserted and
then quietly reversed, a time of day that cannot follow the one before it,
the kind of soft inconsistency that lives in the meaning rather than in a
number or a name. Because it costs, it is *explicit, cost-capped, and
opt-in* on both surfaces:

#screen(caption: "The two ways to run the coherence pass")[```
CLI     inkhaven continuity check --coherence
                [--max-cost 8000] [--force]

editor  k  in the Ctrl+B Shift+I ledger dashboard
           (results land in Output · source coherence)
```]

The pass respects the `magic:` ledger's declared exceptions — a world where
someone genuinely can be in two places is not a bug — and it needs a
configured LLM provider. Its cost is previewed against your daily cap before
it runs. As everywhere in Inkhaven, that cost *informs*; it never blocks.
`--max-cost` sets a soft token budget for the preview, and `--force` ignores
the cache to re-run a pass you have run before.

#subsection("The ambient watch")

The last surface is the quietest. Turn on `continuity.ambient` and SENTINEL
re-checks continuity on *every save* — but only over what the edit actually
touched. It reads the edited paragraph's entities and chapter from the
graph, re-runs the deterministic detectors against just that scope, and
surfaces the delta immediately in the Output pane. Because the core is
deterministic and free, this happens inline, without a background job and
without a pause you would notice. It is off by default; when you turn it on,
the book watches itself as you write, and a break you introduce is flagged
in the same breath you introduce it.

#callout(label: "Cooldown")[
  The ambient watch throttles itself with `ambient_cooldown_secs` (default
  30) so a burst of rapid saves does not re-check on every keystroke-flush.
  The floor is a throttle, not a queue — it collapses a flurry of saves into
  one re-check rather than running the check that many times.
]

#subsection("Configuration")

Everything above is governed by one config block, all fields optional; the
values shown are the defaults.

#screen(caption: "The continuity: block")[```
continuity: {
  enabled: true             // review-pass ledger switch
  ambient: false            // re-check on every save
  ambient_cooldown_secs: 30 // throttle floor (seconds)
  co_location: true         // per-detector toggles
  timeline: true
  numeric: true
  char_facts: true
  introduce: true
  introduce_tolerance: 0    // chapters tolerated (0=strict)
}
```]

Two knobs deserve a word. `enabled` is the master switch for the *review-pass
line* only — turn it off and the deterministic ledger stops riding the
`Ctrl+B Shift+C` review pass, but the standalone `inkhaven continuity check`
command still runs, because you invoked it explicitly. And
`introduce_tolerance` is how much *referenced-early* the invariant will
forgive: `0` is strict, flagging any reference before the introduction;
raise it to allow a name to appear a chapter or two ahead of its scene
without complaint, for a book that deliberately foreshadows.

#subsection("From a script — the Bund words")

The ledger is readable from the embedded Bund language of Part VIII, so a
hook or a script can gate on it. Two words, both read-only and deterministic
— the coherence pass, because it costs, is deliberately *not* exposed to
Bund.

#screen(caption: "ink.continuity.* — read-only")[```
ink.continuity.findings  ( -- list )
    the ranked, deduped findings as dicts:
    {kind, severity, chapter, source,
     message, entities}

ink.continuity.check     ( -- dict )
    summary counts:
    {total, contradictions, warnings,
     info, by_kind}
```]

#subsection("What SENTINEL is, and is not")

It is the *unification* of the continuity detectors you already had, plus
the one invariant nobody had — not a new pile of them. It is
*deterministic-first*: the fuzzy passes stay explicit and cost-capped, and
the core is free. It is *advisory*: it flags, it never rewrites, and it adds
no new runtime dependency to the tool. Every finding carries the detector
that raised it, so the honest question — *does this work in Russian?* —
answers itself per detector: `introduce`, `co_location`, `timeline`, and
`char_facts` are multilingual as built, and `numeric` names its EN / FR / ES
coverage and skips cleanly elsewhere.

#section("KEN — who knows what, when")

Inkhaven watches several kinds of continuity — where and when (SENTINEL),
whether an entity exists yet (the `introduce` invariant), whether the world
stays consistent (Facts), whose head we are in (the voice reader's POV). None
of them watch the one axis that breaks *plots*: what a character *knows*. KEN
is that watch. It takes SENTINEL's cardinal move — flag a thing *named
before it exists* — and carries it into knowledge: flag a character *acting
on a fact before they could have learned it*. That is a mystery's cardinal
sin and the most common invisible plot-hole in any book with a secret in it.

What makes KEN a *native* intelligence — a finding a generic AI could not
produce — is that reconstructing who-knows-what across a whole book needs the
timeline, the event-participant lists, the character bible, and scene POV all
at once, and those are exactly what Inkhaven already maintains and a chat
model does not have. KEN never guesses what a character knows. It *derives*
it.

#subsection("The grant — when could they know it?")

The heart of KEN is the *grant*: the earliest point at which a character
could know a given topic. It is derived two deterministic ways, never
guessed.

#screen(caption: "The two sources of a grant")[```
PRESENCE   a character in a timeline event's
           participant list knows that event from
           the moment it happens — free, from the
           structure you already keep.

DECLARED   you mark it with a tag:
  secret:<topic>        <topic> is a secret; a
                        reference by anyone ungranted
                        is a leak.
  know:<topic>          grants the scene's POV char.
  know:<topic>@<name>   grants a named character.
  reveals:<topic>       on an event's paragraph, binds
                        a terse title to the topic it
                        reveals (a matchable handle).
```]

A *use* — a character referencing a topic — is caught the same deterministic
way SENTINEL matches names: by Unicode-aware matching of the topic in that
character's *attributed dialogue* or in the *narration of their POV scene*.
Presence and the tags grant knowledge; a use before any grant is a break.

#term("Ken")[
  A character's *ken* is the range of what they know — the set of topics
  their grants cover, up to a given point in the book. KEN the feature is
  named for it. The whole check is a forward walk that builds each
  character's ken chapter by chapter and flags any use that outruns it.
]

#subsection("What it catches")

Four findings, three of them deterministic and free, the fourth an opt-in
LLM pass for the subtle case the structure cannot see.

#screen(caption: "The KEN findings")[```
premature_knowledge   a character references a topic
   (⊗ break)          before their earliest grant.
                      "Bob names the murder in ch. 4;
                       he learns of it in ch. 6."

leaked_secret         a secret: topic referenced by a
   (⊗ break)          character never granted it.

dropped_reveal        a declared know: reveal whose
   (● notice)         topic never surfaces again —
                      dangling knowledge, the
                      epistemic unpaid setup.

implied_irony         a character acts informed or
   (--deep, opt-in)   ignorant without naming the
                      topic — the subtle case only an
                      LLM can see.
```]

The first two are *breaks* — the hard errors. `dropped_reveal` is a softer
notice: you told the tool a reveal happens, and then the topic never comes
back, which is either a thread you forgot to pay off or a tag you can
retire. `implied_irony` is the only one that costs, and the only one that is
not on by default.

#subsection("Running it — inkhaven knowledge")

#screen(caption: "The command and its flags")[```
inkhaven knowledge          # the deterministic check
inkhaven knowledge --json   # findings as JSON
inkhaven knowledge --deep   # + the implied_irony pass
    [--max-cost 8000]       #   soft budget for --deep
    [--book-name SLUG]      #   restrict to one book
```]

A plain run prints each finding with its severity glyph — `⊗` a break, `●` a
notice, `·` information — its kind in brackets, and its message.

#screen(caption: "A run with two breaks")[```
$ inkhaven knowledge
⊗ [premature_knowledge] Bob speaks of "the murder"
  in ch. 4 — before learning it in ch. 6.
⊗ [leaked_secret] Sella references "the heir's true
  name" in ch. 3 — never established to know it.

2 finding(s): 2 break(s), 0 other.
```]

Like `continuity check`, `knowledge` *exits non-zero on any hard break*
(`premature_knowledge` or `leaked_secret`), so it is a CI gate too. And it is
*self-gating*: with no `secret:` or `know:` tags and no timeline events,
there is simply nothing to check, so it does nothing and costs nothing. You
pay for KEN only in proportion to the epistemic structure you have actually
declared. `--book-name` narrows a multi-book project to one book; without it,
the whole project is checked.

#subsection("The knowledge dashboard — Ctrl+B Shift+Z")

In the editor, `Ctrl+B Shift+Z` opens the *knowledge dashboard* — the
findings grouped by kind, the same shape as the continuity ledger. `↑↓`
scroll; `Enter` jumps to the offending paragraph; `Esc` closes.

#screen(caption: "Ctrl+B Shift+Z — the knowledge dashboard")[```
┌─ Knowledge · who knows what, when · 3 ──────────────┐
│ BREAKS                                              │
│ ▌⊗ premature_knowledge  Bob: "the murder" ch. 4     │
│    grant: ch. 6 · the-confession                    │
│  ⊗ leaked_secret  Sella: "the heir's true name"     │
│    ch. 3 · never granted                            │
│                                                     │
│ NOTICES                                             │
│  ● dropped_reveal  "the ledger" declared, never     │
│    surfaces again                                   │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll · Enter jump · Esc close                  │
└─────────────────────────────────────────────────────┘
```]

#subsection("The deep pass — implied_irony")

The whole KEN core is a forward walk, set membership, and Unicode mention
matching: *no model, about zero cost, independent of book length.* Cost
scales with declared topics and scenes, not pages. The *only* LLM touchpoint
is the opt-in `--deep` pass, which hunts the case the structure cannot reach
— a character who *acts* on knowledge, or *acts* ignorant, without ever
naming the topic, so no mention match can catch it. It runs cost-capped
under the daily cap, on the same rail as the world fact-checker, never
automatically and never over the whole book at once. There is, by deliberate
design, no *have the AI judge what everyone knows* pass — that would be
guessing, and KEN does not guess.

#subsection("From a script — the Bund words")

Three read-only words expose the grants and the findings. The `--deep` pass,
because it costs, is not among them.

#screen(caption: "ink.knowledge.* — read-only")[```
ink.knowledge.grants   ( -- list )
    the who-could-know-what ledger:
    {character, topic, chapter, source}

ink.knowledge.findings ( -- list )
    the deterministic breaks:
    {kind, severity, chapter, character,
     topic, message}

ink.knowledge.check    ( -- dict )
    {premature, leaked, dropped, clean}
```]

The `clean` field of `ink.knowledge.check` is `true` when no hard break
stands — a one-word pre-submit gate for a hook.

#subsection("What KEN is, and is not")

It is not an all-knowing oracle: it reasons only over what it can ground —
events, tags, named mentions — and stays *silent* where it cannot, rather
than inventing a break. It is not a fact-checker: Facts asks *is the world
consistent*, KEN asks *could this character know this yet* — a different
axis entirely. It is not LLM-first, and it is not a rewriter. Its
multilingual reach it inherits from SENTINEL and the dialogue-attribution
layer: the mention matcher is Unicode-aware, and attribution ships English,
Russian, German, French, and Spanish conventions, so topics and names are
matched in the project's language.

#section("One core, two feeds")

The two intelligences share a shape worth naming plainly, because it is what
makes them cheap enough to leave on. Both are *deterministic and free at the
core* — a forward walk over structure you already keep, no model, the same
answer every run, a cost that does not grow with the book. Both keep their
one fuzzy pass *explicit, opt-in, and cost-capped* — SENTINEL's coherence
pass behind `--coherence` and the `k` key, KEN's implied-irony pass behind
`--deep` — so the price is only ever paid on purpose.

And both pour their findings into the same two downstream surfaces you will
meet next. The first is the *review pass*, `Ctrl+B Shift+C`, which runs every
fast checker at once: SENTINEL's ledger rides it under the `continuity`
source, and KEN's breaks join it too, each finding anchored so you can jump
straight to it in the Output pane. The second is the *Editorial Pass*,
`Ctrl+V Shift+R` — the revision worklist of Chapter 19 — where continuity and
knowledge findings take their place in one ranked list beside every other
reader. There, a knowledge break is routed as a *decision* (*fix the leak,
move the reveal, or add a grant?*) rather than a mechanical rewrite, because
resolving one is a choice about the story, not a phrasing. `inkhaven revise`
synthesises that same worklist into an editorial letter. The intelligences
watch; the Editorial Pass is where watching turns into an edit — under the
snapshot-first, diff-reviewed contract that means no word of your prose ever
changes without your say-so.

#two_track(
  [For *fiction* this pair is close to the whole reason the world and
  timeline exist. Keep the timeline's event-participant lists honest and tag
  your secrets with `secret:` / `know:` / `reveals:`, and KEN will catch the
  leaked confidence and the too-early deduction that a mystery lives or dies
  by — the errors beta readers find and you cannot.],
  [For *non-fiction* SENTINEL still earns its place: the `introduce`
  invariant flags a term, a person, or an acronym used before it is defined —
  the reference-before-introduction that trips a reader of a manual or a
  monograph exactly as it trips a reader of a novel. KEN's grant model is
  fiction-shaped and simply finds nothing to check.],
)

#chord_table((
  chord_row("Ctrl+B Shift+I", "Open the SENTINEL continuity ledger — deterministic findings, ranked and grouped."),
  chord_row("Enter", "In the ledger, jump to the finding's paragraph."),
  chord_row("k", "In the ledger, run the opt-in, cost-capped LLM coherence pass over the open book."),
  chord_row("Ctrl+B Shift+Z", "Open the KEN knowledge dashboard — who knows what, when, grouped by kind."),
  chord_row("Ctrl+B Shift+C", "The review pass — runs every fast checker; SENTINEL and KEN ride it."),
  chord_row("Ctrl+V Shift+R", "The Editorial Pass — the revision worklist both intelligences feed."),
))

#recap((
  [*SENTINEL* watches *continuity* — five deterministic detectors
  (`co_location`, `timeline`, `numeric`, `char_facts`, and the
  *referenced-before-introduced* `introduce` invariant), deduped and ranked
  into one ledger.],
  [`inkhaven continuity check` runs it — `--only` / `--skip` select
  detectors, `--json` for scripts, `--coherence` for the opt-in LLM pass —
  and *exits non-zero on any contradiction*, so it gates CI.],
  [`Ctrl+B Shift+I` opens the ledger dashboard (`↑↓` scroll, `Enter` jump,
  `k` runs the coherence pass); `continuity.ambient` re-checks the edit's
  scope on every save, deterministic and inline.],
  [*KEN* watches *knowledge* — a character's *ken* is derived from timeline
  *presence* plus the `secret:` / `know:` / `reveals:` tags, never guessed;
  it flags `premature_knowledge`, `leaked_secret`, and `dropped_reveal`.],
  [`inkhaven knowledge` runs it (also non-zero on a hard break, and
  self-gating); `--deep` adds the opt-in `implied_irony` LLM pass;
  `Ctrl+B Shift+Z` opens the dashboard.],
  [Both are *deterministic and free* at the core, keep their one fuzzy pass
  *explicit and cost-capped*, expose read-only `ink.continuity.*` /
  `ink.knowledge.*` Bund words, and feed the `Ctrl+B Shift+C` review pass
  and the `Ctrl+V Shift+R` Editorial Pass. The full treatment is in *Know
  Your Book*.],
))
