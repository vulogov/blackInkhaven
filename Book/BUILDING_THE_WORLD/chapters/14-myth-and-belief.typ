#import "../design.typ": *

#chapter(number: 14, title: "Myth and Belief")

When you gave your peoples their cultures, the world handed each of them a
*belief* — a single line. Ancestor veneration. A sky-pantheon. Nature spirits of
river and stone. It was enough to know what a people held sacred, enough to give
your characters something to swear by. But a belief on its own is a seed, not a
mythology. It tells you *that* these people revere the sky; it does not yet carry
the raven that means betrayal, the locked door that keeps returning, the herald
who arrives to break the peace. Those are the working parts of a living
mythology, and they live somewhere else: in a system book of their own, declared
by your hand and watched over by a reader built for the purpose.

#term("Mythology (the system book)")[
  A dedicated home for your story's *declared* symbolic vocabulary — its symbols,
  its recurring motifs, and its archetypal roles. Unlike the World system, which
  *generates* and *proposes*, the Mythology book holds only what you write into
  it. Its reader never discovers a symbol you did not name, never interprets one
  for you, and never touches your prose. It reads your declarations and tells you
  whether the manuscript keeps faith with them.
]

This is the same authority discipline as the magic ledger of the last chapter,
pointed at a different target. There you declared what your world's powers cost
and let Inkhaven flag the scene that spent nothing; here you declare what your
symbols *mean* and let it flag the scene that betrays them. Both are the author's
hand, written down so the machine can hold you to your own word.

#section("From a belief to a mythology")

A mythology, as the system understands it, is three kinds of thing, and it helps
to name them before you build any. A *symbol* is a piece of vocabulary that
carries a meaning: the word "raven", wherever it falls in your prose, drags a
freight of betrayal behind it. A *motif* is a pattern that recurs: the locked
door refused and then crossed, the drowned singer, the gift that curses the
giver. An *archetype* is a role a character plays in the shape of the story: the
mentor, the herald, the shadow. Declare these three well and you have told
Inkhaven what your book *means to mean* — the standard it can then measure the
prose against.

#insight[
  A belief answers "what do these people hold sacred?" A mythology answers "and
  what, in my prose, will carry that reverence?" The first is a fact about the
  world; the second is a contract about the writing. The Mythology book is where
  the fact becomes the contract.
]

#section("The three declarations")

Each entry in the Mythology book is a paragraph you tag — `para:myth-symbol`,
`para:myth-motif`, or `para:myth-archetype` — with a small HJSON block beneath
it. A symbol names its *vocabulary* (the words that carry it), its *meaning*, a
*valence* (whether it reads as positive, negative, or ambiguous), and the
*traditions* that hold it:

#hjson[```
{ myth_symbol: {
  vocabulary: ["raven", "ravens"]
  meaning: "a herald of betrayal"
  valence: "negative"
  traditions: ["the northern clans"]
} }
```]

A motif names itself and describes the pattern it stands for:

#hjson[```
{ myth_motif: {
  name: "the locked door"
  description: "a threshold refused, then crossed"
  valence: "ambiguous"
} }
```]

And an archetype binds a *role* to one of your characters by name, saying what
that character is *for* in the architecture of the story:

#hjson[```
{ myth_archetype: {
  role: "herald"
  character: "Seren"
  function: "announces the rupture that starts the story"
} }
```]

#note[
  Open the Mythology book in a fresh project and you will not find it empty. It
  ships with a short guide and one worked example symbol, so you can see the shape
  of a declaration before you write your own. Duplicate the example, change its
  words, and you have your first symbol; delete it when you no longer need the
  scaffold.
]

#section("The bridge from the world")

Here the two systems meet. Your cultures already carry beliefs, and a belief is
the raw material of a symbol. So the world offers to make the first draft for
you. Run `realworld propose-myth` and it reads every culture's belief and turns
it into a Mythology proposal: a sky-pantheon becomes a symbol of sky and storm
and thunder; nature spirits of river and stone become a symbol of the small gods
bound to the land; a cult of the seasons becomes the motif of the turning year.
Two peoples who share a belief are gathered into one entry, and the *traditions*
field records which of them hold it — the thread that ties the symbol back to the
world that suggested it.

Nothing commits on its own. Each proposal waits in the same queue as your Place
proposals, and you work it the same way — `realworld proposals list` to read
them, then accept the ones that ring true. An accepted proposal is written into
the Mythology book as a real tagged entry, ready for you to sharpen.

#propose_accept()

The world is a generous but literal collaborator here. It can tell that a people
who revere the sky will have a sky-symbol; it cannot tell you that in *your* book
the sky-god is a tyrant whose worship the heroine will learn to refuse. That
meaning is yours to write. Take the proposal as a rough-cut stone — the right
material, roughly the right shape — and carve it into the symbol your story
actually needs.

#question[
  Read back the symbols the world proposed from your cultures. For each, ask: is
  this what the symbol means to *me*, or only what it would mean to the people who
  hold it? A drowned-god symbol a coastal people reads as protection might, in
  your protagonist's hands, mean grief. Declare the meaning your book will honour,
  not the one the culture would recite.
]

#section("Archetypes and your characters")

Symbols and motifs live in the words; archetypes live in your people. When you
declare an archetype you name a character who already exists in your Characters
book, and you say what role they play — herald, mentor, shadow, trickster, or a
custom role of your own. This is the one declaration that reaches across into
another system book, and it is worth the reach: it lets Inkhaven notice when a
role you promised goes unfilled, or when the character you cast as the herald
never actually heralds anything.

#note[
  An archetype needs a character to inhabit it. Declare the role before you have
  cast it and the Mythology reader will tell you the role stands vacant — a
  useful nudge when you have planned a mentor into your structure but not yet
  written the person who mentors.
]

#section("Keeping the mythology honest")

Once your declarations are in place, two commands hold the manuscript to them.
`inkhaven myth scan` is the quiet one: it builds a density heatmap of where your
symbols actually appear across the chapters — pure counting, no AI, no network —
so you can see at a glance whether the motif you swore ran through the whole book
in fact clusters in its first act and vanishes. `inkhaven myth check` is the
searching one: it asks whether a symbol is ever used against its declared
meaning, whether a motif you promised ever lands in the final act, whether an
archetype's character does the thing their role requires. Its findings are
advisory — it flags, it never edits — and every one can be suppressed when you
have looked and decided the prose is right.

#insight[
  The Mythology reader can only be as honest as your declarations are complete. It
  will never invent a symbol you did not name, which means it will never scold you
  for a pattern you never claimed. Declare what you mean your book to mean, and it
  becomes your most patient reader — the one who has memorised your intentions and
  quietly checks the pages against them.
]

#tryit[
  Take a peopled world and run `realworld propose-myth`, then accept one symbol
  into the Mythology book. Open the book, sharpen the accepted symbol's meaning to
  fit your story, and add one motif of your own. Write a scene that uses the
  symbol's vocabulary, then run `inkhaven myth scan` and watch it appear in the
  heatmap. You have carried a people's belief all the way from a line in a culture
  card to a checked, living pattern in your prose.
]

#recap((
  [The *Mythology* system book holds your *declared* symbolic vocabulary —
   *symbols* (words that carry a meaning), *motifs* (patterns that recur), and
   *archetypes* (roles bound to your characters). It reads only what you declare.],
  [A culture's *belief* is the seed of a mythology. `realworld propose-myth` reads
   your peoples' beliefs and proposes symbols and motifs, crediting the peoples who
   hold them in the *traditions* field — the World × Mythology bridge.],
  [Proposals wait in the same queue as Places; you accept the ones that ring true,
   and an accepted proposal becomes a real tagged entry you then sharpen. The world
   proposes the material; the meaning is yours.],
  [*Archetypes* reach across to the Characters book, so Inkhaven can notice a role
   left vacant or a herald who never heralds.],
  [`inkhaven myth scan` maps where your symbols fall (zero-AI);
   `inkhaven myth check` asks whether the prose keeps faith with their declared
   meanings. Both advise; neither edits. The author always has the last word.],
))
