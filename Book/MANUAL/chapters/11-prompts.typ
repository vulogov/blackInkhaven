#import "../design.typ": *

#chapter(number: 11, title: "Reusable Prompts")

You will find yourself typing the same instruction to the assistant again and
again. _Tighten this without changing the meaning. Make the tone darker but
keep every fact. Read this paragraph for places I am telling instead of
showing._ A good instruction is a small piece of craft — it took you three
tries to word it so the model stops adding new plot — and retyping it from
memory each time is both a chore and a slow erosion of the phrasing that
worked. Inkhaven's answer is the *reusable prompt*: write the instruction once,
give it a short name, and from then on summon it in two keystrokes, with the
passage you are working on already stitched in. This chapter is the whole of
that system — where prompts live, how you reach them, what gets substituted
into them, and how the same machinery quietly powers the assistant's built-in
passes like grammar-check and fact-check.

#section("Why keep a prompt around")

A reusable prompt is nothing more than a saved block of text with two
conveniences bolted on. First, it has a *name* you can type — `/tighten`,
`/darker` — so you never scroll a history buffer looking for the wording you
liked. Second, it carries *placeholders* that Inkhaven fills in at the moment
you send it, so the same template works on whatever paragraph is open without
you pasting anything. Between those two, a prompt you tuned once becomes a
verb: you _tighten_ a passage the way you _save_ a file.

#term("Prompt")[
  A named, reusable instruction to the AI assistant. It is plain text plus the
  placeholders `{{selection}}` and `{{context}}`. When you dispatch it, the
  placeholders are replaced with the passage you are working on and its
  location in the book, and the result lands in the AI prompt bar ready to
  send — nothing is sent until you press Enter.
]

Prompts are advisory, like everything the assistant does. Expanding a prompt
fills the input bar; it never edits your prose on its own. You see the fully
substituted text, you can edit it, and only your Enter sends it. That is the
same contract the whole AI layer honours (Chapter 9): the model proposes, you
dispose.

#section("Two libraries: the file and the book")

Inkhaven draws prompts from two places at once, and the picker shows them
merged into a single list.

The first is a plain file, `prompts.hjson`, in the root of your project. These
are your *system prompts* — portable, versionable templates you carry between
books. `inkhaven init` seeds the file from a shipped default, and you edit it
with any text editor (or the built-in editors described later). In the picker
they wear a cyan `[ system ]` chip.

The second is a system book named *Prompts*, sitting in the Tree alongside
Notes, Facts, and the rest. Every paragraph you file under it becomes a prompt.
These are your *book prompts* — project-local, authored in the TUI like any
other paragraph, and carried inside the manuscript itself. They wear a green
`[ book ]` chip.

#term("The Prompts system book")[
  One of Inkhaven's built-in system books. Paragraphs nested under it are not
  prose — they are prompt templates. A paragraph's *slug* supplies its
  `/name`; its *title* supplies the description shown in the picker; its body
  is the template. System books are excluded from the manuscript corpus (word
  counts, concordance, exports), so your prompts never leak into the book.
]

The two libraries serve different instincts. Keep in `prompts.hjson` the
*signature* templates you want in every project — your house style for
tightening, your favourite critique framing. Keep in the Prompts book the
templates that only make sense _here_, because they lean on this manuscript's
voice, its glossary, its world. When the two disagree — a `/tighten` in both —
the book copy wins, on the principle that the project-local, hand-authored
version is the more deliberate one. The exact ladder is spelled out under
_Localized prompts_ below, because it interleaves with language; the short form
is *Prompts book, then `prompts.hjson`, then Inkhaven's embedded default*.

#section("Opening the picker")

There is no chord for the prompt picker, and you do not need one: it is bound
to the character that already means "I am about to name something," the forward
slash. Focus the AI prompt bar with `Ctrl+I` and type `/`. A floating magenta
panel opens just above the bar, listing every prompt from both libraries. Keep
typing and the list filters live — the text after the slash is matched, case
insensitively, against both the name and the description of each prompt.

#screen(caption: "The prompt picker, filtered to `/ti`")[```
  +-- Prompts -----------------------------------------------+
  |  [ system ]  /tighten                                    |
  |             Tighten the prose without changing meaning   |
  |  [ book ]    /tighten-quay-voice                         |
  |             Tighten, in the harbour-chapter register     |
  |  [ system ]  /timeline-health                            |
  |             Review the story timeline for consistency    |
  +----------------------------------------------------------+
  AI prompt >  /ti|
```]

The panel is routed to the prompt bar — it has no separate focus, so the same
keys that edit the bar also drive the list.

#chord_table((
  chord_row("Ctrl+I", "Focus the AI prompt bar (from any pane)."),
  chord_row("/", "As the first character, opens the picker; refiltered as you type."),
  chord_row("Up / Down", "Move the selection through the list."),
  chord_row("Enter / Tab", "Expand the selected template into the bar (substituted)."),
  chord_row("Esc", "Close the picker without expanding anything."),
))

Selecting a prompt with Enter or Tab does *not* send it. It expands the
template — running the substitutions, stripping any editor chrome — and drops
the finished text into the prompt bar, leaving the picker. Now you can read
exactly what the model will receive, edit it (append a one-off note, delete a
clause), and press Enter when you are satisfied. The status line confirms the
pick: `loaded prompt `tighten` [system] — Enter to send`.

When the filter is empty the list keeps its natural order — system prompts
first, then book prompts. As you type, matches are ranked: a prompt whose
*name* begins with what you typed sorts above one where a later *description*
word begins with it, which sorts above a mere substring hit anywhere. So `/ti`
surfaces `tighten` before it surfaces a prompt merely _described_ as "review
the timeline."

#section("Direct invocation: slash a name")

Once a prompt's name is in your fingers, you can skip the picker entirely. Type
the whole name yourself and send:

#screen(caption: "Dispatching a prompt by name")[```
  AI prompt >  /tighten
```]

Enter looks the name up — system library first, then the Prompts book by slug
or title — expands the template against the current selection, and sends in one
motion. If nothing matches, the input is left alone and the status line says
`no prompt `tighten` — type / to see the list`, so a typo never fires a blind
inference.

Two subtleties are worth knowing. First, text you type _after_ the name is
ignored by the resolver — the template is what gets sent, not your trailing
words. To add a one-off note, expand the prompt into the bar (Enter inside the
picker) and edit it there before submitting. Second, two prefixes are special
and are *not* prompts: a leading `Help!` (case-sensitive) routes the rest of
the line through the grounded Help search (Chapter 9), and a bare line with no
leading slash is simply sent as an ordinary question.

#section("The substitutions")

A template is plain text until you dispatch it, at which point Inkhaven
replaces two placeholders. There are exactly two — no others — and both are
filled from the editor state at the instant you expand the prompt.

#term("{{selection}}")[
  The text you are working on. If you have an active selection in the editor,
  that selected span is substituted verbatim. If nothing is selected, the
  *entire open paragraph* is substituted instead. This is the working text
  most prompts operate on, so a prompt "just works" whether you highlighted a
  sentence or want the whole paragraph.
]

#term("{{context}}")[
  A breadcrumb of titles locating the open paragraph in the book, joined with
  ` › ` — for example `Rain › The City › the-quay`. It lets you tell the model
  _where_ in the manuscript you are without flooding it with the surrounding
  prose. Cheap orientation, a few tokens, no retrieval.
]

You can use either, both, or neither. A template frames them however you like;
a common shape wraps the working text in visible fences so the model can see
exactly where it starts and stops:

#screen(caption: "A template using both placeholders")[```
  Context: {{context}}

  Tighten the passage below without changing meaning or
  voice. Aim for 10-20% fewer words. Keep Typst markup
  verbatim.

  --- begin
  {{selection}}
  --- end
```]

The substitution happens once, in memory, the moment you expand or dispatch the
prompt. What the model receives is finished plain text with the placeholders
already gone; the leading and trailing whitespace of the whole template is
trimmed. Inkhaven never rewrites your `prompts.hjson` or your book paragraphs
to do this — the source templates are untouched.

#callout(label: "Selection beats paragraph")[
  Because an empty selection falls back to the whole paragraph, you rarely need
  to select anything for a paragraph-level task. Reach for a selection only
  when you want the model to work on _part_ of the paragraph — a single line of
  dialogue, one clause — and leave the rest out of its view.
]

#section("System prompts in prompts.hjson")

The system library lives at `<project-root>/prompts.hjson`. It is HJSON — JSON
with comments, unquoted keys, and triple-quoted multi-line strings — and its
shape is a single `prompts` array of entries:

#screen(caption: "prompts.hjson — one entry")[```
  {
    prompts: [
      {
        name: "darker"
        description: "Make the tone darker, keep facts"
        template: '''
          Rewrite the passage in a darker tone -- somber
          palette, unease, understated dread. Keep every
          fact, name, and timeline intact. Preserve Typst
          markup.

          Context: {{context}}

          --- begin
          {{selection}}
          --- end
        '''
      }
    ]
  }
```]

Each entry has three required fields and one optional one.

#chord_table((
  chord_row("name", "The /name identifier. Lowercase, no spaces; hyphens are fine."),
  chord_row("description", "The one-line blurb shown in the picker. Keep it scannable."),
  chord_row("template", "The prompt body. Use ''' triple-quotes for multi-line text."),
  chord_row("language", "Optional ISO 639-1 code (en, ru, fr, de, es) — see below."),
))

To add a prompt, append an entry to the array and save. Inkhaven reads
`prompts.hjson` when the TUI launches; it does *not* watch the file for live
edits, so close and reopen the project (or restart the binary) to pick up a
change. A malformed file is reported at load and the library falls back empty
rather than crashing — fix the HJSON and relaunch.

Because the file is plain text in your project root, it versions with the rest
of your work: commit it, diff it, branch it. That is exactly why it is the
right home for the templates you want to _travel_ between books.

#section("Book prompts in the Prompts book")

A book prompt is authored the way you author anything in Inkhaven — as a
paragraph. Navigate to the *Prompts* system book in the Tree, add a paragraph
(`+`, or `P` to insert after the cursor), give it a title in the Add modal,
and write the body in the editor. Save with `Ctrl+S` and it appears in the
picker immediately — no restart, because the book prompts are read live from
the store on every keystroke in the picker.

Two fields fall out of the paragraph automatically. Its *slug* — the
filesystem-safe form of the title — becomes the `/name`. Its *title* becomes
the picker description. A title of `Worldbuilding consistency pass` slugs to
`worldbuilding-consistency-pass`, so typing `/world` filters straight to it.

#callout(label: "The heading is stripped for you")[
  Inkhaven seeds a new paragraph with a `= Title` Typst heading line. When a
  book prompt is expanded, that leading heading — and the blank lines after it
  — are removed before the template is used, so the model never sees editor
  chrome, only your instruction. Write the prompt body under the heading and
  forget the heading is there.
]

A book prompt earns its place over a file entry in four ways. It *travels with
the manuscript*: anyone who clones the project gets your prompts for free, no
side file to remember. It is *editable in the TUI*, with no round-trip through
an external editor. It is *indexed like prose*, so semantic search can surface
it. And it is *snapshot-able* — `F5` captures a version before you tune a
prompt, `F6` picks an older one back out — so your best wording is never one
careless edit from lost.

#section("Localized prompts")

Every AI feature in Inkhaven keys off the project's working language, and
prompts are no exception. A prompt can be tagged with the language it was
written for, and the resolver prefers the tag that matches the language you are
writing in.

On a `prompts.hjson` entry the tag is the optional `language` field, an ISO
639-1 code. On a book prompt it is a `lang:<code>` tag on the paragraph — for
instance `lang:ru`. Untagged prompts (every prompt from before this feature
existed, and every one you never bother to tag) are treated as language-neutral
and always remain eligible.

When a name is resolved — whether from `/name`, the picker, or a built-in pass
— Inkhaven walks three passes, and within each pass it checks the Prompts book
before `prompts.hjson`, and tries both the hyphenated and the spaced form of
the name (`grammar-check` and `grammar check`):

#screen(caption: "The three-pass resolution ladder")[```
  Pass 1  strict   prompt tagged for the ACTIVE language
                     Prompts book  ->  prompts.hjson
  Pass 2  untagged  prompt with NO language tag (back-compat)
                     Prompts book  ->  prompts.hjson
  Pass 3  any       prompt tagged for ANY other language
                     Prompts book  ->  prompts.hjson
                     ... then the embedded default (English floor)
```]

So a Russian `grammar-check` beats an untagged one, which beats an
English-tagged one, which beats Inkhaven's built-in English fallback — and at
every rung a book prompt outranks a file prompt. This is the full form of the
_book, then file, then embedded_ precedence mentioned earlier: language sorts
first, then source.

Which language counts as "active" is itself controllable. `Ctrl+B Shift+N`
cycles the prompt-language mode through three settings: deferring to the
`editor.prompt_language_mode` value in your project HJSON, using the project's
declared `language` for every resolution (`book_defined`), or detecting the
language of the live paragraph and resolving to _that_ (`paragraph_detected`).
The AI pane's title bar shows a `lang=` chip — `ru (book)` versus
`ru (paragraph)` — so you always know which language the next prompt will
resolve against.

#callout(label: "Bootstrapping a whole language at once")[
  You do not have to translate the built-in prompts by hand. From the command
  line, `inkhaven prompts bootstrap russian` asks your configured model to
  produce Russian variants of the seven embedded prompts and prints a ready-to-
  paste HJSON snippet; add `--update` to merge them into `prompts.hjson` in
  place (with a timestamped backup, never clobbering your own entries). Covered
  under _From the command line_ below.
]

#section("Named flows and the resolution ladder")

The same resolver that answers `/tighten` also supplies the prompts behind
Inkhaven's built-in AI passes — and that is deliberate, because it means every
one of them is *customisable* by exactly the mechanism you have just learned.
A pass looks up a well-known name, and if you have defined a prompt of that
name (in the Prompts book, or in `prompts.hjson`, in the language you are
working in), yours is used instead of the shipped default.

The clearest example is fact-check (`Ctrl+B Shift+X`, Chapter 14). It resolves
the name `fact-check` through the ladder: a `fact-check` paragraph in the
Prompts book, then a `fact-check` entry in `prompts.hjson`, then the embedded
multilingual default. Override the first two to bend the house style of the
check; leave them absent and the built-in prompt runs. Grammar-check (`F7`)
works the same way against the name `grammar-check`, as do the show-don't-tell
scan, the sentence-rhythm rewrite, the diagnostic explainer, the critique
passes, and the timeline-health review.

#screen(caption: "The eight embedded prompt names you can override")[```
  grammar-check            F7 copy-edit pass
  show-dont-tell           Ctrl+B Shift+T telling-vs-showing scan
  sentence-rhythm-rewrite  Ctrl+B Shift+M rhythm rewrite
  explain-diagnostic       Ctrl+F12 explain a Typst error
  critique-edit            F12 weakest-elements critique
  critique-changes         F12 evaluate a revision
  critique-compare         F12 compare two paragraphs
  timeline-health          Timeline modal consistency review
```]

Two flows deliberately stand outside this system. The Help question (`F1`, or a
`Help!` prefix) uses a fixed strict-grounding prompt pinned to Local mode, so
the model can never invent a feature — it is not yours to override. And the
grammar-check apply key expects the corrected text between
`<<<CORRECTED>>>` / `<<<END>>>` markers; if you _do_ override the
`grammar-check` prompt, keep those markers in your version or the one-key apply
will have nothing to extract.

#section("From the command line")

Two subcommands touch prompts from outside the editor.

`inkhaven prompts bootstrap <language>` is the localizer described above. It
sends the eight embedded prompt names to your configured model, asks for
faithful variants in the target language (preserving Typst tokens and the
`{{selection}}` / `{{context}}` placeholders verbatim), and prints an HJSON
snippet to standard output. Nothing is written to your project unless you pass
`--update`, which merges the results into `prompts.hjson` in place — overwriting
only same-name, same-language entries, appending the rest, and leaving a
timestamped backup under `.config-backups/`. An optional `--genre` biases word
choice toward the register you write in.

`inkhaven prompts-editor` launches a standalone four-pane workbench for
`prompts.hjson` alone: the prompt list on the left, an editor with the same
chord set as the main paragraph editor in the centre, an AI response pane on
the right, and a prompt bar across the bottom — a focused place to draft and
review your file prompts without the whole manuscript around them.

#callout(label: "Editing the file inside the TUI")[
  You can also open `prompts.hjson` — and the rest of the project config — from
  inside a running session with `Ctrl+B 0`, the in-app HJSON editor. It
  syntax-highlights, saves atomically, and warns you that prompt changes take
  effect on the next launch. For a book prompt there is no such delay: it is
  live the moment you save the paragraph.
]

#section("Scripting: setting the system prompt")

There is no `ink.` Bund word for the prompt _picker_ — the picker is a
keyboard affordance, not a scripted surface. The one prompt-adjacent scripting
hook is `ink.ai.set_system_prompt`, which sets the *system* message the
assistant runs under (distinct from the reusable templates this chapter is
about). It takes one string off the stack:

#screen(caption: "Overriding the system prompt from Bund")[```
  "You are a terse copy-editor. Never explain."
  ink.ai.set_system_prompt

  ""  ink.ai.set_system_prompt   \ empty string clears it
```]

An empty string clears the override and the assistant falls back to the default
system prompt for the current inference mode (Local versus Full, Chapter 9).
This is a runtime, session-local change — it is not persisted, and it does not
touch your prompt libraries. Use it in a Bund macro that puts the assistant in
a specific frame of mind before a batch of work; reach for reusable prompts
when you want a named, substitutable _instruction_ instead.

#section("How prompts sit beside scope and mode")

Reusable prompts are one of three orthogonal dials on what the assistant sees,
and it helps to hold them apart. A *prompt* supplies the instruction and, via
its placeholders, the working text and its location. *Scope* (`F9`) prepends a
separate context block — the selection, the paragraph, the chapter, the whole
retrieval-grounded book — _above_ your prompt; the model sees both. *Inference
mode* (`F10`) swaps the underlying system prompt, forbidding or permitting
general knowledge.

For a one-shot task — tighten this, run a grammar check — the prompt's own
`{{selection}}` is usually all you need, and you can leave scope at None. For an
ongoing conversation about the work — brainstorm a subplot, plan a chapter —
lean on scope and let chat history accumulate instead, clearing it with
`Ctrl+B C` when you want a fresh thread. Prompts and scope compose cleanly; you
will reach for one, the other, or both as the task asks.

#recap((
  [A *reusable prompt* is a named instruction with two placeholders; expanding
  it fills the AI prompt bar but never sends until you press Enter.],
  [Prompts come from two libraries: portable *system prompts* in
  `prompts.hjson` (cyan `[ system ]`) and project-local *book prompts* under
  the Prompts system book (green `[ book ]`).],
  [Open the picker by focusing the bar with `Ctrl+I` and typing `/` — there is
  no chord; `Enter`/`Tab` expand, `Esc` closes — or dispatch a template
  directly by typing `/name`.],
  [There are exactly two substitutions: `{{selection}}` (the selection, or the
  whole open paragraph) and `{{context}}` (the title breadcrumb).],
  [Resolution runs *Prompts book, then `prompts.hjson`, then the embedded
  default*, and within that a language ladder of strict → untagged → any keys
  the active-language variant first.],
  [Built-in passes like fact-check and grammar-check resolve named prompts
  through the same ladder, so you can override any of them; `inkhaven prompts
  bootstrap` and `inkhaven prompts-editor` manage the file from the command
  line.],
))
