# Tutorial 103 — The Research Assistant

*Inkhaven 1.5.0*

Your manuscript stands on a foundation of facts: the geography that must stay consistent, the history a
character half-remembers, the physics of an invented world. In Inkhaven those live in the **Facts**
system book — the ground truth that the world-checker (WORLD-4/5/6), the Book-scope RAG chat, the Inner
family, and the Mythology library all draw on. The trouble was always *populating* it: you had to leave
the writing environment, search, read, decide, and hand-type each entry.

**`inkhaven research`** closes that loop. It is a **separate, full-screen TUI** — its own layout and
keymap — where you conduct AI-assisted research and transfer **verified** findings straight into the
Facts (or Notes) corpus, with a confirmation step on every insertion. The output of a session is not a
chat log; it is a richer, audited, *yours* knowledge base that every other feature can immediately use.

This tutorial walks the whole tool top to bottom.

---

## 1. Launching

```sh
inkhaven research                       # opens the thread picker, or your one/default thread
inkhaven research --thread rome         # open (or create) a named, resumable session
inkhaven research --list-threads        # name · last-active · turns · cost   (--format json)
inkhaven research --export-thread rome  # the session as Markdown   (--format json, --out FILE)
```

No `--project` is needed inside a project directory. The screen needs **≥ 80 columns** (it shows a
resize hint below that). Quit any time with **`q`** (outside a text field), or **`Ctrl+Q`** / `Ctrl+C`
from anywhere — the terminal is always restored.

If you forget a key, press **`Ctrl+B h`** for a full quick reference of every chord and `/command`.
**`?`** toggles the one-line context hints at the bottom.

---

## 2. The layout

```
┌─ Facts ───────────────┬─ Research · thread: rome ─────────────────┐
│ (the Facts book tree) │ (streaming RAG chat)                      │
│  navigate · pin · edit │  ❯ query 1                                │
│                        │  your question (accent colour)            │
│                        │  the model's answer …                     │
├────────────────────────┴───────────────────────────────────────── ┤
│  context-sensitive hint line  (? toggles)                          │
├──────────────────────────────────────────────────────────────────┤
│ Query  /fact "extract the capacity figure" → Facts/Rome           │
├──────────────────────────────────────────────────────────────────┤
│ [RAG: Facts+Full]  [~$0.031]  [⬡ Rome/Engineering]  [?:help q:quit]│
└──────────────────────────────────────────────────────────────────┘
```

Three focus targets — the **Facts tree** (left ~40 %), the **chat** (right ~60 %), and the **query
prompt**. **`Tab`** / **`Shift+Tab`** cycle them; the active pane's border lights up in the theme's
focus colour. Colours come from your project / global `theme:` config, so the assistant matches the
editor.

The bottom status bar shows the live **RAG mode**, the running **session cost** estimate, your **pinned
nodes**, and any transient message.

---

## 3. Asking — retrieval-augmented chat

Type a question in the query prompt and press **`Enter`**. The answer streams into the chat, **grounded
by your Facts book**: before each query the assistant retrieves the most relevant facts and prepends
them as context, so the model reasons over *your* world, not just its training data.

**`F10`** cycles the RAG mode (also `/rag`):

- **Facts+Full** — your facts *plus* the model's general knowledge (default);
- **Facts only** — answer strictly from your corpus ("if it's not in the provided context, say so");
- **Full only** — no retrieval; the model's knowledge alone.

The query prompt is a real editable field: **`←` `→` `Home` `End`** and `Backspace`/`Delete` edit;
**`↑` `↓`** move between lines and recall **prompt history** at the top/bottom edge; **`Alt+Enter`**
inserts a newline for a multi-line query. The chat pane scrolls with **`j`/`k`/`g`/`G`** (focus it with
`Tab`), and **`Ctrl+F`** searches it (`n`/`N` jump between matches). Queries render in the accent
colour, answers in the default — easy to tell apart while scrolling.

---

## 4. Pinning context

The retrieval is automatic, but you can **force** a fact into every query's context. In the Facts tree,
**`Ctrl+P`** pins (or unpins) the cursor node — up to three, marked `⬡`, shown in the status bar. Pinned
text is always prepended (ahead of the semantic results), and pins persist with the thread. Use it to
keep a load-bearing fact — a timeline, a core rule of your magic system — in front of the model for a
whole line of questioning.

---

## 5. Turning research into facts — the confirmation step

Every insertion is a deliberate, reviewed act. Nothing reaches your corpus without you seeing and
approving it.

### `/fact "instruction" [→ path]`

Extracts **one** titled fact from the **last response**:

```
/fact "extract the daily capacity figure"
/fact "the founding date" → Facts/Rome/History
```

A second LLM call distils the response into `{title, fact}` — **in your project's language** (it never
silently drops to English) — and opens the **editable confirmation overlay**:

```
┌ Confirm insertion → Facts ───────────────────────────┐
│ Title: Aqua Claudia Daily Capacity                   │
│ ──────────────────────────────────────────────────── │
│ The Aqua Claudia carried approximately 190,000 m³ of │
│ water per day, the largest aqueduct in Rome.         │
│ → facts/rome/history                                 │
│ [Tab: field]  [Ctrl+S / Ctrl+Enter: confirm]  [Esc]  │
└──────────────────────────────────────────────────────┘
```

`Tab` switches between **Title** and **Body**; edit freely (full arrow/Home/End editing); **`Ctrl+S`**
(or `Ctrl+Enter`) inserts; **`Esc`** discards. An empty body is refused — you'll be told to type it or
cancel, so you never get a title-only stub.

The `→ path` is optional: with it, the fact lands at that slug path; without it, at the Facts-tree
cursor (a branch hosts it as a child; a paragraph as a sibling). Inserted facts are **re-embedded
immediately**, so they're instantly retrievable by `/diff`, the writing-mode RAG, and every Facts
consumer — no rebuild.

### `/note "instruction" [→ path]`

The same flow, but into the **Notes** book and preserving a **speculative** voice ("this *might*
connect…"). Facts are verified ground truth; Notes are your own thinking. Keeping them separate protects
the integrity of the corpus other features depend on.

### `n` — type a fact by hand

In the Facts tree, **`n`** opens a two-step inline entry (title, then `Ctrl+S` body). The research mode
is the single home for all fact entry — whether the source is the model, a `/fact` extraction, or your
own knowledge.

---

## 6. Checking before — and after — you commit

- **`/diff`** — embed the last response and show the **most similar facts already in your corpus**, with
  similarity scores, so you spot a near-duplicate *before* adding one.
- **`/verify`** — pull the specific, checkable claims out of the last response (years, quoted titles,
  quantities, named entities) and have the model self-assess each as **HIGH / MEDIUM / ⚠ LOW**
  confidence. Guidance for what to investigate before you `/fact` it.
- **`/factcheck`** — a post-hoc audit of the **whole Facts corpus** (multiple LLM calls): every fact is
  judged for real-world accuracy (`ACCURATE` / `DUBIOUS` / `INACCURATE`), then the full set is checked
  for facts that **contradict each other**. The report lands in the chat in two sections — factual
  accuracy and mutual consistency. Read-only; it never edits your corpus. Run it periodically as your
  knowledge base grows.

---

## 7. Deeper research

- **`/chain q1 → q2 → q3`** — a sequential pipeline: each step's answer becomes context for the next.
  Good for "what were the challenges → what solutions emerged → which are archaeologically verified".
- **`/goto facts/path/slug`** — jump the Facts tree to a node (and focus it). Handy before a `/fact`
  when you know exactly where it should land.
- **`/rag [facts+full|facts|full]`**, **`/clear`** (empty the chat), **`/save [name]`** (rename the
  thread) round out the namespace.

---

## 8. Organising the corpus — tree editing

The Facts tree is a first-class editor, on par with the main Tree pane (and the Outline's copy/move):

| Key | Action |
|:---|:---|
| `j`/`k` · `g`/`G` | Navigate · top/bottom |
| `h`/`l`/`Enter` | Collapse / expand / step |
| `R` | Rename the cursor node |
| `c` / `s` | New **chapter** (under the Facts book) / **subchapter** (under the enclosing chapter) |
| `-` / `D` | Delete a **paragraph** / **branch** (with a `y`·N confirmation) |
| `K` / `J` | Move the node up / down among its siblings |
| `y` / `x` / `p` | **Copy** / **cut** / **paste** a node across parents |
| `Ctrl+P` | Pin / unpin for RAG context |
| `n` | New fact (manual entry) |

Use chapters and subchapters to group facts by domain (Geography, History, Magic-System) so retrieval
and navigation stay sharp as the corpus grows.

---

## 9. Threads — persistent, resumable sessions

Every session is a **named thread** stored at `.inkhaven/research-threads/<slug>.json`: its queries,
fact/note insertions, pinned nodes, and RAG mode all persist and **resume** exactly where you left off.
`↑`/`↓` in the prompt recall your history (queries *and* the `/fact` commands you ran).

- **Launch rules:** 0 threads → a `default` thread; 1 → opened directly; 2+ → a **picker** (`Enter`
  open, `n` new, `d` delete, `Esc` quit).
- **`inkhaven research --list-threads`** prints every thread (name · last-active · turns · cost), and
  **`--export-thread <name>`** writes its full history as Markdown or JSON (`--out FILE`, default
  stdout) — a clean record of how a fact came to be.

---

## 10. The Inner family already knows your facts

Because inserted facts live in the real Facts book and the shared vector index, they immediately enrich
everything else: the **Book-scope** RAG chat (`Ctrl+B J`), the **Inner Socrates "Facts"** scope, the
**world-coherence** checks, and the **Mythology** library all read the same corpus you just grew. No
coupling, no rebuild — that's the whole point.

---

## 11. Configuration

The optional `research:` block (see [CONFIGURATION.md](../CONFIGURATION.md)) tunes everything; omit it
for the defaults:

```hjson
research: {
  default_thread: null          // null = picker / `default`
  rag_top_n: 5                  // max Facts prepended per query
  session_budget_warn: 0.50     // cost note (informs, never blocks)
  max_pinned_nodes: 3
  show_keybind_hints: true
  min_width: 80
  split_ratio: 4                // 4 = 40% tree, 60% chat
  diff_top_n: 3
  verify_min_sentence_words: 8
}
```

Colours follow your `theme:` block. Costs are a coarse, clearly-marked `~` estimate — they inform, never
block.

---

## 12. What it is not (yet)

Web search and document/PDF import are **RESRCH-2**. Today the assistant works from the configured
model's knowledge and your existing corpus — and turns them into a verified one, in your language, that
you control entirely. Every insertion is yours; nothing is written without your confirmation; nothing
edits your prose.

> **The minute loop:** ask → `/diff` to check for duplicates → `/verify` the shaky claims → `/fact` the
> keepers (review, edit, confirm) → `/factcheck` the corpus now and then. That's the whole discipline.
