# 3 — The project tree

The tree pane is the manuscript's spine. Every piece of prose, every script, every note hangs from it. Understanding the kinds of nodes, the parent-child rules, and how to reorder them is the first move toward feeling comfortable in inkhaven.

## Node kinds

| Kind         | Role |
|--------------|------|
| Book         | Top-level container. A project can have multiple. System books (Notes, Help, …) sit alongside user books. |
| Chapter      | First-level container inside a book. |
| Subchapter   | Optional second-level container inside a chapter. |
| Paragraph    | A leaf. Carries the prose. Has metadata: status, tags, word target, links, snapshots. |
| Script       | Bund (.bund) leaf. Stores executable code instead of prose. See Chapter 29. |
| Image        | PNG / JPG leaf. Referenced by paragraphs through `#image()` typst calls. |

The hierarchy rule: Book > Chapter > Subchapter > leaves. Books can hold paragraphs directly (no chapter), chapters can hold paragraphs directly (no subchapter), but you can't nest two chapters or put a book inside another book.

## Tree-pane chords

With focus on the Tree pane (`Ctrl+2`):

| Chord | What it does |
|-------|--------------|
| `↑` / `↓` / `PgUp` / `PgDn` | Navigate. |
| `←` / `→` | Collapse / expand a branch. |
| `Enter` | Open the paragraph (or focus an image / script). |
| `F2` | Rename the cursor node. |
| `B` / `C` / `A` / `+` | Add book / chapter / subchapter / paragraph as a child of the cursor. |
| `V` / `S` / `P` | Insert chapter / subchapter / paragraph as a sibling AFTER the cursor. |
| `D` | Delete the cursor's branch (confirm prompt). |
| `-` | Delete the cursor paragraph (no children — quick path). |
| `U` / `J` | Reorder up / down among siblings. |
| `Z` / `X` | Collapse cursor's subchapter / collapse every branch. |
| `Space` | Multi-select toggle (see Chapter 14 for the picker workflows that use marks). |

## Anatomy of a paragraph row

![figure: tree-paragraph-row](images/tree-paragraph-row.png) — A paragraph row carries five things at a glance: indent (depth), kind glyph (¶), status letter (N/F/R/…), title (truncated), and tag pips (#draft, #weather).

The pieces from left to right:

| Element | Role |
|---------|------|
| Indent | Two spaces per depth level. |
| Glyph | ¶ for paragraph, λ for script, ▣ for image, ▾/▸ for expanded/collapsed containers, ► for the open paragraph. |
| Status letter | N (Napkin), F (First), … R (Ready). Space reserved when status = None. See Chapter 9. |
| Title | Truncated past 36 chars. |
| Progress pip | Tiny circle (○ ◔ ◑ ◕ ●) when the paragraph has a word target. See Chapter 9. |
| Tag pips | Up to two #tag chips + a +N count. See Chapter 14. |

## Reordering

`U` (up) and `J` (down) move the cursor node among its siblings. The shift is one position per press; long moves take repetition. The change is immediate — there's no "save the tree" step.

When you rename a node with `F2`, both its title AND its slug update. The slug is what shows up in URLs and file paths; it's auto-generated from the title (kebab-case) but you can edit it independently in the metadata.

## Multi-select

`Space` toggles whether the cursor node is "marked". Marked nodes get a `✓` glyph; status badges and reorder actions apply to all of them at once. This is covered in detail in Chapter 14 alongside the tag workflows that take advantage of it.

## Folding for focus

A 200-paragraph manuscript is overwhelming in a flat tree. `X` collapses every expanded branch; `Z` collapses just the cursor's enclosing subchapter; `←` collapses the cursor's direct branch. The opposite (expand) is `→`.

> **Tip:** Combine `X` with the fuzzy paragraph picker (`Ctrl+V P`, Chapter 10): collapse everything, then jump to whatever you actually want to work on. Reduces visual noise to about one row per chapter title.

## Recap

- Tree kinds: Book → Chapter → Subchapter → Paragraph / Script / Image.
- Tree-pane chords mirror file-manager intuitions: arrows, Enter, F2, D.
- `U`/`J` reorder siblings; `B`/`C`/`A`/`+` create children; `V`/`S`/`P` create siblings.
- Multi-select with `Space` powers project-wide picker workflows.
- `X` collapses everything — combine with the fuzzy picker for focused work.
