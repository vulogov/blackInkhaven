# Tutorial 66 — From draft to submission: Word, package, tracker

*Inkhaven 1.3.0+ (`.docx`), 1.3.1+ (package + tracker)*

You have a finished manuscript. 1.3.0 took it to *print* (Tutorial 65).
This is the other path: getting it in front of an **agent or editor**.
Three pieces — the format they require, the package around the manuscript,
and a record of where it went — all over the same pure-Rust core, no
external apps.

## 1. The Word document agents actually want

`inkhaven manuscript` (1.2.19) emits a Shunn-format *typst* document. But
most agents require **Word**. So:

```sh
inkhaven docx --book-name "My Novel" \
    --author "Jane Writer" \
    --contact "Jane Writer\n12 Wharf Rd\njane@example.com" \
    --font times                       # or: courier
    # → my-novel-manuscript.docx
```

Same Shunn layout as `manuscript` — title page (contact corner + rounded
word count + centred title/byline), double-spaced 12 pt, 1″ margins, ½″
first-line indent, a `Surname / KEYWORD / page` running header from page 2,
scene breaks as a centred `#`, each chapter on a fresh page. It is a
hand-rolled `.docx` (OOXML in a zip) — no `docx` dependency, no Word
needed to produce it.

As a **book-take**: add `docx` to `output.extra_formats` and `Ctrl+B O`
writes `<stem>.docx` beside the PDF on every build.

## 2. The submission package (AI)

A novel doesn't fit a prompt, so the package generators work against a
compact **digest** — title / author / length / a one-line summary of each
chapter + your Characters and Threads books. Build it once:

```sh
inkhaven submission digest            # caches .inkhaven/digest-<slug>.json
```

It rebuilds automatically when the manuscript's structure changes (or with
`--refresh`). Then generate the pieces:

```sh
inkhaven submission query             # a one-page query letter
inkhaven submission synopsis          # one-page synopsis (full arc)
inkhaven submission synopsis --long   # 2–3 page synopsis
inkhaven submission comps             # comparable-title suggestions
inkhaven submission logline           # logline + elevator pitch
```

Each drafts into the **Submissions** system book (overwriting the same
piece rather than piling up) and prints it. A synopsis spoils the ending
by design; comp titles are *suggestions* from the model's general
knowledge — labelled as such, never citing sales figures — so curate them.

The drafts are ordinary prose paragraphs: edit them, snapshot them, run
them through F7 grammar-check like anything else.

### From the editor

Press **`Ctrl+V q`** (Q for *query*) to pick a generator and stream it
into the AI pane — read it, iterate in chat, copy what you want. It uses
the cached digest, so run `inkhaven submission digest` first. The system
prompt resolves through the usual three tiers: a paragraph in your
**Prompts** book named `submission-query` (etc.) → `prompts.hjson` → the
built-in, language-aware — so you can tune the house voice.

## 3. The tracker

Where did it go, when, and what came back? The **`Ctrl+V u`** tracker (the
`.inkhaven/submissions.json` log):

```sh
inkhaven submissions add --market "Dream Literary" --agent "A. Reader" --status sent
inkhaven submissions list                     # or --json / --status sent / --open
inkhaven submissions status S1 offer          # stamps a response date
inkhaven submissions add-note S1 "got a call — requested first 50 pages"
inkhaven submissions remove S1
```

Link a generated draft to a record with `--draft <slug>` (the generators
print the slug to use). The **`add-note`** command is the event trail:
each note is timestamped, so a record reads as a history — *sent →
requested edits → moving to round two*. In the `Ctrl+V u` modal, `↑↓`
move, `Space`/`s` cycle a record's status, `d` removes; the selected
record expands to show its note trail.

## Scripting the whole hand-off (Bund)

The export formats are `ink.export.*` words, so a release script can emit
every artefact in one pass (alongside the `ink.pdf.*` print pipeline from
Tutorial 65):

```forth
"my-novel" "out/my-novel.docx"  ink.export.docx
"my-novel" "out/my-novel.epub"  ink.export.epub
"my-novel" "out/my-novel.md"    ink.export.markdown
```

Each is `( book path -- )`; the path is sandboxed to the project root and
the words are `fs_write`-gated, like every other disk-crossing `ink.*`.

## Where to go next

- The print path: [Tutorial 65 — Hand-binding](65-hand-binding.md).
- Every chord: [`../KEYBINDING.md`](../KEYBINDING.md).
- The design: [SUBMISSION-1 plan](../PROPOSALS/SUBMISSION-1_PLAN.md).
