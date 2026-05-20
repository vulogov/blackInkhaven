# 13 — AI full-screen mode

The default four-pane layout is great for normal writing — but
sometimes you want to give the AI the whole terminal: a long
conversation, a chat search across previous turns, or a side-by-side
view of the streaming reply and the history. That's **AI
full-screen mode**, toggled with `Ctrl+B K`.

This tutorial covers the layout, persistence across sessions,
search inside the chat history, and the selection mode that pipes
turns to the clipboard or directly into your manuscript.

## Toggle it: Ctrl+B K

Press `Ctrl+B` then `K` from any pane. The screen reshapes:

```
┌──────────────────────────────┬──────────────────────────────┐
│  AI                          │  Chat history · 6 turn(s)    │
│                              │                              │
│  (current / streaming        │  ❯ User                      │
│   inference — same content   │    Why does Typst page       │
│   as the normal AI pane,     │    break weakly default?     │
│   just full-height)          │                              │
│                              │  ← Assistant                 │
│                              │    `pagebreak()` does a hard │
│                              │    break; `weak: true` only  │
│                              │    breaks when the page has  │
│                              │    content. So it's a no-op  │
│                              │    after a fresh page break. │
│                              │                              │
├──────────────────────────────┴──────────────────────────────┤
│  AI prompt (full width)                                     │
└─────────────────────────────────────────────────────────────┘
```

The same `Ctrl+B K` returns to the normal layout. The two modes
are mutually exclusive — entering AI full-screen disables
Ctrl+B W typewriter mode and vice versa.

Tree, editor, and search bar are hidden. The editor's buffer is
still loaded — the `t`-action in selection mode (below) writes
into it — but you can't see it until you exit the layout.

## Persistence across sessions

When you **exit** AI full-screen (`Ctrl+B K` toggles off, or you
close the TUI cleanly), Inkhaven writes the current `chat_history`
to `<project>/.inkhaven-chat.json` (pretty-printed JSON).

When you **enter** AI full-screen and the in-memory history is
empty, Inkhaven reads that file back. The status bar reports the
restoration:

```
AI fullscreen · restored 14 turn(s) · Ctrl+B K to exit · Ctrl+F to search history
```

Behaviour notes:

- A **live history is never overwritten** on entry. The restore
  only fires when `chat_history.is_empty()`. So if you've sent
  even one message in this session, the layout picks up where the
  current chat left off, not what was on disk.
- An **empty history removes the file** on exit, so a stale write
  from a previous session can't resurrect a phantom.
- **Save and load failures log + carry on.** A wedged filesystem
  can't lock you out of the layout.
- `Ctrl+B C` (clear chat history) also nukes the file the next
  time you toggle out.

## Scrolling the chat history

Up / Down / PageUp / PageDown scroll the right pane, regardless of
which sub-pane has focus (typically you're focused on the AI
prompt typing the next message):

| Key | Scroll |
| --- | ------ |
| `↑` | 1 line up (further into history) |
| `↓` | 1 line down (back toward latest) |
| `PageUp` | 10 lines up |
| `PageDown` | 10 lines down |

The default is to **pin the newest turn to the bottom of the
pane** — chat-window UX. Older turns scroll up off-screen as new
ones arrive. When you scroll up manually, the pane title shows
the offset:

```
 Chat history · 12 turn(s) · ↑ 30 line(s) · ↑↓ / PgUp / PgDn
```

The scroll resets whenever:
- You send a new message (so the streaming reply is visible).
- You toggle the layout off and back on.
- You clear the chat (`Ctrl+B C`).

**Exception:** while the `/` prompt-library picker is open in the
AI prompt input, `↑` / `↓` still navigate the picker's selection.
The interception only fires when the picker isn't showing.

## Search the chat history: Ctrl+F

In the full-screen layout, `Ctrl+F` opens a chat-history search
modal. It's a different code path from the editor's Ctrl+F —
inside this layout the editor isn't visible, so the chord is
reclaimed.

```
 Chat search — Ctrl+F
 Search chat history:
 › │
 Enter starts from the newest match · Ctrl+X advances to older · Esc cancels
```

Submit a query with Enter. Inkhaven:

1. Scans the rendered chat-history pane for case-insensitive
   substring matches.
2. **Starts at the newest match** — closest to the bottom — per
   spec. The status bar reports the position:

   ```
   chat search: `dragonglass` · 1/4 (newest)
   ```

3. Highlights matches in the same colour scheme the editor uses:
   - Each match's word is painted with `search_match_bg` (pink).
   - The current match is painted with `search_current_bg` (lighter
     pink) plus bold.
   - Foreground is `pane_bg` (dark) so the matched word reads
     clearly against the pink — no pink-on-pink swallowed text.
4. **Centres** the current match line vertically in the pane —
   same logic as the editor's find-modal centring.

`Ctrl+X` advances to the **next-older** match (wrapping back to
newest after the oldest). The cycle is one-directional because
"start at newest, walk older" is the natural reading direction
for chat history.

Press `Esc` to clear the search — highlights drop, scroll snaps
back to the bottom pin.

## Chat selection mode: Ctrl+C

Outside this layout, `Ctrl+C` in the editor is "copy to
clipboard". Inside the AI full-screen layout, the chord is
re-used: it enters **chat selection mode**.

```
 chat selection mode · ↑↓ navigate · c=copy · t=insert into editor · Esc to exit
```

A turn (one User message or one Assistant reply) is highlighted as
a block with `current_line_bg` — the same colour the editor uses
to mark its current row, so the cue is consistent:

```
 ❯ User
   Who is Arthur Dent?
 ← Assistant                            ┐
   Arthur Dent is the everyman pro-     │  selection block,
   tagonist of Douglas Adams' The       │  whole turn
   Hitchhiker's Guide to the Galaxy …   ┘
```

Initial cursor lands on the **newest** turn — the assistant
reply you most likely just received.

Keys:

| Key | Action |
| --- | ------ |
| `↑` / `↓` | Step turn-by-turn (overrides scroll while selection is active) |
| `Home` / `End` | First / last turn |
| `c` / `C` | Copy the selected turn's text to the **system clipboard** (via `arboard`) |
| `t` / `T` | Insert the text at the **editor's cursor** in the open paragraph |
| `Esc` / `Enter` | Exit selection mode |

The `t` action is useful when an Assistant reply is the right
next paragraph body, or when a User question is what you want to
quote in your prose. The editor is hidden in this layout, but the
buffer it owns is still there — pressing `t` writes into it; when
you toggle back to the four-pane layout (`Ctrl+B K`) you'll see
the insertion at the cursor.

Selection mode is wiped whenever you toggle the layout or clear
the chat history.

## A typical workflow

A natural session:

1. `Ctrl+B K` enters AI full-screen.
2. The history from the previous session restores automatically.
3. Type a question in the AI prompt → Enter. The streaming reply
   lands in the left pane.
4. When done, the completed exchange moves into the chat history
   on the right (the next user message triggers the move).
5. After several turns, press `Ctrl+F` to look back: "what did
   Claude say about typst page breaks earlier?" → search → cycle
   with `Ctrl+X`.
6. Found a useful answer → `Ctrl+C` to enter selection mode →
   `↑` / `↓` to land on the right turn → `t` to insert it into
   your manuscript at the editor cursor.
7. `Ctrl+B K` exits back to the four-pane layout. The session is
   saved to `.inkhaven-chat.json` for the next launch.

## When to use it

- **Long Q&A sessions** with many turns where the editor pane
  becomes noise.
- **Reviewing prior context** — searching back through what the
  AI said hours ago is much faster in this layout than scrolling
  the regular AI pane.
- **Bulk-inserting AI output** into the manuscript — selection
  mode + `t` is the cleanest way to pipe an Assistant reply into
  a paragraph.

If you just want to **see less chrome** while typing prose, use
`Ctrl+B W` (typewriter mode) instead — that's editor-only,
zero-AI.

## Next steps

- [`12-configuring-ai-providers.md`](12-configuring-ai-providers.md)
  — picking and switching between the bundled providers.
- [`14-document-status.md`](14-document-status.md) — workflow
  tracking on paragraphs, useful as you accumulate AI-touched
  drafts that need a "needs review" flag.
