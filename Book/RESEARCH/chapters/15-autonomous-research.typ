#import "../design.typ": *

#chapter(number: 15, title: "Research That Researches Itself")

`--batch` answered a list of questions you *wrote*. But the hardest part of research
is often not answering the questions — it is knowing which questions to ask. You start
with a topic and a vague sense that there is more you don't know than you do. This
chapter is about the tools that close that gap: research that plans its own questions
and follows its own leads. Two of them — an autonomous research *loop*, and citation
*snowballing* — and one principle that governs both: they fill your fact base and your
sources, but they never decide what you trust.

#section("`--agentic` — research that plans itself")

Give the assistant a *topic* rather than a question, and it does what a diligent
researcher does: breaks the topic into the specific questions it raises, answers each
one against your sources, writes down what it finds, and then looks at what it has and
asks what's still missing — and keeps going until the topic is covered or its budget is
spent.

```
inkhaven research --agentic "the 1918 pandemic"
```

Watch a run unfold. Each *round* plans a batch of sub-questions, researches them, and
emits a Fact — *straight into your Facts book* — for each; then a critic proposes
follow-ups for the gaps:

#screen(caption: "inkhaven research --agentic — a run in progress")[```
» agentic round 1: 5 sub-question(s)
· What caused the 1918 influenza pandemic?
· How many people did it kill worldwide?
· Why was it called the "Spanish" flu?
· Which age groups were most affected?
· How did the pandemic end?
! contradiction: "~50 million dead" ⇄ "17–100 million,
  a contested range"
» agentic round 2: 2 sub-question(s)
· What explains the wide range in the death-toll estimates?
· Why did it kill healthy young adults so often?
✓ agentic run complete — 7 Fact(s) over 2 round(s),
  converged · ! 1 contradiction(s) — resolve before
  trusting (model provenance, untrusted).
  Triage in the Facts book: /review, then /factcheck.
```]

Three things just happened that are the whole point of the feature. It *wrote its own
questions* — you never listed them. It *noticed a contradiction* between two facts it
had gathered and, in the next round, asked a question aimed squarely at resolving it.
And it *stopped on its own* — not after a fixed number of steps, but when the critic
had nothing left to add.

#subsection("The result is facts in your Facts book — nothing else")

There is no document to open at the end of a run, and that is the point. Agentic research
is not a report generator; it is a way to *build your fact base*. Every finding is written
straight into your Facts book as an ordinary Fact — a paragraph carrying its provenance,
tagged `agentic`, wearing the `·` model tier, and marked *untrusted*, exactly as if you had
researched it by hand and not yet confirmed it. When the run ends, you open the Facts tree
and the findings are simply *there*:

#screen(caption: "the Facts book after the run — the actual result")[```
Facts
└ the 1918 pandemic
   · Cause of the 1918 pandemic
   · Worldwide death toll                ≠
   · Origin of the "Spanish flu" name
   · Which age groups were hit hardest
   · How the pandemic ended
   · Why the death-toll estimates vary so widely
   · Why it killed healthy young adults

7 new Facts — model (·), agentic thread, untrusted.
Nothing here is trusted until you say so (/review).
```]

So an agentic run leaves you exactly where a good afternoon in the library would: with
a stack of sourced notes, some of which disagree, none of which you have decided to
believe yet. The trust ladder (Chapter 8) and everything on it — fact-checking,
refutation, the undisputed mark, and the `/review` queue (Chapter 16) — apply to these
facts natively, because they *are* ordinary facts. Nothing about the pipeline is bypassed;
research from a topic just got faster. You still decide, one fact at a time, what is true.

#subsection("An optional receipt — not a document to write from")

If you want a record of *what the run did* — which sub-questions it asked, which facts it
emitted, which contradictions it found, where it stopped — pass `--out` (or just read the
same summary it prints to the terminal). Understand what this is: a *run log*, an audit
trail of the session. It is emphatically *not* a research article, and you do not write
your book from it. The facts it lists already live in your Facts book; the log only
narrates how they got there.

```
inkhaven research --agentic "the 1918 pandemic" --out run-log.md
```

#screen(caption: "run-log.md — an audit trail of the run, not a document")[```
# Agentic research report

**Topic:** the 1918 influenza pandemic

7 sub-question(s) · 2 round(s) · 7 Fact(s) emitted into
the Facts book (untrusted — review) · stopped: converged.

## ! Contradictions among the emitted facts (1)

- **~50 million died worldwide** ⇄ **estimates range
  from 17 to 100 million** — settled figure vs. contested

## 1. What caused the 1918 influenza pandemic?
**Cause of the 1918 pandemic** → facts/cause-1918
_confidence 0.86 · the fact now lives in the Facts book_
…
```]

Read it as a receipt and throw it away; the corpus it describes is what you keep. If you
never pass `--out`, you lose nothing — the facts are already home.

#callout(label: "Why it flags its own contradictions")[
  The risk of any tool that writes facts automatically is that it quietly fills your
  base with claims that disagree with each other. So after each round the agentic loop
  runs the same `⇄` contradiction check you'd run by hand (Chapter 8) over *its own*
  emitted facts, feeds any clash back to the critic to resolve, and flags what remains
  at the top of the report. Autonomous emission is only safe because the conflicts it
  introduces are surfaced, not hidden — review is targeted, not a blind audit.
]

#subsection("Where it stops, and the switch that turns it off")

An autonomous loop must never run away. This one stops at the first of three bounds,
and the run always tells you which: it *converges* (the critic finds no more gaps),
it hits the *round cap*, or it spends its *question budget* — a hard ceiling on how
many facts one run can emit, and therefore on its cost. Sub-questions dropped for want
of budget are logged, never silently cut.

All three are yours to set, and the whole behaviour is a switch — on by default, off
whenever you want it off. In your project's `inkhaven.hjson`:

```hjson
research: {
  agentic: {
    enabled: true          // false disables agentic runs
    max_subquestions: 6    // total Facts budget per run
    max_rounds: 3          // iterate rounds (1 = one pass)
  }
}
```

With `enabled: false` the command simply refuses with a hint; your ordinary,
hands-on research is untouched. Autonomy is a tool you pick up, never one that is
running whether you asked for it or not.

#section("`--snowball` — following the citations")

The other way research grows is not by asking more questions but by following the
*sources* you already have. One good paper cites a dozen others and is cited by a
hundred more; that neighborhood is where the rest of the literature lives. Snowballing
walks it for you. Give it a seed — a title, a DOI, a topic — and it follows the
citation graph both ways:

```
inkhaven research --snowball "attention is all you need"
```

#screen(caption: "inkhaven research --snowball — the citation neighborhood")[```
· seed: Attention Is All You Need  (openalex:W2626778328)

## References (backward) — 10 works the seed cites
- Deep Residual Learning for Image Recognition
    — He et al., 2016    openalex:W2194775991
- Effective Approaches to Attention-based Neural MT
    — Luong et al., 2015    openalex:W1902237438
- Generating Sequences With Recurrent Neural Nets…

## Citations (forward) — 10 works that cite the seed
- An Image is Worth 16x16 Words (Vision Transformer)
    — Dosovitskiy et al., 2020    openalex:W3094502228
- DistilBERT, a distilled version of BERT — Sanh, 2019
- Highly accurate protein structure prediction with
    AlphaFold — Jumper et al., 2021    openalex:W3177…
```]

*Backward* are the works your seed stands on; *forward* are the works that stand on it,
most-cited first — often the reviews and landmark follow-ups that tell you where a line
of research went. In one command you've turned a single paper into a map of its
scholarly surroundings.

#subsection("It surfaces, it doesn't swamp")

Notice what snowball did *not* do: it did not drag thirty papers into your Sources book.
It reported them, each with its identifier, and left the choosing to you — because a
research corpus is only useful if it's *curated*, and thirty auto-ingested papers of
uneven relevance is not a corpus, it's a mess. Bring in the ones that matter with
`/openalex` (Chapter 5); the rest you now simply know exist. Same discipline as the
agentic loop: the machine widens your reach, you decide what enters.

#recap((
  [`--agentic "<topic>"` researches a topic *autonomously*: it plans its own
   sub-questions, answers each, and *emits the findings as untrusted Facts into your
   Facts book* — never a standalone article — so the whole trust ladder applies to them.],
  [It *iterates* — a critic proposes follow-ups for the gaps — and stops on the first of
   three bounds it reports: converged, round cap, or question budget. It never runs away.],
  [After each round it runs the `⇄` contradiction check over its *own* facts, feeds
   clashes back to the critic, and flags what remains — so autonomous emission stays
   trustworthy and review is targeted.],
  [The whole behaviour is `research.agentic` in HJSON — *on by default*, and switched off
   with `enabled: false`.],
  [`--snowball "<seed>"` follows a paper's citations *backward* (its references) and
   *forward* (its citers) on OpenAlex, reporting the neighborhood for you to ingest
   selectively — widening your sources without swamping them.],
))
