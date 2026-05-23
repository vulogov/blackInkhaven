#import "../design.typ": *

#chapter(number: 1, part: "Part I — Foundations",
  title: "What Inkhaven is")

#dropcap("I")nkhaven is a single binary that, when you run it
in a project directory, paints a four-pane terminal user
interface and lets you write a book. Behind the panes sits
a small database that stores every paragraph as a node in
a tree, every snapshot, every wiki-link, every tag — and an
indexer that makes the manuscript searchable both
semantically (by meaning) and by exact text.

That's the whole shape. Everything else this book teaches
you is a way of moving through that tree, sending parts of
it to a language model, or rendering it to Typst.

#section("The four panes")

#figure_slot(
  id: "tui-overview",
  caption: "The four-pane layout: Tree (left), Editor (centre), AI (right). Search bar runs along the bottom; status line under that.",
  height: 70mm,
)

The #strong[Tree pane] (left) shows the manuscript as a folder
hierarchy. Books contain chapters; chapters contain
subchapters or paragraphs; paragraphs contain prose.

The #strong[Editor pane] (centre) holds the open paragraph.
This is where you type. Saves go to disk and the database
together; nothing waits in unsaved buffers when you Ctrl+S.

The #strong[AI pane] (right) holds the conversation with the
language model. By default it's a chat history; in
full-screen mode (`Ctrl+B K`) it grows to fill the screen
when you want to think aloud with the model for a stretch.

The #strong[Search bar] (bottom) is dual-mode: type a query,
hit Enter, and the result list overlays the tree pane.

#section("Local first")

Every byte you write lives in one folder. The database is
DuckDB; the prose is Typst markup; the vectors live next to
the database in a `vectors/` directory. You can rsync the
whole project to a backup drive, push it to git, or carry it
on a USB stick. The format is plain text + a few binary
indices you can rebuild from the prose.

Inkhaven does not phone home, does not require an account,
does not require an internet connection unless you ask
the AI pane for help. The first time you run it, no
analytics, no telemetry, no welcome video. Just a TUI.

#section("Typst as the typesetter")

Inkhaven manuscripts are written in Typst — a modern
typesetting language designed for academic books and
literary work. You don't need to learn Typst to use
inkhaven; the editor accepts plain prose and the bundled
templates handle layout. But the moment you want a fancy
heading or a margin note, Typst is there.

#callout(label: "If you've used LaTeX")[
  Typst is what LaTeX would look like if it had been
  designed in 2019 instead of 1985. Same idea (markup →
  professional typesetting), much friendlier syntax, faster
  compile. Inkhaven embeds the Typst compiler directly so
  you don't need to install it separately.
]

#section("AI when you want it")

The AI surface is the part that polarises. Inkhaven's
position: AI is a writing partner, not a writing tool. You
ask it for grammar review, for critique, for "what's weak
here", for help untangling a Typst diagnostic. It never
edits your paragraph without an explicit accept step
(`r` or `g` in the AI pane → diff modal → `a`).

You configure which LLM provider to talk to in
`inkhaven.hjson`. Six providers ship in the binary:
Gemini, Claude, OpenAI, DeepSeek, Grok, and Ollama (local).
Switch live with `Ctrl+B L`.

If you don't want AI in your writing room at all, simply
don't configure a provider. Inkhaven works fully without
one.

#section("What it is, in one sentence")

Inkhaven is a TUI for writing a book. The tree is the book.
The paragraphs are the prose. Everything else is a
convenience you can ignore until you need it.

#recap((
  [Single binary, single folder per project — local first.],
  [Four-pane TUI: Tree · Editor · AI · Search bar.],
  [Typst under the hood for typesetting; bundled compiler.],
  [AI is opt-in, scope-limited, and never auto-applies edits.],
  [Everything else is a layer you can engage or ignore.],
))
