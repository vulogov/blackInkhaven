#import "../design.typ": *

#chapter(number: 26, title: "Technical Documentation")

Every other reading intelligence in this book watches for the thing a *story*
gets wrong — a contradiction, a broken promise, a character who knows a secret
before it is told. Technical writing has a different enemy, and it is quieter.
A reference manual, an API guide, a textbook, a standard-operating-procedure
binder — none of these will contradict itself so much as *go stale*: a sentence
that was true of last release and is now a small, confident lie about this one.
The code example that no longer compiles. The cross-reference to a section you
renamed. The URL that has rotted to a `404`. The chapter you swore you would
finish before shipping and did not.

Inkhaven's whole temperament is to check prose against a ground truth. The
technical-documentation tools — the `docs` subsystem and the back-of-book index
— turn that temperament on the documentation author's *own* ground truths: the
manual is made to answer to the *code* it shows, the *cross-references* it
promises, the *release* it ships against, and the *vocabulary* it defines. None
of it is AI. None of it edits your prose. Each check simply *reports*, and each
one exits non-zero when it finds trouble, so it drops straight into the last
gate before you publish.

#section("Who this chapter is for")

If you write fiction, you can skim this chapter and lose nothing — your ground
truths are the world and the cast, covered in Parts IV and V. This chapter is
for the *other* author: the person writing non-fiction, reference, or technical
documentation, whose claims must be right the way a compiler is right, and whose
book has an appendix a reader turns to last.

It belongs to Part VII — *Producing the Book* — because these are the tools you
run around the moment of shipping, alongside the PDF, EPUB, and web exports of
the previous chapters. Where those chapters turn your manuscript into a
finished artefact, this one makes sure the artefact is *true* before it leaves
your hands: the examples run, the links resolve, nothing half-baked slips
through, and the index that ships with it was built from the same source as the
prose.

#callout(label: "Two subsystems, one job")[
  This chapter covers two separate commands that share a purpose. `inkhaven
  docs` (with its three subcommands `verify`, `links`, and `review`) is the
  *currency* toolkit — it keeps a living document honest against its code, its
  cross-references, and its release. `inkhaven index` builds the *back-of-book
  index*. Both are deterministic, both are free of any model, and both read your
  *user* books only — system and companion books are skipped.
]

#section("The docs subsystem — three checks, three ground truths")

The `docs` command is a small family of three checks, each answering to a
different ground truth a manual can drift away from.

#term("TDOC")[
  *TDOC* is Inkhaven's shorthand for its technical-documentation checks: `docs
  verify` (the code examples still run), `docs links` (the cross-references
  still resolve), and `docs review` (the manuscript is current). All three are
  *deterministic* and *advisory* — they compute a report and change nothing.
  Only the opt-in external-link sweep touches the network; no model is ever
  consulted, anywhere in the subsystem.
]

Three properties hold across all three subcommands, and they are worth stating
once so you can lean on them everywhere below. First, each scans your *user
books only* — the system books (Facts, Notes, Timeline) and the companion
reference books are never treated as manuscript prose. Second, each restricts
to one book with `--book-name X`, taking the book's title or slug. Third — the
property that makes them useful in a release script — each *exits non-zero* the
moment it finds a problem. A clean run is silent and returns success; a run with
findings prints them and fails, so `inkhaven docs verify && inkhaven docs
links && inkhaven docs review` is a one-line pre-flight you can wire into
`make release` or a CI job.

#section("docs verify — the examples still run")

A code listing that no longer compiles is the most embarrassing kind of stale
documentation, because the reader discovers it the hard way — by typing it in.
`docs verify` catches it first, by actually running the examples through a
compiler or interpreter you configure and reporting the ones that fail.

#subsection("Marking a listing for verification")

Nothing is verified unless you ask for it, block by block. A fenced code
listing inside a `para:code` paragraph is marked by adding the word `verify` to
its fence *info string* — the text on the opening fence line after the language:

#screen(caption: "A verify-marked Rust listing in a para:code paragraph")[````
```rust verify
fn greeting(name: &str) -> String {
    format!("Hello, {name}!")
}
```
````]

Only paragraphs tagged `para:code` are scanned, and within them only blocks
carrying the `verify` flag are run — an ordinary example with no flag is left
alone. The flag can be space- or comma-separated from the language — an info
string of `rust verify` or `rust,verify` both work — and a single guard word,
`no-verify`, always wins: a fence of `rust verify,no-verify` is skipped no
matter what. That gives you an escape hatch for the one listing that
is illustrative rather than runnable — pseudocode, a fragment, a deliberately
broken example — without turning verification off everywhere.

#subsection("The two safety gates")

Running code that lives in a manuscript is a real capability, so Inkhaven puts
two deliberate gates between a fresh clone and an executed command.

#screen(caption: "docs verify with neither gate open — it refuses, safely")[```
$ inkhaven docs verify
Error: docs.verify is off. Enable it in inkhaven.hjson
(`docs: { verify: { enabled: true } }`) and configure
runners, then re-run. Use `--dry-run` to preview.
```]

The first gate is the config switch `docs.verify.enabled`, which is `false` by
default. On a project that has never turned it on, `docs verify` refuses and
points you at the setting rather than running anything. The second gate is
`--yes`: even with the feature enabled, the bare command *lists* the blocks and
the exact runner commands that would execute with your privileges, then stops,
requiring you to re-run with `--yes` to actually run them.

The safe way to look before you leap is `--dry-run`, which previews the resolved
command for every block — with `{file}` shown in place — and never touches the
config gate at all. It is the reviewer's-eye view of what a project *would* do:

#screen(caption: "docs verify --dry-run — resolved commands, nothing executed")[```
$ inkhaven docs verify --dry-run
docs verify --dry-run: 2 block(s) would run:

  api-basics/hello [rust]  →  rustc --edition 2021 \
      --crate-type lib <code>.tmp -o <code>.tmp.dir/out
  guide/quickstart [python]  →  python3 <code>.tmp
```]

#subsection("Running the checks")

With the feature enabled and `--yes` supplied, `docs verify` pulls every
verify-marked block from every user book (or the ones you narrow to), writes
each to a temporary file, runs it through the runner configured for its
language, and reports the outcome per block.

#screen(caption: "A real run — one pass, one failure, one skip")[```
$ inkhaven docs verify --yes
  ✓ api-basics/hello [rust]
  ✗ guide/quickstart [python]
      Traceback (most recent call last):
        File "…", line 3, in <module>
      NameError: name 'reqeusts' is not defined
  – legacy/old-sample [go] — no runner configured for `go`

docs verify: 1 passed · 1 failed · 1 skipped
Error: 1 code block(s) failed verification
```]

Two flags narrow the sweep when you do not want the whole book: `--book-name X`
restricts to one book, and `--paragraph <slug>` restricts to a single paragraph
named by its slug-path. Everything else about the run is unchanged — the same
gates, the same runners, the same tally.

#subsection("The four outcomes")

Each block resolves to exactly one of four states, and knowing them tells you
how to read a report:

- *pass* — the runner exited `0`. Quiet; counted and moved past.
- *fail* — the runner exited non-zero *or* ran past its timeout. The report
  carries the last forty lines of the runner's combined stdout and stderr — the
  informative tail, where the compiler's actual complaint lives.
- *skip* — no runner is configured for that block's language. This is *not* an
  error; it is how you leave a language unverified on purpose. Skips never fail
  the run.
- *errored* — the block could not be run at all (the temp file could not be
  written, or the runner could not be spawned). This *is* counted as a failure,
  because an example you could not check is not an example you can trust.

The command exits non-zero if any block failed or errored; skips and passes
alone leave it green. A runner is capped by `docs.verify.timeout_seconds` (30 by
default), so a listing with an infinite loop fails on the clock rather than
hanging your release script — the timeout is reported as a failure carrying
`timed out after Ns`.

#callout(label: "How a block actually runs")[
  Each block is written to a temp file whose suffix comes from the language's
  entry in `docs.verify.extensions` (`rust` → `.rs`, `python` → `.py`, and so
  on; an unknown language falls back to `.txt`). The runner command is executed
  through `sh -c`, with `{file}` replaced by that temp file's path and `{dir}`
  by its parent directory. Output is captured to a file rather than a pipe, so a
  chatty compiler can never deadlock the run, and both temp files are cleaned up
  afterward. Nothing runs but the command *you* wrote in your own config.
]

#section("docs links — the cross-references still resolve")

The second ground truth is the web of references a document makes — to its own
other sections, and to pages out on the internet. `docs links` checks both.

#screen(caption: "docs links --external — one dead internal link, one dead URL")[```
$ inkhaven docs links --external
  ✗ guide/webhooks → 550e8400-… (linked paragraph no
    longer exists)
  ✗ reference/appendix → https://old.example/spec
    (404 Not Found)
  (12 external URL(s) checked)

docs links: 1 internal · 1 external broken
Error: 2 broken link(s)
```]

The *internal* check runs always, project-wide over every user book, and is
purely deterministic: for each paragraph it walks the `linked_paragraphs`
cross-references and flags any whose target paragraph no longer exists in the
hierarchy — the classic broken link left behind when you rename or delete a node
that something else pointed at. No network, no ambiguity.

The *external* check is opt-in, behind `--external`, because it reaches the
network. It gathers every `http(s)` URL embedded in your prose — both bare URLs
and those inside a `#link("…")` call — deduplicates them so each distinct URL is
reported once (with the first place it was found), and checks each for link-rot.
Here `--book-name X` narrows the external sweep to a single book, which is handy
when one chapter cites the wider web and the rest do not.

#callout(label: "Conservative on purpose")[
  The external sweep uses the same cautious classifier as the research
  assistant's dead-source check: only a hard `404`/`410` or an outright
  connection failure counts as *dead*. A host that is merely slow, rate-limited,
  or behind an authentication wall is *not* reported — the check would rather
  miss a genuinely dead link than cry wolf on a live one and train you to ignore
  it. It exits non-zero when any link, internal or external, is broken.
]

#section("docs review — the manuscript is current")

The third check is not about correctness but about *readiness*. Every paragraph
in Inkhaven carries a status on a seven-rung ladder, and a long document is
never uniformly finished — some sections are polished, some are still notes to
self. `docs review` is the dashboard that shows you the distribution and,
crucially, what is still below the bar you mean to ship at.

#term("The readiness ladder")[
  Each paragraph's status is one of seven rungs, lowest to highest: `none` →
  `napkin` → `first` → `second` → `third` → `final` → `ready`. `docs review`
  measures the whole manuscript against a *floor* on this ladder and lists every
  paragraph that sits below it — the "nothing half-baked ships" gate.
]

#screen(caption: "docs review — per-chapter breakdown and what is below the floor")[```
$ inkhaven docs review
docs review — API Reference

  Getting Started
    ready 5 · final 1   [1 below `ready`]
  Endpoints
    ready 6 · third 2 · napkin 1   [3 below `ready`]

Below `ready` (needs work):
  - getting-started/authentication  (final)
  - endpoints/webhooks  (third)
  - endpoints/pagination  (third)
  - endpoints/rate-limits  (napkin)

docs review: 15 paragraphs · 11 ready · 4 below `ready`
```]

Per chapter, the report prints the status breakdown (how many paragraphs sit at
each rung) and flags the count still below your floor; then it lists each of
those paragraphs by slug-path with its current rung. Two flags shape the view:

- *`--floor <rung>`* sets the bar — `napkin`, `first`, `second`, `third`,
  `final`, or `ready` (the default). Everything below the floor is listed as
  needing work; run `--floor final` while drafting to ignore the last coat of
  polish and see only what is genuinely unfinished.
- *`--since <ref>`* marks every paragraph whose `.typ` file has changed since a
  git tag or commit — the "what do I need to re-read since the last release"
  view. A changed paragraph in the below-floor list is tagged `← changed since
  ref`, and the summary line counts the changes. If the project is not a git
  repository or the ref is unknown, change detection is skipped with a note and
  the rest of the report still runs.

`docs review` exits non-zero whenever any paragraph is below the floor, so
`docs review --floor final` in a release script is a hard stop on shipping a
section you never finished.

#section("Verifying the open listing — Ctrl+B Shift+D")

The CLI is the whole-book, CI-shaped tool; in the editor there is a single chord
for the tighter loop of checking the one listing in front of you. Open a
`para:code` paragraph and press `Ctrl+B Shift+D`.

#chord_table((
  chord_row("Ctrl+B Shift+D", "Verify the open code listing — run every verify-marked block in the current para:code paragraph and report to the Output pane."),
))

Every verify-marked block in the open listing is run synchronously — one
listing is quick — through the same configured runners the CLI uses, gated on
the same `docs.verify.enabled` switch. Passing blocks are quiet. A failure lands
in the *Output pane*, anchored on the paragraph, so it colours that paragraph's
tree badge and answers the `t` (this-paragraph) filter, exactly like every other
finding; the status line reports the tally.

#screen(caption: "A failed verification in the Output pane, on the paragraph")[```
┌─ Output · 1/1 · doc-verify ─────────────────────────┐
│ ⚠ doc_verify                                        │
│   code example failed `python` verification         │
│   Traceback (most recent call last):                │
│     File "…", line 3, in <module>                   │
│   NameError: name 'reqeusts' is not defined         │
├─────────────────────────────────────────────────────┤
│ ↑↓ select  o expand  Enter jump  a ask  d dismiss   │
└─────────────────────────────────────────────────────┘
 docs verify · 0 passed · 1 failed · 0 skipped
```]

Re-running the chord clears the paragraph's prior verify findings before
checking again, so the Output pane always shows the current state rather than a
pile of stale failures. If the feature is off, or the open paragraph is not a
code listing, or the listing has no verify-marked blocks, the status line says
so and nothing runs. For a whole-book or CI pass, drop back to `inkhaven docs
verify`.

#section("Configuring verification")

Everything `docs verify` does is driven by one config block. It lives under
`docs:` in `inkhaven.hjson`, and only the `verify` sub-block has any runtime
behaviour — the surrounding `docs:` block also carries the HTML export settings
(Chapter 25) and the index settings below.

#screen(caption: "The docs.verify block — off by default, runners you supply")[```
docs: {
  verify: {
    enabled: false          # master switch — nothing runs
    timeout_seconds: 30     # per-block wall-clock cap
    runners: {              # language → command, run via sh -c
      rust:   "rustc --edition 2021 --crate-type lib
               {file} -o {dir}/out"
      python: "python3 {file}"
    }
    # extensions: seeded (rust→rs, python→py, sh→sh,
    #   go→go, …); an unknown language falls back to .txt
  }
}
```]

The pieces are few and each does exactly one thing. `enabled` is the master
switch — `false` and nothing ever runs. `timeout_seconds` is the per-block cap.
`runners` is the heart of it: a map from a fence language to a shell command,
where `{file}` becomes the temp file holding the block and `{dir}` its parent
directory. A language with *no* runner is skipped, never failed — which is the
lever you use to decide, per project, which languages are verified at all.
`extensions` maps a language to the temp-file suffix and comes seeded with the
common ones (Rust, Python, shell, Go, JavaScript, C, and more); you rarely touch
it, but it is there to override when a runner cares about the file name.

#callout(label: "Runners run with your privileges")[
  A runner is an arbitrary shell command you wrote, executed as you. The two
  gates — `enabled` plus `--yes` — exist precisely because this is real
  execution, not a sandbox. Configure runners you trust, verify code you wrote,
  and treat a strange project's `docs.verify` block the way you would treat any
  script: read it (a plain `--dry-run` shows every resolved command) before you
  pass `--yes`.
]

#section("Single-sourcing — docs.variables")

One more staleness trap is the fact repeated in fifty places — most often the
version number — where you update forty-nine and miss the fiftieth.
`docs.variables` is Inkhaven's single-sourcing answer. It is not a `docs`
subcommand; it is a substitution applied at *export assembly time*, across every
export format.

#screen(caption: "A variable defined once, used anywhere in prose")[```
# in inkhaven.hjson
docs: {
  variables: {
    version: "3.0.0"
    api_base: "https://api.example.com/v2"
  }
}

# in any paragraph body
Install release {{version}} and point your client at
{{api_base}} to begin.
```]

Anywhere `{{key}}` appears in a paragraph body, the export replaces it with the
value of that key — so the version lives in exactly one place, and every PDF,
EPUB, and HTML build carries the same current number. Because the substitution
happens at build time and not in the stored `.typ` file, your source stays
readable with the placeholders intact; only the *exported* artefact has the
resolved value. Leave the map empty and nothing is substituted.

#section("The back-of-book index")

A scholarly or technical book earns an index — the alphabetised appendix a
reader turns to last, each term pointing at the chapters where it appears.
`inkhaven index` builds one, and it builds it from vocabulary you already
maintain rather than asking you to mark up the text.

#term("The back-of-book index (INDEX-1)")[
  `inkhaven index` builds a term → chapters index from your Glossary's canonical
  terms (plus any extras you list), locating each in the manuscript by
  whole-word match, deduplicating its hits to the chapter, and rendering an
  alphabetised index in Markdown, Typst, or JSON. It is deterministic and free —
  pure text search, no model — and it is *not* the semantic-search index, nor
  the Index Locorum or Index Verborum, which are separate scholarly commands.
]

#subsection("Where the terms come from")

The list of terms to index is the union of two sources. The first is your
*Glossary*: when `docs.index.from_glossary` is on (the default), every canonical
term becomes an index entry, and every Glossary synonym becomes a *see-reference*
pointing at its canonical term. The second is `docs.index.terms`, a config list
of extra terms — proper names, topics, anything you never put in the Glossary.
If neither source yields a term, the command errors rather than emit an empty
index.

#subsection("How a term is located")

Each term is searched across every user book's prose (or one book with
`--book-name`). A paragraph's Typst is first stripped to plain text, then matched
*whole-word* and *case-insensitively*: the term "art" matches the standalone
word and never "artist" or "start". Multiple hits within the same chapter
collapse to a *single* location, because the index points at chapters, not at
every occurrence — this is an index, not a concordance. A term found nowhere in
the manuscript is silently dropped, and the finished entries are sorted
case-insensitively by term. A see-reference is emitted only when its canonical
term actually made it into the index (a cross-reference to an absent term is
useless), and a synonym identical to its canonical is skipped.

#subsection("Output formats")

`--format` picks the renderer and `--out FILE` (short `-o`) writes to a file
instead of standard output.

#screen(caption: "The three renderers of inkhaven index")[```
$ inkhaven index                       # Markdown, all books
$ inkhaven index --book-name "Treatise"
$ inkhaven index --format typst -o appendix-index.typ
$ inkhaven index --format json

# md    → **term** — Chapter A, Chapter B
#         **term** — *see* canonical
# typst → *term* — Chapter A, Chapter B   (under `= Index`)
#         *term* — _see_ canonical
# json  → { "index": [ { term, see,
#            locations: [ { chapter, anchor } ] } ],
#          "count": N }
```]

Markdown (the default, `md`) is the portable form. Typst (`typ` or `typst`)
emits markup under an `= Index` heading, ready to `#include` as a printed
appendix. JSON exposes the full structure — each location carrying both its
`chapter` and an `anchor` — for downstream tooling.

#subsection("Anchors and the web index")

Every located term carries an `anchor` alongside its `chapter`. In the
standalone command the anchor resolves to the chapter (the CLI's unit is the
chapter), and only the JSON format surfaces it as a field; the Markdown and
Typst renderers print the chapter name alone. The anchor earns its keep in the
*HTML static-site export* (Chapter 25): with `docs.html.include.index` on, the
site build runs *the very same index builder* over the chapters and folds in an
`appendix-index.html` page where each location is a live
`<a href="…">chapter</a>` link into the chapter's HTML. So the index is clickable
on the web and a clean alphabetised appendix in print — both produced by one
pure function, so what the CLI emits and what the site folds in can never drift
apart.

#subsection("Configuring the index")

#screen(caption: "The docs.index block, and the HTML include toggle")[```
docs: {
  index: {
    from_glossary: true    # seed from Glossary terms + synonyms
    terms: []              # extra terms beyond the Glossary
  }
  html: {
    include: { index: true }   # fold a hyperlinked index into
  }                            # the HTML site (off by default)
}
```]

`from_glossary` is on by default; `terms` starts empty; and the HTML site's
`include.index` is *off* by default — turn it on when you want the appendix
folded into the exported site.

#section("What these tools are not")

It is worth naming the boundaries plainly, because they are the same boundaries
every Inkhaven intelligence respects. None of this is AI — the cores are
deterministic text and process work, and the single network touch is the opt-in
external-link sweep, which still consults no model. None of it is an autopilot:
`docs verify` runs nothing until you both enable it and pass `--yes`, and even
then it runs only the commands you wrote. None of it is a rewriter — every check
reports and none edit your prose. And `docs verify` is not a linter for
arbitrary code: it runs only the blocks you marked `verify`, in the languages
you gave a runner. The index, likewise, is not a concordance and not a model's
guess at what matters — it is whole-word search over your own term list.

These are the tools of the last mile, the ones you run in the same breath as the
export. Chapter 24 and Chapter 25 turned your manuscript into a PDF and a
website; this chapter is how you know, before either goes out, that the examples
in it still run, its links still resolve, no section shipped half-finished, and
the reader has an index to find their way back in.

#recap((
  [The *`docs` subsystem* is three deterministic, advisory checks over your user
  books, each exiting non-zero on a finding so it fits a pre-release / CI gate:
  `verify`, `links`, and `review`.],
  [*`docs verify`* runs the fenced blocks whose info string you mark with
  `verify` (in `para:code` paragraphs) through per-language *runners* you
  configure; gated by
  `docs.verify.enabled` (off by default) *and* `--yes`, with `--dry-run` to
  preview. Outcomes: pass · fail (non-zero or timeout, last 40 lines) · skip (no
  runner) · errored.],
  [*`docs links`* flags internal `linked_paragraphs` cross-references that no
  longer resolve (always), and with `--external` checks `http(s)` URLs for
  link-rot conservatively (only `404`/`410` and hard failures).],
  [*`docs review`* is a currency dashboard over the readiness ladder (`none` …
  `ready`); `--floor` sets the bar, `--since <ref>` flags paragraphs changed
  since a git tag.],
  [*`Ctrl+B Shift+D`* verifies the *open* code listing, posting failures to the
  Output pane on the paragraph; `docs.variables` single-sources `{{key}}` values
  at export time across every format.],
  [*`inkhaven index`* builds an alphabetised back-of-book index from Glossary
  terms + `docs.index.terms` (whole-word, chapter-deduplicated) in md / typst /
  json; the HTML export's hyperlinked index page uses the same builder.],
))
