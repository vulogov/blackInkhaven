# Tutorial 94 — Structural Paragraphs

*Inkhaven 1.4.11*

If you write technical or nonfiction books, your chapters aren't pure prose —
they're prose interleaved with **code listings, warning boxes, equations,
procedures, and tables**. Inkhaven used to treat all of those as ordinary
paragraphs: same `¶` glyph, same Inner Editor style notes, all counted toward
your word total. STRUCT-2 lets you mark them for what they are.

It also hardens deletion, so removing a chapter is no longer a leap of faith.

## Mark a paragraph as structural

Press **`i`** in the Tree pane. A small picker opens:

```
⌨  code listing
⚠  admonition: note / warning / tip / caution
∫  math
≡  procedure
⊞  table
```

Pick **code listing**, press Enter, name it. You get a new paragraph with a
**`⌨`** glyph in the tree and its body pre-seeded with Typst:

```typst
#figure(
  caption: [Caption here.],
)[
```rust
// your code here
```
]
```

Fill in the code, save, assemble — it renders as a proper figure. The same goes
for admonitions (a coloured `#block`), math (`$ … $`), procedures (`+` steps),
and tables (`#table`).

## What being structural means

That `⌨` paragraph behaves differently from prose:

- **The prose companions leave it alone.** The Inner Editor and Inner Socrates
  don't fire on it — style and Socratic questions don't apply to a code block.
  (Procedures are the exception: steps *are* prose, so they still get feedback.)
- **It's out of your word count.** Open **Book Info** (`Ctrl+B I`) and you'll see
  prose `paragraphs` and `structural` counted separately — your word total
  reflects the writing, not the code and tables.

## It's just a tag

A structural paragraph is a normal `.typ` file; the **`para:code`** *tag* is what
marks it. So you can change your mind freely with the tag picker
(**`Ctrl+B ]`**): add `para:table` to an existing paragraph to make it
structural, or remove the tag to turn it back into prose. The body never moves.

## Deleting is safer now

While you're here, deletion got three upgrades (press **`d`** on a node to see):

1. **The confirmation shows the word count** — `Delete chapter `Act II` and 12
   descendants (15,342 words)?` — so you know what's at stake.
2. **Branch deletes are recoverable.** Deleting a chapter stashes every paragraph
   into the kill-ring; **`Ctrl+B U`** brings them back one at a time (in order),
   and the status tells you the word count of each.
3. **Pre-delete snapshots.** Before a chapter/book delete, inkhaven snapshots
   every paragraph (annotated `pre-delete: …`), so even after the kill-ring
   cycles you can recover any of them from the **`F6`** snapshot picker.

Three layers: you see what you'll lose, you get instant undo, and you have a
durable safety net.

---

**See also:** [STRUCTURAL_PARAGRAPHS.md](../STRUCTURAL_PARAGRAPHS.md) ·
[KEYBINDING.md → `i` (Tree)](../KEYBINDING.md) ·
[Jinja templates (Tutorial 93)](93-jinja-templates.md).
