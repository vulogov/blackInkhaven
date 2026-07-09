#import "../design.typ": *

#chapter(number: 8, title: "The technical track")

Technical documentation describes something that _exists and keeps changing_ — a
piece of software, an API, a machine. You invent almost nothing; the system is
already there. Your obligations are three: be precise, be findable, and stay in
sync with the thing you describe. The characteristic failure is not a wrong idea
but _staleness_ — prose that was true of last release and is now a quiet lie. This
track leans hardest on structure, on controlled terminology, and on reuse, and
lightest of all on invention.

#section("Frame — the reference template and the audience")

Start from the template shaped for it, and set the genre:

```
inkhaven init "widget-api-guide" --template technical
```

#config("inkhaven.hjson", [```hjson
genre: "technical"
```])

`documentation`, `docs`, `api_docs`, and `reference` share this frame. It tells the
readers to judge for precision and for the reader's task, never for imagery or
voice. And because documentation is read by someone _trying to do a thing_, decide
early who that someone is — the newcomer with none of your context, or the
practitioner who knows the domain and wants the specific answer. The track's
readers can be either, and the difference changes every sentence.

#section("Structure — the deepest investment on this track")

Nobody reads a manual front to back; they arrive by search, mid-task, needing one
answer. So structure is not scaffolding here — it is the product. Build the tree so
every node is _reachable and self-contained_: a task has its own page, a reference
entry stands alone, nothing requires the section before it. Use the Outline
(`Ctrl+2`) and `inkhaven outline` to keep the whole shape in view, and the status
marks (`Ctrl+B r`) to track which pages are current and which are stubs awaiting the
next release.

#subsection("Reusable blocks")

Documentation repeats itself — the same warning, the same setup snippet, the same
parameter table — and every copy is a copy that can go stale independently. The
*Snippets* book holds a reusable block once and lets every page include it, so a
fix lands everywhere at once.

#term("Snippet")[
  A block of content authored once in the Snippets book and referenced from many
  pages, rather than copied into each. On a track whose enemy is staleness, the
  snippet is a direct weapon: the boilerplate that appears in forty places lives in
  one, so correcting it is one edit, not a hunt.
]

#section("Terminology — say the same thing the same way")

In technical writing, a synonym is a bug. If a thing is a "widget" on one page and a
"component" on another, the reader cannot tell whether they are the same. The
*Glossary* book governs this: it holds your canonical terms and the synonyms you
have banned, and an overlay (`inkhaven terms check`, or `Ctrl+V z` in the editor)
flags where the manuscript has drifted — a "component" where the glossary says
"widget."

#note[
  Terminology governance feels bureaucratic until the first time a reader files a
  bug that was really a naming inconsistency. `terms suggest` can propose canonical
  terms from the manuscript itself; `terms check` enforces them. On a large document
  with more than one author, this is the difference between a reference and a
  guessing game.
]

#section("Keep the examples true — verified code blocks")

The most common way documentation goes stale is the code example that no longer
compiles against the current release. Inkhaven can catch that the way the fiction
track catches a contradiction: by running the example. Mark a `para:code` listing's
fence with `verify` and name a runner for its language, and Inkhaven will run the
snippet and flag it if it fails — a stale example becomes a red mark on the exact
paragraph, like any other finding.

#config("inkhaven.hjson", [```hjson
docs: {
  verify: {
    enabled: true
    runners: {
      rust:   "rustc --edition 2021 --crate-type lib {file} -o /dev/null"
      python: "python -m py_compile {file}"
      bash:   "bash -n {file}"
    }
  }
}
```])

Only blocks that opt in are run — the fence carries the flag:

````
```rust verify
fn greet(name: &str) -> String { format!("Hello, {name}") }
```
````

Then `Ctrl+B Shift+D` verifies the open listing, or — for the whole book, or a CI
step — `inkhaven docs verify` (exits non-zero when any block fails; `--dry-run`
previews the commands, `--yes` confirms execution).

#pitfall[
  Verification runs the commands your config names, with your privileges — so it is
  *opt-in twice*: nothing runs unless you enable it and name a runner, and only
  blocks marked `verify` are executed (illustrative or pseudo-code stays untouched).
  Prefer compile-only or lint-only runners (`rustc`, `py_compile`, `bash -n`) over
  full test runners, and never point a runner at code you haven't read. Inkhaven
  will not run a freshly-cloned project's examples without your explicit `--yes`.
]

#insight[
  A verified example is the technical track's version of the fiction track's
  fact-check: a deterministic, zero-AI check against ground truth. The prose can
  still drift, but the _code_ in your docs now answers to the compiler — and the
  compiler doesn't forget what changed last release.
]

#section("Links that resolve")

The other face of staleness is the broken link — a cross-reference to a section you
renamed, or an external URL that has rotted away. `inkhaven docs links` checks both.
Internal cross-references are checked always and cost nothing: a link to a paragraph
that no longer exists is reported with its location. External URLs are opt-in behind
`--external` (it makes network requests), and — like the research dead-source sweep —
it is deliberately conservative: only a hard `404`/`410` or an unreachable host
counts as dead, so a bot-block or a transient hiccup never cries wolf.

```
inkhaven docs links              # internal cross-references (fast, offline)
inkhaven docs links --external   # also check http(s) URLs for link-rot
```

Like `docs verify`, it exits non-zero when anything is broken — drop it into the same
pre-release check.

#section("Write once — variables and profiles")

Two mechanisms let one source serve many outputs. *Variables* replace a token
everywhere at build time: define them once and write `{{product}}` or `{{version}}`
in your prose, and every export resolves them. Change the product name in one place
and the whole book follows.

#config("inkhaven.hjson", [```hjson
docs: {
  variables: { product: "Inkhaven", version: "1.6.9" }
}
```])

*Profiles* let one manuscript carry more than one edition. Tag a paragraph
`profile:edition:enterprise` or `profile:audience:expert` (the ordinary tagging
surface, `Ctrl+B ]`), then choose a slice at export:

```
inkhaven export pdf --profile edition=enterprise --profile audience=expert
```

A paragraph tagged for a dimension is emitted only when a matching value is
requested; a paragraph with no tag for that dimension is unconditional and always
appears. Dimensions you do not name are left alone — so the plain `export` with no
`--profile` gives you the full authoring view with every variant present.

#insight[
  Variables and profiles are the technical author's answer to duplication. The moment
  you find yourself maintaining two near-identical pages — a community and an
  enterprise version, a beginner and an expert path — reach for a profile before you
  reach for copy-paste. Two copies drift; one source with two profiles cannot.
]

#section("Publish it as a website")

Technical documentation usually lives as a website, so Inkhaven exports one directly:

```
inkhaven export html -o site/
```

That produces a *100% self-contained* static site — one page per chapter, a sidebar
table of contents, a clean reading theme, and your images copied alongside. There are
no external dependencies: no CDN, no web fonts, no scripts loaded from elsewhere, so
the folder opens from disk or drops onto any static host and renders identically
offline. The chrome is localised to the book's `language`, single-sourcing variables
and `--profile` slices apply exactly as they do for PDF, and site-wide values (title,
author, subtitle) come from an `html.hjson` file.

You do not need to write any templates — a bare `export html` renders a complete,
styled book from templates embedded in the binary. When you want to customise, write
the defaults out and point the exporter at your copy:

```
inkhaven export html --eject-templates my-templates/
inkhaven export html -o site/ --templates my-templates/
```

The templates split into `functional/` (the page skeleton and navigation — the
machinery) and `theme/` (the stylesheet and visual partials — the look). Any file you
keep overrides that one default; everything else keeps working. So a designer can
restyle `theme/theme.css` without ever touching the navigation logic.

#insight[
  The split between _functional_ and _visual_ templates is deliberate. The machinery
  — how the contents tree becomes a sidebar, how pages link — is the part you want to
  leave alone; the look is the part you want to own. Keeping them apart means you can
  make the site yours without inheriting the maintenance of the parts you did not
  change.
]

#section("Read — precision, and the reader who has your context and the one who doesn't")

The reading pass here is about clarity and completeness, not craft. Turn the
audience personas on a finished page: the `domain-newcomer` will show you every
place you assumed knowledge the reader lacks; the `end-user` will show you where the
page describes the system instead of helping them _do_ something; the
`expert-reviewer` will find the imprecise claim. Ask the Inner Socrates what a
procedure _presupposes_ — the installed dependency you never mention, the state the
reader must already be in — because an unstated precondition is the most common way
a correct instruction fails a real user.

#insight[
  Everything on this track serves one goal: that the document keep matching the
  system as the system moves. Snippets shrink the surface that can drift;
  terminology governance keeps the naming stable; status marks show what has been
  reviewed against the current release; reference structure means a change touches
  one findable page. You cannot stop the software from changing. You can make your
  documentation cheap to change with it.
]

#section("Produce")

`export pdf|epub|docx|html` renders the manual; scope by status so a release ships
only the pages reviewed against it (`--status ready`), leaving the stubs for next
time. Before you cut, `inkhaven docs review` shows the readiness of every chapter and
lists what is still below `ready` — and `docs review --since <release-tag>` flags the
pages whose source changed since your last release, so you re-read exactly those.
Many technical documents also live as a website — `export html` renders a
self-contained one directly (see the companion, _Publish Your Book to the Web_).

#section("Hands-on: two procedures")

#subsection("Reuse a block so a fix lands everywhere")

+ Open the Snippets book and author the block once — a warning, a setup step, a parameter table — as its own paragraph.
+ From any page that needs it, reference the snippet with a `#include` rather than pasting a copy.
+ When the block changes, edit it once in the Snippets book. Every page that included it now shows the correction — no hunt through forty copies.

#subsection("Govern a term across the whole document")

+ Let Inkhaven propose canonical terms from your own prose: `inkhaven terms suggest`.
+ In the Glossary book, record the canonical word and the synonyms you are banning — "widget", not "component" or "part".
+ Enforce it: `inkhaven terms check` reports every place the manuscript drifted from the canonical term, and the overlay `Ctrl+V z` shows the same inside the editor as you write.
+ Read a finished page through the reader who lacks your context: `Ctrl+B J`, then the `domain-newcomer` persona, to surface every assumed step; and the `end-user` persona to catch where the page describes the system instead of helping the reader _do_ something.

#recap((
  [Technical writing describes a *changing system*; its enemy is *staleness*. Set `genre: "technical"` (or `documentation`) and the `technical` template, and decide whether your reader is the newcomer or the practitioner.],
  [*Structure is the product*: build a reachable, self-contained reference tree, and track currency with status marks.],
  [Fight repetition with *Snippets* (one block, many pages) and enforce naming with the *Glossary* and `terms check` (`Ctrl+V z`) — a synonym is a bug.],
  [*Read for precision and unstated preconditions* through the audience personas (`domain-newcomer`, `end-user`, `expert-reviewer`); ship only status-`ready` pages so the doc matches the current release.],
))
