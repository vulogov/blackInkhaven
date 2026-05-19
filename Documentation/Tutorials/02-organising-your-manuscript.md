# 2 — Organising your manuscript

This tutorial covers everything you do in the **Tree pane**: building
the hierarchy, reordering, folding, renaming, and the six pre-seeded
system books.

We pick up from a project with a single book / chapter / paragraph
created in [`01-getting-started.md`](01-getting-started.md). If you
skipped that, run `inkhaven init ~/Books/sample-novel` first.

## The four-level hierarchy

```
Book → Chapter → Subchapter → Paragraph
```

By default, Inkhaven enforces exactly these four levels. **Paragraph**
is the leaf (a `.typ` file on disk); the other three are directories.

Paragraphs can attach to **any** branch — they don't need to sit at the
bottom. A book-level "Preface" paragraph is fine, as is a
chapter-level "Chapter intro" paragraph. The hierarchy is about
*containment*, not depth.

```
Sample Novel
├── Preface                                    ← paragraph directly under book
├── Chapter One
│   ├── Chapter intro                          ← paragraph directly under chapter
│   ├── Morning
│   │   ├── Opening scene                      ← paragraph under subchapter
│   │   └── Storm breaks
│   └── Afternoon
└── Chapter Two
```

If your project needs deeper structure (large legal documents, deeply
nested technical specifications), set in `inkhaven.hjson`:

```hjson
hierarchy: { unbounded_subchapters: true }
```

Subchapters then nest under subchapters indefinitely.

## Building the tree

The Tree pane has plain-letter shortcuts for adding hierarchy items.
They work without a modifier — focus the Tree pane and press the
letter.

### Append at end

These add an item at the **end of the parent's children**. The parent
is chosen by walking up from the cursor to the nearest legal container.

| Key  | What it adds |
| ---- | ------------ |
| `B`  | Book (always at the root level, **above** the system block) |
| `C`  | Chapter |
| `A`  | Subchapter |
| `+`  | Paragraph |

So pressing `+` with the cursor on a chapter row adds the new
paragraph at the bottom of that chapter's children.

### Insert after current

The same idea with a different rule: place the new item **immediately
after the cursor's same-kind ancestor**. Bumps later siblings by +1.

| Key  | What it inserts |
| ---- | --------------- |
| `V`  | Chapter after the current chapter |
| `S`  | Subchapter after the current subchapter |
| `P`  | Paragraph after the current paragraph |

This is what you want when restructuring — adding a paragraph between
two existing ones, splitting a chapter, etc.

### Both forms open the same Add modal

```
┌── Insert paragraph after current ───────────────┐
│  Parent: sample-novel/chapter-one               │
│      Where: after `Opening scene`               │
│  Title : Storm breaks│                          │
│                                                 │
│  Enter to confirm · Esc to cancel               │
└─────────────────────────────────────────────────┘
```

Empty title is fine — the paragraph gets `Untitled paragraph` as a
placeholder and Inkhaven derives a real title from the first sentence
on first save.

## The cheat-sheet card

Keep this one handy for the tree pane:

```
B  add book                  V  insert chapter after current
C  add chapter               S  insert subchapter after current
A  add subchapter            P  insert paragraph after current
+  add paragraph

D  delete branch (book/ch/subch)
-  delete paragraph

U  move current node up among siblings
J  move current node down among siblings

Z  collapse cursor's enclosing subchapter
X  collapse every expanded branch

→  expand cursor's branch
←  collapse cursor's branch (or step to parent if already collapsed)

F2  rename (changes display title)
F3  file picker (load file / import directory)
Enter  open paragraph in editor
```

All of these also have meta-prefix equivalents (`Ctrl+B` then `B`,
`Ctrl+B` then `C`, …) for terminals that eat plain-letter inputs in
some configurations.

## Reorder: U / J

Place the cursor on a paragraph (or branch). Press `U` to move it up
one slot; `J` to move it down. Inkhaven renames the corresponding
`.typ` file or directory (swapping the `NN-` order prefix) and bumps
the in-database `order` integer.

The reorder also re-sorts the on-disk listing — `ls books/...` reflects
the new order immediately, so a separate `git diff` tells you exactly
what changed.

## Delete: D / -

Two keys for safety:

- `D` deletes a **branch** (book, chapter, subchapter — anything with
  potential descendants). If the cursor is on a paragraph, `D` shows
  a hint to use `-` instead.
- `-` deletes a **paragraph** specifically. If the cursor is on a
  branch, the hint suggests `D`.

Both open a confirm modal:

```
┌── Delete subchapter ────────────────────────────┐
│  Delete subchapter `Morning` and 4 descendant(s)?│
│                                                  │
│  y / Y to confirm · n / N or Esc to cancel       │
└──────────────────────────────────────────────────┘
```

`y` proceeds; anything else aborts. The on-disk directory tree is
removed and the bdslib records are dropped.

**System books cannot be deleted.** Trying to delete one returns a
status like `'Notes' is a system book — it can't be deleted or
renamed`.

## Rename: F2

Press `F2` with the cursor on any node. The Rename modal pre-fills
with the current title:

```
┌── Rename chapter ───────────────────────────────┐
│   Chapter One│                                  │
│                                                 │
│   Enter to confirm · Esc to cancel              │
└─────────────────────────────────────────────────┘
```

Type a new title and Enter. The slug and the on-disk filename **do
not change** — only the displayed title and the search index. This
means renaming is safe for cross-references in your Typst markup
(`#link("…/chapter-one")[]`).

Renaming a paragraph from the editor side: `Ctrl+B T` (in the editor
pane) re-derives the title from the current first sentence — useful
after rewriting an opening line.

System books cannot be renamed.

## Folding: ← / → / Z / X

Long manuscripts need folding. Inkhaven has four folding chords:

| Key  | Effect |
| ---- | ------ |
| `→`  | Expand the cursor's branch. |
| `←`  | Collapse the cursor's branch. If already collapsed, step the cursor to the parent. |
| `Z`  | Collapse the cursor's **enclosing subchapter**. Cursor jumps to the folded subchapter row so you see what disappeared. |
| `X`  | Collapse **every** expanded branch in the entire tree. Useful when you want to start fresh after a deep navigation. |

After `X`, only the books are visible at the root level. From there,
`→` expands one at a time.

## The six system books

Every project ships with these, in this order:

```
├─ Notes        (editorial notes, TODOs)
├─ Research     (background research)
├─ Prompts      (project-local AI prompts; see PROMPTS.md)
├─ Places       (locations — yellow overlay; see LOCATIONS.md)
├─ Characters   (people — cyan overlay; see CHARACTERS.md)
└─ Help         (Inkhaven's own help manual; F1)
```

User-added books are inserted **above** Notes — your own writing
always sits at the top. The system block always sits at the bottom of
the root level. None of the six can be deleted or renamed.

### What each system book is for

- **Notes** — like a writer's notebook. Plot ideas, TODOs, "fix this
  before publication", reminders. Plain editable.
- **Research** — sources, references, background. Inkhaven indexes
  this just like prose, so semantic search finds it alongside your
  manuscript.
- **Prompts** — project-local AI prompt templates. Surface via `/` in
  the AI prompt picker. See [`../PROMPTS.md`](../PROMPTS.md).
- **Places** — populated with locations. Editor highlights mentions
  of these names in cyan. `Ctrl+B P` asks the AI about a selected
  place using book content as RAG. See
  [`../LOCATIONS.md`](../LOCATIONS.md).
- **Characters** — same idea, but for people. Yellow overlay.
  `Ctrl+B C` ask-the-AI chord. See
  [`../CHARACTERS.md`](../CHARACTERS.md).
- **Help** — Inkhaven's own help manual. Empty by default; populate
  via `inkhaven import-help --documents-directory <dir>`. Press F1 in
  the TUI to ask grounded questions over it.

## Importing existing files (F3)

If you already have prose lying around as `.md` / `.txt` / `.typ`
files, F3 in the Tree pane opens a file picker. You can:

- Press `Enter` on a **file** to import it as a new paragraph (inserted
  after the cursor's current node).
- Press `Enter` on a **directory** to recursively import the whole
  tree — subdirectories become chapters / subchapters and files become
  paragraphs. Filenames and directory names supply the titles.

If the source tree is deeper than four levels, files beyond the depth
limit are flattened into the deepest legal branch. With
`unbounded_subchapters: true`, depth is unlimited.

For populating the Help book specifically, use the
`inkhaven import-help` CLI — see
[`08-importing-existing-docs.md`](08-importing-existing-docs.md).

## What you have learned

- The hierarchy is Book → Chapter → Subchapter → Paragraph, with
  paragraphs attachable at any level.
- Plain-letter chords build the tree (B / C / A / + and V / S / P).
- `D` / `-` delete with confirmation; `U` / `J` reorder; `F2` renames.
- `Z` and `X` fold; `←` / `→` expand and collapse.
- System books are pre-seeded, protected, and serve specific features.
- User books sit above the system block at root level.

## Next steps

- [`03-the-editor.md`](03-the-editor.md) — the editor pane in depth.
- [`07-places-and-characters.md`](07-places-and-characters.md) — using
  the Places and Characters books for worldbuilding.
- [`08-importing-existing-docs.md`](08-importing-existing-docs.md) —
  pulling an existing folder of prose into Inkhaven.
