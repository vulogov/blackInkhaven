# Book corpus

This directory holds the long-form books written *about* and *with*
inkhaven. Each book lives in its own versioned subdirectory so the
corpus can grow without the volumes colliding — drop a new
`<version>_<NAME>/` folder beside the ones below and add a row here.

## Books

| Book | Directory | Format | Length | Notes |
|------|-----------|--------|--------|-------|
| **The Book of Inkhaven** | [`1.2.6_MANUAL/`](1.2.6_MANUAL/) | Typst → PDF (+ Markdown mirror) | 34 chapters | The companion manual: walks every feature in the order a working author meets them, from install through scripting. The `markdown/` mirror feeds `inkhaven import-help` so the F1 RAG help covers the book. |

## Conventions

- **One book per subdirectory**, named `<inkhaven-version>_<SHORTNAME>/`
  — the version records which release the book was written against,
  not a hard dependency.
- Each book carries its own `README.md` with build instructions and a
  chapter index.
- Source is Typst (`*.typ`); the bound `*.pdf` is committed alongside
  so readers don't need a typst toolchain. A `markdown/` mirror is
  optional and exists where the book should also be searchable via the
  in-app help RAG.

## Building

Each book builds independently — see its own README. For the manual:

```
typst compile Book/1.2.6_MANUAL/BOOK_OF_INKHAVEN.typ
```
