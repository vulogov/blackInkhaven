#import "../design.typ": *

#chapter(number: 2, title: "Meet Inkhaven")

*Inkhaven* is a writing tool — a program for authors who write books, with a
text editor, a place to keep notes and characters and research, and tools for
turning a manuscript into a finished, formatted book. Built into it is a
complete *conlang workshop*: the set of features, called the *ConLang Suite*,
that this book is about. You do not need to know anything about the rest of
Inkhaven to use it.

#section("Two ways to talk to Inkhaven")

Inkhaven has a full-screen interface you can run in a terminal — the *TUI*, for
"text user interface" — where you write and edit. But almost everything in this
book is done through *commands*: short lines you type at a terminal prompt, each
beginning with the word `inkhaven`. This is deliberate. Commands are precise,
they are easy to show in a book, and they are easy to repeat.

#term("Command line")[
  A way of using a program by typing instructions, rather than clicking buttons.
  You type a line, press Enter, and the program does one thing and prints the
  result. The conlang tools all live under one command word:
  `inkhaven language …`.
]

Every conlang command has the same shape:

```text
inkhaven language <action> <language-name> [options]
```

For example, `inkhaven language add-word Eldar makil --type noun --translation
sword` runs the *add-word* action on the language named *Eldar*, with three
pieces of extra information. Throughout the book, the word after `language` is
the action, the next word is usually your language's name, and the parts that
begin with `--` are *options* that fine-tune what happens.

#callout(label: "Getting help on any command")[
  Add `--help` to any command to see exactly what it does and what options it
  takes — for example `inkhaven language add-word --help`. This works for every
  command in the book and is the fastest way to check a detail.
]

#section("Where your language lives")

Inside an Inkhaven project, your work is organised into *books* — not printed
books, but containers for related material. There is a special built-in one
called the *Language* book, and each language you create becomes a sub-book
inside it. When you create a language, Inkhaven sets up five *chapters* inside
its book, each holding one kind of information:

#term("The Language book")[
  Inkhaven's home for constructed languages. Creating a language with `inkhaven
  language init` adds a sub-book under it, pre-divided into five chapters:
  *Meta*, *Dictionary*, *Grammar*, *Phonology*, and *Sample texts*.
]

You will spend most of your time putting information into these chapters — sounds
into *Phonology*, words into *Dictionary*, grammar rules into *Grammar*. The book
is always the master copy of your language; the tools read from it and write back
to it. We will see exactly how in the next chapter.

#section("The role of artificial intelligence")

Some of Inkhaven's conlang tools can call on an *AI* (a large language model, the
kind of program behind chat assistants) to help — to invent a batch of words on a
theme, to draft a letter-shape from a description, or to write a study guide.
These features are powerful, but they are always *optional* and always
*advisory*: the AI proposes, and nothing is saved to your language until you
approve it. Wherever a command uses AI, the book says so plainly, and you can
skip it.

#term("AI provider and API key")[
  An *AI provider* is a company that runs a language model you can connect to
  (examples include DeepSeek, OpenAI, and others). An *API key* is a secret
  password that lets your copy of Inkhaven use that provider. You set one up
  once; Chapter 3 shows how. If you choose not to use AI at all, every essential
  part of building a language still works without it.
]

#callout(label: "What needs AI, and what does not")[
  *Does not need AI:* choosing sounds, generating word-shapes, building the
  lexicon by hand, all grammar and morphology, sound change and family trees,
  importing and compiling fonts, and producing the dictionary and grammar
  reference. *Uses AI (optional):* generating themed vocabulary, drafting glyph
  artwork from a description, reconstructing a proto-form, the grammar *study
  guide*, and the learner's *tutorial*.
]

#recap((
  [Inkhaven is a writing tool with a built-in *ConLang Suite*.],
  [You drive the conlang tools with *commands* of the form
   `inkhaven language <action> <name> [options]`; `--help` explains any of them.],
  [Each language lives in a *Language* sub-book with five chapters: Meta,
   Dictionary, Grammar, Phonology, Sample texts.],
  [*AI* features are optional and advisory — nothing is saved without your
   approval, and everything essential works without AI.],
))
