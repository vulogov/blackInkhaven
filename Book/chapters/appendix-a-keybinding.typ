#import "../design.typ": *

#appendix(letter: "A", title: "Keybinding reference")

#dropcap("E")very chord that ships with inkhaven, by layer.
The canonical machine-readable list lives at
`Documentation/KEYBINDING.md`; this appendix is the printed
companion.

#section("Global (anywhere)")

#chord_table((
  chord_row("Ctrl+Q", "Quit (autosaves dirty paragraph)."),
  chord_row("Tab / Shift+Tab", "Cycle Tree → Editor → AI."),
  chord_row("Ctrl+1/2/3/4/5", "Focus Editor / Tree / AI / Search / AI prompt."),
  chord_row("Ctrl+T", "Focus Tree."),
  chord_row("Ctrl+S", "Save current paragraph."),
  chord_row("Ctrl+/", "Focus search bar."),
  chord_row("Ctrl+I", "Focus AI prompt."),
  chord_row("Ctrl+B", "Meta prefix (next key = action)."),
  chord_row("Ctrl+Z", "Bund prefix."),
  chord_row("Ctrl+V", "View prefix."),
  chord_row("F1", "Help RAG query."),
  chord_row("F7", "Grammar check (editor scope)."),
  chord_row("F8", "Typst diagnostics list (1.2.6+)."),
  chord_row("F9 / Shift+F9", "Cycle AI scope forward / backward."),
  chord_row("F10", "Toggle inference mode (Local ↔ Full)."),
  chord_row("F12", "AI critique — mode-aware (1.2.6+)."),
  chord_row("Ctrl+F12", "AI explain diagnostic (1.2.6+ — was F11)."),
  chord_row("Esc", "Close overlay / cancel."),
))

#section("Tree pane")

#chord_table((
  chord_row("↑ / ↓ / Home / End", "Navigate."),
  chord_row("PgUp / PgDn", "Jump by 10."),
  chord_row("← / →", "Collapse / expand branch."),
  chord_row("Enter", "Open paragraph."),
  chord_row("F2", "Rename current node."),
  chord_row("F3", "File picker — load file / import tree."),
  chord_row("B / C / A / +", "Add book / chapter / subchapter / paragraph."),
  chord_row("V / S / P", "Insert chapter / subchapter / paragraph after current."),
  chord_row("D", "Delete branch (confirm)."),
  chord_row("-", "Delete paragraph at cursor."),
  chord_row("U / J", "Reorder up / down among siblings."),
  chord_row("Z / X", "Collapse subchapter / collapse every branch."),
  chord_row("Space", "Multi-select toggle."),
  chord_row("T", "Cycle type (works on marks)."),
  chord_row("O", "Cycle status (works on marks)."),
  chord_row("g", "Tag picker (works on marks)."),
))

#section("Editor pane")

#chord_table((
  chord_row("Ctrl+S", "Save."),
  chord_row("F5", "Snapshot with annotation prompt (1.2.6+)."),
  chord_row("F6", "Snapshot picker."),
  chord_row("F4", "Toggle split-edit."),
  chord_row("Ctrl+F4", "Accept split-edit snapshot as the new baseline."),
  chord_row("F7 / F12 / Ctrl+F12", "Grammar / critique / explain (see Global)."),
  chord_row("F8", "Diagnostics list (1.2.6+)."),
  chord_row("Ctrl+F", "Find in buffer."),
  chord_row("Ctrl+H", "Find and replace in buffer."),
))

#section("AI pane")

#chord_table((
  chord_row("r / R", "Replace buffer (routes through diff modal)."),
  chord_row("g / G", "Replace with grammar-corrected text only."),
  chord_row("i / I", "Insert at cursor."),
  chord_row("t / T", "Prepend (top)."),
  chord_row("b / B", "Append (bottom)."),
  chord_row("c / C", "Copy to clipboard."),
  chord_row("Ctrl+F", "Search chat history."),
  chord_row("Ctrl+C", "Enter selection mode."),
))

#section("Meta prefix (Ctrl+B)")

#chord_table((
  chord_row("Ctrl+B A", "Schedule assemble."),
  chord_row("Ctrl+B B", "Schedule build (PDF)."),
  chord_row("Ctrl+B O", "Schedule extra-format builds."),
  chord_row("Ctrl+B C", "Clear chat history."),
  chord_row("Ctrl+B G", "Notes RAG."),
  chord_row("Ctrl+B H", "Cheat-sheet overlay."),
  chord_row("Ctrl+B I", "Book info modal."),
  chord_row("Ctrl+B K", "Toggle AI full-screen."),
  chord_row("Ctrl+B L", "LLM provider picker."),
  chord_row("Ctrl+B M", "Show inference mode."),
  chord_row("Ctrl+B N / U / P / C / A", "Open Notes / Research / Places / Characters / Artefacts listing or RAG."),
  chord_row("Ctrl+B R / Shift+R", "Cycle status forward / backward."),
  chord_row("Ctrl+B 1..7", "Status filter modal."),
  chord_row("Ctrl+B V", "Credits."),
  chord_row("Ctrl+B ] / }", "Tag picker / tag search (1.2.5+)."),
))

#section("Bund prefix (Ctrl+Z)")

#chord_table((
  chord_row("Ctrl+Z R", "Run current buffer as Bund."),
  chord_row("Ctrl+Z N", "New Script node."),
  chord_row("Ctrl+Z E", "Open eval modal."),
  chord_row("Ctrl+Z ?", "Script picker."),
))

#section("View prefix (Ctrl+V)")

#chord_table((
  chord_row("Ctrl+V 1 / 2", "Markdown extract: paragraph / subchapter."),
  chord_row("Ctrl+V S", "Toggle similar-paragraph mode."),
  chord_row("Ctrl+V G", "Open progress modal."),
  chord_row("Ctrl+V t / Shift+T", "Per-¶ word-count target / open timeline view (1.2.6+)."),
  chord_row("Ctrl+V A / I", "Add outgoing / incoming link."),
  chord_row("Ctrl+V L / K", "List outgoing / incoming links."),
  chord_row("Ctrl+V B / M", "Toggle / list bookmarks."),
  chord_row("Ctrl+V P", "Fuzzy paragraph picker."),
  chord_row("Ctrl+V R", "Render paragraph preview."),
  chord_row("Ctrl+V N / Shift+N", "Next / previous diagnostic."),
  chord_row("Ctrl+V w / Shift+W", "Paragraph mini story view / book story view (1.2.6+)."),
  chord_row("Ctrl+V e", "Event picker (1.2.6+)."),
  chord_row("Ctrl+V Shift+E", "New event from any pane (1.2.6+)."),
  chord_row("Ctrl+V Shift+I", "Edit open event's start | end | track (1.2.6+)."),
))

#section("Inside the timeline view (Ctrl+V Shift+T)")

#chord_table((
  chord_row("← / → / PgUp / PgDn", "Scroll."),
  chord_row("+ / = / - / _", "Zoom in / out."),
  chord_row("0", "Reset zoom."),
  chord_row("Home / End", "Jump to first / last event."),
  chord_row("u / U / d / D / b / B / p / P", "Up / down / book / project scope."),
  chord_row("Tab", "Cycle highlighted track."),
  chord_row("Enter", "Open the event closest to cursor."),
  chord_row("n / N", "New event at cursor tick."),
  chord_row("y / Y / Ctrl+Y", "AI critique — track / scope / book-wide."),
))

#section("Inside Ctrl+V R (render preview)")

#chord_table((
  chord_row("← / → / Home / End", "Page navigation."),
  chord_row("+ / = / - / _", "Zoom (1.2.6+)."),
  chord_row("0", "Reset zoom."),
  chord_row("S", "Save current page."),
  chord_row("A", "Save every page."),
))
