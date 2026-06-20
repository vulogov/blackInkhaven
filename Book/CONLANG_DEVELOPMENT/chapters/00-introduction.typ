#import "../design.typ": *

#v(1cm)
#text(font: body_family, size: 22pt, weight: "bold")[Before You Begin]
#v(6mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(8mm)

This is a book about making a language — one that has never existed, with sounds
you choose, words you coin, a grammar you design, and even its own alphabet. The
craft is called *conlanging*, and the language you build is a *constructed
language*, or *conlang*. People make conlangs for novels and games, for films,
for the pure pleasure of the puzzle, or simply to understand how human language
works by building one from the inside.

You will do all of it inside *Inkhaven* — a writing tool with a built-in
conlang workshop. By the last page you will have taken a language from an empty
project to three finished, printable books: a dictionary, a reference grammar,
and a beginner's textbook for your own invented tongue.

#section("Who this book is for")

This guide assumes *no prior knowledge*. You do not need to be a linguist: every
technical term is defined the first time it appears, in a marked box like the
one below. You do not need to be an Inkhaven expert: we start from installing it
and creating your first project. If you can type commands into a terminal and
edit a text file, you have everything you need.

#term("Linguistics")[
  The scientific study of human language — its sounds, its words, its grammar,
  and how these change over time. You will meet small, friendly pieces of it
  throughout this book. You do not need a background in it; that is what the
  term boxes are for.
]

#section("How this book is organised")

The book follows the natural order in which a language is built, one layer at a
time. Each layer rests on the one before:

#set enum(numbering: "1.")
+ *Foundations* — what a conlang is, what Inkhaven is, and getting set up.
+ *The sounds* — choosing the building blocks of pronunciation and the shapes
  words can take.
+ *Words* — coining a vocabulary and keeping it consistent.
+ *Grammar* — how words change and combine to make meaning.
+ *History* — giving your language a past, and spawning related languages.
+ *A writing system* — designing an alphabet and compiling it into a real font.
+ *The books* — turning all of it into printed documents.
+ *A complete walkthrough* — building one small language end to end.

At the back you will find three references you will return to often: a list of
every command, a list of every configuration block, and a glossary of every
linguistic term used in the book.

#callout(label: "How to read the examples")[
  Lines you type into a terminal are shown in a monospace box and begin with the
  program name, like `inkhaven language init Eldar`. Configuration you write
  into your language is shown as `hjson` blocks. Throughout, we build one
  running example language called *Eldar*; in the final chapter we build a
  second, *Avesha*, from nothing to finished books, so you can watch the whole
  process at once.
]

#section("What you will need")

Three things, all free, all covered in Chapter 3: a copy of *Inkhaven*; a
terminal to run it in; and — for the parts that use artificial intelligence,
which are always optional — an *API key* for an AI provider. We will explain
each when we reach it. You will also want *Typst* (a free typesetting program)
if you wish to turn your finished books into PDFs, but that too can wait.

Turn the page, and let us begin with the most basic question of all: what,
exactly, is a constructed language?
