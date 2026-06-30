# Tutorial 103 — The Research Assistant

*Inkhaven 1.5.0*

The **Facts** system book is Inkhaven's ground-truth corpus — the source WORLD-4/5/6, BOOK_RAG, the
Inner family, and MYTH-1 all draw on. But filling it used to mean leaving the writing environment.
`inkhaven research` closes that loop: a **separate TUI screen** where you conduct AI-assisted research
and transfer verified findings straight into the Facts (or Notes) corpus — with a confirmation step
that keeps you in control of every insertion.

```sh
inkhaven research                       # the thread picker, or your one/default thread
inkhaven research --thread rome         # open (or create) a named, resumable session
inkhaven research --list-threads        # name · last-active · turns · cost  (--format json)
inkhaven research --export-thread rome  # the session as Markdown  (--format json, --out FILE)
```

## The layout

Left 40%: the **Facts tree** (navigate, pin, add). Right 60%: the **streaming chat**. Below: a two-line
**query prompt**. `Tab` / `Shift+Tab` cycle the three; the active pane's border is bright. The screen
needs ≥ 80 columns.

Type a question and press Enter — the assistant answers, grounded by Retrieval-Augmented Generation
over your Facts book. **F10** cycles the RAG mode: *Facts+Full* (your facts + the model's knowledge),
*Facts only* (answer strictly from your corpus), *Full only* (no retrieval). Pin up to three Facts
nodes with **Ctrl+P** in the tree — pinned text is always prepended to the context.

## Turning research into facts

Every insertion is a deliberate, reviewed act:

- **`/fact "clarifying instruction"`** — extract ONE titled fact from the last response. The title and
  body open in an **editable confirmation overlay**: `Tab` switches fields, edit freely, **Ctrl+S** (or
  Ctrl+Enter) inserts, **Esc** discards. Add `→ path` to aim the insertion (`/fact "..." →
  Facts/Rome/Engineering`); otherwise it lands at the tree cursor.
- **`/note "..."`** — the same flow, but into the **Notes** book and preserving a speculative,
  tentative voice. Facts are verified; Notes are your own thinking.
- **`n`** in the Facts tree — type a fact by hand (title, then Ctrl+S body). The research mode is the
  single home for all fact entry.

Inserted facts are **immediately** indexed (the write re-embeds into the shared vector index), so they
are instantly available to `/diff`, the writing-mode RAG, and every Facts consumer — no rebuild.

## The investigative commands

- **`/diff`** — embed the last response and show the most similar facts already in your corpus, so you
  spot a near-duplicate *before* adding one.
- **`/verify`** — extract the specific, checkable claims (years, quoted titles, quantities, named
  entities) and have the model self-assess each as HIGH / MEDIUM / **⚠ LOW** confidence — guidance for
  what to investigate before you `/fact` it.
- **`/factcheck`** — a post-hoc audit of the **whole Facts corpus** (multiple LLM calls): every fact is
  assessed for factual accuracy (ACCURATE / DUBIOUS / INACCURATE), then the full set is checked for
  facts that **contradict each other**. Read-only; it reports into the chat and never edits the corpus.
- **`/chain q1 → q2 → q3`** — a sequential pipeline: each step's answer becomes context for the next.
- **`/goto facts/path/slug`** — jump the tree to a node. **`/rag`**, **`/clear`**, **`/save [name]`**
  round out the namespace.

## Threads, search, and the rest

Sessions are **named, persistent threads** under `.inkhaven/research-threads/`: queries, insertions,
pinned nodes, and RAG mode all persist and resume. `↑`/`↓` in the prompt recall your history.
**`Ctrl+F`** searches the chat (highlight + `n`/`N`). **`Ctrl+B h`** opens a full quick-reference of
every chord and `/command`; `?` toggles the keybind hints; `q` / `Ctrl+C` / `Ctrl+Q` quit.

## Configuration

The optional `research:` block tunes it — `rag_top_n` (5), `max_pinned_nodes` (3), `split_ratio` (4 =
40% tree), `diff_top_n` (3), `verify_min_sentence_words` (8), `session_budget_warn` (0.50, informs
never blocks), `min_width` (80), `show_keybind_hints` (true). Zero new dependencies; reads the Facts /
Notes books you already have.

Web search and document import are RESRCH-2. For now, the Research Assistant turns the model's knowledge
and your existing corpus into a richer, verified, *yours* one.
