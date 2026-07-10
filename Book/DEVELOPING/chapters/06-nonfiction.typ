#import "../design.typ": *

#chapter(number: 6, title: "The nonfiction track")

Nonfiction turns the fiction track inside out. You invent nothing; your ground is
the world as it actually is, and your one obligation is that the confident sentence
be true. Where the novelist builds a world and checks the prose against it, the
nonfiction writer _gathers what is known_ and checks each claim against that. The
tools shift accordingly: away from the world simulation, toward the research
assistant, the Facts book, and the fact-checker. This chapter is the loop for the
writer of the guide, the history, the argument, the memoir.

#section("Frame — set the genre and the structure")

Start from the nonfiction template and declare the genre:

```
inkhaven init "a-short-history" --template nonfiction
```

#config("inkhaven.hjson", [```hjson
genre: "nonfiction"
```])

`memoir` and `business` share this frame; the genre tells the readers to judge for
clarity, argument, and the reader's understanding rather than for imagery. Then,
unlike fiction, _plan first_. Nonfiction rewards a spine: use `inkhaven plan` and
the Outline (`Ctrl+2`) to lay out the argument before you write it, so each chapter
knows what it must establish and what it may assume.

#section("Gather — build a corpus you can trust")

This is the heart of the track. Instead of growing a world, you grow a *corpus* of
verified material, and the Research Assistant is where you do it.

```
inkhaven research
```

Inside it you ask questions in plain language and keep the answers as *Facts* — and
because a fact drawn from the open web or from the model is checkable, the assistant
grounds each on a real source: structured data from Wikidata, places from GeoNames,
scholarly papers, public-domain books, live web pages. Each fact carries its
_provenance_ — where it came from and how much to trust it — so months later you
can still answer "how do you know?"

#term("Provenance")[
  The recorded origin of a fact — its source and its rung on a trust ladder, from a
  deterministic computation down through a cited paper to an unverified model guess.
  On the nonfiction track provenance is not bookkeeping; it is the difference between
  a claim you can stand behind and one you merely remember reading somewhere. It
  travels with the fact silently and answers, at any time, the only question a
  reader really has.
]

The full craft of this — triangulating a claim across sources, upgrading a guess to
a cited fact, catching a source that has gone dead — is the subject of the
companion volume, _The Research Assistant with Inkhaven_. For the nonfiction track,
the essential habit is simple: _keep what you learn as facts with provenance, and
write from the corpus rather than from memory._

#section("Draft — write from the corpus")

Now the Facts book earns its keep. As you draft, the assistant can synthesise a
grounded overview of a topic from your kept facts alone (`/synthesize`), or turn
them into an outline you write into (`/outline`) with each point backed by a cited
fact and any gap marked _needs research_. You write into a structure that already
knows what it can support — and you see, plainly, where it cannot yet.

#section("Read — check the claims, not the world")

The nonfiction reading pass measures a different thing than fiction's. The
`inkhaven fact-check` command (or `Ctrl+B Shift+X`) audits your prose against your kept facts. Inside
the assistant, `/factcheck` sweeps the whole Facts book for per-claim accuracy and
for _contradictions between facts that are each fine alone_. And because a body of
nonfiction lives or dies on consistent terminology, the Glossary book governs your
terms — canonical words and their banned synonyms — with an overlay (`Ctrl+V z`)
that flags where the manuscript drifts.

#note[
  The AI readers serve this track too, with a different cast. Told a nonfiction
  genre, the Inner Socrates roster offers the audience personas the track needs: the
  `skeptical-practitioner` who will act on your advice, the `domain-newcomer` who
  has none of your context, the `expert-reviewer` who looks for the hole, and the
  `end-user` trying to get something done. Read a finished chapter through the one
  who most resembles your real reader.
]

#insight[
  Fiction's risk is a world that contradicts itself; nonfiction's is a _writer_ who
  contradicts the world. The whole apparatus — provenance, the Facts book, the
  fact-checker, the controlled glossary — exists to make the second failure as
  visible as the compiled world makes the first. You are not being asked to
  remember everything. You are being given a place to put it so you never have to.
]

#section("Produce")

Mark chapters ready and `export pdf|epub|docx`. If your nonfiction carries citations
— many do — the Sources book and its bibliography belong to you as much as to the
scientist; the next chapters on the scientific and scholarly tracks cover that
machinery in full, and everything there applies to any nonfiction that cites.

Two checks guard the citation itself. `inkhaven sources check` catches a `@key` you
cite but never defined. Its mirror, `inkhaven sources coverage`, catches the opposite
and more insidious slip — a sentence that makes a checkable claim (a statistic, a
date, a quotation, an attributed finding) and cites _nothing_. It lists each such
sentence so you can source it before you publish; where a passage is genuine common
knowledge, tag the paragraph `no-cite` and it stops asking.
While drafting, `Ctrl+V Shift+C` runs the same pass on the open paragraph and drops
the flags into the Output pane, right beside the `Ctrl+V @` cite picker you use to fix
them — a picker that now floats the sources most relevant to the paragraph you are
writing to the top, marked with a star, so the citation your claim needs is usually the
first one you see. And `sources coverage --ai` goes further: it finds subtler claims the plain scan
misses and checks each against your Facts book, telling you which claims your own
research already backs — just add the citation — and which still need a source.
Between them, every claim you make either carries a source or is a choice you made on
purpose.

A finished nonfiction book usually wants an index, and Inkhaven builds one from terms
you already curate. `inkhaven index` takes your Glossary's canonical terms (and any
extra names or topics you list under `docs.index.terms`), finds where each appears,
and writes an alphabetised index — a synonym becomes a _see_-reference to its
canonical term. It emits Markdown, Typst, or JSON, and on the web the index folds into
the site (`docs.html.include.index`) with every entry a real link to the section it
names.

There is also a first, modest look at the _shape_ of your argument. `inkhaven argue`
reads a chapter and writes back its central claims and the support each one rests on —
a citation, a line of reasoning, or nothing — and flags the two cheapest weak joints: a
load-bearing claim the text backs with nothing, and a citation that supports no claim at
all. It is deliberately small — an outline, not a diagram, and it quotes your own
sentences rather than inventing an argument on your behalf — but it is often enough to
show you the one place your case rests on air.

#section("Hands-on: two procedures")

#subsection("From a question to a cited paragraph")

+ Open the assistant: `inkhaven research`.
+ Ground a claim on a real source: `/web what was the average marching distance of a Roman legion?`. The answer comes cited; a `/fact` taken from it is fact-checked before it commits.
+ Keep it: at the confirmation gate, edit if needed and accept. The fact enters your Facts book carrying its provenance.
+ When you have gathered enough, build structure from the corpus: `/outline the logistics of a Roman campaign`. Each point is backed by a cited fact, and any gap is marked _needs research_.
+ Write into that outline, then check the prose: `inkhaven fact-check` (or `Ctrl+B Shift+X` on a paragraph) audits your sentences against the facts you kept.

#subsection("Set up continuity for a long work")

+ Seed the continuity book for your kind of nonfiction: `inkhaven facts init --genre historical` (the genre chooses the categories the book watches).
+ Find claims in your manuscript that belong in it: `inkhaven facts scan`.
+ Audit the whole book at once, including contradictions between facts that are each fine alone: `inkhaven facts check`, or `/factcheck` inside the assistant. Each fact is marked with a verdict glyph you can see in the Facts tree.

#recap((
  [Nonfiction *inverts* fiction: you invent nothing, your ground is the world as it is, and your obligation is that each claim be true — so plan the argument first (`plan`, `Ctrl+2`).],
  [*Gather* a trustworthy corpus in the Research Assistant (`inkhaven research`): keep answers as *Facts* grounded on real sources, each carrying its *provenance*.],
  [*Draft from the corpus* — `/synthesize` and `/outline` build grounded structure with cited points and marked gaps — rather than from memory.],
  [*Read* with `fact-check` and `/factcheck` (per-claim accuracy + contradictions), govern terms with the Glossary, and read chapters through the *audience personas* that match your real reader.],
))
