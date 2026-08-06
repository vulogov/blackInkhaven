#import "../design.typ": *

#chapter(number: 13, title: "Places and Characters")

A long book is held together by two things it can never afford to get
wrong: *where* it happens and *who* it happens to. Names of towns and
rivers, of protagonists and the people they wrong, recur across
hundreds of pages, and the reader keeps a quiet ledger of every detail.
A city founded in one chapter must not be founded again in another; a
character with grey eyes on page 12 must not have brown ones on page
280. Inkhaven gives you two dedicated places to keep that ledger — the
*Places* book and the *Characters* book — and then it does something no
notebook can: it reads your prose against them, lights up every mention,
and lets you ask an assistant a question grounded in your own canon.

This chapter is the tour of that machinery. It covers what the two
system books are and how you fill them, the coloured overlays that make
your world visible inside the editor, the multilingual stemming that
matches an inflected name to its entry, the two RAG chords that turn a
selected name into a grounded question, and — at an operator's level —
the character-arc tracker that watches a declared arc across a whole
manuscript. It closes on the quiet part: the Characters roster is not a
private notebook feature. Half of Inkhaven's reading intelligences read
it too.

#section("Two books, seeded for you")

Every project Inkhaven creates carries a small block of *system books* —
Notes, Research, Prompts, Places, Characters, Help, and a handful more
depending on your version. They are ordinary books in every visible
respect: they hold chapters, subchapters, and paragraphs, and you edit
their prose exactly as you edit your manuscript. What sets them apart is
that Inkhaven seeded them at `init`, treats them as metadata rather than
manuscript, and wires specific behaviour to their contents.

#term("System book")[
  A book Inkhaven creates and reserves — marked internally by a
  #emph[system tag] (`places`, `characters`, `facts`, and so on). You can
  add, edit, group, and delete the paragraphs #emph[inside] it freely, but
  you cannot delete or rename the book itself. Its contents drive a
  feature rather than appearing in your finished document, and the
  concordance and other prose tools deliberately exclude it from the
  corpus.
]

Places and Characters are the two worldbuilding system books. They sit
near the bottom of the Tree pane, below your own books and above Help:

#screen(caption: "The Tree — your books above, the system block below")[```
  My Novel                 (your book)
  Interludes               (your book)
  ─────────────────────────────────────
  Notes
  Research
  Prompts
  Places          ← cyan overlay · Ctrl+B P
  Characters      ← yellow overlay · Ctrl+B C
  Help
```]

The two books are structurally identical twins. Everything this chapter
says about a Place is true of a Character with three substitutions: the
book, the overlay colour (cyan versus yellow), and the RAG chord
(`Ctrl+B P` versus `Ctrl+B C`). We will lean on the Places book to teach
the mechanics, then note where Characters differs.

#section("Recording a place, recording a person")

An entry is a paragraph, and *the paragraph title is the entity's name.*
That single rule is the whole data model. To add one:

+ Move the tree cursor to the *Places* row and press `→` to expand it.
+ Optionally press `C` first to add a chapter for grouping — `Cities`,
  `Regions`, `Buildings` — if you expect many entries.
+ Press `+` to add a paragraph. The Add modal asks for a title. *Type
  the place name:* `King's Landing`, `Apartment 12`, `Москва`,
  `San Francisco — 1906`.
+ Press Enter; the new paragraph opens in the editor.
+ Type whatever you want to remember — physical description, history,
  who lives there, why it matters to the plot. This is ordinary Typst
  prose: headings (`= History`), bold (`*important*`), lists all work.
+ `Ctrl+S` to save.

You can put paragraphs directly under the book or nest them as deep as
your project's depth allows. Every paragraph anywhere in the subtree is
picked up — there is no separate "register this entry" step; saving is
registration.

#screen(caption: "A Places book that mixes flat and grouped entries")[```
  Places
    Cities                   (chapter)
      Major                  (subchapter)
        Москва               (paragraph)
        Санкт-Петербург      (paragraph)
      Minor
        Воронеж
    Regions                  (chapter)
      Сибирь
    Дача на Волге            (paragraph, directly under Places)
```]

The Characters book works the same way, and the same grouping instinct
applies — most authors organise it by role (Protagonists, Antagonists,
Supporting), by house or family, or by the part of the story where a
character enters. The title is the character's *canonical name*:
`Aragorn`, `Anna Karenina`, `Robb Stark`, `Дмитрий`.

#two_track(
  [Your Characters book is the cast list, and your Places book is the
  map's index. Put voice notes, relationships, secrets, and the chapters
  a character enters and exits into the body — the assistant reads it
  back when you ask, and the arc tracker checks against it.],
  [For non-fiction the same two books hold recurring *entities*: the
  people your argument keeps naming and the sites, institutions, or
  regions it returns to. The overlay then doubles as a consistency
  check — a name you spelled two ways stops lighting up on the variant.],
)

#callout(label: "One entry per paragraph")[
  Keep entries atomic. If you find yourself describing two locations in
  one paragraph, split them — the overlay, the RAG lookup, and the arc
  tracker all key off the paragraph title, so a paragraph that covers two
  things is only ever found under one name.
]

#section("The highlight overlays")

Open any paragraph in your manuscript and look at it. Every word that
matches a recorded Place name renders in *cyan and bold*; every word
that matches a Character name renders in *yellow and bold*. Your world
becomes visible in the prose without any tagging on your part — the
lexicon is compiled from the two books' titles and applied on render.

Because a monospace page cannot show colour, picture the overlay as a
highlight laid over the matching words:

#screen(caption: "A manuscript paragraph, mentions lit by the overlay")[```
  Editor · manuscript / ch.03 / 02-arrival

  Из Москвы поезд шёл всю ночь. К утру Анна
  снова была в Москве, и город встретил её
  дождём.

  lit cyan   → Москвы · Москве   (Place: Москва)
  lit yellow → Анна              (Character: Анна)
```]

Note what happened there. The Place entry is titled `Москва`, but the
prose says `Москвы` and `Москве` — two inflected forms — and both light
up anyway. That is the *stemming*, covered next. Note too the collision
rule: when a Place name and a Character name would land on the same
word, *Place wins by design*. This matters for surnames that are also
place names; if it bites you, give one of the two a more distinctive
title.

The overlay refreshes at three moments, so it is always current:

- *Live as you type* — each render re-checks the visible buffer against
  the lexicon, so a sentence you just wrote lights up immediately.
- *On every save* — adding a new entry and pressing `Ctrl+S` starts
  highlighting that name everywhere else at once.
- *On project open* — the lexicon is compiled from scratch at startup.

The colours are yours to change. In `inkhaven.hjson`:

#screen(caption: "Overriding the overlay colours")[```hjson
  theme: {
    places_fg:     "#a6e3a1"   # green instead of cyan
    characters_fg: "#fab387"   # peach instead of yellow
  }
```]

The defaults are `#89dceb` (sky blue) for Places and `#f9e2af` (yellow)
for Characters. Restart the TUI to pick up a change.

#subsection("Stemming: one entry, every form")

Names inflect. English is gentle — `city` and `cities`, `Aragorn` and
`Aragorn's` — but a language like Russian gives a single name six or
more surface forms: `Анна`, `Анне`, `Анной`, `Анну`, `Анны`. You cannot
be expected to file a separate entry for every grammatical case. So the
matcher does not compare words literally; it compares *stems*.

Inkhaven runs Snowball stemmers (from the `rust-stemmers` crate), which
reduce an inflected word to its root. The entry `Москва` and the prose
word `Москве` stem to the same root, so the overlay lights the prose
word even though it never matches the title character-for-character.

The stemmer language is driven, in order, by:

+ The top-level `language` field in `inkhaven.hjson` — it wins whenever
  it is non-empty. The default is `"english"`.
+ `editor.stemming.languages` — a fallback list, used only when
  `language` is empty, that runs several stemmers at once.

Set `language` to the dominant language of your manuscript:

#screen(caption: "A Russian project — one line does it")[```hjson
  language: russian
```]

The supported set is `arabic`, `danish`, `dutch`, `english`, `finnish`,
`french`, `german`, `greek`, `hungarian`, `italian`, `norwegian`,
`portuguese`, `romanian`, `russian`, `spanish`, `swedish`, `tamil`, and
`turkish`. If you write in a language not on the list, set `language: ""`
to disable stemming — the overlay then falls back to *exact* word
matches, which is correct for an uninflected language and safe for any.

Multi-word titles are handled as a *sequence of stems*. `King's Landing`,
`North Tower`, `Анна Каренина` — the matcher splits the title into
tokens, stems each independently, and looks for the same run of stems in
the prose. So `Анна Каренина` matches the full name in any case. It does
*not*, however, match the parts in isolation: an entry for
`Anna Karenina` lights the full name but not a standalone `Anna`. If
both forms appear often, add a second short entry (`Anna`) or title the
entry with the form the prose actually uses most.

#callout(label: "Does it work in Russian?")[
  Yes — that is the point of stemming. The overlay, the multi-word
  matcher, and the RAG lookups all key off the project `language`, so a
  Russian, German, French, or Spanish project gets inflection-aware
  matching with no extra configuration. For a bilingual project (say,
  translation work) set `language: ""` and list both stemmers under
  `editor.stemming.languages` so a `Moscow` entry lights English
  inflections and a `Москва` entry lights Russian ones.
]

#section("Asking the AI: Ctrl+B P and Ctrl+B C")

The overlay is passive — it shows you what you have recorded, but it
never talks back. To *ask* the assistant about an entry, using your own
canon as the ground truth, use the two RAG chords. They are the active
half of this feature.

#term("RAG context")[
  Retrieval-Augmented Generation: before the model answers, Inkhaven
  retrieves the relevant entries from your Places or Characters book and
  prepends them to the prompt. The model answers from #emph[your] text,
  not from whatever it half-remembers about a real or famous place. It is
  how you keep the assistant inside your canon.
]

The flow is the same for both chords:

+ In the editor, select the name in your prose, or just place the cursor
  inside the word.
+ Press `Ctrl+B P` for a Place, or `Ctrl+B C` for a Character.

Inkhaven sweeps the matching book for every paragraph whose title
contains your term (case-insensitive), builds a context block from their
bodies, and then behaves one of two ways depending on the AI prompt bar:

- *If the prompt bar is empty*, the context is *stashed* as the next RAG
  prefix and focus jumps to the prompt bar. The status line reads
  `Place RAG armed for 'Москва' — type your question and Enter`. Type
  your question and press Enter; the model answers using the stash.
- *If the prompt bar already has text*, the inference fires
  *immediately* with the context prepended, and focus moves to the AI
  pane so you can watch the answer stream in.

The block the model receives looks like this:

#screen(caption: "The context block prepended to your question")[```
  ── Place context for `Москва` (1 match(es)) ──

  ── Place: Москва ──
  = Москва

  = History
  Столица России. Основана в 1147 году.

  = Geography
  На реке Москве, в Центральной России.
  ── end place ──
```]

Two dials sharpen the answer. `F10` toggles the inference mode: with
*Local*, the model is constrained to use *only* the context you supplied
— ideal when you want a faithful summary of your canon and nothing
invented from training data. Switch to *Full* if you want the model to
reach beyond the entry. And `F9` sets the *scope* — combine it with the
RAG chord to give the model both the surrounding chapter and the entry:
put the cursor in a city's name, press `F9` to widen scope to Paragraph
or Chapter, then `Ctrl+B P`, and ask "how does she feel about being
back?" The model sees the scene *and* the city's recorded history.

If your selection matches no entry, the status reports `Place RAG: no
entry titled like 'XYZ' in the Places book` and nothing fires — add the
entry first. The chords route through the same meta-prefix dispatcher, so
they work from any focus, but selecting in the editor first is the
natural path.

#chord_table((
  ("Ctrl+B P", "Place RAG — sweep the Places book for the selected name, arm or fire a grounded question."),
  ("Ctrl+B C", "Character RAG — the same against the Characters book."),
  ("F10", "Toggle inference mode Local ↔ Full (Local = answer only from the stashed context)."),
  ("F9", "Cycle the AI scope; stack it under a RAG chord for scene + entry grounding."),
))

#callout(label: "Pronouns are not names")[
  The lexicon is a noun-phrase index, not a coreference resolver. It
  highlights literal titles (stemmed), so pronouns — `he`, `she`,
  `они` — are never lit and aliases need their own entries. For a "who
  is she here?" question, widen `F9` scope so the model has the passage
  to reason over, then ask in the chat.
]

#section("Character arcs — a first tour (CHAR-1)")

The Characters book earns a second life as the ground truth for
Inkhaven's *character-arc tracker*. This is a large feature with its own
deep treatment in the #emph[Know Your Book] companion; here we give it an
operator's tour — enough to run it and read its output, with pointers to
where the full story lives.

The premise: you *declare* the arc you intend, and Inkhaven watches the
manuscript to see whether the prose delivers it. You declare an arc by
adding a `character_arc` block to a Characters-book entry, written as
HJSON in the paragraph body:

#screen(caption: "A declared arc in a Characters entry body")[```hjson
  {
    character_arc: {
      arc_type: "positive_change"
      desired_state_start:    "Mara defers to her family."
      desired_midpoint_state: "Mara's first open defiance."
      desired_state_end:      "Mara acts without permission."
    }
  }
```]

The `arc_type` names one of five structural shapes the tracker
understands — `positive_change` (a false belief overcome), `flat` (the
character holds firm while the world changes), `corruption`, `fall`, and
`disillusionment` — plus any other string, which falls back to a generic
probe. Only `desired_state_start` and `desired_state_end` are required;
the midpoint is optional. Omit the whole block and the entry simply has
no arc to check.

Everything else is computed from the prose. Three commands drive it from
the terminal:

- `inkhaven character refresh` recomputes the *agency score* (fast,
  deterministic) and re-extracts the chapter-by-chapter *observable-state
  chain* (an LLM pass, run lazily — only chapters whose text changed are
  re-extracted). `--name` limits it to one character.
- `inkhaven character check` runs the *completeness checks* against the
  declaration and is built to be a pre-submission gate: it exits `1` on
  any gap or stall and `2` if the ending or the earned-arc check fails,
  so you can wire it into a script. `--json` emits machine-readable
  findings.
- `inkhaven character plan` reports *Planning-Board coverage gaps*
  deterministically — a declared arc that no scene card names, or one
  confined to the book's first half. It exits `1` on any gap.

#term("Agency score")[
  A deterministic, zero-cost number between 0 and 1 for a character in a
  chapter: the share of their appearances where they #emph[act] (their
  name precedes a transitive action verb) versus where they are
  #emph[acted upon] (a passive construction, or their name follows the
  verb). Computed from a per-language action-verb list — English,
  Russian, German, French, and Spanish ship built in, and you can fold
  genre verbs in via `char.extra_action_verbs`. A protagonist whose
  agency sags for a stretch of chapters is a protagonist going passive.
]

The stall detector is the other deterministic half: it finds the longest
run of chapters where the character shows *no observable change* after
their baseline appearance, and flags it when the run reaches
`char.stall_threshold` (default 4). The four alignment and earned checks
— start, midpoint, end, and "was this arc earned?" — are the LLM half,
judged only on observable evidence in the prose and run only for
characters with a declaration and at least `char.min_chapters_for_check`
(default 3) chapters of extracted state.

`inkhaven character arc <name>` prints the whole picture read-only from
the cached `char.duckdb` — declaration, state chain, agency, stalls,
checks, and planning gaps together:

#screen(caption: "inkhaven character arc Mara")[```
  Character arc — Mara · `My Novel`
  ────────────────────────────────────────────────
  Declared arc: positive_change
    start    : Mara defers to her family.
    midpoint : Mara's first open defiance.
    end      : Mara acts without permission.

  State chain (6 chapters):
    · ch. 1  agency 0.42  (3a/4p)  Silent at dinner.
    · ch. 2  agency 0.38  (2a/3p)  Runs an errand.
    ✦ ch. 4  agency 0.61  (5a/3p)  Refuses to lie.
    · ch. 5  agency 0.55  (4a/3p)  Holds the line.

  Arc checks:
    [earned] Arc earned — the refusal in ch.4 is
             seeded by the small evasions before it.
  ────────────────────────────────────────────────
```]

A `✦` marks a chapter where the character's state changed; a `·` marks
one where it held. Inside the editor the same view lives one chord away:
*`Ctrl+V Shift+N`* opens the arc modal for the nearest character (one
named in the open paragraph, else the first tracked one) — the declared
arc, the state chain with its agency scores, the completeness checks, and
any planning gaps, scrollable with `↑↓`, `Esc` to close. It is read-only
over the cache, so populate the cache first with a `refresh`/`check`, or
run the zero-AI `Ctrl+B Shift+C` review pass, whose deterministic arc
findings also land in the Output pane's `character` category.

#callout(label: "Where the full treatment lives")[
  This is the operator's tour. The arc taxonomy in depth, the agency
  windows and cross-system enrichment signals, the exact wording of the
  LLM checks, and the Planning-Board integration are covered in the
  #emph[Know Your Book] companion and in Chapter 17 (Continuity and
  Knowledge). The `char:` config block tunes every threshold; see the
  configuration reference.
]

#section("How the roster feeds the rest of Inkhaven")

Here is the quiet payoff. The Characters book is not a private notebook
that only the overlay and the RAG chord read. The list of entry titles —
the *roster* — is the canonical cast of the whole project, and a
surprising number of Inkhaven's reading intelligences read it:

- *Continuity* (SENTINEL, Chapter 17) builds its "referenced-before-
  introduced" invariant on the roster, and the status-bar POV chip names
  the most-mentioned rostered character in the open paragraph.
- *Knowledge* (KEN, Chapter 17) grants each rostered character an
  epistemic timeline — who could know what, and when — keyed off their
  presence in timeline events, so a name absent from the roster gets no
  grant.
- *Voice at scale* (CHORUS, Chapter 18) resolves a scene's POV to the
  most-mentioned rostered character and checks each one's distinct voice
  against the others.
- *Dialogue attribution* resolves "who said this" to a roster-canonical
  name, and the arc tracker's agency and stall scoring rides on the same
  roster.

The lesson is that filling in the Characters book is not busywork for one
feature — it is the single act that turns on continuity, knowledge,
voice, dialogue, and arc tracking together. A name you never record is a
name none of them can watch. This is why the roster is drawn from the
entry *titles* directly: record a character once, as a paragraph, and the
whole reading apparatus knows who they are.

#recap((
  [The *Places* and *Characters* system books are seeded at `init`; you
  record an entity as a paragraph whose *title is its name*, and saving
  is registering.],
  [Mentions light up in the editor — *cyan* for Places, *yellow* for
  Characters — with *Snowball stemming* keyed to the project `language`,
  so one entry matches every inflected form; Place wins a collision.],
  [`Ctrl+B P` and `Ctrl+B C` sweep the matching book and *arm or fire* a
  question grounded in your own canon; `F10` Local keeps the answer inside
  that context, and `F9` scope stacks scene grounding on top.],
  [The arc tracker (CHAR-1) checks a declared `character_arc` block against
  the prose: `inkhaven character refresh` / `check` / `plan` / `arc` and
  the `Ctrl+V Shift+N` modal, with a deterministic *agency score* and
  *stall* detector plus LLM completeness checks.],
  [The Characters *roster* is shared ground truth — *continuity, knowledge,
  voice, dialogue, and arcs* all read it, so one entry switches on many
  readers at once.],
))
