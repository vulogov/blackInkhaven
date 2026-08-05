# Know Your Book

*The Intelligence That Reads Along With You* — the companion guide to Inkhaven's
knowledge features.

A book is a *system too large for one head*. This guide covers every intelligence
Inkhaven has for holding it: the facts beneath it, the knowledge graph over it, the
continuity that watches it, who knows what within it (KEN, the 2.6 flagship), how it
reads (LECTOR), how its voices sound (CHORUS), and whether it is getting better draft
to draft (CHRONICLE). For fiction and non-fiction authors alike, assuming no prior
knowledge.

Written in the voice of the *Research* companion, and teaching with monospace
terminal `screen()` mockups rather than screenshots — the app *is* a terminal, so a
faithful frame is truer than a picture and keeps the book self-contained.

## Building

```sh
typst compile Book/KNOW_YOUR_BOOK/KNOW_YOUR_BOOK.typ
```

Output: `KNOW_YOUR_BOOK.pdf`. Each chapter is its own file in `chapters/`.

## Contents

- **Before You Begin** — what it means to *know* your own book.
- **I · The Ground Truth** — the Facts bible; the knowledge graph (`graph ask`).
- **II · The Book Watches Itself** — continuity (SENTINEL); who knows what, when (KEN).
- **III · The Book Reads Itself** — the read-through (LECTOR); the voices (CHORUS).
- **IV · Knowing You're Getting Somewhere** — did it get better? (CHRONICLE).
- **V · All Together** — one scene through every check.
- **Appendix A** — the knowledge commands, chords, and tags.
- **Appendix B** — glossary.

The companion books excluded from the crate ship in the repository and are
re-rendered on every release tag.
