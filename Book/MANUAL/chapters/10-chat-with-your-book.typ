#import "../design.typ": *

#chapter(number: 10, title: "Chat With Your Book")

An assistant that has never read your book can only ever guess at it. Ask a
general model whether Mara doubts the captain and it will write you a fluent,
confident paragraph about a character it has never met — plausible, ungrounded,
and wrong as often as not. The panes you met in Chapter 3 already give you a
conversation; this chapter is about what that conversation is *grounded in*. The
whole point of writing inside your own material is that the assistant can be made
to answer *from* it — to retrieve the passages that actually bear on your
question, quote them back by name, and refuse to invent what the text does not
say. That is the difference between chatting *with* a model and chatting *with
your book*.

The control that governs all of this is a single key, `F9`, which cycles the AI
pane's *scope*. You have already met the near end of that dial — the scopes that
send the model a slice of prose around your cursor. This chapter is about the far
end: the *structured scopes*, where the pane stops shipping raw text and starts
*retrieving*, grounding each answer in evidence it can cite. Book, Facts, Graph,
and the two reader conversations all live there, and each grounds in a different
part of what Inkhaven knows about your work.

#section("The scope dial — F9 and what an answer stands on")

Every question you send the AI pane carries a scope, and the scope decides what
context travels with it. Press `F9` from anywhere and the scope advances one
notch; the pane's title and the status bar both name the current one
(`scope=Book`). Ten stops sit on the ring, and they fall into two families.

#term("AI scope")[
  The *scope* is what the AI pane sends the model alongside your prompt. `F9`
  cycles it. The first five stops are *manuscript slices* — a literal span of
  prose around your cursor. The last five are *structured scopes* — Book, Facts,
  Graph, Socrates, and Editor — which retrieve or assemble grounding rather than
  sending text verbatim, and are *sticky*: the conversation stays in that scope,
  reasoning over the same evidence, until you cycle away.
]

The first family is the plain one. `None` sends nothing but your words;
`Selection` sends the highlighted span; `Paragraph`, `Subchapter`, and `Chapter`
send exactly the prose those names enclose. What you see is what the model gets.
These are covered where they belong, in the editor chapters; they are unchanged
and unsurprising.

The second family is the subject of this chapter, and it behaves differently on
purpose. A Book-scope question does not send your book — it *searches* it. A
Facts-scope question loads the world's invariants as ground truth. A Graph-scope
question folds in how your book's parts connect. Socrates and Editor seat a
particular reader across the table from you. None of these send a fixed span of
text; each *builds* its grounding from a different store, and each is worth
understanding on its own.

#screen(caption: "F9 walks the scope ring; the last five are structured")[```
  manuscript slices          structured (retrieval / grounded)
 ┌────────────────────┐   ┌──────────────────────────────────────┐
 │ None → Selection → │   │ Book → Facts → Socrates → Editor →   │
 │ Paragraph →        │ → │ Graph → (wraps back to None)         │
 │ Subchapter →       │   │                                      │
 │ Chapter ───────────┼───┘ each retrieves or grounds; sticky    │
 └────────────────────┘                                          │
         send text                assemble evidence, then answer
```]

#callout(label: "Sticky scopes retrieve once")[
  A structured scope holds its ground across a whole conversation. It gathers its
  evidence when you enter it (or send the first question), and your follow-ups
  reason over that same evidence rather than re-gathering on every turn. To make
  it look again — a new line of inquiry, or freshly edited prose — clear the chat
  history, and the next question re-grounds from scratch.
]

#section("Book scope — retrieval-augmented conversation")

Book scope is the flagship of this chapter, and the feature it implements has a
name worth learning, because it is the same idea that quietly powers half the
tool.

#term("Retrieval-augmented generation (RAG)")[
  Instead of handing a model everything and hoping it finds the relevant part,
  *retrieval-augmented generation* first *retrieves* the handful of passages that
  actually bear on the question — by semantic similarity, not keyword match —
  and hands the model only those, as labelled evidence to reason from. The answer
  is grounded in real, quotable text rather than in the model's diffuse memory of
  everything it has ever read.
]

Book scope used to be the tool's weakest surface. It sent your whole manuscript
along with every question — fine on a ten-page draft, ruinous on a real book. The
model skimmed a hundred pages, grounded in none of them, and answered from a
blur. Modern Inkhaven replaces that with RAG, and the change is total: Book scope
*is* retrieval now. There is no switch to flip and no "send the whole book" mode
to fall back to, because the whole-book mode never worked at scale.

Here is what happens behind a Book-scope question. Inkhaven runs a semantic
search for your query over the same on-save vector index every other semantic
feature uses, keeps the most similar paragraphs, widens each one with a neighbour
or two so it reads in context rather than as a fragment, drops duplicates, and
packs the result into a focused, token-budgeted block of evidence — best hits
first, ordered by their position in the book. That block, and only that block, is
what the model sees. Then it answers.

#screen(caption: "A question in Book scope, and a cited answer")[```
┌─ AI · Scope: Book ──────────────────────────────────┐
│ > does Mara ever doubt the captain?_                │
└─────────────────────────────────────────────────────┘

She does, twice. Her first real doubt surfaces on the
crossing, when the captain orders the night run
despite the glass falling — she calls it "a wager with
other men's lives" [act-two/the-night-run]. By the
storm chapter that doubt has hardened into open
defiance [act-three/storm-at-sea].
```]

You asked a question about your book and got back specific passages, by name,
instead of a plausible paragraph the model invented. Every claim it makes about
the book is anchored to a citation — and those citations are the heart of why the
answer is trustworthy.

#subsection("Citations you can trust — and the ones you can't")

Each bracketed label is a passage's *location path* — `[chapter-slug/scene-slug]`
— not an opaque database id. You can read *where* a claim comes from and jump
straight to it. The system prompt makes the contract explicit and ships localised
for the five baseline languages (English, Russian, Spanish, French, German, with
English as the fallback): ground every claim about the book in a retrieved
passage, cite it by repeating its location label exactly, and never invent a
label you were not given.

That last clause is not left to the model's good behaviour. Inkhaven scans the
answer for citation-shaped tokens and checks each against the passages it
actually retrieved. Any location the model cites that *was not* in what it was
handed is flagged inline, right where it appears:

#screen(caption: "A hallucinated citation, caught and flagged")[```
She returns in [act-two/the-storm] and again later in
[act-three/the-reckoning] [citation could not be
validated: act-three/the-reckoning].
```]

The first citation was in the evidence, so it stands untouched. The second was
not — the model reached past what it was given — and the flag says so plainly. A
Book-scope answer whose citations are all clean is one you can trust down to the
paragraph; one with a flag is telling you exactly which sentence to distrust.
This is a structural guarantee, not a stylistic nicety: the check runs on every
answer, and it leaves ordinary bracketed prose (a `[note]`, an `[aside]`) alone,
because a real citation contains a slash and no spaces and an aside does not.

When nothing in your book addresses the question, the model is instructed to say
so — "the retrieved passages don't address that directly" — and then either ask
you to sharpen the question or offer general knowledge *clearly marked as not
from the book*. It confabulates far less readily when the honest answer is
built into its instructions.

#subsection("Seeing what it retrieved — the transparency toggle")

You never have to take the grounding on faith. Above the conversation, a
collapsed line reports how many passages grounded the answer; press `p` to expand
it (and `p` again to collapse). Expanded, it lists each passage's similarity
score, a `★` for a *direct hit* versus a context-expansion neighbour pulled in
around one, the passage's location path, and its opening line.

#screen(caption: "p expands the retrieved-passages transparency section")[```
▼ Retrieved passages (6) · p to collapse
  ★ 0.851  manuscript/act-ii/storm-at-sea
           On the ninth night the storm took them.
           Waves the height of…
    0.806  manuscript/act-i/the-harbour
           The harbour at Vell was a forest of masts…
  …
  (retrieved once for this chat — clear history to
   retrieve again)
```]

That closing line is the rule that keeps a conversation coherent. Retrieval runs
*once per chat session*, so your follow-ups reason over a stable set of passages
instead of yanking the ground out from under the thread with every question. The
transparency section is the honest window onto that set: if an answer surprises
you, expand it and see the evidence the model was actually standing on.

#subsection("Re-grounding after you edit")

Because a chat holds its retrieval, editing the book underneath a live
conversation creates a small hazard — the passages the model has are now the
*pre-edit* prose. Inkhaven watches for exactly this. Save an edited paragraph
while a Book conversation is open and it nudges you, once:

#screen(caption: "A gentle nudge when the book moves under a chat")[```
book changed since retrieval — clear chat to re-ground
on the new text
```]

It is a reminder, not an interruption. The open conversation stays valid and
usable; when you want the model to see the new prose, clear the history and the
next question retrieves afresh. Re-grounding is always your call, never a
surprise mid-thread.

#section("What's in the pool — the retrieval scope")

Retrieval is anchored to the *user book your cursor is in* — its chapters,
subchapters, and paragraphs. But a question about your story often wants your
*notes* about the story, so a curated set of author-content system books joins
the pool. By default that means *Notes*, *Research*, *Places*, *Characters*,
*Artefacts*, *World*, and *Language*. Ask "what is the significance of the brass
key?" and a Characters or Artefacts entry can ground the answer right alongside
the prose that mentions it.

The internal and meta system books stay out. Scripts, Prompts, Typst, Help,
Intent, Sources, Glossary, and Snippets never enter retrieval — they hold the
machinery of the project, not its content, and grounding a story question in a
Bund script or a Typst template would only add noise. The split between "author
content, searchable" and "project machinery, excluded" is what keeps a
Book-scope answer about the *book*.

#callout(label: "Only paragraphs ground an answer")[
  Retrieval considers paragraph nodes only. A chapter or book node is a container,
  not prose, so the pool is always the actual writing — never a heading standing
  in for the scene beneath it. Each hit is expanded with its sibling paragraphs,
  so a retrieved passage arrives with the lines that surround it in the book.
]

#section("Tuning retrieval — the book_rag config block")

Every dial on the retrieval sits in one config stanza, `book_rag`. There is no
on/off knob — Book scope *is* RAG — so the block only shapes *how* it retrieves,
never *whether*. Five keys, each doing one thing:

#screen(caption: "The book_rag block, shown with its defaults")[```
book_rag: {
  top_k: 5
  context_expansion: 1
  max_context_tokens: 8000
  include_system_books: [notes research places
    characters artefacts world language]
  exclude_system_books: [scripts prompts typst help
    intent sources glossary snippets]
}
```]

`top_k` is how many paragraphs the semantic search keeps as direct hits — raise
it to ground a broad question more widely, lower it to keep the evidence tight.
`context_expansion` is how many sibling paragraphs are pulled in *around* each hit
(a value of `1` means one on either side), so a passage reads in context instead
of as an orphaned line. `max_context_tokens` is the hard ceiling on the assembled
evidence block, estimated at roughly four characters to the token; when hits
would overflow it, the best-scoring ones win and the rest are dropped. The two
book lists are named by each system book's lowercase tag: `include_system_books`
is the author-content set that joins the manuscript in the pool, and
`exclude_system_books` is the meta set that never does — and exclusion wins, so a
tag in both lists stays out. Trim the include list to a leaner pool, or extend it
if you keep story content in a book not listed by default.

#callout(label: "The permissive principle, here too")[
  None of these are guardrails. They shape cost and focus — a smaller `top_k` and
  a tighter token budget make a cheaper, narrower call — but nothing in `book_rag`
  will ever refuse a question or cap a conversation. As everywhere in Inkhaven,
  the numbers inform; they never block.
]

#section("Inspecting retrieval from the terminal")

Before you spend a single model call, you can see precisely what a question would
retrieve — and confirm the grounding is sound — with `inkhaven book-rag
retrieve`. It runs the *exact same* retrieval core the pane uses (they share one
function, so they can never drift), but stops before the LLM: no answer is
generated, no config is written, nothing is charged. It is the
inspect-what-the-model-sees tool.

#screen(caption: "book-rag retrieve — the passages, no model call")[```
$ inkhaven book-rag retrieve \
    "what does Mara fear about the voyage?"
Book-RAG retrieval — `The Long Road`
  (6 passages, 3 direct hits, ~288 tokens)

★ 0.806  manuscript/act-i/dawn-departure
        id: 019efc4b-ff42-7603-9a5d-4f83eedf50cb
        The ship slipped its moorings before sunrise…
  0.806  manuscript/act-i/the-harbour
        id: 019efc4c-05b9-7033-b7a2-7623b3112421
        The harbour at Vell was a forest of masts…
★ 0.851  manuscript/act-ii/storm-at-sea
        id: 019efc4c-0c27-7d61-9c1e-8df95471c961
        On the ninth night the storm took them…
```]

Three flags shape the run. `--book-name <name>` picks the book in a multi-book
project by title or slug (optional when there is only one). `--top-k <n>`
overrides `book_rag.top_k` for this run only, leaving your config untouched —
useful for feeling out how wide a question's grounding gets before you commit.
And `--context` prints the literal composed evidence block the model would
receive — passage headers and full bodies, the exact `── Retrieved passages ──`
prefix — rather than the summary listing. When you want to know why an answer
came out the way it did, that block is the ground truth it was built on.

#section("Facts scope — chat against the world's invariants")

Cycle `F9` one past Book and you reach *Facts*, the odd sibling among the scopes.
Where Book scope retrieves a *slice* of the manuscript, Facts scope loads the
*whole* Facts system book — every established invariant of your world: its
climate and geography, its distances and seasons, its chronology — as ground
truth, and seeds the conversation with a fact-analysis system prompt. It is not a
part of the book; it is the reference the whole book answers to, which is why it
sits *after* Book on the ring rather than among the manuscript slices.

Reach for Facts scope when your question is about the world's rules rather than
its prose. "How long is the ride from Vell to Rillmark?" "Which season is it when
the fleet sails?" "Is it plausible for snow to fall in the southern reach?" The
answer is grounded in what you have *established*, so the model reasons from your
canon instead of a genre cliché.

#callout(label: "Two siblings share the Facts grounding")[
  The same fact grounding powers two nearby chords. `Ctrl+B Shift+X` runs a
  one-shot *fact-check* of the open paragraph against every world fact, flagging
  any claim that contradicts the canon. `Ctrl+B Shift+S` opens a semantic *Facts
  search*: query the Facts book, mark the handful that matter, and send just
  those into a targeted chat — the scalable path when the Facts book grows too
  large to load whole. Both reuse the fact-analysis grounding the F9 Facts scope
  is built on. The world, and everything you can do with it, has its own chapter.
]

#section("Graph scope — chatting with how your book connects")

The last stop on the ring is *Graph*, and it answers a different kind of question
than any other scope: not what your book *says* but how its parts *connect*. It
is the conversational face of the knowledge graph — the typed-edge layer over
your nodes that records which fact contradicts which source, which paragraph
links to which, how a scene is sourced. A flat manuscript cannot tell you what
contradicts what; the graph can, and Graph scope lets you ask it in plain
language.

A Graph-scope question retrieves the relevant passages exactly as Book scope does
— same semantic search, same location-path citations, same hallucination flag —
and then, beneath each passage, folds in the graph edges touching it:
`contradicts`, `sourced_from`, `links_to`, `cites`, and the rest. The answer
stands on both the prose *and* those relations. Like Facts, it is a sticky scope
that retrieves once per chat and re-grounds when you clear history, and pressing
`p` expands a transparency section showing the passages *and* the subgraph the
answer was built on.

#subsection("Walking the graph — one hop, or many")

Graph scope reads *one hop* — the edges immediately touching each retrieved
passage. When a question needs the model to *follow* the graph — chaining from
node to node, several relations deep — there is a heavier instrument: `graph
ask`, the traversal loop.

#screen(caption: "graph ask — the model walks the graph turn by turn")[```
$ inkhaven graph ask \
   "which of my claims about the harbour
    contradict each other?"

  (walk prints to stderr; the answer to stdout)
```]

Given a question, `graph ask` searches for seed nodes, then issues read-only
graph queries turn by turn — neighbours, contradictions, loci, paths — until it
can answer, grounding the reply in what it actually observed. The exploration
transcript prints to stderr so you can watch the path it took; the answer prints
to stdout so you can pipe it. It is honest by construction: when the relations do
not record what you asked, it says so rather than inventing a connection — the
graph is only as complete as you have made it.

The same walk runs *inside the editor*, streamed live. Type a question in the AI
prompt, then press `Ctrl+B z → w`, and the AI pane shows each step unfold — a
search, a neighbours query, a contradiction check — before the grounded prose
answer lands as a normal chat turn; `Esc` stops the walk at any point. Because a
walk is several model calls where the one-hop Graph scope is one, it is an
explicit action you opt into per question. Both the CLI and the in-editor walk
are bounded by two knobs — `ask_max_steps` (the most LLM turns before a forced
answer) and `ask_search_width` (seed nodes per search) — which, true to the
permissive principle, cap the cost of a question without ever refusing it.

#section("The reader conversations — Socrates and Editor")

Two scopes on the ring are not about grounding in data at all but in a
*perspective*. Cycle past Facts and you reach *Socrates*, then *Editor* — the
conversational modes of two of Inkhaven's inner readers. Where Book, Facts, and
Graph ground the model in your *material*, these ground it in a particular way of
*reading* that material, seated across the table to talk with you about the
paragraph in front of you.

*Socrates* scope seats the active Reader Persona. The model reads the open
paragraph in that persona's voice, carries in whatever questions the Inner
Socrates has already raised about it, and discusses them with you — asking, never
prescribing, and never rewriting your prose. It is the scope for interrogating a
passage's assumptions rather than polishing its surface.

*Editor* scope seats the Inner Editor instead. It carries in that reader's
literary and stylistic observations of the open paragraph — its notes on
richness, tautology, vocabulary, and craft — and talks them through with you as a
thoughtful editor would, observing rather than commanding. When you cycle into
either scope, the status bar names it and tells you how many of that reader's
findings are queued for the conversation; both are sticky, and re-cycling `F9`
refreshes the seed from the paragraph you are now on.

#callout(label: "The readers have their own chapters")[
  Socrates and Editor scopes are the *conversational* face of the Inner Socrates
  and Inner Editor. What those readers notice, how they engage a paragraph, and
  how you tune them each fill their own chapters. Here it is enough to know that
  `F9` seats them for a chat: the same reader you meet as a pass over your prose,
  now willing to talk about what it found.
]

#section("Living with the chat — cost and habits")

Every structured-scope answer is one retrieval (cheap and local — embeddings and
a vector query, no network) plus one grounded model call, far less than shipping a
whole manuscript ever cost. Those calls are tagged in the cost dashboard —
`inkhaven cost`, or `Ctrl+B $` — under the `book_rag` category, so you can
see across a day what chatting with your book actually costs. As with every
figure in Inkhaven, it *informs*; no cap will ever refuse a question.

A few habits make the whole surface fluent. Cycle `F9` deliberately and read the
scope off the status bar before you send — the same question means very different
things in Book, Facts, and Graph. Press `p` when an answer surprises you and read
the evidence it stood on. Clear the history when you change subject or edit the
prose, so the next question re-grounds. And cycle `F9` back toward `None` when you
want the model's plain help with no grounding at all — the far end of the dial is
always one key away.

#chord_table((
  chord_row("F9", "Cycle the AI scope one notch; the status bar names it."),
  chord_row("F9 → Book", "RAG chat over the manuscript + author-content system books."),
  chord_row("F9 → Facts", "Chat grounded in the whole Facts book (the world's invariants)."),
  chord_row("F9 → Graph", "Chat grounded in prose + the graph edges that connect it."),
  chord_row("F9 → Socrates", "Converse with the active Reader Persona about this paragraph."),
  chord_row("F9 → Editor", "Converse with the Inner Editor about this paragraph's craft."),
  chord_row("p", "In the AI pane: expand / collapse the retrieved-passages section."),
  chord_row("Ctrl+B z → w", "Walk the graph to answer the AI prompt, streamed live."),
  chord_row("Ctrl+B Shift+X", "One-shot fact-check of the open paragraph against the Facts book."),
  chord_row("Ctrl+B Shift+S", "Semantic Facts search → ground a chat in a chosen handful."),
  chord_row("Ctrl+B $", "The cost dashboard — see the book_rag category's spend."),
))

#section("Scripting retrieval — the ink.book_rag.* words")

The same retrieval that grounds the pane is reachable from a Bund script, so an
automation can retrieve passages, compose the exact grounding block, inspect the
config, and even validate an answer's citations without touching the TUI. Every
word is *read-only* — nothing here mutates the store or calls a model; the
retrieval is local, and generating the answer is left to the script author. All
eight gate under the default-allowed `store_read` category, so a cautious project
can disable the whole surface at once.

#screen(caption: "Retrieve and compose the grounding block from Bund")[```
"manuscript" "does Mara doubt the captain?"
  ink.book_rag.context .        # the composed evidence block

"manuscript" "the harbour market"
  ink.book_rag.retrieve         # list of {id, breadcrumb,
                                #          body, score, is_hit}
  dup ink.book_rag.cited_ids    # the valid citation tokens
```]

`ink.book_rag.retrieve` takes an anchor (any book, or a node inside one) and a
query and returns the passages as dicts; `ink.book_rag.context` returns the
composed grounding block a model would receive; `ink.book_rag.scope` returns the
node ids in the retrieval pool; and `ink.book_rag.config` returns the live
`book_rag` block as a dict. Four pure helpers round it out:
`ink.book_rag.system_prompt` gives the localised grounding contract for a
language, `ink.book_rag.estimate_tokens` the rough token count of a string,
`ink.book_rag.cited_ids` the valid citation tokens of a passage set, and
`ink.book_rag.validate_citations` runs the same hallucination check the pane runs
— flagging, in a candidate answer, any bracketed location the passages do not
support. The knowledge-graph side is scriptable too, through the `ink.graph.*`
words, though the LLM-driven `graph ask` is not a synchronous word.

#recap((
  [The AI pane's *scope*, cycled with `F9`, decides what an answer is grounded
  in. The first five stops send a *slice of prose*; the last five — *Book, Facts,
  Graph, Socrates, Editor* — are *structured scopes* that retrieve or assemble
  evidence and are *sticky* until you cycle away.],
  [*Book scope* is retrieval-augmented generation: it semantically searches your
  manuscript, keeps the top passages, expands and token-budgets them, and grounds
  the answer with *location-path citations* — flagging inline any citation the
  passages do not support.],
  [Retrieval runs *once per chat*; press `p` to see the passages it stood on, and
  clear the history to *re-ground* after you edit — Inkhaven nudges you once when
  the book changes under a live conversation.],
  [The pool is your *user book* plus the *author-content system books* (Notes,
  Research, Places, Characters, Artefacts, World, Language); the meta books are
  excluded. Tune it all in the `book_rag` block — `top_k`, `context_expansion`,
  `max_context_tokens`, and the two book lists.],
  [`inkhaven book-rag retrieve` shows exactly what a question would retrieve with
  *no model call*; `--top-k` and `--context` inspect the grounding, and the CLI
  shares the pane's retrieval core so they never disagree.],
  [*Facts* scope grounds in the world's invariants; *Graph* scope grounds in how
  the book's parts connect (with `graph ask` / `Ctrl+B z → w` to *walk* deeper);
  *Socrates* and *Editor* seat an inner reader for a conversation about the open
  paragraph.],
  [The whole surface is scriptable through the read-only `ink.book_rag.*` Bund
  words — retrieve, compose the grounding block, inspect the config, and validate
  citations — and its cost is tagged under `book_rag`, informing but never
  blocking.],
))
