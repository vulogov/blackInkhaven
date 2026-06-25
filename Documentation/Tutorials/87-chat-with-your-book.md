# Tutorial 87 — Chat with your book

*Inkhaven 1.4.1*

The AI pane's **Book** scope used to send your whole manuscript along with
every question. On a short book that was fine; on a real one it was slow,
expensive, and vague — the model skimmed everything and grounded in nothing.
1.4.1 replaces it with **retrieval-augmented generation**: a Book-scope
question now retrieves the handful of paragraphs that actually bear on it,
hands those to the model as evidence, and asks for an answer that **cites**
them. You ask "does Mara ever doubt the captain?" and you get back specific
passages, by name, instead of a plausible paragraph the model invented.

This is always on for Book scope — there is no switch to flip and no "send the
whole book" mode to fall back to. **The other scopes are untouched:**
Selection, Paragraph, Subchapter, and Chapter still send exactly the text they
name. Book is the one that changed, because Book is the one that never worked
at scale.

## 1. Ask a question in Book scope

Put the cursor anywhere inside a manuscript book, then cycle the AI pane's
scope with **F9** until the header reads **Book**:

```
┌─ AI · Scope: Book ─────────────────────────────────────────────────┐
│ > does Mara ever doubt the captain?_                                │
└─────────────────────────────────────────────────────────────────────┘
```

Type your question and send it. Behind the prompt, inkhaven runs a semantic
search over the book (the same on-save vector index every other semantic
feature uses), keeps the most relevant paragraphs, widens each with a
neighbour or two so it reads in context, and composes that into a focused,
token-budgeted block of evidence. The model answers from that block — and
every claim it makes about the book is a citation:

```
She does, twice. Her first real doubt surfaces on the crossing, when the
captain orders the night run despite the glass falling — she calls it
"a wager with other men's lives" [act-two/the-night-run]. By the storm
chapter that doubt has hardened into open defiance [act-three/storm-at-sea].
```

Each citation is the passage's **location path** — `[chapter-slug/scene-slug]`
— so you can see *where* in the book a claim comes from and jump straight to
it, rather than reading an opaque id. If the model ever cites a location that
**wasn't** in what it was given, inkhaven flags it inline —
`[citation could not be validated: …]` — so a hallucinated reference can't pass
as grounded. An answer with clean citations is an answer you can trust to the
paragraph.

When nothing in the book addresses your question, the model is told to say so
plainly rather than confabulate — "the retrieved passages don't address that
directly" — and then either ask you to refine the question or offer general
knowledge clearly marked as *not from the book*.

## 2. See what it retrieved

You don't have to take the grounding on faith. Above the conversation, a
collapsed line shows how many passages grounded the answer:

```
▶ Retrieved passages (6) · p to expand
```

Press **`p`** to expand it (and again to collapse). Each passage lists its
similarity score, a **★** for a direct hit (versus a context-expansion
neighbour pulled in around one), its `[location path]`, and its opening:

```
▼ Retrieved passages (6) · p to collapse
  ★ 0.851  manuscript/act-ii/storm-at-sea
           On the ninth night the storm took them. Waves the height of…
    0.806  manuscript/act-i/the-harbour
           The harbour at Vell was a forest of masts and gull-cry…
  …
  (retrieved once for this chat — clear history to retrieve again)
```

That last line is the rule that keeps a conversation coherent: retrieval
happens **once per chat session**. Your follow-up questions reason over the
same passages instead of yanking the ground out from under the thread. When
you want to re-ground — a new line of inquiry, or you've edited the text —
clear the chat history; the next Book question retrieves afresh.

If you save an edited paragraph while a Book conversation is open, inkhaven
notices the retrieval is now grounded in pre-edit text and nudges you once:

```
book changed since retrieval — clear chat to re-ground on the new text
```

It's a reminder, not an interruption — the existing conversation stays valid;
clear it when you're ready for the model to see the new prose.

## 3. What's in scope

Retrieval is always anchored to the **user book your cursor is in** — its
chapters, subchapters, and paragraphs. But a question about your story often
wants your *notes* about the story, so a curated set of author-content system
books joins the pool: by default **Notes, Research, Places, Characters,
Artefacts, World,** and **Language**. Ask "what's the significance of the brass
key?" and a Characters or Artefacts entry can ground the answer alongside the
prose. The internal/meta system books — Scripts, Prompts, Typst, Help, Intent
— never enter retrieval.

You tune all of this in the **`book_rag`** config block — how many hits ground
an answer (`top_k`), how much context surrounds each (`context_expansion`), the
size of the evidence block (`max_context_tokens`), and exactly which system
books join in (`include_system_books` / `exclude_system_books`). See
[CONFIGURATION.md → `book_rag`](../CONFIGURATION.md#book_rag-141--ai-pane-book-scope-retrieval).

The questions in this pane answer in the language you ask them in; the grounding
contract (cite the passages, admit when they're silent) ships localized for the
five baseline languages — English, Russian, Spanish, French, German — and falls
back to English for any other.

## 4. Inspect retrieval from the terminal

Before (or instead of) spending a model call, you can see precisely what a
question would retrieve with **`inkhaven book-rag retrieve`**. No LLM runs; it
prints the passages the Book chat would ground on:

```sh
$ inkhaven book-rag retrieve "what does Mara fear about the voyage?"
Book-RAG retrieval — `The Long Road`  (6 passages, 3 direct hits, ~288 tokens)

★ 0.806  manuscript/act-i/dawn-departure
        id: 019efc4b-ff42-7603-9a5d-4f83eedf50cb
        The ship slipped its moorings before sunrise. Mara stood at the rail…
  0.806  manuscript/act-i/the-harbour
        id: 019efc4c-05b9-7033-b7a2-7623b3112421
        The harbour at Vell was a forest of masts and gull-cry…
★ 0.851  manuscript/act-ii/storm-at-sea
        id: 019efc4c-0c27-7d61-9c1e-8df95471c961
        On the ninth night the storm took them…
```

Useful flags:

- `--book-name <name>` — pick the book in a multi-book project (title or slug);
  optional when there's only one user book.
- `--top-k <n>` — override `book_rag.top_k` for this run only; your config is
  untouched. Good for feeling out how broad a question's grounding gets.
- `--context` — print the exact composed grounding block the model would
  receive (passage headers, ids, full bodies), instead of the listing. This is
  the literal evidence the answer is built on.

```sh
$ inkhaven book-rag retrieve "the harbour market" --top-k 2 --context
── Retrieved passages (grounding evidence) ──

[019efc4b-ff42-7603-…] manuscript/act-i/dawn-departure
The ship slipped its moorings before sunrise. Mara stood at the rail…

[019efc4c-05b9-7033-…] manuscript/act-i/the-harbour ★
The harbour at Vell was a forest of masts and gull-cry…

── end retrieved passages ──
```

The CLI and the pane share the same retrieval core, so what you see here is
exactly what grounds the chat.

## 5. Cost

Every Book-scope answer is one retrieval (cheap, local) plus one grounded model
call — far less than shipping the whole manuscript. The model calls are tagged
in the cost dashboard (`inkhaven cost`, or **`Ctrl+B Shift+$`**) under the
**`book_rag`** category, so you can see what chatting with your book costs over
a day. As everywhere in inkhaven, that figure **informs; it never blocks** — no
cap will refuse a question.

---

**See also:** [Tutorial 12 — Configuring AI providers](12-configuring-ai-providers.md)
· [Tutorial 79 — Socratic conversation](79-socratic-conversation.md)
· [CONFIGURATION.md → `book_rag`](../CONFIGURATION.md#book_rag-141--ai-pane-book-scope-retrieval)
