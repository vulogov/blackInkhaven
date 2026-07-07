#import "../design.typ": *

#chapter(number: 25, title: "Sharing your language")

Your language does not have to live only inside Inkhaven. You may want to typeset
a paper about it, study it on your phone, hand the lexicon to a collaborator,
publish a dictionary website, or bring in a language you started building in
another program. This chapter is about moving a lexicon *out of* and *into*
Inkhaven, and about the file formats and tools the wider community of linguists
and conlangers actually uses — so that whichever one you meet, you know what it
is and how to bridge it.

There are two directions, and one command each. *Export* writes your lexicon to a
file in some other format; *import* reads a foreign file back into your
Dictionary. Everything here works on the lexicon you have already built with
`add-word` and `generate-lexicon`; nothing new about the language itself is
required.

#section("The landscape: who uses what")

Before the commands, it helps to know the formats by name. Each of the following
is a real tool or standard you may run into when you look for conlang resources or
linguistic software. The term boxes define them once; later sections show how
Inkhaven reads or writes each.

#term("Standard Format (SFM / MDF)")[
  A plain-text dictionary format where every field is tagged with a backslash
  marker — `\lx` for the headword (#emph[lexeme]), `\ps` for part of speech,
  `\ge` for the English gloss, `\xv` for an example, and so on. *MDF*
  (Multi-Dictionary Formatter) is the standard set of those markers. It is decades
  old, plain text, and the common tongue of descriptive lexicography: many tools
  read and write it.
]

#term("SIL Toolbox / FieldWorks")[
  Field-linguistics software from SIL International used to record and analyse
  real languages in the field. Both read and write *Standard Format*, so a lexicon
  exported from Toolbox is an SFM file — exactly what Inkhaven's Toolbox importer
  expects.
]

#term("Lexique Pro")[
  A free program that turns a *Standard Format* dictionary into a browsable,
  hyperlinked dictionary application and website. Because its source format *is*
  SFM, a Lexique Pro database imports into Inkhaven the same way a Toolbox one
  does.
]

#term("PolyGlot")[
  A popular free desktop application built specifically for constructed languages,
  with a lexicon, a grammar, and a conjugation engine. It saves to a `.pgd` file,
  which is really a #emph[ZIP] archive containing an XML dictionary — Inkhaven
  unzips and reads it for you.
]

#term("ConWorkShop (CWS)")[
  A large online community and toolkit for conlangers, with shared dictionaries
  and sound-change tools. Its lexicon export is a spreadsheet (CSV), which comes
  into Inkhaven through the CSV import path.
]

#term("CAT tool / translation memory")[
  *Computer-assisted-translation* software (OmegaT, memoQ, Trados, the web tool
  Weblate) that helps translate text by remembering past translations. The
  remembered pairs live in a #emph[translation memory]; the standard interchange
  file for one is *XLIFF*.
]

#term("XLIFF")[
  *XML Localisation Interchange File Format* — the OASIS standard a *CAT tool*
  reads. Each entry is a #emph[translation unit] pairing a source-language phrase
  with its target-language equivalent. Exporting your lexicon as XLIFF turns it
  into a translation memory between your working language and your conlang.
]

#term("Anki")[
  A widely used spaced-repetition flashcard program. It imports decks from a
  simple comma-separated file, so an Anki export of your lexicon becomes a
  vocabulary deck you can drill on your phone.
]

#term("linguex")[
  A LaTeX package linguists use to typeset numbered, glossed example sentences in
  papers and grammars. An export in this format is a ready-to-compile LaTeX
  document of your lexicon, headwords and examples included.
]

#section("Exporting your lexicon")

The single command is `export`, with a `--format` choosing what to write. By
default it prints to your screen; add `--out FILE` (or redirect with `>`) to save:

```sh
inkhaven language export Eldar --format xliff   --out eldar.xlf
inkhaven language export Eldar --format anki     > eldar-deck.csv
inkhaven language export Eldar --format ipa-chart
```

The formats, and when to reach for each:

/ `json`: A complete structured dump — dictionary, grammar, phonology, and sample texts — for your own scripts or an archival backup. (It is not one of the formats `import` reads back; for a round trip through Inkhaven, use `csv`.)
/ `csv`: A spreadsheet of your entries, twelve columns, and the *only* format that comes back in through Inkhaven's import path. Use it to edit your lexicon in a spreadsheet and bring it back (see the round trip below), and as the bridge for tools like ConWorkShop.
/ `anki`: A flashcard deck (word, translation, type, example) importable into *Anki* and similar spaced-repetition apps.
/ `xliff`: A *translation memory* (each entry a working-language → conlang pair) loadable into a *CAT tool* such as OmegaT, memoQ, or Weblate — useful if you actually translate text into your language.
/ `linguex`: A LaTeX document using the *linguex* package — bold headwords with glosses and numbered examples — to paste into an academic paper or a hand-written grammar.
/ `ipa-chart`: A Markdown inventory of your phonemes, consonants and vowels grouped, each with its romanization — a quick appendix or forum post.
/ `dictionary-twocol` / `grammar` / `phrasebook`: Typeset *Typst* documents (a printable two-column dictionary, a grammar reference, a phrasebook). These need `--out FILE.typ`, then you compile them just like the books in the previous chapter.

To make the two least-familiar formats concrete: a single XLIFF entry pairs the
meaning with the word, and a linguex headword carries its gloss and a numbered
example —

```xml
<trans-unit id="1"><source>bird</source><target>kira</target></trans-unit>
```

```latex
\textbf{kira} \textit{noun} `bird'\\
\ex. kira nami
```

#callout(label: "Which export should I use?")[
  If you want to *read it back into Inkhaven later*, use `csv` (the one
  round-trippable format). If you want a *backup* or to feed your own scripts, use
  `json`. To *study* the words, use `anki`. To *typeset* them, use `linguex` (a
  paper) or the Typst formats (a finished book). To carry your lexicon into an
  *external CAT tool* (OmegaT, memoQ, Weblate), use `xliff`. The IPA chart is for a
  quick reference.
]

#term("Translating without leaving Inkhaven")[
  XLIFF is for handing your words to *another* program. You do not need to leave
  Inkhaven to translate at all: the previous chapter's built-in engine
  (`language translate` / `reverse` / `cross`) turns sentences into and out of your
  language using its own lexicon and grammar, and keeps its own *translation memory*
  — exported as an `.itm` pack, not XLIFF. Reach for XLIFF only when the translating
  happens in a tool other than Inkhaven.
]

#section("Importing a lexicon")

Going the other way, `import` reads a foreign file and adds its entries to your
Dictionary. It has one safety habit worth knowing: it *previews by default* and
writes nothing until you add `--yes`. So you always see what it found first:

```sh
inkhaven language import Eldar --file lexicon.sfm --format toolbox        # preview
inkhaven language import Eldar --file lexicon.sfm --format toolbox --yes  # write
inkhaven language import Eldar --file MyLang.pgd  --format polyglot --yes
```

Two foreign formats are read directly:

/ `toolbox`: A *Standard Format* (SFM / MDF) database — the `\lx … \ps … \ge …` marker file written by *SIL Toolbox*, *FieldWorks*, and *Lexique Pro*. Inkhaven maps the standard markers onto its own fields (headword, part of speech, gloss, example, pronunciation, etymology, notes) and folds multi-line fields together.
/ `polyglot`: A *PolyGlot* dictionary. Pass either the native `.pgd` archive — Inkhaven unzips the dictionary XML inside it automatically — or a raw exported `.xml`. The part-of-speech table is resolved so each word arrives with its word class.

A lexicon exported from *ConWorkShop* (or any tool that produces a spreadsheet)
comes in through the CSV path instead, which lives on the `add-word` command:

```sh
inkhaven language add-word Eldar --import cws-export.csv
```

#callout(label: "Imported words still face the rules")[
  Importing does not bypass your language. The CSV path runs the same
  phonotactic pre-flight as a hand-typed word, so a row whose spelling uses a
  letter or sound your language has not declared is flagged before anything is
  written (use `--force` only when you mean to). And every importer refuses to
  overwrite a word you already defined — duplicates are skipped with a warning, so
  a re-import never clobbers your edits.
]

#section("A round-trip workflow")

The CSV format is built to make a loop: export your lexicon, edit it somewhere
comfortable, and bring it back. A common pattern is bulk-editing in a spreadsheet:

+ `inkhaven language export Eldar --format csv --out eldar.csv`
+ Open `eldar.csv` in a spreadsheet; fix translations, add examples, tidy
  registers across many rows at once.
+ `inkhaven language add-word Eldar --import eldar.csv` — new rows are added,
  rows you already have are skipped, so your edits merge cleanly.

The same loop lets two people collaborate (one exports, the other imports), or
lets you move a language between machines without copying the whole project.

#callout(label: "What travels, and what does not")[
  Interchange moves the *lexicon* — words, glosses, parts of speech, examples, and
  the fields a row carries. It does not move your phonology rules, paradigms,
  sound-change chains, or font: those are Inkhaven-specific structure with no
  equivalent in a Toolbox or PolyGlot file. To move a whole *project*, copy the
  project folder; to move just the *words*, use interchange.
]

#recap((
  [*Export* writes your lexicon out (`export --format …`); *import* reads a
   foreign lexicon in (`import --format … [--yes]`).],
  [*Standard Format* (SFM/MDF) is the shared text format of *Toolbox*,
   *FieldWorks*, and *Lexique Pro*; import it with `--format toolbox`.],
  [*PolyGlot* `.pgd` files (zip + XML) import with `--format polyglot`;
   *ConWorkShop* and other spreadsheet exports come in via `add-word --import`.],
  [*XLIFF* feeds *CAT tools*, *anki* feeds flashcards, *linguex* feeds LaTeX, and
   the Typst formats produce finished books.],
  [Import *previews until `--yes`*, re-runs the phonotactic check, and never
   overwrites an existing word; `csv` makes a clean export-edit-import *round
   trip*.],
  [Interchange moves *words*, not phonology / paradigms / fonts — copy the project
   folder to move everything.],
))
