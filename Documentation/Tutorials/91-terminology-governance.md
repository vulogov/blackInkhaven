# Tutorial 91 — Terminology Governance

*Inkhaven 1.4.8*

Across three hundred pages, the same concept drifts: "access token" in chapter
one becomes "auth token" in chapter seven and "authentication token" in the
appendix. Each is clear on its own; together they make a reader wonder whether
they're the same thing. For technical writers and large-document nonfiction
authors, this terminology drift is the quiet trust-killer.

TERMS-1 gives you a **Glossary**: canonical terms with their banned synonyms.
Inkhaven red-underlines the synonyms as you write, scans the whole project on
demand, and — because you sometimes *do* mean the variant — lets you declare an
exception. It's self-gating: with an empty Glossary, nothing changes.

## Define a term

The Glossary is a system book (it sits among Notes, Sources, Facts, … in the
tree). Put the cursor on it and press **`P`** — the title you type becomes the
canonical `term`, and the body is pre-seeded:

```hjson
{
  term: access token
  definition: A short-lived credential the client sends with each request.
  synonyms: [
    auth token
    authentication token
  ]
  // scope: global   // or a book slug to enforce only in that book
  // note: chosen for consistency with the API reference
}
```

Only `term` is required. `synonyms` are the forms you want to stamp out (1–3
words each). Set `scope` to a book slug to enforce a term in only one book; leave
it `global` (the default) to enforce everywhere.

## See it as you write

In the editor, banned synonyms are **red-underlined** the moment you type them —
"auth token" lights up; the canonical "access token" stays clean. Put the cursor
on a flag and the footer tells you the fix:

```
terms: "auth token" → use "access token"
```

**`Ctrl+V z`** toggles the overlay off and on (it's on by default, within the
master style toggle `Ctrl+B Shift+F`). The colour is the theme key
`style_warning_banned_synonym_fg` (default red).

## Sometimes you mean it

Not every variant is a mistake — maybe "frontend" is your house style and you
don't want it flagged. With the cursor on the underline, press **`Ctrl+V
Shift+Z`** to declare that term a **deliberate variant**. It records an intent in
the shared ledger, and the overlay (and the scan) stop flagging it. The same
declaration is available to scripts via `ink.terms.declare_intent`.

## Scan the whole project

The overlay only shows the open paragraph. To catch every occurrence:

```sh
$ inkhaven terms check
terms check: 2 banned-synonym occurrence(s) in 1 paragraph(s):
  guide/03-tokens/01-intro line 2: "auth token" → use "access token"
  guide/03-tokens/01-intro line 5: "auth token" → use "access token"
```

It **exits non-zero** when it finds anything, so it drops straight into a
pre-build or CI step. `--book <slug>` scopes it; `--json` emits a machine-readable
report. (Use `inkhaven list` to copy a book's slug.)

## Let the model find the drift

Don't have a glossary yet? **`inkhaven terms suggest --book <slug>`** clusters
words that appear in multiple surface forms and asks the model to propose
Glossary entries for the genuine terminology drift (skipping mere plurals and
stylistic variation). Review the proposals, or add `--auto-create` to drop them
straight into the Glossary book as drafts to edit.

## From a script

```
ink.terms.list                ( -- list )   every entry { term, definition, synonyms, … }
ink.terms.get                 ( term -- dict | NODATA )
ink.terms.check               ( book_slug -- list )   findings { path, line, synonym, canonical }
ink.terms.declare_intent      ( canonical scope -- )  declare a deliberate variant (store_write)
```

`list` / `get` / `check` are read-only; `declare_intent` writes the ledger and
needs the `store_write` category enabled.

---

**See also:** [CONFIGURATION.md → Terminology governance](../CONFIGURATION.md) ·
[KEYBINDING.md → `Ctrl+V z`](../KEYBINDING.md) · `inkhaven terms --help`. Pair it
with [Tutorial 89 — Bibliography & Citations](89-bibliography-and-citations.md)
for nonfiction projects.
