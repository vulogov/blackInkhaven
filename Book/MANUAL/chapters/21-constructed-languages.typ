#import "../design.typ": *

#chapter(number: 21, title: "Constructed Languages")

Two of Inkhaven's tools work on language itself rather than on your prose, and
because they sit near each other in the interface it is easy to confuse them, so
this chapter names them apart before it does anything else. The first is the
*ConLang Suite* — a workbench for *inventing* a language from nothing: sounds,
words, grammar, even a script and a font, all held inside the editor. The second
is the *WordNet thesaurus* — a lookup over a *real*, existing language, for
finding a sharper word when you are writing ordinary prose. One builds a
language that has never been spoken; the other consults the accumulated senses
of one that has. They share a chapter because they both live in the same window
and both are reached by a chord, and they share almost nothing else.

#callout(label: "Two tools, one line to keep straight")[
  The ConLang Suite is for a language you are *making up* — the tongue your
  elves speak, the trade creole of your invented port. The WordNet thesaurus is
  for the *actual* language you are writing in — English, French, German,
  Spanish, Russian — and it never touches an invented language. If you are
  reaching for a synonym in your own prose, that is WordNet; if you are coining a
  word that does not exist, that is the ConLang Suite.
]

This chapter is the *operator's tour* of both: what each is, where it lives, and
how to run it. The ConLang Suite is deep enough to have its own companion book —
*Developing a Constructed Language* — which is the full guide, from a first
phoneme to a printed grammar; when a topic here is only sketched, that is where
the long treatment lives.

#section("Part one — the ConLang Suite")

A constructed language in Inkhaven is not a file format or a wizard. It is a
*book* — an ordinary sub-book under the *Language* system book, with chapters you
can open, read, and edit like any other. What makes it a *language* is that a set
of engines reconstructs a working model of it from typed *HJSON blocks* you place
in those chapters. The book stays the home of record: everything the suite knows
about your language is text you wrote into it, and everything the suite produces
you can trace back to a block.

#term("HJSON block")[
  A small, human-friendly JSON dialect — commas optional, comments allowed,
  quotes often unneeded. Each block is one paragraph in the language's book: a
  list of phonemes, a set of morphemes, a dictionary entry. You author them by
  hand or let a command write them; the engines read them to build the language.
]

#subsection("Scaffolding a language")

You start a language with one command, which creates the sub-book and its
chapters ready to fill:

#screen(caption: "inkhaven language init — the scaffold")[```
$ inkhaven language init Eldar

Created Language ▸ Eldar
  · Meta / overview     iso_code, alphabet, world context
  · Phonology           sounds, classes, templates, stress
  · Grammar             morphology, typology, varieties
  · Dictionary          one HJSON paragraph per word
  · Sample texts        register anchors for translation

Next: add a `phonemes` block under Phonology,
      then `inkhaven language add-word Eldar …`.
```]

Each chapter is a destination for a particular kind of block. The table below is
the map you keep in your head: which block goes where, and which engine it feeds.

#chord_table((
  chord_row("Phonology", "Phonemes, classes, templates, constraints, allophony, stress, tone, romanization, the font block — drives the phonology engine."),
  chord_row("Grammar", "Morphemes, paradigms, typology answers, varieties, contact, idioms, metaphors — drives morphology and syntax."),
  chord_row("Dictionary", "One HJSON paragraph per word — the lexicon, overlay, and generation target."),
  chord_row("Meta / overview", "The ISO-style code, alphabet, and world context — resolves `:lang:` and sorts the buckets."),
))

#callout(label: "Quote your inline enums")[
  HJSON lets an unquoted string run to the end of the line, so an inline value
  like `kind: consonant` would swallow anything after it. Quote the short enum
  values — `kind: "consonant"`, `position: "suffix"` — and the parser reads them
  cleanly. It gives a clear error if you forget, but it is the one HJSON gotcha
  worth learning first.
]

#subsection("Phonology — the sounds and how they combine")

The *Phonology* chapter is where a language gets its voice. A phoneme block
lists the sounds (each with its IPA symbol, a romanization, and whether it is a
consonant or a vowel), groups them into named *classes*, gives *templates* for
building syllables, and adds *constraints* that forbid the shapes the language
does not allow. From this alone the suite can generate sayable words.

#screen(caption: "A phoneme block under the Phonology chapter")[```
{
  phonemes: [
    { ipa: "p", romanize: "p", kind: "consonant" }
    { ipa: "ʃ", romanize: "sh", kind: "consonant" }
    { ipa: "a", romanize: "a", kind: "vowel" }
  ]
  classes:   { C: ["p", "ʃ"], V: ["a"] }
  templates: { root: [ { pattern: "C V (C)", weight: 1.0 } ] }
  constraints: [
    { kind: "max_cluster_size", value: 2 }
    { kind: "no_geminate" }
  ]
  stress: "penultimate"
}
```]

That is enough to speak. `inkhaven language generate-word Eldar` draws a
phonotactically valid form from the templates; `syllabify`, `ipa`, `stress`, and
`romanize` each inspect one facet of a word you give it. Two further blocks
deepen the phonology: an *allophony* list rewrites sounds in context (SPE rule
notation, `k > tʃ / _ i` — "k becomes tʃ before i"), and a *tone* block gives the
language pitch. The companion book teaches the full notation; here it is enough
to know the block is where the sound lives.

#subsection("The lexicon — words, and where they come from")

A word is an HJSON paragraph under *Dictionary*, and it carries more than a
translation — a register, a domain, an era, whatever you want to filter on later:

#screen(caption: "One dictionary entry")[```
{ word: "makil", type: "noun", translation: "sword",
  register: "formal", domain: ["weapon"], era: "third_age" }
```]

You add words by hand (`add-word`), import them from a CSV, or — the interesting
path — have the AI generate a themed batch. The key idea is that *the forms are
never the AI's to invent*. The deterministic generator produces phonotactically
legal candidate forms from your own templates; the model only assigns *meanings*
to them. And before any generated word is kept, it must pass the *dedup gate*.

#term("The dedup gate")[
  The consistency check every generated word must survive before it can join the
  Dictionary. It rejects a form that is *phonotactically illegal*, a *homophone*
  of a word you already have, a *duplicate meaning*, or a *near-synonym* that
  would crowd an existing sense. What survives is genuinely new — a legal sound
  the language did not already carry, for a meaning it did not already cover.
]

#screen(caption: "language generate-lexicon — meanings onto legal forms")[```
$ inkhaven language generate-lexicon Eldar \
      --topic seafaring --count 6

  keep   toran    n.  tide
  keep   miluva   n.  harbor
  reject vesh         homophone of existing word
  reject koran   n.  mast   near-synonym of "toran"
  keep   dalmar   n.  hull

  3 kept · 2 rejected by the dedup gate
  Re-run with --yes to write the survivors.
```]

Nothing is written without `--yes` — like every AI feature in Inkhaven, this one
proposes and you commit. `language audit` hunts the existing lexicon for the same
faults after the fact (illegal forms, homophones, duplicate meanings);
`language query` filters it by the rich fields; `language gaps` tells you which
core concepts you have not coined yet, ready to hand straight to
`generate-lexicon`; and `language stats` prints a descriptive profile of the
whole inventory.

#subsection("Morphology and grammar — briefly")

Beyond sounds and words, a language has *structure*, and the suite models it —
but this is the part the companion book carries, so here is only the shape. A
*morphology* block in the Grammar chapter declares *morphemes* (prefixes,
suffixes, infixes, circumfixes, and non-concatenative processes like ablaut and
reduplication) and the *paradigms* that arrange them, so `language paradigm` can
inflect a root through a full case-and-number table. A *typology* answer set
(`language grammar Eldar --set word_order=sov`) records the language's
big structural choices against a WALS-aligned catalog, and the *syntax* engine
(`language sentence`) uses them to assemble a real clause — ordering the words,
case-marking the nouns, running agreement — and prints it with an interlinear
gloss. From there the suite reaches into diachronics (sound change and daughter
languages), dialects, borrowing, translation, and script design. It is a large
country; the CLI is how you travel it.

#subsection("The inkhaven language CLI")

The suite's depth lives on the command line — the editor holds a viewer, the CLI
holds the operations. There are far more verbs than fit here, grouped by what
they touch; the companion book documents each in full. This is the operator's
inventory: enough to know a verb exists and what family it belongs to.

#chord_table((
  chord_row("init · add-word · import", "Create a language; add or import words."),
  chord_row("generate-word · syllabify · ipa · stress · romanize · scan", "Inspect a form — build one, break it into syllables, pronounce it, mark its stress, romanize it, scan its verse."),
  chord_row("generate-lexicon · audit · query · gaps · stats · scan-manuscript", "Grow and inspect the lexicon; find undefined conlang words in your prose."),
  chord_row("paradigm · derive · agree · gloss", "Inflect and derive words; gloss a line."),
  chord_row("grammar · sentence · relative · complement · coordinate", "Set typology; assemble clauses of rising complexity."),
  chord_row("compose", "Generate names, prose, verse, or (AI) a blessing / curse / incantation from the lexicon."),
  chord_row("sound-change · derive-lexicon · family-tree · cognates", "Diachronics — evolve a proto into daughters."),
  chord_row("varieties · lect · dialects · borrow · areal", "Dialects, registers, and language contact."),
  chord_row("translate · reverse · cross · remember · corpus · eval", "Rule-based translation and its memory."),
  chord_row("dictionary · grammar-book · tutorial", "Render the language as a printable book."),
  chord_row("export · import · link-place · link-character", "Interchange with other tools; link speakers to your world."),
))

#subsection("Scansion — language scan")

Because a phonology already knows syllables and stress, it can *scan verse* in
the invented language. `inkhaven language scan Eldar --text "…"` takes one or
more lines, syllabifies each word, marks every syllable stressed or unstressed,
and names the metre — the same scansion engine the poetry tools use, pointed at
your conlang's own sounds. Stress is resolved per word: an explicit accent mark
in the text wins, then the lexicon's `stress` field, then the language's stress
rule; an unmarked one-syllable word stays *flexible* and may bend to fit the beat.

#screen(caption: "language scan — verse in an invented tongue")[```
$ inkhaven language scan Eldar --text "makil toran dala"

scan · Eldar   (/ stressed  × unstressed  · flexible)

  ma  kil   to  ran   da  la
   ×   /     /   ×      /   ·
  → trochaic · 3 feet
```]

#subsection("In the editor — the hub, insertion, and translation")

Three things bring the suite into the writing window itself, so you are not
always at the CLI.

*The ConLang hub — `Ctrl+B X`.* From any pane, `Ctrl+B X` opens a read-only
overview of every language under the Language book: for each, its phoneme
inventory split into consonants and vowels, its template and constraint and
allophony counts, its prosody (the stress rule, whether it has tone), its
romanization schemes, its lexicon size, and how many places and characters speak
it. It is a dashboard, not a workbench — the deep operations stay on the CLI, and
the hub's footer reminds you which ones. Scroll with the arrows; `Esc` closes.

#screen(caption: "Ctrl+B X — the read-only ConLang hub")[```
┌─ ConLang suite ─────────────────────────────────────┐
│ Language: Eldar                                     │
│   Phonemes      : 22 (16 C, 6 V)                    │
│   Templates     : 2 root · 4 constraint(s)          │
│   Allophony     : 3 rule(s)                          │
│   Prosody       : stress penultimate · tone —        │
│   Romanization  : 1 scheme(s)                        │
│   Lexicon       : 214 entr(y/ies)                    │
│   Speakers      : 3 place(s) · 5 character(s)        │
│                                                     │
│ Ctrl+B Q translate · CLI: language audit ·          │
│   generate-lexicon · query · scan-manuscript        │
└─────────────────────────────────────────────────────┘
```]

*Inserting a word — `:lang:`.* While writing, type a colon, the language's name
or ISO code, and a closing colon — `:Eldar:` — and a lexicon picker opens on that
language. Choose a word and it is inserted in place of the trigger, so you can
drop invented vocabulary into your prose without leaving the keyboard or
memorising your own dictionary.

*Translating a paragraph — `Ctrl+B Q` and `Ctrl+B Shift+Q`.* In the Editor,
`Ctrl+B Q` translates the open paragraph *into* an invented language: it gathers
the target language's grammar, phonology, a filtered slice of its dictionary, and
its sample texts into a prompt, and streams the result into the AI pane, wrapped
in markers so the AI pane's apply chord lifts only the translation.
`Ctrl+B Shift+Q` runs it in reverse — *from* the invented language back to your
own. Translate a passage one way, paste the result into the next paragraph, and
translate it back: when the round trip drifts, you have found an inconsistency in
the grammar or dictionary the manuscript would eventually have tripped over.

#callout(label: "One chord, two meanings by pane")[
  `Ctrl+B Q` is *translate into an invented language* only in the *Editor*. With
  the *Tree* focused, the same chord is the imposition preview for hand-binding —
  a different tool entirely (Chapter 33). The two never collide because they
  belong to different panes; it is the pane-specific chord table from Chapter 3
  at work.
]

#section("Part two — the WordNet thesaurus")

Now the other tool, and the distinction from the first is the whole point. The
WordNet thesaurus has nothing to do with invented languages. It is a
*sense-based* thesaurus over a *real* one — the language your manuscript is
actually written in — and its job is to hand you a better word when you are
writing ordinary prose.

#term("Sense-based")[
  A plain thesaurus offers "big → large, huge, enormous" without asking which
  *big* you meant. WordNet organises words by *sense* — the distinct meanings a
  word carries — and offers synonyms, antonyms, and related words for the
  *specific sense you pick*. "Bank" the riverside and "bank" the institution are
  different senses with different neighbours, and you choose which you meant.
]

#subsection("Installing a dictionary")

The thesaurus needs a language's WordNet index installed before it can work.
English fetches openly today; French, German, and Spanish arrive through the
Open Multilingual WordNet sources. For a language whose data cannot be openly
downloaded — Russian is the standing case — you build the index from a local
WN-LMF file you supply.

#screen(caption: "inkhaven wordnet — install and inspect")[```
$ inkhaven wordnet list          # sources + what's installed
$ inkhaven wordnet fetch en      # download + index English
$ inkhaven wordnet import ru russian-wordnet.xml.gz
```]

The index is stored under your data directory, at
`<data_dir>/inkhaven/wordnet/`, and is *shared across every project* — it is a
property of your machine, not of one book, because the English language does not
change from book to book. Nothing about a lookup ever leaves your computer; the
fetch is the only step that touches the network, and after that the thesaurus is
entirely local.

#subsection("Looking a word up")

`inkhaven wordnet lookup <word>` prints a word's senses, each with its own set of
related words. This is the command-line face of the same index the editor uses.

#screen(caption: "wordnet lookup — two senses, two neighbourhoods")[```
$ inkhaven wordnet lookup bank

bank  (noun)
  · sense 1 — a financial institution
      syn: depository, banking company
      hyper: financial institution
  · sense 2 — sloping land beside water
      syn: riverside, embankment
      hyper: slope
```]

A `--lang` flag chooses which language's index to search; without it, the lookup
uses English.

#subsection("In the editor — Ctrl+V Shift+Y")

The everyday way to use the thesaurus is not the CLI but the editor chord. Put
the cursor on a word (or select it) and press `Ctrl+V Shift+Y`. The thesaurus
opens on that word's senses; pick the sense you meant, then a synonym, antonym,
hypernym, or hyponym, and the chosen word *replaces the original in place* — no
retyping, and the Typst markup around it is preserved. It is the fastest path
from "this word is not quite right" to a better one, without leaving the line.

#screen(caption: "Ctrl+V Shift+Y — pick a sense, then a replacement")[```
┌─ Thesaurus · "huge" ────────────────────────────────┐
│ sense 1 — unusually great in size or extent         │
│   ▌syn  enormous                                    │
│    syn  immense                                     │
│    syn  vast                                        │
│    ant  tiny                                        │
│   ─ sense 2 — enormous in scope or degree ─          │
│    syn  colossal                                    │
├─────────────────────────────────────────────────────┤
│ ↑↓ select · Enter replace in place · Esc cancel     │
└─────────────────────────────────────────────────────┘
```]

The chord is `Ctrl+V Shift+Y` — under the *view* prefix, because it opens a view
onto your word rather than acting on the book. (It briefly lived on the meta
layer in an early release; if an old note says `Ctrl+B` something, the documented
home is `Ctrl+V Shift+Y`.)

#subsection("Across languages, and the AI fallback")

WordNet carries an *interlingual index* that links senses across languages, so a
lookup can cross from one to another: the German word for a concept your Russian
text names is a lookup on the shared sense. This is why a non-English lookup
still works — the relations expand through that shared index.

And when a language has *no* installed WordNet and nothing to download — Russian,
if you have not imported one — the chord does not simply fail. It falls back to
the AI, which offers sense-grouped alternatives keyed to your project language,
so the workflow is identical everywhere. You always press `Ctrl+V Shift+Y`; what
answers is WordNet where it exists and the model where it cannot.

#callout(label: "The line, once more")[
  If you take one thing from this chapter, take the seam down its middle. The
  ConLang Suite (`Ctrl+B X`, `inkhaven language …`, `:lang:`) *invents* a
  language and generates within it. The WordNet thesaurus (`Ctrl+V Shift+Y`,
  `inkhaven wordnet …`) *consults* a real one for your ordinary prose. They are
  never the same operation, and neither reaches into the other's territory.
]

#recap((
  [The *ConLang Suite* builds an *invented* language as a *book* under the
  Language system book: *HJSON blocks* for phonology, lexicon, and morphology,
  scaffolded by `inkhaven language init` and read by the suite's engines.],
  [Words come from a *deterministic generator* (legal forms) plus *AI meanings*,
  and every generated word must pass the *dedup gate* — no illegal form,
  homophone, duplicate meaning, or near-synonym survives.],
  [The `inkhaven language` CLI is the deep surface — inspection, generation,
  morphology, syntax, diachronics, translation, and printable books; the
  companion *Developing a Constructed Language* is the full guide.],
  [In the editor: `Ctrl+B X` opens the read-only ConLang hub, `:lang:` inserts a
  lexicon word in place, and `Ctrl+B Q` / `Ctrl+B Shift+Q` translate a paragraph
  into and out of an invented language.],
  [The *WordNet thesaurus* is a separate tool for *real* prose: *sense-based*,
  multilingual (fetch en/fr/de/es, import ru), stored at
  `<data_dir>/inkhaven/wordnet/` and shared across projects.],
  [`inkhaven wordnet` handles `fetch` / `import` / `lookup` / `list`; in the
  editor `Ctrl+V Shift+Y` replaces the word under the cursor in place, crossing
  languages through the interlingual index and falling back to the AI when no
  index exists.],
))
