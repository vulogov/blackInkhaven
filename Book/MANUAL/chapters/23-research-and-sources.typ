#import "../design.typ": *

#chapter(number: 23, title: "Research and Scholarship")

A book that makes claims about the world has to get them right, and a book with
its own invented world has to stay true to itself. Both are the same problem
seen from two sides: a manuscript is only as trustworthy as the facts under it,
and facts left untended drift, contradict each other, and quietly rot. Inkhaven
answers this with four distinct tools that share one job — giving a book a spine
of fact you can stand behind. This chapter is the operator's tour of all four:
the *Research Assistant* for gathering and checking facts, the *Sources* book
that turns your reading into a bibliography, the *Glossary* that keeps your
terminology from wandering, and the *scholarly apparatus* a serious work of
theology, philosophy, classics, or law is expected to carry.

The Facts themselves — the research books, the story bible, the graph that ties
them together — belong to Part IV; this chapter is about the machinery that
*surrounds* them: how a fact gets gathered and vetted, how a citation becomes a
reference list, how a term is held to one spelling, and how a scholarly index is
built. For the full, patient treatment of research as a craft — the trust
ladder, cross-checking, computing facts, composing from a corpus — this manual
defers to its companion, *Grounding Your Book in Fact*, which teaches the same
tools as a workflow rather than a feature list. Here we tell you what each is,
how to reach it, and how to run it.

#section("The Research Assistant — a room of its own")

Most of Inkhaven lives in the four-pane editor. Research does not. The Research
Assistant is a *separate full-screen program* you launch from the shell, a quiet
room you step into to gather and check facts and step back out of to write:

```
inkhaven research
inkhaven research --thread "1920s Vienna"
```

It opens on its own two-pane screen — a *Facts tree* down the left (about 40% of
the width), a *streaming chat* on the right, and a two-line query prompt beneath
them — and it wants at least 80 columns. Nothing here is a pane of the editor;
it is its own window with its own keys, and quitting it (`q`) drops you back at
the shell exactly as you left it. Because it is separate, everything it does is
insulated from your manuscript: it writes only to your research books, never to
your prose.

#term("Thread")[
  A *thread* is one research conversation, saved. You keep one per line of
  inquiry — "the aqueduct", "1920s Vienna", "cell biology for chapter 9" — so
  unrelated research does not tangle. `--thread <name>` opens or creates one;
  `--list-threads` shows them, and `--export-thread <name> --format <fmt> --out
  <file>` writes one out. Threads persist between sessions.
]

#screen(caption: "inkhaven research — the two-pane Assistant")[```
┌─ Facts ───────────────┬─ thread: siege-of-1849 ──────┐
│ ▾ Fortifications      │ > how thick were the         │
│   ✓ The citadel wall… │   citadel's outer walls?     │
│   ✓ Bastion spacing…  │                              │
│ ▾ Logistics           │ Roughly 4 m at the base,     │
│   ? Grain reserves…   │ tapering with height.        │
│   ※ (novel: the north │ ── src: Wikidata Q… · ✓      │
│      postern gate)    │                              │
│                       │ /fact → keep as a Fact?      │
├───────────────────────┴──────────────────────────────┤
│ [RAG: Facts+Full]   $0.03   ?:help   q:quit          │
└───────────────────────────────────────────────────────┘
```]

#subsection("Two targets: Facts and Notes")

The left tree fills as you work, and every entry lands in one of two books. The
distinction is the whole point of keeping research honest.

#term("Facts and Notes")[
  A *Fact* is something you trust enough to lean on; a *Note* is something you
  have written down but are not yet sure of. `/fact <claim>` keeps a claim as a
  Fact — it must cross a confirmation gate first; `/note <claim>` keeps it as a
  Note instead, no gate. When a Note earns its trust, `/promote` lifts it into
  the Facts book. A third book, *Sources*, holds the citations behind them, and
  the next section is entirely about it.
]

A fourth state sits on top of the Facts book, for fiction. Pressing `u` on a
fact in the tree marks it *undisputed* — an authorial axiom, drawn with a `※`,
that you have simply *decided* is true of your world. An undisputed fact is
exempt from fact-checking against the real world (there is no outside source for
"the north postern gate"), but it is still checked for internal coherence, as
you will see below.

#subsection("Two ways to speak")

You address the Assistant in one of two registers, and learning the difference
is most of learning the tool. The first is *plain language* — you type a
question the way you would ask a knowledgeable friend, and the answer is grounded
on whatever you have already kept. This is for thinking: exploring, orienting,
working out what you need to know. The second is a *slash command* — `/fact`,
`/geonames`, `/triangulate` — which does something precise and repeatable. Type
a single `/` and a hint bar lists the commands matching what you have typed so
far; `Tab` completes one, and `Ctrl+B h` opens the full on-screen reference. You
never have to memorise the set.

#subsection("Where a fact comes from — the trust ladder")

Not every fact is equally trustworthy, and the Assistant records *where each one
came from* so you always know how firm the ground is.

#term("Provenance")[
  *Provenance* is the record of a fact's origin — its rung on the *trust
  ladder*. From firmest to loosest: *computed* (you can re-run the sum),
  *structured* (a database id like a Wikidata Q-number), *scholarly* (a real
  paper with a DOI), *documents* (a source you imported), *web* (a cited page),
  and *model* (an educated guess from the language model). Higher rungs are more
  verifiable; the whole of research is a climb upward. Appendix B of *Grounding
  Your Book in Fact* is the full reference.
]

The source commands each start a fact higher than a bare guess and, where a
citation exists, file it to your Sources book for free. `/wikidata <query>`
returns structured facts by Q-id; `/geonames <query>` looks a real place up in a
gazetteer (needs a free `research.geonames.username`); `/openalex` and `/arxiv`
return the top scholarly work or preprint and auto-file its citation;
`/gutenberg` (alias `/pg`) ingests a public-domain book; `/web <query>` searches
and grounds a cited answer on real pages; and `/calc <expr>` *computes* a fact —
unit conversions, great-circle distances, compound growth, domain formulas — to
land it on the top rung, `computed`.

#subsection("Checking what you keep")

Gathering is half the job; the other half is not trusting yourself.

#chord_table((
  chord_row("/triangulate", "Cross-check a claim against Wikidata and the two scholarly indexes at once; reports SUPPORTS / CONTRADICTS / SILENT. Alias /tri."),
  chord_row("/factcheck", "Audit the whole Facts book — per-fact truth and cross-fact consistency — marking each with a verdict: ✓ accurate, ? dubious, ✗ inaccurate."),
  chord_row("/whatswrong", "Explain why the selected flagged fact failed, and what the correct information appears to be."),
  chord_row("/undisputed", "Check your undisputed (authorial) facts for internal coherence: PLAUSIBLE / ODD / INCOHERENT, in the project language."),
  chord_row("/upgrade", "Re-ground a model fact on a corroborating source and raise its provenance in place — the wording is never changed."),
  chord_row("/stale", "List model / web facts older than N days (default 90) for re-verification."),
  chord_row("/deadsources", "Scan kept web sources for link-rot and flag the ones that no longer resolve."),
))

The two glyph systems are worth keeping straight. `/triangulate` holds one claim
up against three independent sources at once and reports whether they *support*
it, *contradict* it, or stay *silent* — a claim all three support, with none
against, is far firmer than any single lookup. `/factcheck` sweeps the whole
Facts book after the fact and stamps each entry with `✓`, `?`, or `✗`, the same
glyphs you see in the tree; `/whatswrong` then explains a failure. Invented
facts sidestep all of this — there is nothing outside your book to check them
against — so `/undisputed` gives them their own audit: not "is this true?" but
"does this hang together with everything else I decided?", answered PLAUSIBLE,
ODD, or INCOHERENT.

#subsection("Following the citations — snowball")

When one paper is the right paper, its bibliography is a map of the field around
it. `inkhaven research --snowball "<seed>"` follows a work's citations backward
and forward across OpenAlex and reports the neighbourhood, so you can pull in the
few that matter rather than search blind. It is citation-chasing made a single
command.

#subsection("Working headless — batch and the agentic mode")

The Assistant does not need you sitting in front of it. Three headless modes let
it run while you do something else.

#chord_table((
  chord_row("--batch <file>", "Research a list of questions unattended; proposes findings by default. Add --auto-confirm --confidence <0..1> to insert those above a threshold and report the rest. --out writes a report."),
  chord_row("--agentic \"<topic>\"", "Research a topic autonomously, emitting findings as untrusted Facts to triage later with /review. --out writes a run log."),
  chord_row("--import / --sync", "--import <path> ingests a document or folder; --sync <folder> re-imports only the files that changed since last launch."),
))

The `--batch` mode closes the loop opened by the Assistant's own `/gaps`
command: ask for the open questions your corpus cannot answer, write them to a
file, and hand the file back for it to research unattended. The `--agentic`
mode goes further — it is the *deep* mode, an autonomous loop that chooses its
own next question and files what it finds directly into the Facts book, marked
*untrusted*. Nothing autonomous is ever silently trusted: you step through the
results afterward with `/review`, where each candidate is *accepted* (`a`),
*deleted* (`d`), or marked *undisputed* (`u`), and contradictions are flagged
with `≠`.

#callout(label: "Nothing happens to your manuscript by accident")[
  The Research Assistant never touches your prose. It writes only to your
  research books — Facts, Notes, and Sources — and only the deliberate acts you
  take (keeping a fact, recording a source) change anything on disk. You can ask
  anything, chase any tangent, and run any command as a test drive, knowing the
  book on the other screen is exactly as you left it.
]

#subsection("The keys of the room")

Inside the Research screen the navigation is its own small vocabulary, closer to
the Tree's than to the editor's.

#chord_table((
  chord_row("Tab / Shift+Tab", "Cycle focus: Facts tree → query prompt → chat."),
  chord_row("F10", "Cycle the RAG mode a plain question is grounded on: Facts+Full → Facts only → Full only. (The /rag command does the same.)"),
  chord_row("Ctrl+P (tree)", "Pin / unpin the cursor fact as RAG context — up to three, marked ⬡."),
  chord_row("n (tree)", "Add a fact by hand: type a title, press Enter, type the body, then Ctrl+S to save."),
  chord_row("Ctrl+B h", "The full quick reference — every key and every /command."),
  chord_row("? / q", "Toggle the hints bar · quit (Ctrl+C and Ctrl+Q also quit)."),
))

The chat also scrolls with `j`/`k`/`g`/`G` and searches with `Ctrl+F`; a
confirm overlay (the gate a `/fact` crosses) switches title and body with `Tab`
and inserts with `Ctrl+S`. That is enough to run the room; the companion book is
the place to learn it as a craft.

#section("Sources and the bibliography")

Every time the Assistant grounds an answer on a scholarly paper or a page, it
files a proper citation into the *Sources* book without your lifting a finger.
Sources is the third research book — Facts holds what you trust, Notes what you
are unsure of, Sources the citable works behind them — and it is a *system book*
like the others: a real book in your tree, one paragraph per citation, that
Inkhaven manages. By the end of a project it is a bibliography you never sat down
to write.

#term("Citation key")[
  A *citation key* is the short handle a source is cited by — `@vienna1925`,
  `@kant`, `@bible`. In your prose it appears with an `@` sigil, the same one
  Typst uses natively; at book assembly each key resolves to a full reference.
  A citation's key is the *title* of its paragraph in the Sources book, so
  naming a Sources paragraph `vienna1925` is what defines `@vienna1925`.
]

#subsection("Citing as you write — Ctrl+V @")

You do not type citation keys from memory. In the editor, `Ctrl+V @` opens the
*cite picker* — a fuzzy search over every entry in your Sources book. Type any
part of a key, author, or title to narrow it; `↑↓` moves the selection, `Enter`
inserts the bare `@key` at your cursor, `Esc` closes. Each row shows the key
beside its year, author, and title, so you pick the right work by sight rather
than by remembering its handle. If the list is empty, your Sources book has no
entries yet — add a paragraph under Sources (its title becomes the key) or import
a `.bib` file, and try again. When the chosen source carries a reference scheme —
scripture, or a `scheme:` you declared — the picker inserts `@key[]` and leaves
your cursor between the brackets for the passage, the `@key[locus]` form the
index locorum is built from.

#screen(caption: "Ctrl+V @ — the cite picker over the Sources book")[```
┌─ Cite · Sources ──────────────────────────────────────┐
│ > vien▏                                              │
├───────────────────────────────────────────────────────┤
│ ▌@vienna1925   1998 · Maderthaner — Die Ringstraße   │
│  @vienpop      2011 · Weigl — Vienna, a Demography    │
├───────────────────────────────────────────────────────┤
│ type to filter · ↑↓ select · Enter inserts @key      │
└───────────────────────────────────────────────────────┘
```]

#subsection("The bibliography assembles itself")

Because each Sources entry was filed from a real work with a real identifier — a
DOI, a Q-number, a catalogue id — the reference list is not a guess at how a
citation *should* look but the actual, resolvable reference. When you assemble
the book (Chapter 24), Inkhaven collects the cited keys, generates a `sources.bib`
BibTeX file, and emits a Typst `#bibliography` so the formatted reference list
renders into the finished PDF. The citation you inserted with `Ctrl+V @` and the
entry in the back matter are the same record, and they can never drift apart,
because one is generated from the other.

#subsection("Managing Sources from the shell")

For interchange with a reference manager, `inkhaven sources` works the book from
outside the editor:

#chord_table((
  chord_row("sources list", "List every citation defined in the Sources book."),
  chord_row("sources check", "Validate the entries and find prose @keys with no matching source; exits non-zero on a problem, so it drops into a CI step."),
  chord_row("sources import <f.bib>", "Bring citations in from a BibTeX file — a Zotero or Mendeley export — as new Sources paragraphs."),
  chord_row("sources export --format bibtex|csl-json", "Write the citations out; --out <file> to a file. CSL-JSON closes the round-trip with Zotero, BibTeX suits LaTeX."),
))

The `check` subcommand is the one to wire into a build. It reports every `@key`
you cited in prose that has no matching entry in the Sources book — a dangling
citation or a misspelled key that would otherwise surface only as a broken
reference in the final PDF — and it fails the process when it finds one. A
companion pass, `inkhaven sources coverage`, works the other way: it flags
sentences that make a checkable factual claim — a statistic, a date, an
attributed finding — while carrying no citation at all (and with `--ai`, checks
each against your Facts book), so a non-fiction manuscript can be gated on both
undefined keys and unsupported claims.

#subsection("The Bund surface — ink.sources.*")

Everything above is scriptable. The `ink.sources.*` namespace exposes the
Sources book to the embedded Bund language (Chapter 27), all of it read-only:
`ink.sources.list` pushes every entry as a dict, `ink.sources.get` takes a key
and returns the one entry (or `NODATA`), `ink.sources.bibtex` returns the whole
compiled BibTeX as a string, and `ink.sources.check` returns the list of
undefined `@key` citations found in prose. They mirror the CLI exactly, for a
script that gates a commit or splices a bibliography into a larger pipeline.

#section("Terminology governance — the Glossary")

A long book slips its terms without noticing. The thing you called an *access
token* in Chapter 2 becomes an *auth token* in Chapter 9 and an *authentication
token* in the appendix, and a reader — or a reviewer — is left wondering whether
you mean three things or one. The *Glossary* book, and the detector built on it,
hold your terminology to a single canonical form.

#term("Canonical term")[
  A *canonical term* is the one spelling you have chosen for a concept; its
  *banned synonyms* are the variants you want caught and corrected. A Glossary
  entry pairs them — canonical `access token`, synonyms `auth token` and
  `authentication token` — with an optional definition, a note on why you chose
  that form, and a *scope* (project-wide by default, or one book). Entries are
  authored as small HJSON paragraphs in the Glossary system book.
]

#subsection("The banned-synonym overlay — Ctrl+V z")

With a Glossary in place, Inkhaven watches your prose for the banned forms. The
detector red-underlines every occurrence of a synonym — so "auth token" is
flagged while the canonical "access token" is left clean — and when your cursor
sits on a flagged word the footer names the fix. `Ctrl+V z` toggles this
terminology overlay. It is *on* by default (nested within the master style
toggle, `Ctrl+B Shift+F`) and it is *self-gating*: an empty Glossary flags
nothing, so the feature costs you nothing until you decide to use it.

#screen(caption: "The terminology overlay — a banned synonym, underlined")[```
┌─ Editor · api-tokens [modified] ──────────────────────┐
│  1  The service issues an auth token on login, and   │
│                          ‾‾‾‾‾‾‾‾‾‾                   │
│  2  the client presents that token on every call.    │
│                                                       │
├───────────────────────────────────────────────────────┤
│ terms: "auth token" → use "access token"             │
└───────────────────────────────────────────────────────┘
```]

#subsection("The escape hatch — Ctrl+V Shift+Z")

Sometimes a "banned" form is the right one just here — a character says it in
dialogue, a quoted document uses it. With the cursor on a red-underlined synonym,
`Ctrl+V Shift+Z` *declares the term deliberate*: it records the canonical term
as a deliberate variant in the intent ledger, and from then on the overlay and
`inkhaven terms check` stop flagging its synonyms. It is the "I meant to write it
this way" release valve, so the detector can stay strict without becoming a
nag. (Move onto the red underline first; away from a hit, the command has
nothing to declare and says so.)

#subsection("From the shell — inkhaven terms")

Two subcommands work the Glossary from outside the editor:

#chord_table((
  chord_row("terms check", "Scan prose for banned synonyms and report each occurrence with its location; --book scopes to one book, --json for a machine report. Exits non-zero on any finding — a CI gate."),
  chord_row("terms suggest", "Ask a model to propose Glossary entries for genuine terminology drift it finds in a book. --provider, --max-cost (default 8000), --force past the cap, --auto-create to write the drafts under the Glossary book."),
))

`terms check` is the deterministic, free gate: it walks every paragraph, matches
one-, two-, and three-word banned forms Unicode-aware (so Cyrillic and accented
terms behave exactly as Latin ones), prints `path line: "synonym" → use
"canonical"` for each, and fails the run if any remain. `terms suggest` is the
one command here that spends a model: it clusters the near-variants actually
present in a book's prose and asks for canonical entries, skipping mere
inflection and proper names — a way to *bootstrap* a Glossary from a manuscript
you already have.

#subsection("The Bund surface — ink.terms.*")

The Glossary is scriptable through `ink.terms.*`: `ink.terms.list` and
`ink.terms.get` read the entries, `ink.terms.check` takes a book slug and returns
the banned-synonym findings as dicts, and `ink.terms.declare_intent` takes a
canonical term and a scope and records the deliberate-variant suppression — the
scripted twin of `Ctrl+V Shift+Z`. The three reads are default-allowed;
`declare_intent` writes to the ledger, so it needs the store-write category
enabled. The Glossary's canonical terms also appear in the story-bible overview
(`Ctrl+V Shift+L`), alongside your characters, places, and facts.

#section("The scholarly apparatus")

A critical edition carries an apparatus at the back: an *index locorum* of every
passage it cites, an *index verborum* of its key terms, a working lexicon of
those terms' senses, and — behind it all — an argument that holds together.
Inkhaven builds each from the manuscript you already have, reading your prose and
your Sources and Glossary books. They *measure and report*; they never rewrite.
Three of the four are deterministic and free; only `argue` calls a model. This
is the toolkit for theology, philosophy, classics, and law.

#subsection("inkhaven index-locorum — the index of places")

An `@key[locus]` citation — `@bible[John 3:16]`, `@kant[A51/B75]`,
`@quran[2:255]` — is a *primary-source* reference: a citation that carries a
specific passage in its brackets. `index-locorum` harvests every one across the
manuscript, groups them under their source, and sorts the passages *naturally*
(so `3:2` precedes `3:16`), listing the chapters that cite each. A bare `@key`
without a locus is an ordinary citation and belongs in the bibliography, not
here.

```
inkhaven index-locorum [--book-name <NAME>]
                       [--format md|typst|json] [-o <FILE>] [--strict]
```

Scripture keys (`bible`, `quran`, `book-of-mormon`) validate and *canonicalize*
with zero configuration — `Jn 3.16`, `иоанна 3:16`, and `John 3:16` collapse
into one entry — and any other source can declare its own reference scheme.
Malformed loci are reported on stderr with the format they should have taken;
`--strict` turns that advisory into a failing exit, a gate you can put in CI.

#screen(caption: "An Index Locorum, grouped by source and sorted by passage")[```
Index Locorum

Augustine — Confessiones
  1.1      ch. 2, ch. 7
  8.12     ch. 7
  10.27    ch. 4, ch. 9

The Bible
  John 1:1     ch. 1
  John 3:16    ch. 3, ch. 6
  Romans 5:8   ch. 6
```]

#subsection("inkhaven index-verborum — the index of terms")

The term-level twin: every scholarly-lexicon term that *actually appears* in the
manuscript, with its original-language form, its distinct senses, and the
chapters that use each. It reads the lexicon terms from your Glossary — an entry
counts only if it carries an `original_forms` field or `senses`, not a plain
consistency entry — and a term you defined but never used is *dropped*, so the
index cannot flatter you. Where you tag a use with the Typst superscript
convention `term#super[N]`, the index records *which sense* was used where.

```
inkhaven index-verborum [--book-name <NAME>]
                        [--format md|typst|json] [-o <FILE>]
```

#callout(label: "Used versus declared")[
  `index-verborum` reports the terms you *used*; `inkhaven lexicon list` reports
  the inventory you *declared*, whether or not each term has appeared yet. They
  answer different questions and will differ exactly when you have defined a term
  you have not yet written into the prose.
]

#subsection("inkhaven lexicon list — the sense inventory")

The working sense-inventory behind the index: the terms your work tracks, each
with its original-language form(s), numbered senses, and whether it is *watched
for equivocation* — an argument sliding between a term's senses. `list` is the
only subcommand.

```
inkhaven lexicon list [--book <NAME>] [--watched] [--json]
```

A term is equivocation-watched only when it both sets `watch_equivocation` *and*
declares at least two senses — declaring the senses is what lets the tool tell a
legitimate polysemy from a genuine equivocation, so a single-sense term is never
policed. Watched terms are marked `⊬`, and `--watched` narrows the list to just
them. This is the inventory the reasoning-rigor reader consults when it polices a
paragraph.

#subsection("inkhaven argue — the argument outline")

The one command here that calls a model. Per chapter, `argue` reports the
load-bearing claims the chapter argues for and the support it gives each, and
flags the two cheapest structural gaps: a *central claim with no support*, and an
*orphan citation* — a source cited but supporting no identified claim.

```
inkhaven argue [--book-name <NAME>] [--provider <NAME>] [--json]
```

It hands the model each chapter's plain prose plus the `@key` citations that
chapter uses, and asks for the claims, their support, and any orphans. An
anti-hallucination guard keeps it honest: every returned claim must be quoted
from the chapter, and a claim whose words are not actually present is *dropped*,
so the report can only ever contain arguments you really made. A *gap* is an
unsupported claim or an orphan citation, and `argue` *exits non-zero when it
finds one* — so, like `index-locorum --strict` and `sources check`, it drops
straight into a CI step that will not let a structurally broken chapter pass.

#two_track(
  [If you write *fiction*, the apparatus is rarely your deliverable, but the
  Research Assistant is: gather real texture with the source commands, keep the
  load-bearing facts fact-checked, and let `undisputed` guard the coherence of
  the world you invented.],
  [If you write *non-fiction* or *scholarship*, this whole chapter is your
  deliverable's spine — a fact-checked corpus, a bibliography that built itself,
  a glossary that held your terms, and, for a critical edition, indices and an
  argument audit you can gate a build on.],
)

#recap((
  [The *Research Assistant* (`inkhaven research`) is a separate TUI: a Facts tree
  and a chat, keeping *Facts* (trusted, gated) and *Notes* (speculative), with
  `※ undisputed` authorial facts exempt from real-world checks.],
  [Every kept fact records its *provenance* — a rung on the trust ladder from
  *computed* down to *model* — and the source, checking, and headless commands
  (`/triangulate`, `/factcheck`, `--snowball`, `--batch`, the `--agentic` deep
  mode with `/review`) climb and defend it.],
  [The *Sources* book turns your reading into a bibliography: cite with `Ctrl+V
  @`, and assembly emits `sources.bib` and a `#bibliography`; `inkhaven sources
  check/list/import/export` and `ink.sources.*` manage it.],
  [The *Glossary* holds terms to one canonical form: `Ctrl+V z` red-underlines
  banned synonyms, `Ctrl+V Shift+Z` declares one deliberate, and `inkhaven terms
  check/suggest` and `ink.terms.*` gate and grow it.],
  [The *scholarly apparatus* builds a critical edition's back matter from your
  prose — `index-locorum`, `index-verborum`, `lexicon list` (free, deterministic)
  and `argue` (LLM) — measuring and reporting, never rewriting, and gating a
  build with `--strict` or a non-zero exit.],
  [Facts and the graph live in Part IV; the full research craft is the companion
  *Grounding Your Book in Fact*.],
))
