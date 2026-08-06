#import "../design.typ": *

#chapter(number: 8, title: "The Style Overlays")

The last chapter left you writing prose into the Editor. This one is about the
faint marks that appear *underneath* it — amber underlines beneath a hedge word,
a purple one beneath a word you have used three times in as many paragraphs, a
teal one beneath "was angry," a chip in the status bar naming whose eyes you are
seeing through. Inkhaven calls these the *style overlays*, and they are the most
easily misunderstood thing in the whole instrument, so let us be plain at the
outset about what they are for.

They are not a grammar checker, and they are certainly not a style *authority*.
Every one of them is a question the tool poses and then leaves entirely to you:
*you used "just" here — did you mean to?* The overlay has no opinion about the
answer. A filter word is sometimes exactly the right word; a repeated image is
sometimes a deliberate refrain; a line of pure telling is sometimes the fastest
way across a dull stretch of plot. The overlays exist to make those choices
*visible* so they become choices at all, rather than habits you never see. The
whole family is built on one principle, and it is worth stating in its own box
before we open a single one.

#term("An overlay is advisory")[
  A *style overlay* marks a pattern in your prose — a word, a phrase, a
  construction — that *often* rewards a second look. It never changes a
  character of your text, never blocks a save, never scores you. It is a
  coaching mark you toggle on when you want the second pair of eyes and off when
  you want the page clean. You question it; you do not obey it.
]

Two more things are true of the whole family, and both matter enough to say up
front. First, every overlay is *multilingual* — its word lists and its stemming
are keyed to the project's top-level `language` field, so a Russian manuscript
is measured against Russian hedges and a French one against French, never
against English words bleeding into foreign prose. Second, every overlay has a
*master switch* in `inkhaven.hjson` and a *session-local chord* that flips it
for the current run without touching the file. The chords are how you audition
an overlay; the config is how you decide, once, that you want it on for good. We
will meet each overlay in turn and close on the switchboard that governs them
all.

#section("The filter-word overlay — Ctrl+B Shift+F")

The oldest and busiest of the overlays flags *filter words*: the intensifiers
and hedges that leak into a first draft and dilute it — `just`, `really`, `very`,
`quite`, `actually`, `simply`, `seem`, `perhaps`, and their kin. Press
`Ctrl+B Shift+F` in the Editor and every one of them in the open paragraph gains
a faint amber underline. Press it again and they vanish. That is the whole
gesture; the depth is in what gets underlined, and in which language.

#term("Filter word")[
  A word that *filters* the reader's experience through a layer of the author's
  hedging or emphasis instead of letting the image land directly. "The room was
  very cold" asks the reader to trust an intensifier; "the room was cold" and a
  shiver ask them to feel it. Not wrong — but almost always worth a glance.
]

#subsection("The amber underline")

The mark is a coloured underline drawn beneath the word in the Editor, in the
amber `theme.style_warning_filter_word_fg` (default `#f9c44e`). It is
deliberately quiet — a hint at the edge of vision, not a red-pen slash — and its
weight is yours to set: `theme.style_warning_filter_word_modifier` accepts
`underline` (the default), `bold`, `dim`, `reversed`, `italic`, `none`, or
`+`-combined forms like `underline+bold` for terminals where the plain underline
reads faint.

#screen(caption: "The amber filter-word underline in the Editor")[```
┌─ Editor · the-quay [modified] ──────────────────────┐
│  1  She just wanted to see the harbour, and it      │
│         ‾‾‾‾                                         │
│  2  really did seem very far, almost too far.       │
│     ‾‾‾‾‾‾    ‾‾‾‾ ‾‾‾‾                              │
│                                                     │
│  (amber ‾‾‾ = a filter word — question, don't obey) │
└─────────────────────────────────────────────────────┘
```]

Four words are marked here — `just`, `really`, `seem`, `very` — and two words
that look like candidates are *not*: "almost" and "too" are not on the English
list, and "harbour" is plainly innocent. The underline is a prompt to reread the
sentence, nothing more. Strike `Ctrl+B Shift+F` again and the page goes clean
for the next stretch of drafting.

#subsection("The per-language word lists")

Inkhaven ships a curated built-in list for each of its five first-class
languages — English, Russian, French, German, Spanish — and picks the one that
matches your project's `language`. The English list runs to about thirty entries
and gathers the usual suspects into two families: the *intensifier crutches and
hedges* (`just`, `really`, `very`, `pretty`, `quite`, `rather`, `fairly`,
`somewhat`, `slightly`, `actually`, `basically`, `literally`, `simply`,
`definitely`, `absolutely`, `totally`, `completely`, and the notoriously
load-bearing `that`) and the *sensory or hedging verbs* listed in base form
(`seem`, `feel`, `look`, `appear`, `sound`, `notice`, `begin`, `start`), plus
`suddenly`, `perhaps`, and `maybe`. The Russian list is its own vocabulary, not
a translation of the English one — `очень`, `просто`, `именно`, `довольно`,
`слишком`, `весьма`, `крайне`, `вполне`, `достаточно`, and so on — because the
hedges of one language are not the hedges of another.

#callout(label: "Does it work in Russian?")[
  Yes, and by design rather than accident. Tokenisation is UAX-#29-aware
  (Cyrillic, Latin, Greek, and Devanagari word boundaries all resolve), the
  columns are counted in *characters* not bytes so a multi-byte word never
  shifts the underline, and a `ё`→`е` fold is applied so a list entry spelled
  `ещё` still catches the `еще` you actually typed. A language Inkhaven ships no
  list for is left *silent* — never measured against English words — until you
  give it a list of its own.
]

#subsection("Replacing the list versus adding to it")

Two config fields shape the active list, and the difference between them is the
one thing to get right. The per-language array —
`editor.style_warnings.filter_words.english` (or `.russian`, and so on) — is a
*replacement*: leave it empty (the default) and Inkhaven uses its built-in list;
put anything in it and your list *replaces* the built-in one wholesale. The
`extra_words` field is an *addition*: whatever you list there is unioned on top
of whichever list is active, in every language. So the everyday move — "keep the
defaults, but also flag my own two crutch words" — is `extra_words`, not the
language array. Reach for the language array only when you want to curate the
whole list yourself.

#two_track(
  [A novelist who over-uses "somehow" and "a little" adds just those two to
  `extra_words` and keeps the thirty built-in English defaults underneath — the
  fastest way to hunt a personal tic without losing the general net.],
  [A technical writer who finds the literary defaults noisy in reference prose
  can *replace* the English list with a short array of the three or four
  hedges that actually creep into documentation, and leave the rest clean.],
)

There is a third door for a language outside the curated five. The
`filter_words.languages` map takes any language by its lowercased name — an
Italian project can be enabled with a `languages: { italian: ["molto", "solo"] }`
entry, typically written for you by `inkhaven lang bootstrap`. A mapped entry
takes precedence over everything; without one, a non-curated language simply
stays quiet.

#subsection("Stemming — one entry catches every inflection")

You will notice the built-in lists carry *lemmas*, not every surface form —
`seem`, never `seemed`/`seems`/`seeming`. That is because the detector stems
both the list and your prose through the project's Snowball algorithm before
matching, so a single entry catches the whole inflectional family. `seem` on the
list flags `seemed`; Russian `казаться` flags `казался`, `казалась`, `казалось`,
and `казались` alike. The stemmer is also what keeps the overlay honest at the
edges: English Snowball reduces `justice` to `justic`, not `just`, so "justice"
is never flagged for containing "just." If you would rather match exact forms —
listing every inflection yourself — set `filter_words.use_stemming = false`.

#subsection("The siblings under the same switch")

`Ctrl+B Shift+F` is labelled "toggle style warnings," in the plural, because it
governs more than filter words. The `style_warnings` master switch (and the one
chord that flips it) covers a small family of always-on inline detectors that
share the same rendering machinery and the same multilingual foundation:

- The *repeated-phrase* detector slides an `n`-word window (default `n = 4`)
  across the open paragraph and marks, in magenta
  (`style_warning_repeated_phrase_fg`, default `#eb6f92`), any phrase that
  recurs `threshold` times or more (default `3`) — with stop-words excluded so
  "the dog and" does not inflate a count, and stemming applied so "lifted her
  shoulders" and "lifting her shoulders" are recognised as the same gesture.
- The *show-don't-tell* inline detector marks telling constructions in teal; it
  has its own section below.
- The *anachronism* detector marks, in an era-caution orange
  (`style_warning_anachronism_fg`, default `#eba672`), any word that postdates
  your manuscript's setting — a `wristwatch` in an 1840 novel. It is off until
  you set `editor.style_warnings.anachronism.year`, and it ships a built-in
  lexicon (each term carrying its earliest plausible year) that you extend with
  a project `terms` list.

Each has its own per-detector `enabled` flag beneath the master switch, so a
writer who wants filter-word marks without repeated-phrase magenta can silence
just that one. The master switch turns the whole family off at once.

#chord_table((
  chord_row("Ctrl+B Shift+F", "Toggle the style-warning overlays (filter words, repeated phrase, show-don't-tell, anachronism) for the session."),
))

#section("The echo overlay — Ctrl+B Shift+K")

The filter-word family looks *within* the open paragraph. The *echo* overlay
looks *between* paragraphs — it is the live, in-editor companion to the
`echo-repetition` doctor scan, and it catches the revision-stage tic where a
distinctive word gets reused a few paragraphs apart: "she *walked* to the
window… he *walked* across the room… they *walked* out." Each sentence reads
fine on its own; the cluster clangs. Press `Ctrl+B Shift+K` in the Editor and
every word in the *open* paragraph that echoes across nearby paragraphs of the
chapter gains an underline in its own muted purple
(`style_warning_echo_fg`, default `#b48ead`) — deliberately distinct from the
repeated-phrase magenta, so a within-paragraph repeat and a cross-paragraph echo
read as two different findings.

#screen(caption: "The echo overlay — a distinctive word reused nearby")[```
┌─ Editor · the-market ───────────────────────────────┐
│ 12  The lantern swung. A second lantern hung by     │
│         ‾‾‾‾‾‾‾                 ‾‾‾‾‾‾‾              │
│ 13  the door, and a third lantern by the dry well.  │
│                       ‾‾‾‾‾‾‾                       │
│                                                     │
│  echo: "lantern" ×3 within a 5-paragraph window     │
└─────────────────────────────────────────────────────┘
```]

#subsection("Cross-paragraph, and live")

The overlay reads the chapter's paragraphs around the one you are editing —
using the *open* paragraph live, unsaved edits and all, so a fresh repeat you
type this instant is caught without a save. Its sensitivity is governed by three
tunables under `editor`, shared with the doctor scan so the live overlay and the
batch report agree:

- `echo_window` (default `5`) — how many consecutive paragraphs count as
  "nearby." A word must cluster within this span to flag.
- `echo_min_repeats` (default `3`) — how many occurrences within the window it
  takes to raise the echo. Lower is more sensitive.
- `echo_max_global` (default `40`) — the distinctiveness ceiling. A word used
  more than this many times across the chapter is treated as ordinary
  vocabulary an author legitimately reuses, and skipped even when it clusters.

That last knob is the clever part, and it is why the overlay does not need a
frequency dictionary for every language. Distinctiveness is measured *relative
to this chapter's own distribution*: a word must repeat but not be common. A word
used two hundred times across the book is vocabulary, not an echo, however it
clusters; a word used three times total, all within four paragraphs, is a
glaring one. The measure is corpus-relative and language-agnostic, which is what
lets the same detector serve every language honestly.

#callout(label: "Multilingual, with a documented edge")[
  Echo detection stems through the project's Snowball algorithm (with the Russian
  `ё`→`е` fold applied first), so `walked`/`walking` collapse to one echo and
  Russian `корабль`/`корабля`/`корабле` likewise. Languages *outside* the
  Snowball set — Japanese, Chinese — degrade gracefully to exact-surface
  matching: inflected variants will not collapse, but identical repeats still
  flag. The behaviour is documented and consistent with the other overlays'
  fallback, never a silent failure.
]

The master switch is `editor.echo_overlay` (default `false` — it is an opt-in);
`Ctrl+B Shift+K` is the session-local override that flips it without rewriting
your config.

#chord_table((
  chord_row("Ctrl+B Shift+K", "Toggle the echo overlay — words in the open paragraph echoing across nearby paragraphs of the chapter."),
))

#section("Show, don't tell — the overlay and the AI scan")

The most quoted rule of prose craft gets two tools, and the split between them is
the model for how Inkhaven pairs a cheap deterministic detector with an optional
LLM pass throughout the program. One is the always-on inline overlay under the
`Ctrl+B Shift+F` family; the other is a deliberate AI scan on `Ctrl+B Shift+T`.

#subsection("The inline overlay — three telling patterns")

The show-don't-tell detector marks, in teal
(`style_warning_show_dont_tell_fg`, default `#94e2d5`), three specific
constructions that *tell* the reader a feeling instead of *showing* the
behaviour that would let them infer it:

- A *copula and an emotion adjective* — a linking verb (`be`, `seem`, `feel`,
  `appear`, `look`, `become`, `remain`, `grow`, `sound`) followed by an emotion
  word (`angry`, `sad`, `afraid`, …). "She was angry" is flagged across both
  words; "she was running" is not, because "running" carries no emotion label.
- A *manner-of-emotion adverb* — `angrily`, `sadly`, `nervously` — flagged on
  its own, because such adverbs almost always name the feeling outright.
- A *cognition verb* — `realised`, `knew`, `understood`, `wondered`, `decided` —
  flagged on its own, because it reports a character's interior state directly to
  the reader.

#screen(caption: "Show-don't-tell — teal marks on three telling patterns")[```
┌─ Editor · the-quay ─────────────────────────────────┐
│  4  She was angry. He nodded, realised she meant    │
│     ‾‾‾‾‾‾‾‾‾            ‾‾‾‾‾‾‾‾                    │
│  5  every word, and turned away, angrily, at last.  │
│                                  ‾‾‾‾‾‾‾            │
│                                                     │
│  copula+adj · cognition verb · manner adverb        │
└─────────────────────────────────────────────────────┘
```]

Curated built-in lists ship for all five first-class languages; a language
Inkhaven does not ship lists for produces an empty (silent) detector rather than
noise. Matching is Snowball-stemmed by default, so "seemed nervous" is caught
through the `seem` lemma; set `show_dont_tell.use_stemming = false` for exact
forms. The detector is `enabled` by default *under* the master switch — so it
rides `Ctrl+B Shift+F` with its siblings.

#subsection("The AI scan — Ctrl+B Shift+T")

The inline overlay catches the obvious two-grams; it cannot catch the paragraph
that tells you a character is grief-stricken through a page of narrated summary
with no flagged word in it. That is what `Ctrl+B Shift+T` is for. It sends the
open paragraph to your configured model with a prompt asking for telling passages
*and suggested rewrites*, and streams the answer into the AI pane. The two tools
are complementary by intent: the regex overlay is free, instant, and always on,
catching the mechanical cases; the AI scan costs a call and a moment, and earns
it by catching the subtle instances and proposing alternatives. Neither touches
your prose — the scan is advisory, its rewrites are suggestions in the AI pane,
and any change you make from them is yours to type or to route through the
Editorial Pass. The mnemonic is *T for tell*.

#chord_table((
  chord_row("Ctrl+B Shift+T", "AI show-don't-tell scan — send the open paragraph to the model for telling passages plus suggested rewrites, streamed into the AI pane."),
))

#section("The terminology overlay — Ctrl+V z")

Where the filter-word overlay is about literary reflex, the *terminology* overlay
is about discipline: keeping one name for one thing. It reads the *Glossary*
system book — a book of canonical terms, each with a list of *banned synonyms* —
and red-underlines any synonym that appears in your prose, so the canonical form
reads clean while its rivals are flagged. Define "access token" as canonical with
"auth token" among its banned synonyms, and every "auth token" you type earns a
red underline (`style_warning_banned_synonym_fg`, default `#e05a5a`) while
"access token" stays untouched.

#screen(caption: "The terminology overlay — a banned synonym flagged")[```
┌─ Editor · setup-guide ──────────────────────────────┐
│  3  Paste the auth token into the header field,     │
│               ‾‾‾‾‾‾‾‾‾‾                             │
│  4  then reload. The access token never expires.    │
│                     (canonical — clean, no mark)    │
├─────────────────────────────────────────────────────┤
│ terms: "auth token" → use "access token"            │
└─────────────────────────────────────────────────────┘
```]

The overlay is *self-gating*: with an empty Glossary it flags nothing and costs
nothing, so a project that does not govern terminology never sees it. Synonyms
may be one to three words, matched longest-first over UAX-#29 word tokens (so
"auth-token" tokenises and matches the two-word form), and it works in Cyrillic
as readily as Latin. When your cursor sits on a flagged word the Editor footer
names the fix — `terms: "auth token" → use "access token"` — so the correction
is one glance away. It is on by default within the master style toggle;
`Ctrl+V z` is its own session-local override, distinct from `Ctrl+B Shift+F`.

#subsection("The deliberate-variant escape hatch")

Sometimes the variant is intentional — a character who says "auth token" in
dialogue because that is how they speak. With the cursor on a red-underlined
synonym, `Ctrl+V Shift+Z` records that canonical term as a *deliberate variant*
in the intent ledger, and both the overlay and the CI-ready `inkhaven terms
check` stop flagging it. It is the "I meant to write it this way" door, and it is
a first-class part of the design rather than a way to silence the tool wholesale.

#chord_table((
  chord_row("Ctrl+V z", "Toggle the terminology overlay — red-underline Glossary banned synonyms of canonical terms."),
  chord_row("Ctrl+V Shift+Z", "Declare the banned synonym under the cursor a deliberate variant, so it stops being flagged."),
))

#section("The POV chip — Ctrl+B Shift+P")

Not every overlay draws under your words; some speak from the status bar. The
*POV chip* answers a question a long manuscript makes surprisingly easy to lose
track of: whose eyes is this paragraph seen through? Press `Ctrl+B Shift+P` and
the status bar shows, for the open paragraph, the most-mentioned character as its
heuristic point-of-view figure, followed by up to three other named characters
present in the scene.

#screen(caption: "The POV chip riding the status bar")[```
 the-quay   ● 214w   POV Mara ·Corin ·Tyr   scope=None
 └ breadcrumb ┘  dirty+words ┘   └ heuristic POV chip ┘
```]

The chip is a *heuristic*, and honest about it. It counts mentions against the
project's existing `characters` lexicon — no separate per-paragraph tagging
needed — takes the most-mentioned name as POV, and breaks ties by order of first
mention. It will be wrong in a scene where the viewpoint character is silent
while another is named repeatedly, which is precisely the kind of place worth a
second look: a mismatch between who the chip names and who you *intend* is a
prompt, not a verdict. The master switch is `editor.pov_chip_enabled` (default
`true`); `Ctrl+B Shift+P` is the session-local flip. Its colours are
`theme.pov_chip_bg` / `pov_chip_fg` (defaults `#8b1d88` on `#ffffff`).

#chord_table((
  chord_row("Ctrl+B Shift+P", "Toggle the status-bar POV chip — the most-mentioned character in the open paragraph, plus up to three others present."),
))

#section("The sentence-rhythm gauge — Ctrl+B Shift+H")

Prose has a heartbeat, and monotony in it — every sentence the same length —
drones however clean the words are. `Ctrl+B Shift+H` opens the *sentence-rhythm
gauge*, a modal that splits the open paragraph into sentences (with a hand-rolled
walker that suppresses abbreviations — `Mr.`, `Mrs.`, `Dr.`, `e.g.`, `i.e.`,
`Ph.D.` — so they do not cut a sentence short), measures each one's word count,
and reports the mean, the standard deviation, and above all the *coefficient of
variation* (CV) — the standard deviation divided by the mean, a single number for
how much your sentence lengths vary.

#term("Coefficient of variation")[
  The CV is the spread of your sentence lengths *relative to their average*, so
  it compares fairly across dense and sparse prose alike. A low CV means every
  sentence is about the same length — a drone. A high CV means you mix the short
  and the long — the rhythm most strong prose has.
]

The gauge maps the CV to one of four verdicts: *Monotone* (`CV < 0.25` — the
prose drones), *Steady* (`0.25`–`0.45`), *Varied* (`0.45`–`0.80` — the sweet
spot of strong rhythm), and *Choppy* (`CV ≥ 0.80` — so jagged it may jar). It
lists each sentence as a bar and calls out the three shortest and three longest
so you can see the outliers at a glance.

#screen(caption: "The sentence-rhythm gauge — CV to verdict")[```
┌─ Sentence rhythm · the-quay ────────────────────────┐
│ mean 14.2 words · stdev 4.1 · CV 0.29               │
│ verdict: STEADY   (Monotone <0.25 · Varied >0.45)   │
│                                                     │
│  s1  ██████ 6                                       │
│  s2  ███████████████ 15                             │
│  s3  ████████████████████████ 24                    │
│  s4  ████ 4                                          │
│  shortest: s4 (4) · longest: s3 (24)                │
├─────────────────────────────────────────────────────┤
│ ↑↓ scroll · Ctrl+B Shift+M rewrite · any key close  │
└─────────────────────────────────────────────────────┘
```]

The gauge diagnoses; it does not fix. But because the natural next step from a
`MONOTONE` verdict is to break the rhythm up, `Ctrl+B Shift+M` — the AI
sentence-rhythm rewrite — fires from *inside* the gauge as well as from the
Editor, so the diagnose-then-rewrite path needs no extra keystroke: open the
gauge, read the verdict, and if you want it, ask for a rewrite that mixes short
and long while preserving your voice. That rewrite, like every AI prose change in
Inkhaven, arrives as a reviewable diff you accept or reject — snapshot first,
never an unconfirmed write. The mnemonic is *H for heartbeat*.

#chord_table((
  chord_row("Ctrl+B Shift+H", "Open the sentence-rhythm gauge — per-sentence bars, mean/stdev/CV, and a Monotone/Steady/Varied/Choppy verdict."),
  chord_row("Ctrl+B Shift+M", "AI sentence-rhythm rewrite — mix short and long sentences while preserving voice; arrives as a reviewable diff. Also fires from inside the gauge."),
))

#section("The reader-pace preview — Ctrl+B Shift+E")

The last overlay is the one that changes not how your prose *looks* but how you
*read* it. You read your own draft at editing-glance speed — skimming, because
you already know what it says — and at that speed a run-on that drags or a beat
that lands too abruptly is invisible. `Ctrl+B Shift+E` opens the *reader-pace
preview*: a teleprompter that advances a highlight word by word through the open
paragraph at a reader's speed, so you experience the prose the way a first reader
will.

#screen(caption: "The reader-pace preview — prose at reading speed")[```
┌─ Reader pace · 200 wpm ─────────────────────────────┐
│ The rain came sideways off the harbour, and Mara    │
│ counted the ▌lamps▐ as they went dark, one by one.  │
│                                                     │
│ dim = already read · ▌reverse▐ = now · plain = ahead│
├─────────────────────────────────────────────────────┤
│ word 9/48 · 00:12 left · Space pause · ←→ step · r  │
└─────────────────────────────────────────────────────┘
```]

The words behind the highlight dim, the current word is reverse-highlighted, and
the words ahead read normally; the footer shows your position and the time left.
The pace is `editor.reading_wpm` (default `200`, the common silent-reading
average — drop it toward `150` to feel an audiobook narration). `Space` pauses
and resumes (elapsed time carries across cycles), `←` and `→` step the highlight
by a word, `r` restarts from the top, and `Esc` closes. It reads *clean* prose —
the same markup-stripping pass the audiobook export uses — so your Typst markers
never break the flow. The mnemonic is *E for experience*.

#chord_table((
  chord_row("Ctrl+B Shift+E", "Reader-pace preview — a word-by-word teleprompter through the open paragraph at editor.reading_wpm, to feel your prose at a reader's speed."),
))

#section("Master switches and session overrides")

Now the switchboard. Every overlay in this chapter has the same two-level
control, and understanding the pattern once saves you configuring each one from
scratch. The *master switch* lives in `inkhaven.hjson` and is the persistent
default — on or off every time you open the project. The *session-local chord* is
the audition: it flips the overlay for the current run only and never rewrites
your config, so you can turn something on to check a passage and let it lapse
when you quit. When you decide, once, that you want an overlay on for good, you
set its master switch; day to day, you reach for the chord.

The overlays cluster under a handful of config keys, most of them inside the
`editor` block:

#screen(caption: "The overlay config, gathered in inkhaven.hjson")[```
editor: {
  style_warnings: {
    enabled: false          // master — Ctrl+B Shift+F
    filter_words: {
      enabled: true
      use_stemming: true
      extra_words: [ "somehow", "a bit" ]
      english: []           // [] = use the built-in list
    }
    show_dont_tell:   { enabled: true }
    repeated_phrases: { enabled: true, n: 4, threshold: 3 }
    anachronism:      { year: 1840 }   // off until set
  }
  echo_overlay: false       // master — Ctrl+B Shift+K
  echo_window: 5
  echo_min_repeats: 3
  echo_max_global: 40
  pov_chip_enabled: true    // status chip — Ctrl+B Shift+P
  reading_wpm: 200          // reader pace — Ctrl+B Shift+E
}
```]

A few defaults are worth committing to memory because they surprise people. The
`style_warnings.enabled` master is `false` out of the box — the whole
filter-word family is *opt-in*, so a fresh project shows no underlines until you
either flip the master or press `Ctrl+B Shift+F`. The per-detector flags beneath
it (`filter_words.enabled`, `show_dont_tell.enabled`,
`repeated_phrases.enabled`) default `true`, so once the master is on the family
comes on together; you silence an individual detector by setting *its* flag
false. The echo overlay's master (`echo_overlay`) is likewise `false` by default.
The POV chip's master (`pov_chip_enabled`) is `true`. And the terminology overlay
is on within the master style toggle but self-gates to silence on an empty
Glossary.

#callout(label: "Edit the config without leaving the editor")[
  You do not have to leave Inkhaven to change any of this. `Ctrl+B 0` opens
  `inkhaven.hjson` in a full-screen editor with syntax highlighting; `Ctrl+S`
  saves, and because these keys are read at launch a *Restart required* notice
  appears when a change needs the next run to take effect. The session chords are
  there precisely so you rarely need the round-trip mid-draft.
]

Here is the whole family in one table — the chord, and the master key it
overrides — so you can find any overlay from any pane.

#chord_table((
  chord_row("Ctrl+B Shift+F", "Style warnings (filter word / repeated phrase / show-don't-tell / anachronism) — master editor.style_warnings.enabled."),
  chord_row("Ctrl+B Shift+K", "Echo overlay — master editor.echo_overlay."),
  chord_row("Ctrl+B Shift+T", "AI show-don't-tell scan (per-invocation; complements the inline overlay)."),
  chord_row("Ctrl+V z", "Terminology overlay — on within the style master; self-gates on an empty Glossary."),
  chord_row("Ctrl+V Shift+Z", "Declare a banned synonym a deliberate variant."),
  chord_row("Ctrl+B Shift+P", "POV chip — master editor.pov_chip_enabled."),
  chord_row("Ctrl+B Shift+H", "Sentence-rhythm gauge (modal; Ctrl+B Shift+M rewrites from inside it)."),
  chord_row("Ctrl+B Shift+E", "Reader-pace preview (modal; paced by editor.reading_wpm)."),
))

The overlays are the quietest intelligence in the tool — no reports, no letters,
no scans you wait on — just a set of faint marks you can raise and lower over the
page as you draft. Turn them on when you want the second pair of eyes on a
finished paragraph; turn them off when you want to write without one watching.
They only ever ask; the answer, in every case, is yours.

#recap((
  [The style overlays are *advisory* — they mark patterns worth a second look
  (filter words, echoes, telling, banned synonyms, POV, rhythm, pace) and never
  edit your prose, block a save, or score you. You *question* them.],
  [`Ctrl+B Shift+F` toggles the *filter-word* family — amber underlines on
  intensifiers and hedges, keyed to the project `language`, Snowball-stemmed;
  `extra_words` *adds* to the list, the per-language array *replaces* it. The
  same switch governs repeated-phrase, inline show-don't-tell, and anachronism.],
  [`Ctrl+B Shift+K` toggles the *echo* overlay — a distinctive word reused across
  nearby paragraphs — tuned by `echo_window` (5), `echo_min_repeats` (3), and the
  `echo_max_global` (40) distinctiveness ceiling.],
  [*Show-don't-tell* is two tools: the always-on inline overlay (copula+emotion,
  manner adverb, cognition verb) under `Ctrl+B Shift+F`, and the AI scan on
  `Ctrl+B Shift+T` for the subtler cases with rewrites.],
  [The *terminology* overlay (`Ctrl+V z`) red-underlines Glossary banned synonyms
  and self-gates to silence when the Glossary is empty; `Ctrl+V Shift+Z` marks a
  variant deliberate. The *POV chip* (`Ctrl+B Shift+P`), *rhythm gauge*
  (`Ctrl+B Shift+H`), and *reader-pace preview* (`Ctrl+B Shift+E`) round out the
  family.],
  [Every overlay has a *master switch* in `editor.*` (the persistent default,
  most of them off) and a *session-local chord* (the audition that never rewrites
  config). `Ctrl+B 0` edits the config in place.],
))
