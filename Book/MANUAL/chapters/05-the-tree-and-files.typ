#import "../design.typ": *

#chapter(number: 5, title: "The Tree and Its Files")

The last chapter mapped the window; this one goes into the pane on its left and
never really leaves it. The Tree is where a book *has a shape* — where its parts
and chapters and paragraphs stand in order, where you add a scene and move it
three chapters earlier, where you fold a finished act out of the way and open
the one you are still fighting. It is also, quietly, the pane where Inkhaven
stops being only a prose editor: the same outline that holds your chapters holds
data paragraphs, templates, scripts, images, and a family of *structural*
paragraphs for the code, tables, and admonitions a technical book needs. This
chapter teaches the Tree as a builder's tool first — how to grow and reshape the
outline, how to delete without fear — and then walks every kind of leaf that can
hang from it, one at a time, in real depth.

Everything here happens with the Tree pane focused. Reach it with `Ctrl+T`, or
`Tab` around to it; it is the pane that has focus when a project first opens. The
plain-letter shortcuts in this chapter — `B`, `C`, `+`, `D`, `t`, `i`, `e`, and
the rest — fire *only* while the Tree has focus, and they reject `Ctrl` and
`Alt`, so `Ctrl+A` will never quietly add a subchapter. Some carry a `Ctrl+B`
chord form too, but those structural adds are *Tree-scoped*: `Ctrl+B C`, `S`,
`P` add a chapter, subchapter, and paragraph only while the Tree has focus —
from the Editor the same chords are character-lookup, read-aloud, and
place-lookup — and `Ctrl+B B` builds, so a book is the bare `B` alone. The bare
letters exist because terminals and multiplexers so often eat the meta prefix.

#section("What a tree row says")

The Tree draws your project as an indented outline, one node per row, each row
carrying more information than its width suggests. Read a row left to right and
it tells you, in order: how deep it sits (by indentation), whether it is marked
for a batch operation, what *kind* of node it is (a glyph), how far along its
draft is (a status letter), its title, and then a train of small *pips* — a
progress gauge, tag chips, and any findings a reader has posted against it.
Paragraphs also carry a dim `Nw` word count at the end.

#screen(caption: "A tree, read top to bottom — glyphs, badges, and pips")[```
┌─ Tree ──────────────────────────────────────────────┐
│ ▾ Rain Over Quaymouth                               │
│   ▾ 1 Arrival                                        │
│     ► the-quay              R  ●  #opening    214w  │
│     ¶ the-inn               F  ◑            88w      │
│     ⌨ install-listing                               │
│   ▸ 2 The City                          ⚠2          │
│   ▾ 3 Departure                                      │
│     ❴ ledger-of-ships       hjson                    │
│     ⟡ crew-sidebar          jinja                    │
│     ◆ the-parting                       ⊗1          │
│ ▸ Facts                                              │
│ ▸ Notes                                              │
│ ▾ Scripts                                            │
│     λ on-save-guard         bund                     │
└──────────────────────────────────────────────────────┘
```]

#term("Node")[
  A *node* is one entry in the tree — a book, a chapter, a subchapter, a
  paragraph, an image, or a script. Branches (book, chapter, subchapter) can
  hold children; leaves (paragraph, image, script) cannot. The whole book is a
  tree of nodes, and almost everything you do in this pane is adding a node,
  moving a node, changing a node's kind, or deleting one.
]

#subsection("The kind glyph")

The glyph in front of a title is the fastest read on the row — it tells you what
the node *is* without opening it. Branches show a fold arrow: `▾` when expanded,
`▸` when collapsed. Leaves show their nature. Prose is the plain `¶`; the other
leaves each earn a glyph of their own, and this chapter's second half is a tour
of exactly what stands behind each one.

#screen(caption: "Every glyph you will meet in the Tree")[```
  ▾ / ▸   branch, expanded / collapsed (book·chapter·subchapter)
  ►       the paragraph currently open in the Editor (green, bold)
  ¶       prose paragraph — the default leaf (.typ)
  ❴       HJSON data paragraph (.hjson)
  ⟡       Jinja template paragraph (.jinja)
  λ       Bund script (.bund)
  ▣       image (.png · .jpg · .webp · .svg)
  ◆       timeline-event paragraph
  ⌨ ⚠ ∫ ≡ ⊞   structural: code·admonition·math·procedure·table
  ‖ ♩ ‗ ⁚ ⁛ ⇄   verse: line·stanza·couplet·tercet·quatrain·translation
```]

The open paragraph is special: whichever leaf is loaded in the Editor is drawn
with a green, bold `►` in place of its usual glyph, and it keeps that marker
whether or not the Tree has focus. This is your anchor — however far your tree
cursor wanders, the `►` always shows you which paragraph the Editor is holding.
If the cursor happens to land on that same row, the reversed cursor highlight
wins the foreground, but the green underneath still marks it.

#subsection("The badges and pips")

After the glyph comes a run of optional marks. None of them is noise; each is a
readout you can act on.

- A *status letter* rides just before the title on paragraphs — the draft-status
  ladder as a single initial (`N`apkin, `F`irst, `S`econd, `T`hird, `F`inal,
  `R`eady). Cycle it up a rung with `o` in the Tree, or from the Editor. A
  paragraph with no status shows nothing here.
- A *target gauge* appears when a paragraph carries a word-count goal — a single
  filling pip (`○ ◔ ◑ ◕ ●`) that greens as the draft approaches its target and
  turns bold green at 100%. Set or clear the target with `Ctrl+V t`.
- *Tag chips* show up to two of a paragraph's tags as dim `#tag` labels, with a
  `+N` when there are more. Manage tags with `g` in the Tree or `Ctrl+B ]` in
  the Editor.
- A *report-card badge* appears when a reader has posted findings against a node
  — a worst-severity glyph and a count (`⊗2` a contradiction, `⚠3` a warning,
  `●1` an informational note). On a branch the badge is *aggregated* up from the
  paragraphs beneath it, so a collapsed chapter still tells you something inside
  it needs attention. These come from the review pass (`Ctrl+B Shift+C`) and the
  intelligences in Part V.
- A dim `Nw` word count closes every paragraph row.

#section("Moving through the tree")

Navigation is arrow-driven and quiet. `↑` and `↓` walk the cursor one visible
row at a time; `Home` and `End` jump to the first and last rows; `PageUp` and
`PageDown` move ten rows at once. `Enter` *opens* the cursor's node — a
paragraph loads into the Editor and pulls focus there (autosaving whatever was
open if it was dirty), an image pops a preview, and a branch simply prints a
status hint and stays put, since a chapter is a container, not a document.

#subsection("Folding — the four keys")

A long book is unreadable in the Tree unless you can fold the parts you are not
working in. Two motions expand and collapse a single branch; two more fold in
bulk.

- `→` *expands* the branch under the cursor, revealing its children. It is a
  no-op on a paragraph or an already-open branch.
- `←` *collapses* an expanded branch. If the branch is already collapsed — or the
  cursor is on a leaf — `←` instead walks the cursor *up to the parent*, so
  pressing it repeatedly climbs you out of a deep subtree toward the root.
- `Z` folds the cursor's *enclosing subchapter* (or the cursor's node itself if
  it already is one) and lands the cursor on the folded row — the fast way to
  tuck away the section you just finished.
- `X` *collapses everything* — folds every expanded branch in the whole tree,
  leaving you the top-level outline to navigate from. Paragraphs and empty
  branches are left untouched.

#callout(label: "The cursor is not the open paragraph")[
  Two different things move in the Tree: the *cursor* (the reversed highlight you
  drive with the arrows) and the *open paragraph* (the green `►`). Moving the
  cursor changes nothing on disk and opens nothing — it only chooses where the
  next action lands. You have to press `Enter` to actually open a node. This
  separation is what lets you reorder, tag, or delete a paragraph without first
  loading it into the Editor.
]

#chord_table((
  chord_row("↑ / ↓", "Move the cursor one visible row up / down."),
  chord_row("→", "Expand the cursor's branch."),
  chord_row("←", "Collapse the branch, or step up to the parent."),
  chord_row("Home / End", "Jump to the first / last row."),
  chord_row("PageUp / PageDown", "Move the cursor ten rows."),
  chord_row("Enter", "Open the node — load a paragraph, preview an image."),
  chord_row("Z", "Collapse the cursor's enclosing subchapter."),
  chord_row("X", "Collapse every expanded branch in the tree."),
  chord_row("Space", "Mark / unmark the row for a batch operation."),
  chord_row("Esc", "Cycle focus onward to the Search bar."),
))

#section("Building the tree")

A new project is a nearly empty tree — a book and the system books Inkhaven
maintains for you. You grow it by adding nodes, and the Tree gives you a small,
learnable vocabulary for doing so. Every add opens the same green *Add modal*: a
floating box that names the parent it will slot into and takes a title. Type the
title, press `Enter`, and Inkhaven derives a slug, writes the file to disk,
inserts the record, reloads the tree, and moves the cursor onto the new node.
`Esc` cancels; an empty title keeps the modal open with a hint (except for
paragraphs, where an empty title is allowed — the first sentence of the body
becomes the title on the next save).

#screen(caption: "The Add modal — every kind is created through this box")[```
┌── Add chapter ──────────────────────────────────┐
│  Parent: rain-over-quaymouth                    │
│  Title : The Long Road South▏                   │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```]

#subsection("The four building blocks — and where they land")

The everyday structure keys come in two flavours, and the difference is *where
the new node lands among its siblings*. This is the single most useful thing to
understand about building a tree.

- *Append at end.* `B` (book), `C` (chapter), `A` (subchapter), and `+`
  (paragraph) add the new node at the *end* of its parent's children. Inkhaven
  chooses the parent by walking up from your cursor to the nearest node that can
  legally host the kind you asked for — press `+` deep inside a subchapter and
  the paragraph appends to that subchapter; press `C` anywhere in a book and the
  chapter appends to the book.
- *Insert after current.* `V` (chapter), `S` (subchapter), and `P` (paragraph)
  add the new node *immediately after* the cursor's same-kind ancestor, shoving
  every later sibling down by one — the way you add a scene *between* two scenes
  rather than at the end of the act. If there is no same-kind ancestor to insert
  after, these fall back to append-at-end so the key still does something.

The mnemonic is the shape of the keyboard row: the append keys sit under your
fingers as whole words (`B`ook, `C`hapter, `A` for subchapter, `+` for one
more), and the insert-after keys are the ones you reach for when order matters.
These also carry `Ctrl+B` chord forms — `Ctrl+B C`, `Ctrl+B S`, `Ctrl+B P` for
chapter, subchapter, and paragraph — but only while the Tree has focus, not from
any pane; adding a book stays the bare `B`, since `Ctrl+B B` builds instead.

#screen(caption: "Append versus insert-after, on the same chapter")[```
  cursor on 'the-inn', press +        cursor on 'the-inn', press P
  ┌ 1 Arrival ──────────┐             ┌ 1 Arrival ──────────┐
  │   ¶ the-quay         │             │   ¶ the-quay        │
  │   ¶ the-inn   ←here  │             │   ¶ the-inn  ←here  │
  │   ¶ the-market       │             │   ¶ NEW      ←after │
  │   ¶ NEW       ←end   │             │   ¶ the-market      │
  └─────────────────────┘             └─────────────────────┘
```]

#callout(label: "Books slot in above the system block")[
  A new user book (`B`) is inserted *above* the built-in system books — Notes,
  Research, Prompts, Places, Characters, Help, and the rest — which shift down to
  make room. Those system books are *protected*: Inkhaven will not let you rename
  or delete them, because features across the tool find them by tag. One
  context-sensitive exception is worth knowing: pressing `b` on the `Language`
  system book scaffolds a whole conlang sub-book under it, five chapters deep —
  a special case covered in the language chapters.
]

#subsection("Renaming and the file picker")

`F2` opens the *Rename* modal, pre-filled with the current node's title. It
changes only the displayed title and re-embeds the node for search; the slug and
the on-disk filename stay put, so a rename never breaks a cross-reference or a
snippet include. `F3` opens a *file picker* rooted at your shell's working
directory: `Enter` on a file creates a paragraph after the cursor with that
file's contents, and `Enter` on a *directory* recursively imports the whole tree
— subfolders become branches, files become paragraphs, flattened to fit the
hierarchy's depth. (An image file picked this way is routed to the image import
path described later, not turned into prose.)

#section("Reshaping the tree")

Structure is never right the first time. The Tree gives you three ways to move a
node that already exists: reorder it among its siblings, change its nesting
level, and relocate it to a different parent entirely. All three reuse the same
filesystem-aware store primitives, so the `.typ` files on disk are renamed and
renumbered to match whatever you do — the tree and the folder never drift apart.

#subsection("Reordering among siblings")

`U` moves the cursor's node *up* — swapping it with its previous sibling — and
`J` moves it *down*. These are the plain-letter forms of `Ctrl+B ↑` and
`Ctrl+B ↓`; use the letters when your terminal swallows Ctrl with the arrow
keys. Each swap renumbers the two nodes' filesystem entries, so the order you
see is the order the book assembles in. The CLI mirror is `inkhaven mv … up`.

#subsection("Promote and demote — changing nesting level")

Reordering keeps a node at its level; *promote* and *demote* change the level.
These live in the full-screen Outline pane (`Ctrl+2`), the structural sibling of
the side Tree, where `<` *promotes* a childless node one level out (appending it
under its grandparent) and `>` *demotes* it one level in (nesting it into the
preceding sibling). Any move that would break a placement rule — a paragraph
where a chapter belongs — leaves the manuscript untouched and tells you why. The
Outline pane is also where you get a wide, foldable view of the whole book with
a detail panel; it shares the Tree's clipboard, so the two stay in lock-step.

#subsection("Copy and move across parents")

To relocate a paragraph to an entirely different chapter, use the cross-pane
clipboard. `y` *copies* the cursor paragraph onto it; `m` *moves* (cuts) it; then
navigate to the destination and press `f` to *affix* the clipboard paragraph as
the last child of the cursor's effective parent — into a branch when the cursor
is a branch, alongside it when the cursor is a paragraph. A copy duplicates with
a fresh UUID and keeps the clipboard loaded; a move relocates the original and
clears the clipboard. The same `y` / `m` / `f` work in the Outline pane, and the
CLI equivalent is `inkhaven paragraph copy|move <src> <dest>`.

#chord_table((
  chord_row("U  ·  Ctrl+B ↑", "Move the node up one place among its siblings."),
  chord_row("J  ·  Ctrl+B ↓", "Move the node down one place among its siblings."),
  chord_row("< / >  (Outline)", "Promote / demote a childless node one nesting level."),
  chord_row("y", "Copy the cursor paragraph onto the clipboard."),
  chord_row("m", "Move (cut) the cursor paragraph onto the clipboard."),
  chord_row("f", "Affix the clipboard paragraph under the cursor's parent."),
  chord_row("F2", "Rename the node's displayed title (slug + file unchanged)."),
))

#section("Deleting, and getting it back")

Deletion is the one action that can lose work, so Inkhaven wraps it in three
layers of safety — a confirmation that tells you the cost, an immediate undo, and
a durable snapshot for the large deletes. Understanding the layers is what lets
you delete freely.

#subsection("The two delete keys")

Delete is kind-specific on purpose. `-` deletes the cursor's node *only if it is
a paragraph*; press it on a branch and you get a hint to use `D` instead. `D`
deletes *only a branch* (book, chapter, subchapter); on a paragraph it points
you back to `-`. The split is a guard rail: `-` will not nuke a whole chapter
because your cursor drifted onto it, and `D` will not kill a paragraph you meant
to keep. If you genuinely want a kind-blind delete, the global `Ctrl+B D` does
it. Either way a red *Confirm delete* modal opens first.

#subsection("The confirmation tells you the cost")

The confirm names the kind, the title, the descendant count, and — the part that
matters — *the word count you are about to lose*:

#screen(caption: "The delete confirmation counts the words at risk")[```
┌── Confirm delete ───────────────────────────────┐
│  Delete chapter `Act II` and 12 descendants     │
│  (15,342 words)?                                │
│                                                 │
│  Removes files from disk AND records from the   │
│  store.  y / Enter confirm · n / Esc cancel     │
└─────────────────────────────────────────────────┘
```]

A single paragraph reads `Delete paragraph `the-quay` (342 words)?`. Zero-word
deletes — an empty paragraph, an HJSON or Jinja leaf — omit the count, since
there is no prose to lose. `y`, `Y`, or `Enter` confirms; `n`, `N`, or `Esc`
backs out. If the paragraph you had open was inside the deleted subtree, the
Editor closes with it.

#subsection("The kill-ring — immediate undo")

Every deleted paragraph is stashed on a *kill-ring*, and a branch delete stashes
*every paragraph leaf* under it, not just the one you were pointing at. `Ctrl+B U`
restores the front of the ring — the most recent deletion — instantly; the
status line reports the word count that came back. For a branch delete, repeated
`Ctrl+B U` restores the paragraphs one at a time, in their original order.

#term("Kill-ring")[
  A bounded stack (up to ten entries, capped by
  `editor.deleted_paragraph_history`) of recently deleted paragraphs. `Ctrl+B U`
  pops the front; `Ctrl+V Shift+U` opens a *picker* over the whole ring so you
  can choose which deletion to bring back. Restored paragraphs return at their
  original tree position but get a *fresh UUID*, so any cross-reference that
  pointed at the old node stays broken — Inkhaven flags this in the status rather
  than silently mis-linking.
]

#screen(caption: "Ctrl+V Shift+U — the kill-ring restore picker")[```
┌─ Restore deleted paragraph ─────────────────────────┐
│ ▌ the-parting        Act III · 512w · 2m ago        │
│   the-market         Act I  · 88w  · 14m ago        │
│   ledger-of-ships    Act III · hjson · 20m ago      │
├─────────────────────────────────────────────────────┤
│ ↑↓ select · Enter restore in place · Esc cancel     │
└─────────────────────────────────────────────────────┘
```]

#subsection("Pre-delete snapshots — the long-term net")

A very large branch can overflow a ten-slot ring, so before a *branch* delete
Inkhaven takes an annotated snapshot of every paragraph leaf, labelled
`pre-delete: <title> · <date>`. These live in the ordinary snapshot system, so
you can find and restore any of them from the `F6` snapshot picker long after the
kill-ring has cycled — even in a later session. The snapshots are taken *before*
the delete fires, so they survive even a delete that partially fails. Together
the three layers mean a branch delete is fully recoverable: the confirmation
tells you what is at stake, the kill-ring is instant undo, and the `F6`
snapshots are the durable safety net. (Single-paragraph deletes skip the
snapshot — it would only clutter the list, and the kill-ring already covers
them.)

#chord_table((
  chord_row("-", "Delete the cursor's node — paragraphs only."),
  chord_row("D  ·  Ctrl+B D", "Delete the cursor's node — branches (Ctrl+B D is kind-blind)."),
  chord_row("Ctrl+B U", "Restore the most recently deleted paragraph."),
  chord_row("Ctrl+V Shift+U", "Open the kill-ring picker — choose which deletion to restore."),
  chord_row("F6", "Snapshot picker — reach the pre-delete snapshots of a branch."),
))

#section("The leaf types")

So far a leaf has mostly meant a prose paragraph. It need not. A leaf is any
childless node, and Inkhaven ships six kinds of them, each stored as its own file
on disk and each with a job. The rest of this chapter takes them one at a time.
The through-line: *the manuscript is always plain files* — a prose paragraph is a
`.typ`, a data paragraph a `.hjson`, a template a `.jinja`, a script a `.bund`,
an image its own bytes — and the `content_type` field (or the node kind) is all
that distinguishes them.

#subsection("Prose — the .typ paragraph")

The default leaf, and the one you will make thousands of, is a *prose
paragraph*: a Typst `.typ` file holding the words of your book. It shows the `¶`
glyph, counts toward your word totals, and is the surface every prose
intelligence reads — the Inner Editor, the continuity watch, the fact-checker,
the read-through. There is nothing to configure; `+` or `P` makes one, and you
write. Everything the next chapter says about the Editor is about editing these.
The three other *text* leaves below are all, on disk, close cousins — the same
kind of node with a different `content_type` — and you convert between them with
a single morph key covered at the end of the chapter.

#subsection("HJSON — the data paragraph")

Some content is *data*, not prose: a character's dossier, a ship's manifest, a
glossary entry, a table of API fields. For these Inkhaven offers the *HJSON data
paragraph* — a paragraph whose `content_type` is `"hjson"`, stored as a `.hjson`
file and shown with the `❴` glyph. HJSON is JSON for humans: unquoted keys,
comments, trailing commas forgiven. The Editor switches to an HJSON highlighter
and header badge, and the prose intelligences stand down — a data blob is not
prose, so it draws no style notes and does not inflate the word count.

#screen(caption: "An HJSON data paragraph — structured, not prose")[```
┌─ Editor · 01-aria [hjson] ──────────────────────────┐
│  1  {                                               │
│  2    name:    "Aria"                                │
│  3    species: "fox"        // unquoted keys, //     │
│  4    role:    "scout"      // comments, no fuss     │
│  5    age:     19                                    │
│  6  }                                                │
└─────────────────────────────────────────────────────┘
```]

Data paragraphs are useful on their own — the Characters, Places, and Language
system books store their entries as HJSON — but they come into their own as the
*input to a template*. That is the next leaf.

#subsection("Jinja — the template paragraph")

A *Jinja template paragraph* is a paragraph whose `content_type` is `"jinja"`,
stored as `.jinja`, shown with the `⟡` glyph, and marked `[jinja]` in the Editor
header. It is a Jinja2-style template — rendered by `minijinja` — that the
assembler *compiles to Typst* before Typst compiles anything to PDF. The two
layers are strictly sequential, never nested:

#screen(caption: "Two layers, one direction — Jinja out, then Typst")[```
   your .jinja        assembly            typst
   paragraph   ───▶  (minijinja)  ───▶   .typ   ───▶  PDF
                     renders                    compiles
```]

Templates exist so that *structured* content stays in one place instead of being
copy-pasted and left to drift: a character sidebar that reads a linked dossier, an
endpoint table that loops over fields, a shared admonition written once and pulled
into a dozen chapters. The feature is *self-gating* — a project with no Jinja
paragraphs assembles exactly as before, and there is no flag to turn on.

*The render context.* Every template renders with a fixed set of variables: its
own `title` and `slug`; the enclosing `book.title` / `book.slug` / `book.genre`
and `chapter.title` / `chapter.slug`; the project `language` and `genre`; and,
most importantly, `linked[...]`.

#term("linked")[
  The data-injection mechanism. Link an *HJSON* paragraph to a template with the
  ordinary add-link chord `Ctrl+V a`; at assembly its parsed data is exposed
  under `linked["<that paragraph's slug>"]`. Only HJSON-bodied links land here —
  a linked prose paragraph is skipped, because its raw Typst is not meaningful
  template data. This is how a template reads facts instead of repeating them.
]

#screen(caption: "A template reading a linked HJSON dossier")[```
= {{ linked["01-aria"].name }}

#block[
  A {{ linked["01-aria"].species }} who
  serves as {{ linked["01-aria"].role }}.
]

{% if language == "ru" %}Глава{% else %}Chapter{% endif %}
```]

*Reusable fragments — the Snippets book.* A `.jinja` paragraph placed in the
built-in *Snippets* system book is registered as a named template before any
rendering, so manuscript templates can pull it in with
`{% include "snippets/<path>.jinja" %}`. The include name is the snippet's tree
path, lowercased — chapter and subchapter titles become path segments, the
paragraph slug is the filename — so you read the name straight off the tree.

*Assembly order.* At `Ctrl+B A` (or `inkhaven build`) the assembler first
registers every Snippets template, then renders the snippets to `.typ`, then
renders each manuscript template to a `.typ` in the generated tree, then runs
`typst compile`. Because registration precedes rendering, every `{% include %}`
resolves in one pass — no nesting, no two-pass surprises. By default a render
failure — bad syntax, a missing include, a typo'd variable — *aborts the whole
assembly* with the offending paragraph named, so a broken template can never
silently drop content from the PDF; set `jinja.continue_on_error: true` to write
a visible error block into that paragraph's place and keep going while you fix
templates one at a time.

#callout(label: "Two kinds of \"snippet\" — do not confuse them")[
  A Jinja template paragraph generates structured Typst *at assembly*. The
  edit-time text-expansion snippets (the `bund:` expansions that fire *while you
  type*) are a different system entirely. Both coexist; a Jinja paragraph is not
  prose and is skipped by the Inner Editor, Inner Socrates, and the idle
  fact-checker.
]

#subsection("Bund — the script paragraph")

A *Bund script* is a first-class leaf — a distinct node kind, not a paragraph
flavour — stored on disk as a `.bund` file and shown with the `λ` glyph. Its
natural home is the built-in *Scripts* system book, whose `.bund` files are
`eval`'d into Inkhaven's embedded Bund virtual machine when the project opens.
That is where user-authored hook lambdas live — `hook.on_save` and its kin — so a
script under Scripts can watch your saves, enforce a house rule, or extend the
tool. Bund is Inkhaven's embedded scripting language; Part VIII is its full
treatment. For the Tree's purposes what matters is that a script is a real node
you can add, move, and delete like any other, and that you *reach* the bund rung
by morphing a paragraph — the last stop on the type cycle described below.

#subsection("Images — the picture leaf")

Images are first-class nodes too — `NodeKind::Image`, shown with the `▣` glyph,
holding their own bytes on disk (`.png`, `.jpg` / `.jpeg`, `.webp`, `.svg`,
`.gif`). You add one by importing it: the `F3` file picker routes any image file
to the image-import path instead of treating it as prose, dropping a new Image
node into the tree near your cursor. An Image node carries an optional caption and
alt-text; at book assembly Inkhaven emits the right `wrap_image_*` calls and
ships the bytes into the generated Typst tree, so the picture lands in the PDF
with its caption and accessible alt-text intact.

Two conveniences ride on images. `Enter` on an Image row pops a *preview* — a
real in-terminal rendering through `ratatui-image`, using your terminal's best
protocol (kitty, sixel, iTerm2, or a half-block fallback). And from *inside* the
Editor, with the cursor in a Typst `#image("…")` call, `Ctrl+B P` opens a
sibling-image picker so you can fill the path from the images already in the tree
rather than typing it.

#screen(caption: "Enter on an image row — an in-terminal preview")[```
┌─ Image · harbour-map.png · 1240×860 · 214 KB ───────┐
│                                                     │
│        ▟▛▜▙  (rendered inline via kitty /           │
│        ▛  ▜   sixel / iterm2 / half-block)          │
│        ▙▟▛▟                                         │
│                                                     │
│  caption: "The harbour at Quaymouth"                │
│  Esc close                                          │
└─────────────────────────────────────────────────────┘
```]

#subsection("Structural paragraphs — the para:* family")

A technical or nonfiction chapter is not all prose: it has code listings, warning
boxes, display math, numbered procedures, tables. Left as ordinary paragraphs
these would all read as `¶`, draw prose-style craft notes that make no sense on a
code block, and inflate the word count with text that is not narrative.
*Structural paragraphs* fix this. A structural paragraph is an *ordinary `.typ`
paragraph carrying a `para:*` tag* — the tag, not a new content type, is the
whole mechanism.

#screen(caption: "The structural subtypes and their glyphs")[```
  para:code                ⌨   code listing
  para:admonition-note     ⚠   note box
  para:admonition-warning  ⚠   warning box
  para:admonition-tip      ⚠   tip box
  para:admonition-caution  ⚠   caution box
  para:math                ∫   display math
  para:procedure           ≡   numbered steps
  para:table               ⊞   table
```]

Marking a paragraph structural does three things: it swaps the tree glyph for the
subtype's icon, it *excuses the paragraph from the prose companions* (the Inner
Editor and Inner Socrates no longer fire on it — it is not prose), and it *removes
it from the prose word count*, counting it separately (Book Info, `Ctrl+B I`,
shows a `structural: N` line). One subtype breaks the pattern: `para:procedure`
is prose the author writes — real steps — so the companions *do* still run on it;
it is only excluded from the word count.

*Create one — `i` in the Tree.* Press `i` and a picker of the subtypes opens.
Choose one, `Enter`, name it, and you get a `.typ` paragraph tagged `para:*`,
its glyph set, and *matching Typst boilerplate seeded* — a `#figure` code block,
a coloured `#block` admonition, a `#table`, a `$ … $` math block, or a
`+`-stepped procedure — so you start from a working skeleton rather than a blank
buffer.

#screen(caption: "The i picker — pick a structural subtype")[```
┌─ Add structural paragraph · i ─────────────────┐
│   ⌨  code listing                              │
│   ⚠  admonition: note / warning / tip / caution│
│   ∫  math                                      │
│   ≡  procedure                                 │
│   ⊞  table                                     │
│   ↑↓ select · Enter create · Esc cancel        │
└────────────────────────────────────────────────┘
```]

#callout(label: "Tag, not type — the escape hatch")[
  Because a structural subtype is only a tag, you manage it like any tag. Add
  `para:code` to an existing prose paragraph with `Ctrl+B ]` to make it
  structural, or remove the tag to turn it back into prose — the body is
  untouched either way. There is no morph cycle to walk; the tag *is* the
  mechanism. (Poetry rides the same machinery: a stanza is a paragraph tagged
  `para:verse-*`, a sixth family with its own glyphs `‖ ♩ ‗ ⁚ ⁛ ⇄` and its own
  reader, the Inner Poet — the Poetry companion has the full treatment.)
]

#section("The morph cycle — turning one leaf into another")

The four *text* leaves — prose, HJSON, Jinja, and Bund — are close relatives on
disk, so Inkhaven lets you cycle a leaf from one to the next in place. `t` (or
`T`) *morphs the node type* through a fixed loop:

#screen(caption: "t / T cycles a leaf through four types")[```
   paragraph        paragraph        paragraph         script
   (typst .typ) ──▶ (hjson .hjson)──▶(jinja .jinja)──▶ (bund .bund)
        ▲                                                   │
        └───────────────────────────────────────────────────┘
```]

With no rows marked, `t` cycles just the cursor's leaf (branches are skipped);
with a set of paragraphs marked (`Space`), it cycles every marked leaf at once.
The morph *only flips the type and renames the file's extension* — it does not
seed a starter body. That is the practical difference from the two Tree
*pickers*, which create a *fresh, seeded* leaf:

- `i` creates a new *structural* paragraph from the subtype picker, with
  boilerplate seeded (covered above).
- `e` creates a new *Jinja* template — seeded with a documented starter that
  lists the variables available to you — under a user book, or a reusable
  `{% include %}` fragment under the Snippets book. It is rejected elsewhere
  (Notes, Characters, and the other system books do not take templates).

So the rule of thumb is simple: reach for `e` (or `i`) when you want a *new leaf
with a working skeleton*; reach for `t` when you have an *existing* leaf whose
type you want to change and you will write the body yourself. To land on the Bund
rung there is no seeded creator — you morph a paragraph around the cycle to
`script`, or add a `.bund` under the Scripts book and let the tree hold it.

#chord_table((
  chord_row("t / T", "Morph the leaf's type: typst → hjson → jinja → bund (marked set, or the cursor)."),
  chord_row("i", "Create a new structural paragraph (para:*) from the subtype picker, boilerplate seeded."),
  chord_row("e", "Create a new Jinja template (⟡), seeded — a manuscript template, or a Snippets fragment."),
  chord_row("Ctrl+B ]", "Add / remove a tag — including a para:* structural tag — on the open paragraph."),
  chord_row("g", "Tag the marked set (or the cursor row) from the Tree."),
))

#recap((
  [The *Tree* is where the book has a shape. A row reads left to right — depth,
  mark, *kind glyph*, status letter, title, then *pips*: a target gauge, tag
  chips, a finding badge, and a dim word count. The open paragraph always wears a
  green `►`.],
  [Navigate with the arrows; fold with `→` / `←` (single branch), `Z` (enclosing
  subchapter), and `X` (everything). The *cursor* and the *open paragraph* are
  two different things — moving the cursor opens nothing until you press `Enter`.],
  [Build with `B` / `C` / `A` / `+` to *append at end* and `V` / `S` / `P` to
  *insert after* the cursor's same-kind ancestor; each has a `Ctrl+B` twin. New
  books slot above the protected system block.],
  [Reshape with `U` / `J` (reorder siblings), `<` / `>` in the Outline (promote /
  demote a level), and `y` / `m` / `f` (copy / move / affix across parents). The
  files on disk follow every move.],
  [Delete kind-specifically — `-` a paragraph, `D` a branch — behind a confirm
  that *counts the words at risk*. Recover with the *kill-ring* (`Ctrl+B U`, or
  the `Ctrl+V Shift+U` picker) and, for branches, the *pre-delete snapshots* in
  `F6`.],
  [Six leaf kinds: prose `.typ` (`¶`), HJSON data `.hjson` (`❴`), Jinja templates
  `.jinja` (`⟡`, rendered to Typst at assembly, fed by `linked` HJSON), Bund
  scripts `.bund` (`λ`), images (`▣`, previewed with `Enter`), and the `para:*`
  *structural* subtypes (`⌨ ⚠ ∫ ≡ ⊞`) — the last a tag, not a type.],
  [Turn one text leaf into another with the morph key `t` (typst → hjson → jinja
  → bund); use the *pickers* `i` and `e` when you want a fresh, *seeded* leaf
  instead.],
))
