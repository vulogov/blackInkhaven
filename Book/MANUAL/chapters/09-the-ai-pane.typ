#import "../design.typ": *

#chapter(number: 9, title: "The AI Assistant")

Every other chapter so far has been about you and the manuscript. This one adds
a third party to the desk: a language model you can consult without leaving the
window. Inkhaven's stance on that model is stated in a single line of its own
README — #emph[AI is a co-author you steer] — and the word that matters is
#emph[steer]. The assistant never acts on its own, never edits a word you did
not ask it to edit, and never sees more of your book than you decide to show it.
Everything in this chapter is machinery for that steering: how you point the
model at a slice of the manuscript (the *scope*), how much of its own
training you let it draw on (the *mode*), where its answer is allowed
to land (the *destination*), and which of six providers is doing the
thinking. Learn these four dials and the AI pane stops being a chat box and
becomes an instrument.

The pane itself you already met in Chapter 3, as one of the three surfaces the
right-hand region can show. Here we open it up.

#section("Opening and focusing the pane")

The AI feature is really two surfaces working as a pair: the *AI
pane*, where answers stream in and the conversation scrolls, and the *AI
prompt bar*, the thin input line along the bottom where you type. They are
bound together so tightly that Inkhaven treats moving between them as a bounce
rather than a full focus change.

To reach the prompt from anywhere, press `Ctrl+I` (or `Ctrl+5`) — focus drops to
the AI prompt bar and you can start typing immediately. To read and scroll the
answer, `Ctrl+3` focuses the AI pane itself. Once you are on one of the two,
`Esc` #emph[bounces] you to the other: `Esc` from the prompt bar lifts focus up
into the pane so you can scroll the history; `Esc` from the pane drops focus back
to the prompt so you can type a follow-up. This AI ↔ AI-prompt pairing is its own
little loop, independent of the Tree / Editor / Search rotation that `Tab` walks.

#term("The bounce")[
  From the AI prompt bar, `Esc` moves focus to the AI pane (to read); from the
  AI pane, `Esc` moves focus back to the prompt bar (to type). You ping-pong
  between asking and reading with one key, and never fall out of the AI surfaces
  by accident.
]

If the right region is currently showing the Output or Thoughts pane instead,
`Ctrl+B Tab` cycles it around to the AI pane (Output → AI → Thoughts), or
`Ctrl+3` jumps straight there and surfaces it. And when a conversation grows long
enough that the four-pane layout feels cramped, `Ctrl+B K` fullscreens the AI
pane — the chat history and the prompt bar spread across the whole window for an
extended session. Press it again to return to four panes.

#chord_table((
  chord_row("Ctrl+I / Ctrl+5", "Focus the AI prompt bar (bottom). Start typing at once."),
  chord_row("Ctrl+3", "Focus the AI pane and surface it in the right region."),
  chord_row("Esc", "Bounce between the AI pane and the AI prompt bar."),
  chord_row("Ctrl+B Tab", "Cycle the right region Output → AI → Thoughts."),
  chord_row("Ctrl+B K", "Fullscreen the AI pane with its history and prompt (toggle)."),
  chord_row("Enter", "Send the prompt (from the prompt bar)."),
))

#section("The title bar — the pane's state at a glance")

The AI pane wears a dense title strip, and it repays a glance. Read left to
right, it folds in everything about the pane's current state: the provider doing
the work, whether a stream is in flight, the inference mode, the armed scope, and
how deep the conversation has run.

#screen(caption: "The AI pane title — five facts on one line")[```
┌─ AI · gemini · done · infer=Full · scope=Paragraph ──┐
│           │        │        │            └ armed scope (F9)
│           │        │        └ Local / Full (F10)
│           │        └ stream state: streaming… / done / error
│           └ active provider (the llm default)
│  you  Is the pacing of this opening too slow?
│  ai   The image is strong, but four beats land …
└──────────────────────────────────────────────────────┘
```]

While tokens are arriving the stream field reads `streaming…`; when the model
finishes it flips to `done`; if the call fails it shows `error` and the failure
text lands in the pane body. Once a conversation is under way the strip also
carries the turn depth — `3 turn(s)` — so you know how much history the next
prompt will replay. Two of the fields are colour-coded by default so an
accidental setting is obvious at a glance: the scope chip is peach
(`theme.ai_scope_fg`) and the inference-mode chip is teal (`theme.ai_infer_fg`).
When prompt-language resolution is active a `lang=` chip joins them, reading
`ru (book)` or `ru (paragraph)` depending on how the language was decided (see
the last section of this chapter).

The prompt bar has a title of its own, and it echoes the armed scope so you see
it while you type: `AI prompt · scope: Paragraph`. You rarely read either strip
word for word, but between them you are never guessing what the next `Enter`
will do.

#section("Streaming inference")

Press `Ctrl+I`, type a question, press `Enter`, and the answer begins to appear
immediately — not a spinner, then a wall of text, but characters arriving left to
right as the model produces them. Inkhaven streams the tokens into the pane as
they land. This is worth internalising: the model is #emph[literally] writing the
answer one piece at a time, and you are watching it think, so you can start
reading — or decide the answer is going the wrong way and clear it — before it
finishes.

#screen(caption: "A streaming answer, tokens arriving live")[```
┌─ AI · gemini · streaming… · infer=Full ──────────────┐
│ Several darker alternatives:                         │
│                                                      │
│  • stunned sailor — keeps the alliteration           │
│  • shaken helmsman — slightly nautical               │
│  • dazed deckhand — leans on disorientation          │
│                                                      │
│ "Thunderstruck" reads pre-20th-century; if the ▏     │
└──────────────────────────────────────────────────────┘
```]

The pane renders the response as #emph[markdown] as it streams: `*bold*`,
`_italic_`, `` `inline code` ``, `# headings`, bullet and numbered lists, code
fences, and blockquotes all display as formatted text rather than raw asterisks
and hashes. That rendering is purely a display convenience — the raw markdown is
what Inkhaven actually stores and what flows through the apply pipeline described
below.

Underneath, the status bar narrates the call. While the tokens arrive it reports
something like `streaming from gemini (chat turn #3 · scope=Paragraph)…`; when
the model finishes it prints the elapsed time, `gemini responded in 1.8s`. If the
call fails — a missing API key, a mistyped model name, a network hiccup — the
status flips to an error, the title carries `error`, and the error text sits in
the pane body. Fix the cause and re-send; nothing is lost.

#callout(label: "Reading while it writes")[
  You do not have to wait for `done`. Scroll the pane (`Ctrl+3`, then the arrows
  or the wheel) while a stream is still running, or type your next question into
  the prompt bar — Inkhaven queues nothing behind your back, so a follow-up sends
  a fresh call once the current one settles.
]

#section("Scope — telling the AI what to look at")

By default the model sees only the prompt you typed plus the running chat
history. It knows nothing about your manuscript unless you attach a slice of it.
The dial that attaches that slice is the #emph[scope], and you cycle it with a
single key: `F9`, from any pane.

#term("Scope")[
  The #emph[scope] decides what chunk of your book is prepended to the next
  prompt as context. `F9` cycles it. A non-`None` scope prepends its matching
  content to the query you send, then — for the manuscript scopes — #emph[auto-
  resets to `None`] after that one submission, so a follow-up prompt is never
  surprised by stale context you forgot was armed.
]

`F9` walks a ten-stop ring and wraps:

#screen(caption: "The F9 scope ring")[```
  None → Selection → Paragraph → Subchapter → Chapter →
    Book → Facts → Socrates → Editor → Graph → None
```]

The armed scope shows in three places while it is set: the AI pane title
(`scope=Paragraph`), the prompt-bar title (`AI prompt · scope: Paragraph`), and
the status bar, which spells out `AI scope: Paragraph (will prepend matching
context to next prompt)`. After you send, a manuscript scope's chip disappears as
it resets to `None`.

#subsection("The manuscript scopes")

The first six stops draw text from the tree around your cursor — progressively
wider slices of the book you are actually writing.

#screen(caption: "What each manuscript scope sends")[```
 Scope        Sends to the model
 ─────────    ──────────────────────────────────────────
 None         Nothing extra — the prompt + prior chat.
 Selection    The current editor selection. Errors if
              no selection is active.
 Paragraph    The whole open paragraph. In split-edit,
              both the snapshot and the live buffer.
 Subchapter   Every paragraph under the cursor's
              enclosing subchapter.
 Chapter      Same, for the enclosing chapter.
 Book         Same, for the enclosing book — but
              retrieval-grounded (see below).
```]

Two of these have a wrinkle worth stating plainly. #emph[Selection] errors out
with a status message if there is no active selection — it has nothing to attach,
so it refuses rather than send an empty scope. And #emph[Book] does not blindly
stuff an entire novel into one prompt; that would blow past every model's context
window and cost a fortune. Instead it is #emph[retrieval-grounded]: Inkhaven
retrieves the passages most relevant to your question from the book's semantic
index and cites those, rather than sending the whole text. A book-wide question
therefore stays affordable and pointed, at the price of the model seeing the
relevant passages rather than literally every word.

#two_track(
  [Match the scope to the question's reach. Brainstorming one sentence?
  `Selection` or `None`. Tightening a paragraph? `Paragraph`. Checking that a
  scene stays consistent with itself? `Subchapter`. Chasing a plot thread across
  a chapter? `Chapter`. A whole-book sweep? `Book` — long, and best on a
  large-context provider.],
  [For reference and documentation the same ladder holds: `Selection` to reword a
  clause, `Paragraph` to fact-check a claim in place, `Chapter` to ask whether a
  section contradicts an earlier one, `Book` to ask a question that ranges across
  the whole document and let retrieval find the relevant passages.],
)

#subsection("The structured scopes — Facts, Socrates, Editor, Graph")

The last four stops on the ring are a different animal. Where the manuscript
scopes attach #emph[prose from around your cursor], these attach a #emph[curated
body of knowledge] and pre-seed the conversation with a matching system prompt.
And unlike the manuscript scopes, they are #emph[sticky]: they persist across
follow-up prompts until you cycle `F9` away from them, because they open a
standing conversation rather than a one-shot attachment.

#screen(caption: "The four structured scopes")[```
 Scope       Opens a conversation with…
 ─────────   ───────────────────────────────────────────
 Facts       every paragraph of the Facts system book —
             the world's invariants — as ground truth,
             seeded with a fact-analysis system prompt.
 Socrates    the open paragraph read by the active
             Reader Persona (the Inner Socrates).
 Editor      the Inner Editor's craft observations on
             the open paragraph.
 Graph       your knowledge graph — relevant passages
             plus the edges that connect them
             (contradicts / sourced_from / cites / …).
```]

These are the doorways into Inkhaven's reading intelligences from the AI pane, and
each has a whole world behind it. #emph[Facts] turns the AI pane into an
interrogation of your worldbuilding — ask whether a claim holds against the
established canon and the model answers from your Facts book, not from the
internet. #emph[Graph], like `Book`, is retrieval-grounded, but it additionally
folds in the graph relations touching each retrieved passage, so the answer is
grounded in how the book's parts #emph[connect], not just in the prose. #emph[Socrates]
and #emph[Editor] are the conversational faces of the Inner Socrates and Inner
Editor respectively. All four are covered in full in Chapter 10 and the companion
#emph[Know Your Book]; here it is enough to know that `F9` reaches them and that
they stay armed until you cycle past `Graph` back to `None`.

#callout(label: "Sticky vs. resetting")[
  The six manuscript scopes reset to `None` the instant you send — one prompt,
  one attachment. The four structured scopes (`Facts`, `Socrates`, `Editor`,
  `Graph`) stay armed across every follow-up until you press `F9` to leave them.
  The pane title's `scope=` chip is your reminder of which world you are still
  talking to.
]

#section("Mode — local-only RAG versus full knowledge")

Scope decides #emph[what the model sees]. Mode decides #emph[how much of its own
training it may lean on]. There are two settings, and `F10` toggles between them
from any pane.

#screen(caption: "The two inference modes")[```
 Mode    The model is told…                Reach for it when…
 ─────   ───────────────────────────────   ─────────────────────
 Local   use ONLY the supplied context     summarising your own
         and prior chat — refuse rather    canon, fact-checking
         than fall back on general         against worldbuilding,
         knowledge.                        working strictly inside
                                           the manuscript.
 Full    treat the context as ground       brainstorming, craft
         truth where present, but          questions, anything
         general knowledge is fair game.   that benefits from the
                                           model's wider reading.
```]

The default is #emph[Full], because most fresh chats are exploratory and you
#emph[want] the model to suggest a trope, name a craft book, or reach for a
comparison. Flip to #emph[Local] when you are asking the model to work only from
what you have shown it — "summarise what the Facts book says about the northern
climate" should never be answered from the model's guess about real-world
northern climates. The active mode is always shown in the title
(`infer=Local` / `infer=Full`, teal by default), precisely so an accidentally-
armed `Local` mode is never a silent surprise.

#term("Local mode is not privacy")[
  #emph[Local] mode constrains what the model may #emph[draw on] — its own
  training versus your context. It does #emph[not] mean the call stays on your
  machine. With an external provider, a `Local`-mode prompt still travels to that
  provider's servers. On-device privacy is a #emph[provider] choice (Ollama), not
  a mode choice — see the providers section below.
]

Two flows override the toggle. Help inferences (`F1`, or a `Help!` prefix in the
prompt bar) and grammar checks (`F7`) are #emph[pinned to Local] no matter where
`F10` sits — each has its own strict system prompt, and neither should ever
invent a feature or paraphrase a rule from general training data.

#section("Destination — where an answer lands")

An answer in the pane is inert until you decide what to do with it. When an
inference is #emph[done] and has non-empty content, a row of action chips appears
in the pane's footer, and each key sends the response somewhere different. The
chip keys fire only when the #emph[AI pane itself is focused]. After you press
`Enter` to send a prompt, focus stays on the prompt bar, so pressing `r` there
just types the letter — press `Esc` (or `Ctrl+3`) first to focus the pane, then
press the chip key.

#screen(caption: "The apply chips, shown when an inference is done")[```
 r replace   i insert   t top   b bottom   c copy   g grammar
```]

#screen(caption: "What each destination does")[```
 Key   Destination
 ────  ──────────────────────────────────────────────────
 r/R   Replace — overwrite the editor selection with the
       AI text. With no selection, replace the whole
       paragraph.
 i/I   Insert — drop the text in at the cursor.
 t/T   Top — prepend to the paragraph (blank-line gap).
 b/B   Bottom — append to the paragraph.
 c/C   Copy — to the clipboard only; the buffer is not
       touched.
 g/G   Grammar — lift the corrected text from an F7
       grammar-check result (see Chapter 6 / grammar).
```]

Three things hold for every apply. First, #emph[markdown becomes Typst] on the
way in: when the answer lands in the editor (`r`, `i`, `t`, `b`), Inkhaven
converts its markdown to Typst syntax — `#` headings become `=`, `*bold*` stays
`*bold*` (Typst's own bold), `1.` lists become `+`, and so on — so the pasted
text is valid manuscript markup, not raw markdown. Only `c` keeps the raw
markdown, precisely so you can paste it somewhere outside Inkhaven. Second, an
apply sets the buffer #emph[dirty]; press `Ctrl+S` to commit it (or let idle
autosave do so). Third, after `r` / `i` / `t` / `b` the #emph[focus jumps to the
editor] so you land where the change did and can review it in context.

#callout(label: "The grammar chip is special")[
  `g` is not a general "apply". It only works on the output of the `F7` grammar
  check, whose response is wrapped in markers; `g` lifts the corrected text from
  between those markers and applies just that. On any other inference `g` has
  nothing to lift. The full grammar-review flow, with its change highlighting,
  lives in Chapter 6.
]

#section("Chat history — the conversation is continuous")

Every ordinary inference (everything except a one-shot Help or grammar call)
appends a `(you, ai)` turn to an in-memory chat history, and the next prompt
replays the whole history to the model. The conversation is therefore
#emph[continuous]: when you say "now do another pass, especially on the dialogue
tags," the model knows what "this" was because the previous turn is replayed
along with it. The title's `N turn(s)` chip tracks the depth.

A typical multi-turn pass reads like a dialogue with an editor who remembers:

#screen(caption: "A continuous revision loop")[```
 1.  F9 → Paragraph.  Prompt: "Tighten this."  Enter.
 2.  Esc  — focus the pane (chips only fire there).
 3.  r  — replace the paragraph with the tightened text.
 4.  Prompt: "Now another pass, harder on the dialogue
     tags."  Enter.   (History knows what "this" was.)
 5.  Esc, then r  — focus the pane and apply again.
 6.  Ctrl+B C — clear the thread and start fresh.
```]

`Ctrl+B C` clears the chat history along with the currently displayed inference —
the way to start a genuinely fresh conversation when the old context would only
confuse the next question. From the AI pane it doubles as the cancel key for a
stream you no longer want. (Help answers never enter the history at all — they
are deliberately one-shot, so a question about Inkhaven itself never pollutes the
thread you are having about your prose.)

#section("Prompt templates and the Help! prefix")

Two shortcuts live in the prompt bar itself. Typing `/` opens the #emph[prompt
picker], a list of reusable templates drawn from two places: the `prompts.hjson`
file in your project root (shown as #emph[system] prompts) and the paragraphs
under the #emph[Prompts] system book (shown as #emph[book] prompts). Arrow keys
move, `Enter` or `Tab` expands the chosen template into the bar — with any
`{{selection}}` / `{{context}}` substitutions already applied — and you can edit
the expanded text before sending. Direct invocation works too: type `/tighten`
and `Enter` with no picker at all. `Esc` closes it without expanding. The full
template schema is its own subject; the point here is that a prompt you type
often need never be typed twice.

The second shortcut is the `Help!` prefix. Typing `Help! ` followed by a question
routes the rest of the line through the `F1` help-manual flow — a retrieval-
grounded answer over Inkhaven's own Help book, pinned to `Local` so it never
invents a feature — without your leaving the prompt bar. The capitalisation is
exact: `Help!`, not `help!` or `HELP!`, so an actual sentence that happens to
start with the word "help" is never hijacked.

#section("Providers — five pre-configured, and any the router reaches")

Behind the pane sits a #emph[provider]: the company or runtime that actually runs
the model. Inkhaven speaks to all of them through the
#link("https://github.com/jeremychone/rust-genai")[genai] crate, which picks the
right adapter from the #emph[model name] alone — so adding a new model is a matter
of naming it, not writing code. Five providers ship pre-configured in the default
`llm.providers`; Ollama is supported (keyless, local) — add it yourself.

#screen(caption: "The five pre-configured providers")[```
 Provider   Default model        API-key env var
 ────────   ──────────────────   ─────────────────
 gemini     gemini-2.5-pro       GEMINI_API_KEY
 claude     claude-sonnet-4-5    ANTHROPIC_API_KEY
 openai     gpt-4o               OPENAI_API_KEY
 deepseek   deepseek-chat        DEEPSEEK_API_KEY
 grok       grok-2-latest        XAI_API_KEY
```]

You configure them in the `llm` block of `inkhaven.hjson`. Each entry names a
`model` and, for the cloud providers, the environment variable that holds your
key; `default` names which one the TUI uses. Local runtimes like Ollama omit the
key entirely — the auth check is simply skipped when `api_key_env` is absent.

#screen(caption: "The llm config block")[```
llm: {
  default: gemini          # which provider the TUI streams from
  auto_fallback: true      # use any working key if default's unset
  providers: {
    gemini: { model: gemini-2.5-pro, api_key_env: GEMINI_API_KEY }
    claude: { model: claude-sonnet-4-5, api_key_env: ANTHROPIC_API_KEY }
    deepseek: { model: deepseek-chat, api_key_env: DEEPSEEK_API_KEY }
    ollama: { model: llama3.2 }        # no key — runs locally
  }
}
```]

Set a key by exporting its variable before you launch — `export
GEMINI_API_KEY='…'` — and Ollama needs nothing but the daemon running and a model
pulled (`ollama pull llama3.2`). The `default` provider is the one the TUI
streams from; to change it, edit `default` and relaunch. There is no
per-inference provider switch inside the TUI today — the model is a session-level
choice — but the CLI takes a one-shot override: `inkhaven ai "summarise this"
--provider deepseek`.

#term("auto_fallback")[
  When `auto_fallback` is `true` (the default) and your #emph[default] provider's
  key is unset, Inkhaven quietly uses any other configured provider whose key
  #emph[is] available — including a keyless local one like Ollama — rather than
  failing. It is the "use whatever works" setting. Set it to `false` if you want
  the configured provider or a clear error, and nothing else.
]

#callout(label: "Privacy — external providers see your prompts")[
  This is the one privacy fact to internalise. When you use an external provider
  (Gemini, Claude, OpenAI, DeepSeek, Grok), your prompts — including whatever
  manuscript context the scope attaches — #emph[travel to that provider's servers]
  under their terms. Inkhaven adds no inherent privacy over that channel, and
  `Local` mode does not change it. For genuine on-device privacy, set the default
  to `ollama` and run a local model: the prompt never leaves your machine. Every
  #emph[other] Inkhaven subsystem — the RAG embeddings, semantic search, snapshot
  diffs — is already fully local, so switching to Ollama makes the whole tool
  on-device.
]

#section("Prompt-language resolution")

Inkhaven is multilingual to its core, and the AI pane is no exception: a prompt
about a Russian paragraph should be answered in Russian, keyed off Russian word
lists and a Russian-aware system prompt. The dial that decides which language a
prompt resolves against is #emph[prompt-language resolution], set by
`editor.prompt_language_mode` in HJSON and toggled at runtime with
`Ctrl+B Shift+N`.

There are two strategies. #emph[book_defined] (the default) resolves #emph[every]
prompt against the project's top-level `language` field — simplest, and right for
a manuscript written in one language. #emph[paragraph_detected] instead runs the
`whatlang` detector over the #emph[live] paragraph body and resolves against
whatever language it finds — the mode for a genuinely bilingual project where one
chapter is English and the next is Russian. Because `whatlang` is unreliable on
very short text, `paragraph_detected` only attempts detection once the paragraph
has at least `prompt_language_detection_min_chars` of non-whitespace text
(default 50); below that floor it silently falls back to the book language.

`Ctrl+B Shift+N` cycles the mode #emph[for the session] without rewriting your
HJSON: `None` (defer to the config) → `book_defined` → `paragraph_detected` →
`None`. The AI pane title's `lang=` chip reflects the outcome so you always know
which way a prompt was resolved — `ru (book)` when the book language decided it,
`ru (paragraph)` when detection did — and the status bar echoes the new mode as
you toggle.

#screen(caption: "The lang chip records how the language was chosen")[```
┌─ AI · gemini · done · infer=Full · lang=ru (paragraph) ─┐
│  you  Перепиши этот абзац живее.                        │
│  ai   Дождь хлестал наискось с гавани, и Мара …         │
└─────────────────────────────────────────────────────────┘
```]

#recap((
  [The AI feature is a #emph[pair]: the AI pane (answers stream in, history
  scrolls) and the AI prompt bar (you type). `Ctrl+I` focuses the prompt,
  `Ctrl+3` the pane, and `Esc` #emph[bounces] between them; `Ctrl+B K`
  fullscreens the pane.],
  [#emph[Scope] (`F9`) decides what the model sees. Six #emph[manuscript] scopes
  — None / Selection / Paragraph / Subchapter / Chapter / Book — attach prose and
  auto-reset after one send (`Book` is retrieval-grounded). Four #emph[structured]
  scopes — Facts / Socrates / Editor / Graph — open sticky conversations with the
  reading intelligences (Chapter 10).],
  [#emph[Mode] (`F10`) decides how much the model leans on its own training:
  #emph[Local] uses only the supplied context, #emph[Full] (the default) may
  augment with general knowledge. Help and grammar are pinned to Local. Local is
  #emph[not] privacy.],
  [#emph[Destination] chips land a done answer: `r` replace, `i` insert, `t` top,
  `b` bottom, `c` copy (raw markdown), `g` grammar-apply. Editor applies convert
  markdown → Typst and set the buffer dirty.],
  [Six providers are bundled — Gemini, Claude, OpenAI, DeepSeek, Grok, Ollama —
  plus any model genai routes; the `llm` block sets `default`, per-provider models
  and key env vars, and `auto_fallback`. External providers #emph[see your
  prompts]; Ollama keeps them on-device.],
  [Prompt-language resolution (`Ctrl+B Shift+N`) picks the language a prompt
  answers in: #emph[book_defined] (the project language) or
  #emph[paragraph_detected] (`whatlang` over the live paragraph, above a 50-char
  floor); the title's `lang=` chip records which decided.],
))
