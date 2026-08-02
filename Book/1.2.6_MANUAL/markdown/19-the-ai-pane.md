# 19 — The AI pane

The AI pane is the right column of the three centre panes. It holds the chat history and the inference status; the prompt input sits at the bottom of the screen (`Ctrl+I` focuses it). A set of chords applies the AI's last response to the editor.

## Two layouts

| Layout | Description |
|--------|-------------|
| Default (3-pane centre) | AI pane is the right column of the three centre panes. |
| Ctrl+B K (full-screen) | AI pane fills the screen. Editor + tree hidden; the entire space is conversation. |

`Ctrl+B K` is what you reach for when you want to think aloud with the model for a stretch and don't need the manuscript visible.

![figure: ai-pane-fullscreen](images/ai-pane-fullscreen.png) — Full-screen AI pane (Ctrl+B K). Chat history fills the screen; prompt at the bottom.

## Sending a prompt

`Ctrl+I` focuses the AI prompt slot. Type, Enter sends. While the stream is in flight you can keep typing — Inkhaven shows the response as it arrives.

`Esc` from any pane stops the stream cleanly (the partial response stays in history).

## Scope (F9) recap

The scope you pick with F9 gets RAG-loaded as context BEFORE your typed query. So "explain this" with Paragraph scope sends the open paragraph + your query. F9 cycles the full ring:

| Ring | Meaning |
| ---- | ------- |
| None → Selection → Paragraph | the local scopes — the selection, the open paragraph, its enclosing branches. |
| Subchapter → Chapter → Book | widening manuscript context (Book is retrieval-grounded — relevant passages, cited). |
| Facts → Socrates → Editor → Graph | the sticky conversation scopes — they persist across follow-ups until you cycle away. |

## Chat with your graph (Graph scope + the walk)

The *Graph* scope (F9) is the relational sibling of Book scope: it retrieves the passages relevant to your question and folds in the knowledge-graph edges touching them — what each contradicts, is sourced from, links to, cites — so the answer is grounded in how your book *connects*, not just its prose. Press `p` in the AI pane to expand the retrieved passages + their relations. See `Documentation/GRAPH.md` for the graph itself.

To let the model *walk* the graph — following contradictions and citations turn by turn — type a question in the AI prompt and press `Ctrl+B z → w` (the graph hub's *walk*):

| Chord | Effect |
| ----- | ------ |
| `Ctrl+B z → w` | Start a graph walk for the AI-prompt question. |
| (watch) | The pane streams each step — search → neighbours → contradictions → paths — then the grounded prose answer. |
| `Esc` | Stop the walk (any time). The status bar shows `turn k/N`. |

A walk is several model calls (bounded by `graph.ask_max_steps`), so it's an explicit action rather than the one-hop Graph scope — you opt into the depth per question. The CLI equivalent is `inkhaven graph ask "<question>"`.

## Applying answers

Five chords apply the AI's last response to the editor:

| Chord | What it does |
|-------|--------------|
| r / R | Replace — overwrite the whole buffer (gated by the diff modal; see Chapter 21). |
| g / G | Replace with grammar-corrected text only (extracts the corrected block — see Chapter 20). |
| i / I | Insert at the cursor. |
| t / T | Prepend (top of the buffer). |
| b / B | Append (bottom). |
| c / C | Copy to clipboard (no edit). |

`r` and `g` route through the diff-review modal by default (`ai.diff_review_on_apply: true`). `i` / `t` / `b` are additive and skip the modal.

## Chat history

Inkhaven persists the chat history per project. Restart the TUI and the conversation is still there. Two chords manage it:

| Chord | What it does |
|-------|--------------|
| Ctrl+B C | Clear chat history — also clears the F7 grammar-change baseline. |
| Up arrow (prompt slot) | Walk backwards through your previous prompts. |

## Search the chat (`Ctrl+F` in AI pane)

Long chat histories are searchable:

| Chord | What it does |
|-------|--------------|
| Ctrl+F (AI pane focus) | Open the chat-search input. |
| Type | Filter to messages containing the query. |
| n / N | Walk hits. |
| Esc | Close — chat reverts to chronological. |

## Selection mode (`Ctrl+C` in AI pane)

A second mode that lets you pick a specific turn from history and either copy or insert it into the editor:

| Chord | What it does |
|-------|--------------|
| Ctrl+C (AI pane focus) | Enter selection mode. |
| ↑ / ↓ | Navigate turns. |
| c | Copy the selected turn to clipboard. |
| t | Insert the selected turn at editor cursor. |
| Esc | Exit selection mode. |

## Status messages

The status line under the AI pane reports the provider, the model, the inference mode (`Local` / `Full`), and the scope of the most recent send. Useful when you're mixing cloud + local providers and want to confirm where the next turn is going.

## Quick prompts (Ctrl+B G — Notes RAG)

`Ctrl+B G` sends a query against the Notes book — semantic search across every note + the query packaged for the AI. Useful when you've scribbled something three weeks ago and can't remember where.

## Recap

- `Ctrl+I` focuses the prompt slot; Enter sends.
- `F9` cycles scope (None → Selection → … → Book → Facts → Socrates → Editor → Graph).
- `Graph` scope grounds answers in the knowledge graph's relations; `Ctrl+B z → w` *walks* the graph (`Esc` stops).
- Apply chords: `r` replace · `g` grammar replace · `i` insert · `t` top · `b` bottom · `c` copy.
- Chat history persists per project; `Ctrl+B C` clears it.
- `Ctrl+F` searches chat; `Ctrl+C` enters selection mode (copy / insert turns).
- `Ctrl+B K` goes full-screen for long-form AI sessions.
