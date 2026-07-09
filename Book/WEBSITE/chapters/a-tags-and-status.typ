#import "../design.typ": *

#appendix(letter: "A", title: "Tags, subtypes, and status")

Three small things you attach to a paragraph shape what the website does with it —
and because they are easy to confuse, this appendix lays out each one plainly: what it
is, how to set it, and, where it matters for the web, the full list.

They are genuinely _separate_. A paragraph's *status* says how finished it is. Its
*structural subtype* says what kind of block it is. Its *tags* are free-form labels.
None of them is the others.

#insight[
  The quick way to keep them straight: *status* is one value on a ladder (it is not a
  tag); a *subtype* is chosen from a fixed menu and changes how the paragraph looks;
  *tags* are words you invent and reuse. Different purposes, different keys, different
  places they show up on the command line.
]

#section("Status — how finished a paragraph is")

Status is a single value drawn from a fixed ladder, lowest to highest:

#config("the status ladder", [```
none · napkin · first · second · third · final · ready
```])

It is _not_ a tag — it is its own property of the paragraph. Set it in the editor with
`Ctrl+B r`, which cycles a paragraph up the ladder.

#term("`--status` (and `docs review`)")[
  On export, `--status ready` publishes only paragraphs that have reached `ready` or
  above, so an unfinished draft never ships. `inkhaven docs review` prints the
  readiness of every chapter and lists what is still below the line — run it before you
  publish. Status is what both of these read.
]

#section("Structural subtypes — what kind of block it is")

A subtype turns an ordinary paragraph into a _particular kind_ of block — a code
listing, a warning, a table. Choose one in the tree with `Ctrl+B m`, which reshapes the
paragraph and seeds it with the right skeleton. Each renders as proper, styled HTML on
the site:

#gloss("Code listing")[A block of code, shown in a highlighted monospace panel.]
#gloss("Admonition — note / tip")[A calm, teal-bordered callout for an aside.]
#gloss("Admonition — warning / caution")[A coloured callout that draws the eye to a caveat.]
#gloss("Procedure")[A numbered list of steps.]
#gloss("Table")[A grid of rows and columns.]
#gloss("Math")[A formula.]

You do not type these as tags — you pick them from the `Ctrl+B m` menu, and Inkhaven
records the subtype for you. On the website they become the callout boxes, code panels,
and lists you see throughout a well-made page.

#section("Tags — your own labels")

Tags are words _you_ attach to paragraphs and reuse across the book. Open a paragraph
and press `Ctrl+B ]` to open the tag picker:

#chord_table((
  chord_row("A", "Add a new tag — type the word and it joins the project's tag list."),
  chord_row("Space", "Select one or more existing tags from the list."),
  chord_row("T", "Apply the selected tags to the open paragraph."),
  chord_row("D", "Delete a tag everywhere in the project."),
))

`Ctrl+B }` opens the matching search — pick a tag to see every paragraph that carries
it.

Two kinds of tag matter when you publish.

#subsection("Profile tags — editions")

A tag of the form `profile:dimension:value` marks a paragraph as belonging to one
edition — for example `profile:edition:full` or `profile:audience:expert`. At export,
`--profile edition=full` publishes only the matching paragraphs (plus the unlabelled
ones). This is the whole of Chapter 6; the tag is how you mark a paragraph in the first
place.

#subsection("Plain tags — slices and organisation")

Any other word — `draft`, `appendix`, `chapter-notes` — is a plain tag. Beyond helping
you find and organise paragraphs, a plain tag can slice an export: `--tag draft`
publishes only the paragraphs carrying `draft`. (Profiles narrow by _edition_; a plain
`--tag` narrows to a single labelled set.)

#pitfall[
  Neither tags nor profiles are a lock on secrets. Anything you leave unlabelled is
  published in every edition, and a plain export publishes every finished paragraph. To
  keep material off the site for certain, keep it out of the book or below `ready` — do
  not rely on the _absence_ of a tag.
]

#recap((
  [*Status* (`Ctrl+B r`) is a single value on a ladder — not a tag — and drives `--status` and `docs review`.],
  [*Structural subtypes* (`Ctrl+B m`) — code, admonitions, procedure, table, math — decide how a paragraph renders.],
  [*Tags* (`Ctrl+B ]`: A add · Space select · T apply · D delete) are your own labels.],
  [*Profile* tags (`profile:dim:value`) drive `--profile` editions; *plain* tags drive `--tag` slices.],
))
