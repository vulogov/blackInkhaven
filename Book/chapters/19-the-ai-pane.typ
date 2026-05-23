#import "../design.typ": *

#chapter(number: 19, part: "Part VI — Working with AI",
  title: "The AI pane")

#dropcap("T")he AI pane is the right column of the four-pane
layout. It holds a chat history, an inference status, a
prompt input at the bottom, and a set of chords that apply
the AI's last response to the editor.

#section("Two layouts")

#chord_table((
  chord_row("Default (4-pane)", "AI pane is the right column."),
  chord_row("Ctrl+B K (full-screen)", "AI pane fills the screen. Editor + tree hidden; the entire space is conversation."),
))

`Ctrl+B K` is what you reach for when you want to think
aloud with the model for a stretch and don't need the
manuscript visible.

#figure_slot(
  id: "ai-pane-fullscreen",
  caption: "Full-screen AI pane (Ctrl+B K). Chat history fills the screen; prompt at the bottom.",
  height: 60mm,
)

#section("Sending a prompt")

`Ctrl+I` focuses the AI prompt slot. Type, Enter sends.
While the stream is in flight you can keep typing — Inkhaven
shows the response as it arrives.

`Esc` from any pane stops the stream cleanly (the partial
response stays in history).

#section("Scope (F9) recap")

The scope you pick with F9 (Chapter 18) gets RAG-loaded as
context BEFORE your typed query. So "explain this" with
Paragraph scope sends the open paragraph + your query.

#section("Applying answers")

Five chords apply the AI's last response to the editor:

#chord_table((
  chord_row("r / R", "Replace — overwrite the whole buffer (gated by the diff modal; see Chapter 21)."),
  chord_row("g / G", "Replace with grammar-corrected text only (extracts the corrected block — see Chapter 20)."),
  chord_row("i / I", "Insert at the cursor."),
  chord_row("t / T", "Prepend (top of the buffer)."),
  chord_row("b / B", "Append (bottom)."),
  chord_row("c / C", "Copy to clipboard (no edit)."),
))

`r` and `g` route through the diff-review modal by default
(`ai.diff_review_on_apply: true`). `i` / `t` / `b` are
additive and skip the modal.

#section("Chat history")

Inkhaven persists the chat history per project. Restart the
TUI and the conversation is still there. Two chords manage
it:

#chord_table((
  chord_row("Ctrl+B C", "Clear chat history — also clears the F7 grammar-change baseline."),
  chord_row("Up arrow (prompt slot)", "Walk backwards through your previous prompts."),
))

#section("Search the chat (`Ctrl+F` in AI pane)")

Long chat histories are searchable:

#chord_table((
  chord_row("Ctrl+F (AI pane focus)", "Open the chat-search input."),
  chord_row("Type", "Filter to messages containing the query."),
  chord_row("n / N", "Walk hits."),
  chord_row("Esc", "Close — chat reverts to chronological."),
))

#section("Selection mode (`Ctrl+C` in AI pane)")

A second mode that lets you pick a specific turn from
history and either copy or insert it into the editor:

#chord_table((
  chord_row("Ctrl+C (AI pane focus)", "Enter selection mode."),
  chord_row("↑ / ↓", "Navigate turns."),
  chord_row("c", "Copy the selected turn to clipboard."),
  chord_row("t", "Insert the selected turn at editor cursor."),
  chord_row("Esc", "Exit selection mode."),
))

#section("Status messages")

The status line under the AI pane reports the provider, the
model, the inference mode (`Local` / `Full`), and the scope
of the most recent send. Useful when you're mixing cloud +
local providers and want to confirm where the next turn is
going.

#section("Quick prompts (Ctrl+B G — Notes RAG)")

`Ctrl+B G` sends a query against the Notes book — semantic
search across every note + the query packaged for the AI.
Useful when you've scribbled something three weeks ago and
can't remember where.

#recap((
  [`Ctrl+I` focuses the prompt slot; Enter sends.],
  [`F9` cycles scope (None → Selection → Paragraph → … → Book).],
  [Apply chords: `r` replace · `g` grammar replace · `i` insert · `t` top · `b` bottom · `c` copy.],
  [Chat history persists per project; `Ctrl+B C` clears it.],
  [`Ctrl+F` searches chat; `Ctrl+C` enters selection mode (copy / insert turns).],
  [`Ctrl+B K` goes full-screen for long-form AI sessions.],
))
