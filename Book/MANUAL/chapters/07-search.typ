#import "../design.typ": *

#chapter(number: 7, title: "Search")

There are two questions a writer asks a long manuscript, and they are not the
same question. The first is *"where did I write that exact phrase?"* — you
remember the words, you just cannot find the paragraph. The second is harder and
more interesting: *"where did I write the scene where the light goes out and she
loses her way?"* — you remember the *feeling* of the passage, the shape of what
happened, but not one word of how you actually put it. A tool that can only
answer the first question makes you the index. A tool that can answer the second
reads with you.

Inkhaven answers both, but it leans hard on the second, because that is the one
worth building. This chapter is the whole of finding things: the two kinds of
search and what each is for; the Search bar you type a query into and the ranked
results it drops down; the machinery underneath that lets a search for words you
never wrote land on the paragraph you meant; searching inside a single part of
the book; keeping the index honest; and reaching the same search from a shell or
a script. By the end you will treat "where is that passage?" as a keystroke, not
a hunt.

#section("Two kinds of search")

The word *search* covers two genuinely different operations, and knowing which
one you want is most of using them well.

#term("Full-text (literal) search")[
  *Literal* search looks for a run of characters exactly as you typed them —
  the string `lantern`, the phrase `she counted the lamps`. It is precise and
  unforgiving: it finds every occurrence of those characters and nothing else,
  and it misses a paragraph that means the same thing in different words. This
  is the `Ctrl+F` find inside the open paragraph, and `rg` (ripgrep) or `grep`
  over the `.typ` files on disk from a shell.
]

#term("Semantic (meaning-based) search")[
  *Semantic* search looks for passages whose *meaning* is near your query,
  whether or not they share any words with it. It represents both the query and
  every paragraph as points in a "meaning space" and returns the paragraphs that
  sit closest to the query. This is the project-wide Search bar, and it is the
  one this chapter is mostly about.
]

Hold the division in mind, because it draws a clean line down the middle of the
tool. Literal search is *inside* the text you are looking at (the open buffer)
or *outside* it entirely (the files on disk, via your shell). Inkhaven's
*project-wide* search — the one that reaches every paragraph, note, fact, place,
and character at once — is *semantic only*. There is no inverted full-text index
over the whole project, by design: the meaning-based index is the one that earns
its keep for a book, and a second literal index would be a large thing to
maintain for a job `Ctrl+F` and `rg` already do well.

#callout(label: "When to reach for which")[
  Want a *specific word or spelling* — a character's name, a coined term, a
  phrase you are hunting to replace? Use `Ctrl+F` in the editor (it does regex on
  the open paragraph) or `rg <pattern> books/` from a shell. Want *the passage
  about a thing*, remembered by sense rather than wording? Use the Search bar.
  The rest of this chapter is the Search bar.
]

#section("The classic case — the lantern that never appears")

Here is the example that shows why semantic search is worth the machinery.
Suppose, three hundred pages into a draft, you want the scene where the last
light fails and your protagonist is left to find her way in the dark. You type
into the Search bar:

#screen(caption: "A query for a scene you remember by feeling, not by words")[```
┌─ Search ────────────────────────────────────────────────────┐
│ the moment the lantern fails▏                               │
└──────────────────────────────────────────────────────────────┘
```]

Press `Enter`, and the top hit is a paragraph that reads:

#screen(caption: "The paragraph the query finds — which never says any of it")[```
  The last lamp guttered and went out, and Mara counted the
  dark the way she had once counted the lamps: one, then the
  next, then nothing at all, the length of the quay gone to
  soot and the harbour with it.
```]

Read the two side by side. Your query said *lantern*; the paragraph says *lamp*.
Your query said *fails*; the paragraph says *guttered and went out*. Your query
said *the moment*; the paragraph names no moment at all. A literal search for
`lantern fails` would have returned *nothing* — not one of those characters is
in the passage. Yet it is unmistakably the paragraph you meant, and semantic
search puts it first, because *the meanings are neighbours* even though the words
are strangers. That is the whole promise in one example: you search by what a
passage is *about*, and the tool bridges the gap between how you remember it and
how you wrote it.

#section("The Search bar — asking the whole book a question")

The Search bar is the thin input that runs along the bottom of the window, and
it is the front door to project-wide search. You reach it, type, run, read, and
jump — five small moves you will wear a groove into.

#subsection("Opening the bar — Ctrl+/")

`Ctrl+/` focuses the Search bar from anywhere; `Ctrl+4` does the same, for hands
that prefer the number row. The bar takes your keystrokes and a cursor appears in
it. (You can also click it, if a hand is already on the mouse.) Nothing else
about the window changes — the panes stay where they were; you have simply
pointed your typing at the search input instead of the prose.

#subsection("Typing a query")

Anything goes in the bar — there is no syntax to learn and no operators to
remember. A single keyword (`mariner`), an exact-ish phrase you half-recall
(`stood at the rail at dawn`), or, best of all, a loose paraphrase of the thing
you are after (`the moment the storm broke and someone left the deck`). The
query is read as a natural-language description of the passage you want, not as a
pattern to match, so write it the way you would describe the scene to a friend.

#callout(label: "Longer queries search better")[
  It is tempting to type three terse words. Resist it. A fuller query — a
  sentence, even two — gives the model more to work with and usually retrieves a
  sharper result than a keyword does. `a quiet argument between them just before
  the funeral` will beat `argument funeral` almost every time. Semantic search
  rewards description, not keyword-thrift.
]

#callout(label: "Recalling a past query")[
  With the results overlay closed, `↑` and `↓` walk the bar's #emph[query
  history] shell-style — the last search you ran, the one before it, and so on —
  so re-running or tweaking an earlier query costs no retyping. (It is the same
  recall the AI prompt bar has.) The focused bar's title shows a `↑ history` cue
  whenever there is history to step back through.
]

#subsection("Running it and reading the results")

Press `Enter`. A ranked results overlay drops down over the body — the nearest
paragraphs, best match first.

#screen(caption: "The results overlay — ranked by meaning, best first")[```
┌─ Results · "the moment the lantern fails" (5) ──────────────┐
│  0.907  [paragraph] Rain › The City › the-quay              │
│         The quay at dusk                                    │
│         The last lamp guttered and went out, and Mara…      │
│                                                             │
│  0.861  [paragraph] Rain › Arrival › the-crossing           │
│         Losing the road                                     │
│         With the beacon dark she had only the sound…        │
│                                                             │
│  0.842  [note]      Notes › light-and-dark                  │
│         On darkness as a threat                             │
│         I keep returning to the image of a guide…           │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ select · Enter open · Esc close                          │
└─────────────────────────────────────────────────────────────┘
```]

Every row is three lines and tells you three things. The first line carries the
*score*, the *kind* of node (a `paragraph`, a `note`, a fact, a place), and a
human-readable *breadcrumb* built from the node titles — `Rain › The City ›
the-quay`, the trail you would walk down the Tree to reach it. The second line is
the node's *title*. The third is a one-line *snippet* from the body, enough to
recognise the passage on sight.

#term("The score")[
  The number at the head of each row is a *cosine similarity* — how close the
  paragraph's meaning sits to the query's, from roughly `0.0` (unrelated) toward
  `1.0` (nearly identical in meaning). It is a *relative* ranking signal, not an
  absolute grade: read it to compare hits against each other in one result set,
  not to set a universal pass mark. A `0.72` top hit in a sparse book can be
  exactly right; a `0.91` in a dense one can still be the wrong scene.
]

The list reaches across the *whole* project in one ranked column — manuscript
prose, your Notes, the Research book, the Places and Characters entries, the
Facts. A search for a mood can surface a research note you took weeks ago
alongside the scene it informed, because both were indexed the same way. That
breadth is a feature: the thing you are looking for is not always in the
chapter you think it is in.

#subsection("Jumping to a hit")

`↑` and `↓` move the selection through the results, and the overlay scrolls the
selected row into view as you go — so on a short terminal no hit is stranded
off-screen below the fold. `PgUp` and `PgDn` page the cursor by five, and
`Home` / `End` jump to the first / last result; the list holds up to fifty hits. `Enter`
on the highlighted row *opens that paragraph in the Editor* — focus moves there,
the Editor loads the body, and the Tree cursor repositions onto the very same
row, so you land in context and can move to neighbours immediately. One keystroke
took you from "the scene where the light fails" to the cursor blinking inside it.

#subsection("Dismissing the overlay")

`Esc` once closes the overlay while leaving focus in the Search bar, so you can
refine the query and run again. `Esc` a second time steps focus on — to the
Editor if a paragraph is open, otherwise onward through the focus ring. Nothing
`Esc` does here is destructive; it only peels away the overlay.

#chord_table((
  chord_row("Ctrl+/", "Focus the Search bar (top). Ctrl+4 does the same."),
  chord_row("Enter", "Run the query; on an open result, open that paragraph."),
  chord_row("↑ / ↓ (overlay closed)", "Walk the query history; a ↑ history cue shows when there is any."),
  chord_row("↑ / ↓ (overlay open)", "Move the selection through the ranked results (scrolls it into view)."),
  chord_row("PgUp / PgDn (overlay open)", "Page the result cursor by 5."),
  chord_row("Home / End (overlay open)", "Jump to the first / last result."),
  chord_row("Esc", "Close the results overlay; press again to move focus on."),
  chord_row("Ctrl+F", "Literal regex find inside the open paragraph (not project-wide)."),
))

#section("What makes a good query — and what does not")

Semantic search is powerful and a little unlike the search boxes you are used
to, so it pays to learn where it shines and where it is the wrong tool.

#subsection("The cases it is best at")

*Paraphrase* is its home ground — the lantern example above. Describe the passage
in your own words and it bridges to however you actually wrote it.

*Mood and situation* work beautifully, because a passage's emotional register is
part of its meaning. Try `a quiet pause in the action`, `someone delivers bad
news`, `the protagonist doubts herself` — you will find the passages that fit
even when you have forgotten every literal word.

*Character viewpoint* is findable by name plus a verb phrase — `Mara realises`,
`the captain's hand` — because the embedding learns that a passage is *about*
a person from far more than the bare occurrence of their name.

*The long query* beats the short one, as the callout above warned: a sentence of
description retrieves better than a keyword, every time you are patient enough to
type it.

#subsection("Multilingual, by construction")

The search is *multilingual* down to its bones, not as a bolt-on. Set the
project language to Russian and a query like `утренний рассвет на палубе` finds
Russian prose written in different inflections; the model that powers the search
understands roughly a hundred languages, and it maps meaning across the
morphology that trips up a literal search. A book written in Russian, French,
German, or Spanish searches exactly as well as one in English — the same model,
the same index, the same bar. (You will meet the details of *why* in the next
section.)

#callout(label: "Search across the languages of a translation")[
  Because meaning is language-agnostic in the index, a query in one language can
  surface near-neighbours written in another — useful when a manuscript and its
  translation live in the same project. It is not a translation tool, but it will
  often land you on the corresponding passage in the sibling book.
]

#subsection("What it will not do")

It is as important to know the edges. *Literal single-word* search is *not* its
strength: a query for one specific word may skip a paragraph that contains that
exact word, if the paragraph's overall meaning lands far from the query. When you
truly need "every paragraph containing this string," that is a job for `Ctrl+F`
in the editor or `rg` over `books/` — not the Search bar. *Boolean operators*
(`AND`, `OR`, `NOT`) do nothing; the interface is natural language, and the way
to combine ideas is to describe them together in one query or to run two queries.
*Field-restricted* search (`title:foo`) is not supported, and *regex* belongs to
the in-buffer `Ctrl+F`, not to the project-wide bar. None of these are
oversights; they are the deliberate shape of a meaning-first search.

#section("Finding the paragraph like this one — similar-paragraph mode")

The Search bar starts from a query you type. There is a second, quieter way into
the same vector index that starts from *a paragraph you are already reading*:
similar-paragraph mode, on `Ctrl+V S`.

#term("Similar-paragraph mode")[
  `Ctrl+V S` (from the editor) treats the *open paragraph itself* as the query.
  It saves the buffer, runs a vector-similarity search for the paragraphs nearest
  it in meaning, and opens the chosen hit in a second editor *side by side* with
  the first. Press `Ctrl+V S` again to save both and leave. It answers "what else
  in the book reads like this?" without your having to phrase the question.
]

#screen(caption: "Similar-paragraph mode — nearest neighbours of the open buffer")[```
┌─ Similar to · the-quay · 4 near ────────────────────────────┐
│  0.884  Rain › Departure › the-harbour-gate                 │
│         She had watched other ships leave at dusk…          │
│                                                             │
│  0.850  Rain › The City › the-inn                           │
│         The common room was loud with the same rain…        │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ select · Enter open beside · Esc close                   │
└─────────────────────────────────────────────────────────────┘
```]

This is the tool for a different kind of question than the Search bar answers.
The Search bar is for *retrieval* — you know roughly what you want and you go get
it. Similar-mode is for *discovery* — you are standing in one passage and want to
know what else in the book rhymes with it: the other rain scene, the echo you did
not plan, the note that anticipates this beat. It is also how you catch
*unintended* repetition, the paragraph you have written twice in different
chapters without noticing. Both surfaces read the same underlying index; they
differ only in where the query comes from — your fingers, or the paragraph under
your cursor.

#section("Under the hood — how meaning becomes a search")

The magic in the lantern example is worth demystifying, because it is not magic
and understanding it tells you when to trust it. Two ideas do all the work: an
*embedding* that turns text into numbers, and a *vector index* that finds nearby
numbers fast. Both run *entirely on your machine*.

#subsection("Embeddings — text as a point in meaning space")

#term("Embedding")[
  An *embedding* is a passage of text rendered as a list of numbers — a *vector*
  — such that passages with similar meanings produce vectors that sit close
  together. Inkhaven's default model outputs a vector of roughly 384 numbers per
  passage. "The last lamp guttered" and "the lantern fails" land near each other
  in that 384-dimensional space; "the wedding was at noon" lands far away. Nearness
  of vectors *is* nearness of meaning — that is the entire trick.
]

The model that computes these vectors is *MultilingualE5Small*, run locally
through #link("https://github.com/Anush008/fastembed-rs")[fastembed]. It is a
small, fast, multilingual model — the source of the roughly-a-hundred-languages
reach — and it is what makes a Russian query find Russian prose and an English
paraphrase find English prose without a word of either being sent anywhere. The
ONNX weights load once per session (a one-time cost of about half a second, paid
lazily the first time you actually save or search, never at launch), then stay
resident and cheap.

#callout(label: "On-device and private")[
  Every embedding — of your prose and of your query — is computed *on your own
  machine*. No paragraph, no search term, no scrap of the book leaves the
  computer to be searched. There is no cloud call, no API key, and no network
  round-trip in the search path; it works on a train with the wifi off. The model
  weights live in a per-user cache after their first download, and an air-gapped
  install can be pre-seeded with them. Search is a local capability, full stop.
]

#subsection("The HNSW vector index — finding neighbours fast")

Comparing a query against every paragraph's vector one by one would be slow in a
big book. Instead the vectors live in a structure built for exactly this.

#term("HNSW index")[
  *HNSW* (Hierarchical Navigable Small World) is a graph that links each vector
  to its near neighbours, so a search can *navigate* toward the closest points
  instead of checking every one. Inkhaven stores its vectors in
  #link("https://crates.io/crates/vecstore")[vecstore], an HNSW index kept beside
  the metadata database. A query embeds once, then walks the graph to the nearest
  paragraphs in time that barely grows as the book does.
]

The index returns *cosine distance* (lower means closer), which Inkhaven flips
into the *cosine similarity* score you see in the results (higher means closer),
so the number reads the natural way — bigger is better. Each node actually
carries *two* vectors in the index: one for its metadata fingerprint and one for
its content, so both the substance of a paragraph and its identifying details are
findable.

#subsection("How a paragraph gets embedded — on save")

The index stays current because embedding is wired into saving. Every time you
save a paragraph — `Ctrl+S`, or the autosave that fires when focus leaves the
Editor — Inkhaven does two things beyond writing the `.typ` file to disk: it
feeds the prose to the embedding model and updates that paragraph's vector in the
HNSW index, and it refreshes the paragraph's metadata (word count, modified time,
a derived title if the title was still a placeholder). By the time your focus has
reached the next pane, the words are on disk *and* the search index already knows
about them. This is why a passage you wrote a moment ago is findable a moment
later, and it is why the tutorial's advice to "just keep writing" costs the search
nothing — the index maintains itself, one save at a time.

#callout(label: "Why re-embed on every save")[
  Re-embedding on each save (rather than once, long ago) means searches always
  run against the *live* model and the *current* text. Change the embedding model
  in config and every future save re-embeds with it; edit a paragraph and its
  vector moves to match the new words. The index is never a stale snapshot of
  last week's draft — it is a running mirror of what is on disk now.
]

#section("Searching within a scope — Facts search")

Project-wide search is the default, but sometimes the question belongs to *one
part* of the book. The clearest case is the Facts book — the store of things your
world holds to be true — and it has its own scoped search on `Ctrl+B Shift+S`.

#subsection("The Facts search modal — Ctrl+B Shift+S")

`Ctrl+B Shift+S` opens a two-phase modal that runs a semantic search *restricted
to the Facts book*. It uses the very same vector index as the project-wide search
and similar-mode, then post-filters the results to the Facts subtree — so you are
searching by meaning, but only among your facts.

#screen(caption: "Facts search — semantic search scoped to the Facts book")[```
┌─ Facts search ──────────────────────────────────────────────┐
│ query: siege supplies at the north gate▏                    │
├─────────────────────────────────────────────────────────────┤
│ [x] 0.891  Facts › Logistics › grain-stores                 │
│         Three weeks of grain remained in the keep…          │
│ [ ] 0.864  Facts › Defences › the-north-gate                │
│         The north gate had not been rehung since…           │
├─────────────────────────────────────────────────────────────┤
│ ↑↓ select · Space mark · Enter → chat · Esc close           │
└─────────────────────────────────────────────────────────────┘
```]

The first phase is the query: type a description (multi-word is fine) and press
`Enter` to run the scoped search. The second phase is the ranked matches: `↑↓`
navigate, `Space` *marks* several facts for multi-select, and `Enter` sends the
marked facts — or, if you marked none, the cursor's row — into a *targeted Facts
chat* grounded in exactly those facts. Any printable key or `Backspace` in the
results drops you back to refine the query; `Esc` closes.

#subsection("How it differs from the Search bar")

Three differences matter. It is *scoped* — it never returns prose or notes, only
facts, so a big Facts book does not drown your answer in manuscript hits. It is
*two-phase* — search then act, with multi-select in between, where the Search bar
is search-then-jump. And its payoff is not "open the paragraph" but "*ground a
conversation* in these facts": the marked handful becomes the context for a
focused chat, which is the scalable way to reason over a large fact base — you
pull in the relevant few rather than loading the whole book into the model. Where
the Search bar takes you *to* a passage, Facts search hands a passage *to the
assistant*. Same index underneath; a different destination on top.

#section("Keeping the index healthy — reindex")

The index maintains itself for everything you do *inside* Inkhaven. It cannot
know about changes made *outside* it, and it does not automatically follow a
change of model. For those two situations there is one command.

#subsection("What re-embeds automatically")

Saving a paragraph re-embeds it. Renaming a node re-embeds it (the title is part
of what is indexed). Creating, editing, and deleting through the TUI all keep the
index in step. In normal writing you never think about the index at all — it is
already current.

#subsection("When to reindex")

#term("inkhaven reindex")[
  `inkhaven reindex` walks every `.typ` file under `books/` and reconciles the
  store with what is on disk — re-reading each file's content and re-embedding it.
  It is the command that re-aligns the index with reality after something changed
  the files without going through Inkhaven, or after you changed the embedding
  model. It is idempotent: safe to run as often as you like.
]

Reach for it after any of these:

- You *switched `embeddings.model`* in `inkhaven.hjson`. The new model must
  re-embed every paragraph, since old vectors were made by the old model and are
  not comparable to new ones. Search quality looks wrong until you do.
- You *edited a `.typ` file in another editor*, or a `git checkout` brought back
  paragraphs the database had forgotten.
- You *moved or deleted files* under `books/` from a shell, outside the TUI.
- You simply want a *"are my files and the index aligned?"* sanity check.

Two flags refine the walk. `--prune` removes store records whose `.typ` file has
gone missing from disk (use it after deleting files by hand). `--adopt` finds
`.typ` files on disk the store does not yet know about and registers them under
the matching hierarchy branch (use it after dropping new files into a chapter
directory). They combine: `reindex --prune --adopt` does both passes.

#screen(caption: "Re-embedding the whole project after a model switch")[```
$ inkhaven --project ~/Books/my-novel reindex
  re-read 412 paragraphs · re-embedded 412 · pruned 0 · adopted 0
  index aligned with disk.
```]

#callout(label: "Switch the model, then reindex")[
  The one sequence to remember: change `embeddings.model` in the config, then run
  `inkhaven reindex`. The bigger multilingual models (`MultilingualE5Base`,
  `MultilingualE5Large`, or `BGEM3`) buy better recall at the cost of disk and
  inference time; whichever you choose, the reindex is what makes existing
  paragraphs searchable under it. Skip the reindex and old and new vectors mix,
  and the scores stop meaning anything.
]

#section("Search from the shell — the CLI")

Everything the Search bar does is available from a terminal, which is what you
want when you are scripting around a manuscript or piping results into other
tools. The subcommand is `search`.

#screen(caption: "The same semantic search, from a shell")[```
$ inkhaven --project ~/Books/my-novel \
    search "the moment the lantern fails"

0.907  [paragraph ] rain/the-city/the-quay
       The quay at dusk
       The last lamp guttered and went out, and Mara counted…
       id: 7b3e9a04-…

0.861  [paragraph ] rain/arrival/the-crossing
       Losing the road
       With the beacon dark she had only the sound of the…
       id: a1f6c2d8-…
```]

Each hit prints four lines: the score with the node kind and the slug path; the
title; a one-line snippet; and the node's `id`. That last line is the point of
the CLI form — the `id` is a stable handle you can feed to other `inkhaven`
commands or grep out of the stream. The default is the top ten hits; `--limit N`
changes the count.

#screen(caption: "Capping the result count")[```
$ inkhaven --project ~/Books/my-novel \
    search "someone delivers bad news" --limit 3
```]

An empty result set prints `No results.` to standard error and exits cleanly, so
a script can tell "nothing matched" from "the command failed." Typical uses:
listing the top matches to a file before a manuscript-wide edit, piping to `grep`
to pull the `id` column, or feeding a shortlist of paragraphs into a batch job.

#section("Search from a script — the Bund word")

Inkhaven's embedded Bund language exposes the same search to your scripts, so a
macro can find passages and act on them. The word is `ink.search.text`.

#term("ink.search.text")[
  Stack effect `( query limit -- list )`. Takes a query string and a positive
  integer limit, runs the same semantic `search_text` the Search bar and CLI use,
  and pushes a list of hit dictionaries — each with `id`, `title`, `score`,
  `document` (the body), and `kind`. It is the scripting front door to the vector
  index.
]

#screen(caption: "Searching from Bund, then reading the top hit")[```
"the moment the lantern fails" 5 ink.search.text
; -> a list of 5 hit dicts: { id title score document kind }
```]

A companion word, `ink.search.load`, goes one step further: given a query and an
index, it runs the search and *opens the Nth hit in the editor*, returning false
when the search came back empty. It is the scripted equivalent of running a
search and pressing `Enter` on a result — useful for a macro that jumps you to
"the passage about X" as one bound key. Between the two, a Bund script can find
passages by meaning and either process them or navigate to them, with the same
index the whole rest of the chapter has been about.

#recap((
  [There are *two kinds of search*: *literal* (exact characters — `Ctrl+F` in the
  buffer, or `rg` over the files) and *semantic* (meaning-based). Inkhaven's
  *project-wide* search is semantic only.],
  [Semantic search finds a passage by *meaning*, not words: a query for `the
  moment the lantern fails` lands on a paragraph that says "the last lamp
  guttered and went out" and never uses one of your words.],
  [The *Search bar* opens with `Ctrl+/` (or `Ctrl+4`): type a query — longer and
  more descriptive is better — `Enter` runs it, `↑↓` picks a ranked hit, and
  `Enter` opens that paragraph in the Editor. `Ctrl+V S` searches *from* the open
  paragraph, for what reads like it.],
  [Under the hood: an *embedding* (fastembed's *MultilingualE5Small*, ~384
  numbers per passage) turns text into a point in meaning space; an *HNSW* vector
  index finds the nearest points fast. It is *multilingual* and runs *entirely
  on-device* — nothing leaves your machine. Every save re-embeds the paragraph.],
  [*Facts search* (`Ctrl+B Shift+S`) is the same index *scoped to the Facts
  book*, two-phase: search, multi-select, then ground a targeted chat in the
  chosen facts.],
  [Run `inkhaven reindex` after switching `embeddings.model` or after any change
  to the files made outside Inkhaven; `--prune` and `--adopt` reconcile
  deletions and additions. The CLI `inkhaven search "<query>" [--limit N]` and
  the Bund word `ink.search.text` reach the same search from a shell or a script.],
))
