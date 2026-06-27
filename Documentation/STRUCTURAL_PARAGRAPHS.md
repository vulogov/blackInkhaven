# Structural Paragraphs & Deletion Hardening (STRUCT-2)

Two features for technical and nonfiction authors: **structural paragraph
subtypes** (Part A) and **deletion hardening** (Part B). Both are purely
additive — no new `content_type`, no store changes.

---

## Part A — Structural paragraph subtypes

Technical chapters mix prose with **code listings, admonition boxes, math,
procedures, and tables**. By default all of these read as `¶` in the tree, draw
Inner Editor prose-style observations, and inflate the word count. Structural
subtypes fix that.

A structural paragraph is an ordinary `.typ` paragraph carrying a **`para:*`
tag**. The tag — not a content type — is what marks it. Seven subtypes:

| Tag | Subtype | Glyph | Companions | Word count |
|---|---|---|---|---|
| `para:code` | code listing | `⌨` | skip | excluded |
| `para:admonition-note` | note box | `⚠` | skip | excluded |
| `para:admonition-warning` | warning box | `⚠` | skip | excluded |
| `para:admonition-tip` | tip box | `⚠` | skip | excluded |
| `para:admonition-caution` | caution box | `⚠` | skip | excluded |
| `para:math` | display math | `∫` | skip | excluded |
| `para:procedure` | numbered steps | `≡` | **run** | excluded |
| `para:table` | table | `⊞` | skip | excluded |

`para:procedure` is the exception: steps are prose the author writes, so the
Inner Editor and Inner Socrates still run on them. They're excluded from the
prose word count (step text isn't narrative), but otherwise treated as prose.

### Create one — `i` in the Tree pane

Press **`i`** in the Tree pane. A picker opens:

```
┌─ Add structural paragraph · i ────────────────┐
│   ⌨  code listing                             │
│   ⚠  admonition: note                         │
│   ⚠  admonition: warning                      │
│   ⚠  admonition: tip                          │
│   ⚠  admonition: caution                      │
│   ∫  math                                     │
│   ≡  procedure                                │
│   ⊞  table                                    │
│   ↑↓ select · Enter create · Esc cancel       │
└───────────────────────────────────────────────┘
```

Pick a type, `Enter`, type a name. You get a `.typ` paragraph tagged `para:*`,
its tree glyph set, and the matching **Typst boilerplate seeded** — e.g. a
`#figure` code block, a coloured `#block` admonition, a `#table`, a `$ … $` math
block, or a `+`-step procedure.

### What changes

- **Tree glyph** — the subtype's icon (`⌨ ⚠ ∫ ≡ ⊞`) instead of `¶`.
- **Prose companions skip it** — the Inner Editor and Inner Socrates don't fire
  on code / math / table / admonitions (it isn't prose). Procedures still run.
- **Word count** — structural paragraphs are excluded from the prose word /
  sentence / paragraph totals and counted separately. **Book Info** (`Ctrl+B I`)
  shows a `structural: N` line under Structure when N > 0.

### Tag, not content type — the escape hatch

Because the subtype is just a tag, you manage it like any tag with **`Ctrl+B ]`**:
add `para:code` to an existing paragraph to make it structural, or remove it to
turn it back into prose — the body stays either way. There's no morph cycle to
walk; the tag is the whole mechanism.

> A paragraph can be both `content_type: "jinja"` *and* `para:code` (a Jinja
> template that generates a code listing). The jinja check fires first in the
> companion gates, so the structural check is moot there.

---

## Part B — Deletion hardening

Deleting a chapter or book used to be a leap of faith: no word count in the
confirmation, no recovery for branch deletes, no snapshot. Three fixes.

### 1. Word count in the confirmation

The delete prompt now shows how many words you're about to lose:

```
Delete chapter `Act II` and 12 descendants (15,342 words)?
Delete paragraph `The lighthouse scene` (342 words)?
```

Zero-word deletes (HJSON / jinja / empty) omit the count.

### 2. Branch kill-ring

Deleting a chapter/book now stashes **every paragraph leaf** into the kill-ring,
not just single-paragraph deletes. Press **`Ctrl+B U`** to restore them one at a
time (they come back in original order), or **`Ctrl+V Shift+U`** for the picker.
The restore status shows the word count of what came back. Restored paragraphs
get a fresh UUID, so cross-references from other paragraphs to the old UUID stay
broken (flagged in the status).

The kill-ring has a cap (`editor.deleted_paragraph_history`) — a very large
branch can overflow it, which is what the next feature is for.

### 3. Pre-delete snapshots

Before a **branch** delete, inkhaven takes an annotated snapshot of every
paragraph leaf — `pre-delete: <title> · <date>` — so you can find and restore
any of them from the **`F6`** snapshot picker long after the kill-ring has
cycled. The snapshots are taken before the delete fires, so they're safe even if
the delete partially fails. This is the durable recovery for large branch
deletes; single-paragraph deletes don't snapshot (it would pollute the list).

So a branch delete leaves three recovery layers: the confirmation tells you what
you'll lose, the kill-ring gives immediate undo, and the F6 snapshots are the
long-term safety net.

---

**See also:** [Tutorial 94 — Structural paragraphs](Tutorials/94-structural-paragraphs.md) ·
[KEYBINDING.md → `i` (Tree)](KEYBINDING.md) ·
[Jinja templates (STRUCT-1)](JINJA_TEMPLATES.md).
