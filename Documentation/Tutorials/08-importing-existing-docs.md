# 8 — Importing existing documents

You probably have prose lying around outside Inkhaven — old `.md`
drafts, `.txt` notes, exported `.typ` chapters. Three paths bring
them in:

1. **F3 in the Editor** — load one file into the current buffer.
2. **F3 in the Tree** — load a file as a **new paragraph** in the
   tree, or import a whole **directory tree** as nested chapters.
3. **`inkhaven import-help`** — bulk-ingest a directory into the
   **Help** system book (wipes Help first).

We will cover all three.

## Path 1: load one file into the editor

You want to drop a paragraph of draft text into the current paragraph
without making a new hierarchy entry.

1. Make sure the editor pane has focus and the paragraph you want to
   replace is open.
2. Press **`F3`**.

A file-picker overlay opens rooted at your home directory:

```
┌── Load file into editor ───────────────────────────────────┐
│ ▸ 📁 Books                                                 │
│ ▾ 📁 Drafts                                                │
│   📄 chapter-three.md                                       │
│   📄 chapter-three.typ                                      │
│   📄 outline.txt                                            │
│ ▸ 📁 Documents                                              │
│ ▸ 📁 git                                                    │
│                                                             │
│  Enter on file = load · Enter on dir = expand · Esc cancel │
└─────────────────────────────────────────────────────────────┘
```

Navigate with arrows; `→` expands a directory; `←` collapses or
steps to parent. Press **Enter on a file** to replace the editor
buffer with its contents. The buffer is now dirty — review, then
`Ctrl+S` to commit.

The original file on disk is **not** moved or deleted. The Inkhaven
paragraph just took a copy of its content.

## Path 2: import a file or directory into the tree

You want the file (or files) to become **new paragraphs** in the
hierarchy.

1. Focus the **Tree pane**.
2. Move the cursor to a target node (book, chapter, or subchapter)
   — the import will land **after** that node, under its parent.
3. Press **`F3`**.

The same file-picker overlay opens (but in `TreeInsertOrImport`
context — Inkhaven knows you're not just loading into the editor).

### Importing a single file

Navigate to a `.md`, `.txt`, or `.typ` file. Press **Enter on the
file**. A new paragraph is created with:

- **Title** derived from the filename stem (e.g.
  `chapter-three.md` → `Chapter Three`).
- **Body** copied verbatim from the file.
- **Position** immediately after the cursor's node.

You can repeat for each file you want to ingest. Or:

### Importing a directory tree

Navigate to a directory and press **Enter on the directory**.
Inkhaven walks the tree recursively:

- Subdirectories become **chapters / subchapters** (depth follows
  the tree, mapped to your hierarchy).
- Files become **paragraphs**.
- Filenames / dirnames supply the displayed titles.

So a source tree like:

```
~/Drafts/
└── getting-started/
    ├── overview.txt
    └── installation/
        ├── linux.md
        ├── macos.md
        └── windows.md
```

…imports as:

```
├─ Getting Started               (chapter)
│  ├─ Installation               (subchapter)
│  │  ├─ ¶ Linux
│  │  ├─ ¶ MacOS
│  │  └─ ¶ Windows
│  └─ ¶ Overview
```

The hierarchy mapping respects the four-level limit: if your source
tree is deeper than `Book → Chapter → Subchapter → Paragraph`,
files beyond the depth limit are **flattened** into the deepest
legal branch. With `hierarchy.unbounded_subchapters: true`,
Subchapter nests indefinitely and the entire tree is preserved.

Hidden files (dotfiles) are skipped. The original directory tree on
disk is **not modified**.

## Path 3: `inkhaven import-help`

This is the CLI equivalent of Path 2 with one important difference:
it always targets the **Help** system book and **wipes the existing
Help contents first**.

Use it when you want a clean re-ingest. Example: you ship docs as a
git-tracked directory of `.md` files; on every release you sync them
into the project's Help book with one command.

```bash
$ inkhaven --project ~/Books/sample-novel \
    import-help --documents-directory ~/Docs/inkhaven-help
```

Output:

```
cleared 7 existing item(s) from Help
imported 3 branch(es) and 8 paragraph(s) into Help from /home/you/Docs/inkhaven-help
```

The Help book now mirrors the source directory:

```
├─ Help
│  ├─ Getting Started
│  │  ├─ ¶ Installation
│  │  └─ ¶ First steps
│  ├─ Advanced
│  │  └─ ¶ Split-edit
│  └─ ¶ Overview
```

Two effects:

1. **F1 in the TUI** now queries this content. Press F1, type a
   question, the model answers grounded in the imported docs.
2. The Help book is **read-only** in the editor (border turns teal,
   mutating keys are intercepted). You re-run `import-help` to update
   it; you don't edit Help paragraphs in place.

### What the Help book is for

A Help book per project lets you:

- Bundle internal documentation with the manuscript (style guide,
  worldbuilding bible, glossary).
- Make that documentation **queryable through F1** so anyone working
  in the project can ask "how do we spell character names?" and get a
  grounded answer.
- Re-sync whenever the underlying docs change.

`inkhaven import-help` is designed for repeatable, scriptable updates.
For ad-hoc imports into other books, use the Tree pane's F3.

## Tips on titles and filenames

The title-derivation logic for imports is:

- Filename stem → prettified by replacing `_` and `-` with spaces,
  then title-casing each word.
- So `setup-instructions.md` → `Setup Instructions`.
- Directory names → same prettification.

If you want different titles, either:

- Rename the source files / directories before import.
- Or use F2 (rename) on each paragraph after import to set a custom
  display title (the slug stays unchanged).

The body of each imported file is preserved verbatim. If the source
is markdown, you can convert to Typst later by editing the
paragraph; Inkhaven doesn't auto-transform `# Heading` to `= Heading`
on import. (The AI pane's `r` apply uses markdown-to-Typst when
applying AI output, but that's a separate path.)

## Re-indexing after a manual import

If you `cp` files into the `books/` directory **without** going
through F3 / `import-help`, the database doesn't know about them.
Run:

```bash
$ inkhaven --project ~/Books/sample-novel reindex --adopt
```

The `--adopt` flag scans for orphan `.typ` files and registers each
under the deepest matching branch (based on its parent directory's
slug path). New paragraphs get fresh UUIDs.

See [`../MAINTENANCE.md`](../MAINTENANCE.md) for `reindex` details
and other recovery flows.

## Use cases

### Migrating an existing project

You have ~30 chapter `.md` files in a folder. New Inkhaven project:

```bash
$ inkhaven init ~/Books/new-project
$ inkhaven --project ~/Books/new-project add book "My Book"
$ inkhaven --project ~/Books/new-project
# Inside the TUI, with the cursor on "My Book":
# F3 → navigate to your folder → Enter on the directory
```

The folder lands as a chapter with each file as a paragraph under
it. Reorder / rename as needed.

### Bundling style guides

```bash
# In a CI script that runs on every manuscript-style-guide commit:
$ inkhaven --project /path/to/manuscript \
    import-help --documents-directory ./style-guide
```

The Help book is now refreshed. Writers open the manuscript, press
F1, ask "what's our convention for em-dashes?" and get a grounded
answer.

### Pulling in research notes

You keep a research folder under `~/research/topic/`. Drop it into
the manuscript:

```
# In the TUI, on the Research book row:
F3 → navigate to ~/research/topic → Enter on directory
```

The whole tree appears under Research. Semantic search now finds
research alongside prose.

## What you have learned

- **F3 in the Editor** loads one file into the current buffer
  (replaces it).
- **F3 in the Tree** imports a file as a new paragraph, or a whole
  directory as a chapter / subchapter tree.
- **`inkhaven import-help --documents-directory <dir>`** wipes the
  Help book and re-imports — designed for repeatable doc syncs.
- Hierarchy depth maps the source tree to the Inkhaven hierarchy;
  excess depth flattens into the deepest legal branch (or nests
  forever with `unbounded_subchapters: true`).
- Titles are derived from filenames; rename after import if you
  want different display names.
- For files you copied in outside Inkhaven, `inkhaven reindex
  --adopt` registers them retroactively.

## Next steps

- [`09-exporting-to-typst-and-pdf.md`](09-exporting-to-typst-and-pdf.md)
  — going the other direction: turning your Inkhaven project into a
  single Typst file or PDF.
- [`../MAINTENANCE.md`](../MAINTENANCE.md) — reindex / backup /
  restore reference.
