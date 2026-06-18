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

The scope you pick with F9 (Chapter 18) gets RAG-loaded as context BEFORE your typed query. So "explain this" with Paragraph scope sends the open paragraph + your query.

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
- `F9` cycles scope (None → Selection → Paragraph → … → Book).
- Apply chords: `r` replace · `g` grammar replace · `i` insert · `t` top · `b` bottom · `c` copy.
- Chat history persists per project; `Ctrl+B C` clears it.
- `Ctrl+F` searches chat; `Ctrl+C` enters selection mode (copy / insert turns).
- `Ctrl+B K` goes full-screen for long-form AI sessions.
