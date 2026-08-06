#import "../design.typ": *

#chapter(number: 10, title: "Assembling the Book")

For nine chapters *The Ninth Lantern* has been a tree. You have watched it grow
one node at a time: a book, then chapters, then paragraphs, each a small `.typ`
file that Inkhaven writes, indexes, and watches while you work. You drafted the
cold-lantern opening into it, kept Saltmarch's facts straight against it, planted
Aldous's secret so no one acts on it too early, tuned the voices, read the whole
thing forward as a first reader, ran the editorial pass, and — last chapter —
measured that the revision actually helped. The manuscript is, by any honest
reckoning, *done*.

What it is not yet is a *book*. It is still a folder of prose fragments and an
index in a database. This chapter is the short walk from one to the other: from
the buffer you have been living in to a typeset PDF sitting in the folder you
launched from, with the story's name and the day's date on it. It is the most
satisfying two keystrokes in the whole tool, and there is nothing hidden inside
them. Every file the process writes is Typst you can open and read; the whole of
it is rebuilt from your tree each time you ask, so you never have to wonder
whether the PDF matches the manuscript. It always does — it was just made from
it.

#section("The two chords, and the third")

Turning a manuscript into a book is a little ladder of three chords, each doing
everything the one below it did, and then one thing more. You can reach for them
from anywhere — the Tree pane, the Editor, it does not matter — because they act
on *the book your cursor is inside*, not on whichever pane happens to have focus.

#screen(caption: "The three chords, one ladder")[```
  Ctrl+B A   Assemble — tree → a Typst-compilable directory.
             No PDF yet, just source a compiler can run.

  Ctrl+B B   Build — assemble, THEN compile it to a PDF.
             (A failed compile is handed to the AI.)

  Ctrl+B O   Take — build, THEN copy the finished PDF into
             the directory you launched from, timestamped.
```]

Read the ladder from the bottom rung up. `Ctrl+B A` is assembly on its own —
you reach for it when you want to look at the generated Typst, hand it to
`typst watch`, or drive the compiler yourself. `Ctrl+B B` is the everyday chord:
assemble and compile in one motion. `Ctrl+B O` — *O* for the copy you *take*
away — is the finishing move; it does all of `Ctrl+B B` and then lifts the PDF
out of the throwaway build cache and sets it down where your shell can see it.

#callout(label: "It has to be a manuscript")[
  All three chords act on the *user book* your tree cursor is sitting inside. If
  the cursor is parked on a system book — Notes, Facts, Places, Timeline, and the
  rest — the chord politely refuses with a status message asking you to pick a
  real book. Those system books are your author's workshop; they are never part
  of what ships. (The Timeline is skipped even when it lives inside the
  manuscript: it is metadata about the story, not the story.)
]

Whatever you press, Inkhaven saves first. If the paragraph you have open has
unsaved edits, they are flushed to disk before assembly reads a single node, so
the thing you compile is the thing on your screen. Put your cursor anywhere in
*The Ninth Lantern* and press the first chord.

#section("Assemble — Ctrl+B A")

Your manuscript lives as `.typ` files, but not in a shape Typst can compile as
it stands. A paragraph file holds only its prose. A chapter is a *folder* and
carries no words of its own. Nothing anywhere says "include this one, then that
one, then wrap the lot in a book." Assembly's whole job is to synthesise that
missing structure — the include tree, the wrappers, the page setup — into a
fresh directory, and to leave your actual tree untouched.

It does not write that directory into your project. It writes it to a per-user
cache, out of the way, so `git status`, your backups, and shell tab-completion
never trip over build litter. When the chord finishes, the status bar prints the
absolute path of the one file you would ever name to a compiler — the root
`the-ninth-lantern.typ` at the top:

#screen(caption: "The assembled tree for The Ninth Lantern")[```
<cache>/inkhaven/artefacts/<project>/the-ninth-lantern/
├── the-ninth-lantern.typ   ← the root; hand THIS to typst
├── globals.typ             ← your wrap_* definitions
├── settings.typ            ← page / fonts / layout
└── book/
    ├── index.typ           ← the book's include list
    ├── 01-the-cold-lantern/
    │   ├── index.typ
    │   ├── 01-a-dark-pillar.typ
    │   └── 02-crane-is-gone.typ
    ├── 02-the-wrong-man/
    │   ├── index.typ
    │   └── 01-toft-and-the-oil.typ
    └── 03-onto-the-mole/
        ├── index.typ
        └── 01-into-the-fret.typ
```]

Everything under `book/` mirrors your manuscript one-to-one. Every chapter and
subchapter is a folder with its own `index.typ`; every prose paragraph is a
`.typ` file; the `NN-` prefixes are the same order numbers the Tree pane shows
you. The `index.typ` files are the seams — they `#include` their children and
call three helper functions, `wrap_book`, `wrap_chapter`, and `wrap_paragraph`,
which are defined once up in `globals.typ`. That is where your typography lives:
change what `wrap_chapter` does and every chapter opening in Saltmarch changes
with it.

#callout(label: "The artefacts tree is output, not source")[
  Assembly *wipes* `.../the-ninth-lantern/` clean before it writes a byte — so a
  chapter you deleted, a paragraph you renamed, last week's stale PDF, none of
  it lingers to confuse the next compile. The cost is that anything you
  hand-edit in there is gone on the next `Ctrl+B A`. To change your book's look,
  you edit the `globals.typ` / `settings.typ` paragraphs under the *Typst* system
  book and re-assemble — never the copies out here. These are disposable.
]

#section("Build — Ctrl+B B")

`Ctrl+B A` gave you source a compiler can run; `Ctrl+B B` runs it. Press it and
a small splash takes over the screen — a spinner and a seconds counter — while
Typst lays out the pages. It is quick for a book this size. On success the status
bar hands back the path of a real PDF, and *The Ninth Lantern* exists as a
document for the first time.

#screen(caption: "Ctrl+B B — assemble, then compile")[```
        ┌──────────────────────────────────────┐
        │                                      │
        │        ◐  compiling…  1.2 s          │
        │                                      │
        │     the-ninth-lantern · typst        │
        │        Esc to interrupt              │
        │                                      │
        └──────────────────────────────────────┘

  status ▸ PDF ready:
    …/artefacts/…/the-ninth-lantern.pdf
```]

Behind that spinner, Inkhaven can drive Typst two ways. By default it finds the
`typst` binary on your `PATH` and runs it as a child process — the *external*
engine. If you would rather not install a separate tool, an *in-process* engine
(`typst_compile.engine = "inprocess"` in your HJSON) links the compiler straight
into Inkhaven and needs no external binary at all. The splash names which one is
running, so you are never guessing. Assembly, note, never touches Typst; only
this compile step does. If the external binary is missing, the compile fails
with a clean message pointing you at the install docs — or at the in-process
knob.

#section("When Typst complains")

Sooner or later a compile fails, and it is worth seeing that here, because
Inkhaven turns the ugliest moment in publishing into something almost pleasant.

Say that between drafts you slipped down into the *Typst* system book and taught
your chapter openings to carry an epigraph — a line of the keepers' creed, *no
lantern goes dark*, set under each title. You wrote a helper for it in
`globals.typ` and called it `wrap_epigraph`. But in the call you added to the
cold-lantern chapter you fat-fingered it `wrap_epigrah`. Press `Ctrl+B B` and
Typst refuses, because that variable does not exist.

A raw compiler would spit a wall of stderr at you and leave. Inkhaven does two
things instead, with no further keystroke. First it scans the error for dangling
cross-references — a stray `@fig:` or `@eq:` — and promotes each into a precise
finding in the Output pane, because that is a genuine defect, not noise. Then it
opens a *fresh* AI chat: it clears the history so nothing dilutes the context,
forces the inference mode to Full, and loads a system prompt that knows Typst
*and* knows Inkhaven's generated file layout. It packages the stderr with the
book's name, slug, and root path, sends it for you, and jumps focus to the AI
pane. The answer is already streaming in while you are still reading the error.

#screen(caption: "A compile failure, handed to the assistant")[```
┌─ AI · llama · streaming · typst-error ──────────────┐
│ you  Book: `The Ninth Lantern` (the-ninth-lantern). │
│      typst compile failed. Smallest concrete fix?   │
│      --- typst stderr ---                            │
│      error: unknown variable: wrap_epigrah          │
│        ┌─ book/01-the-cold-lantern/index.typ:7:4    │
│                                                     │
│ ai   Line 7 calls `wrap_epigrah` — a typo for       │
│      `wrap_epigraph`. But you never edit these       │
│      generated files; they are rebuilt every run.    │
│      Fix the call in the chapter paragraph under      │
│      the Typst system book (the epigraph you added   │
│      to "The Cold Lantern"), then re-assemble.       │
└─────────────────────────────────────────────────────┘
```]

That last sentence is the whole point. The error names a file down in the
artefacts tree, but that file is generated — editing it would be undone on the
next assembly. The assistant knows this and walks you back to the real source:
the paragraph in the Typst book where you wrote the call. And because a failed
compile *keeps* the assembled tree rather than deleting it, you can also open
that `index.typ` at the very line Typst named, read it in context, and confirm
the diagnosis with your own eyes. Fix the source paragraph, press `Ctrl+B B`
again, and the book compiles.

#two_track[
  You added an epigraph helper and mis-typed its name; the fix lives in your
  Typst book, not in the failing generated file. Same for a missing figure or a
  broken `wrap_chapter` — chase it back to the paragraph you actually wrote.
][
  A non-fiction build fails the same friendly way — a malformed `@key`
  citation, a table macro called wrong — and the same fresh, layout-aware chat
  meets you with the smallest fix. Nothing about this path is fiction-only.
]

#section("Take the book — Ctrl+B O")

The PDF now exists, but it exists in a cache directory with a `<hash>` in the
path that no one wants to type. `Ctrl+B O` is the last rung: it does everything
`Ctrl+B B` does, and then *copies* the finished PDF out of the cache and into the
directory you launched Inkhaven from — under a name you can actually find:

#screen(caption: "What Take drops in your launch directory")[```
the-ninth-lantern-20260508-1730.pdf
```]

The stem is the book's slug, `the-ninth-lantern`; the stamp is `YYYYDDMM-HHMM` —
year, day, month, hour, minute, the same ordering Inkhaven uses for its backup
files. Take *copies*, it does not move: the original stays in the artefacts
cache too, so a later imposition pass or a re-take still has a source PDF to work
from. The status bar reports both the delivered path and the source, and there
is your book, sitting in the folder where you started the morning:

#screen(caption: "Back in the shell where it all began")[```
~/writing/ninth-lantern $ ls
inkhaven.hjson   books/   .inkhaven/

~/writing/ninth-lantern $ inkhaven          # ... write the book ...
                                            # ... Ctrl+B O ...
~/writing/ninth-lantern $ ls *.pdf
the-ninth-lantern-20260508-1730.pdf

~/writing/ninth-lantern $ open the-ninth-lantern-20260508-1730.pdf
```]

That is the buffer-to-book moment: you launched Inkhaven in an empty folder ten
chapters ago, and now a timestamped PDF of *The Ninth Lantern* is sitting right
next to the project you built it from. If you have configured extra formats
(Markdown, TeX, EPUB — the next chapter's business), Take writes them beside the
PDF with the same stem; an extra that fails is reported but never aborts the
take, because the PDF you actually asked for is already on disk before the extras
run.

#section("The same path, from the command line")

Everything the three chords do is available without the TUI at all — for a build
script, a CI job, or just checking that a typography change still compiles.
`inkhaven build` is the headless mirror of the ladder:

#screen(caption: "Building The Ninth Lantern from the shell")[```
$ inkhaven build
Assembling `The Ninth Lantern` (slug: the-ninth-lantern)…
  [12/28] book/01-the-cold-lantern/01-a-dark-pillar.typ
Assembly OK · root: …/the-ninth-lantern.typ (28 files)

$ inkhaven build --compile
PDF: …/the-ninth-lantern.pdf
```]

Without `--compile` it stops after writing the artefacts tree — that is
`Ctrl+B A`. With it, it runs `typst compile` and prints *only* the final PDF
path to stdout; the per-file progress goes to stderr, so the command drops
cleanly into a pipeline. The `--book-name` flag is optional when the project has
exactly one user book, as ours does, and required when it has more. This is the
one place a build can run untended — which is exactly why it is worth having when
the manuscript is stable and the only question left is *did the last change still
build?*

#recap((
  [Three chords climb one ladder: *`Ctrl+B A`* assembles the tree into a
  Typst-compilable directory, *`Ctrl+B B`* also compiles it to a PDF, and
  *`Ctrl+B O`* also copies that PDF — timestamped `the-ninth-lantern-YYYYDDMM-HHMM.pdf`
  — into the folder you launched from.],
  [Assembly writes a *fresh, disposable* tree under a per-user cache: a root
  `the-ninth-lantern.typ`, `globals.typ`, `settings.typ`, and a `book/` subtree
  of `index.typ` files calling `wrap_book` / `wrap_chapter` / `wrap_paragraph`.
  It wipes that directory every run — treat it as output, never source.],
  [Change your book's look in the *Typst system book* paragraphs
  (`globals.typ` / `settings.typ`) and re-assemble; never edit the artefacts
  copies, which are overwritten each build.],
  [A failed compile *routes its stderr into a fresh, Typst-aware AI chat*
  automatically — cleared history, Full mode — and walks you back to the real
  source paragraph; the assembled tree is kept so you can read the offending
  line in context.],
  [`inkhaven build [--compile]` is the headless mirror of the chords, for CI and
  scripts: progress to stderr, the final PDF path to stdout.],
))
