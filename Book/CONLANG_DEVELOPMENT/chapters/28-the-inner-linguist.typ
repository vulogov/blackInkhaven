#import "../design.typ": *

#chapter(number: 28, title: "The Inner Linguist")

Everything so far has been a command you run and read the output of. That is a
fine way to work, but a language is a large thing to hold in your head one
command at a time. Inkhaven gives you a second way in: a full-screen workspace
that opens over all of your languages at once, lets you walk their books, ask
questions about them, and read the analyses of the following chapter side by
side. It is called the *Linguistic companion*, and the assistant inside it is the
*Inner Linguist*.

#term("The Linguistic companion")[
  A full-screen terminal application, sibling to the Research companion, that
  opens over your project's `Language` book. The tree of your languages sits on
  the left; a set of views — a preview, a grounded chat, and the phonology,
  typology and minimal-pair analyses — sit on the right. It reads your languages;
  it does not edit them.
]

#section("Opening it")

From a project directory:

```sh
inkhaven linguistic
```

The screen splits in two. On the left is the tree of your `Language` book: every
language you have created, and — when you expand one — its five chapters and
their contents. Move with the arrow keys or `j`/`k`; open a branch with `l` or
`→`, close it with `h` or `←`. To open focused on one language, name it:

```sh
inkhaven linguistic --language Eldar
```

#section("The views")

The right pane shows one *view* at a time. Press `Tab` to cycle through them, or
jump straight to one with its letter:

/ *Chat*: a conversation with the Inner Linguist (press `i` to ask).
/ *Preview*: the content of whatever node you have selected in the tree.
/ *Phonology* (`m`): the quantitative metrics and the inventory-naturalness
  report of the next chapter.
/ *Universals* (`u`): how your grammar sits against the typological baseline.
/ *Minimal pairs* (`p`): the contrasts your lexicon draws, and their load.

Whichever language your selection sits inside is the one every view describes;
move the cursor to another language and the analyses recompute for it. The title
bar always tells you which language you are looking at.

#section("The grounded chat")

Press `i` and a prompt opens at the bottom of the Chat view. Ask anything about
the selected language — "is this vowel system typologically ordinary?", "suggest
a plausible plural suffix", "what would strengthen the consonant inventory?" —
and the Inner Linguist answers.

What makes the answer useful is that it is *grounded*. Before it replies, the
companion retrieves the most relevant passages from the very language sub-book
you are in — your phonology, your dictionary, your grammar — and hands them to
the model as the material to reason from. So it is talking about *your* language,
not languages in general, and it is told not to invent forms or rules your
book does not support. It answers in your project's language.

#callout(label: "The same retrieval as Research")[
  The grounding is the identical book-aware retrieval the Research companion uses
  over the Facts book — here pointed at the language sub-book your cursor is in.
  If an answer seems thin, it usually means the relevant chapter is thin: define
  more of the language and the assistant has more to stand on.
]

#section("Sessions")

Your conversation is a *session*, and it persists. Every completed exchange is
saved under the project's `.inkhaven/` directory and replayed the next time you
open the companion, so you can put a line of enquiry down and pick it up days
later. Keep separate lines of work in separate named sessions:

```sh
inkhaven linguistic --language Eldar --session phonology-review
```

Without `--session` you get the `default` session. A session is just the chat
transcript; nothing about your language is changed by talking about it.

#section("Slash-commands in the chat")

The chat input does more than ask questions. A line that begins with a slash is
run *locally* — the companion answers it from your own language book instead of
sending it to the assistant, and prints the result inline. None of them change
anything; each is a look, not a leap. There are four.

`/trace` followed by a sound-change rule previews that change across your whole
lexicon — which words would shift, and whether any would collide into new
homophones. It is the Consequence Tracer of the next chapter, at your fingertips
while you think:

```
/trace s > ʃ / _ i
```

`/parse <word>` runs the morphological parser *backwards*: it strips your
language's known affixes from a surface form and reports every analysis that
bottoms out at a dictionary root — including the non-concatenative processes:
reduplication, both full (`kata-kata`) and partial (`ka~kata`), and ablaut, the
internal vowel change of *sing/sang* (recovered by running each root forward
through the ablaut rules and matching). Where the gloss path builds a word from a
root and features, `/parse` takes a word you have written and tells you how it
could be built:

```
/parse katakatai
```

`/check <word>` is the Oracle: it judges a candidate word for well-formedness,
level by level — first its phonotactics (are the segments and clusters legal?),
then its morphology (does it parse to a root, or is it listed?) — and reports
either a clean bill of health or the specific findings against it.

`/tree <verb> <subject> [object] [indirect]` builds the X-bar phrase-structure
tree of a clause, placing heads and complements according to your language's
declared word order, and prints both the indented tree and its bracketed
notation:

```
/tree sees she bird
```

`/clause <verb> <subject> [object] [indirect]` runs the Oracle over a whole clause
rather than a single word, checking its argument structure against the verb's
valence — an intransitive verb given two arguments is flagged, a transitive one
given two is not. (Checking that the verb *agrees* with its subject needs the
subject's grammatical features, so that half lives on the command line, as
`language check-clause`.)

`/igt <sentence>` glosses a whole sentence of your language as *interlinear glossed
text* — the morpheme-segmented sentence (`kata-i`), a line glossing each morpheme
(`stone-PL`), and a literal translation, aligned in columns. An affix reshaped at
its boundary is still split correctly (a dative *kata + d* surfacing as *katat* is
segmented `kata-t`):

```
/igt katai nilo
```

On the command line the same gloss can be *kept*: `inkhaven language igt <lang>
--text "…" --save --name <n>` stores the interlinear in a `Texts` chapter of the
language book, and `inkhaven language texts <lang>` lists what you have gathered —
the beginning of a documented corpus of your language. Add `--format latex` to
export those texts as a `linguex` document, ready to paste into a grammar sketch or
a paper.

Back in the chat, `/texts` lists your stored interlinears, and `/settrans <name> =
<translation>` replaces one's free translation — the auto-generated literal line is
only a scaffold, and this is where you write the sentence as it should read. The
gloss underneath is left exactly as it was.

We meet the tracer, the parser, the Oracle and the syntax engine properly — and
the analyses the other views show — in the next chapter.

#recap((
  [The *Linguistic companion* (`inkhaven linguistic`) is a full-screen workspace
   over your `Language` book — the tree on the left, a set of views on the
   right.],
  [`Tab` cycles the views; `i` asks the Inner Linguist, `m` opens Phonology, `u`
   Universals, `p` Minimal pairs. Every view describes the language your cursor
   sits inside.],
  [The chat is *grounded*: answers are retrieved from your own language book, so
   the Inner Linguist reasons about your language and answers in your project's
   language.],
  [Conversations are saved *sessions* (`--session <name>`), replayed on reopen;
   talking about a language never changes it.],
  [Slash-commands run *locally* over your book and print inline: `/trace <rule>`
   previews a sound change, `/parse <word>` analyses a surface form into root +
   affixes, `/check <word>` is the word Oracle's verdict, `/tree <verb> <subject>
   …` builds the clause's X-bar tree, `/clause <verb> <subject> …` is the clause
   Oracle's argument-structure check, `/igt <sentence>` glosses a sentence, and
   `/texts` lists your stored interlinears. All are read-only except `/settrans
   <name> = <translation>`, which curates a stored text's free translation.],
))
