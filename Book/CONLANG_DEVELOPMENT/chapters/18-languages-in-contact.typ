#import "../design.typ": *

#chapter(number: 18, title: "Languages in contact")

Languages do not live alone. Where two peoples trade, fight, or share a border,
their languages reach into one another — words cross over, and over centuries
even grammar drifts toward a common shape. English took *beef* from French and
*ski* from Norwegian; the languages of the Balkans, though unrelated, came to
build their futures and articles alike from living side by side. This chapter
gives your languages neighbours: how they borrow words from one another, and how
contact pulls a whole region toward a shared type.

#section("Borrowing: a word crosses over")

When a language takes a word from another, it rarely keeps it intact. The
borrowed word is *heard* through the ears of the receiving language and *bent* to
fit its rules. Japanese borrowed English *strike* and made it *sutoraiku* —
because Japanese has no *str-* cluster and no word-final *k*, so vowels were
inserted to break the sounds into syllables it allows.

#term("Loanword")[
  A word taken from one language (the *donor*) into another (the *recipient*),
  and adapted to the recipient's sound system. *Loanword* is itself a loan-
  translation of German *Lehnwort*.
]

#callout(label: "A loanword is a phonotactic repair")[
  The key idea mirrors the last chapter's. A loanword is not a free invention: it
  is the donor word *repaired* to obey the recipient's phonotactics (Chapter 6).
  The recipient perceives the foreign sounds as the nearest ones it has, then
  fixes any sequence its templates forbid. Borrowing reuses the very constraints
  you wrote for word-building.
]

#section("Declaring how a language borrows")

How a language nativises foreign words is declared in a `loan_phonology` block in
its *Phonology* chapter:

```hjson
{ loan_phonology: {
  repair: "epenthesis",          // epenthesis (insert a vowel) | deletion
  epenthetic_vowel: "u",         // empty → the first declared vowel
  substitutions: { "θ": "t", "r": "l" }   // a donor sound we lack → nearest native
} }
```

This says Eldar repairs illegal clusters by *inserting* a vowel (an /u/), and
that it hears the foreign sounds /θ/ and /r/ — which it does not have — as its
own /t/ and /l/.

#term("Epenthesis")[
  Inserting a sound (usually a vowel) to break up a cluster the language forbids,
  turning *str* into *sutu…*. The opposite repair is *deletion* — simply dropping
  the offending consonant. A language picks one strategy as its habit.
]

#term("Substitution")[
  Mapping a donor sound the recipient lacks onto the nearest sound it does have.
  A language with no *th* sound will hear it as *t*, *s*, or *f* depending on
  which is closest in its own inventory.
]

#section("Borrowing a word")

Give the donor form *phonemically* — one symbol per sound — and the recipient and
donor languages, and watch the adaptation:

```sh
inkhaven language borrow Eldar --form tras --from Drake             # tras → tulasu
inkhaven language borrow Eldar --form θuk --from Drake --gloss demon --yes
```

The adaptation runs in two steps you can read in the output. First *perceive*:
apply the substitutions, keep every sound the recipient already has, and map the
rest to the nearest native phoneme. Then *repair*: any consonant run longer than
the recipient's templates allow gets the epenthetic vowel (or loses a consonant,
if the language deletes). So *tras* — with an illegal *tr-* onset and final *-s*
cluster — becomes *tulasu*. With `--yes` and a `--gloss`, the adapted word joins
the recipient's dictionary, the donor recorded in its etymology, so your
borrowings stay historically coherent.

#section("When a whole region converges")

Borrowing moves words. *Contact* over long enough also moves *structure*:
unrelated languages sharing a region drift toward a common grammatical type — the
same word order, the same way of marking the subject — until a stranger could
mistake their blueprints for kin.

#term("Sprachbund (linguistic area)")[
  A group of languages that have grown structurally alike through long contact
  rather than common descent — a *language area*. The German term (\"language
  league\") is standard. The Balkans are the classic example: Greek, Albanian,
  and the local Slavic and Romance tongues share features none inherited.
]

You declare a language's membership in such an area with a `contact` block in its
*Grammar* chapter:

```hjson
{ contact: {
  region: "the Inner Sea"
  with: [ "Sindar", "Khuz" ]                                  // neighbours
  areal_features: { word_order: "sov", alignment: "ergative_absolutive" }
} }
```

#term("Areal feature")[
  A grammatical trait shared across a *Sprachbund* because of contact, not
  inheritance — the thing that spread. The shared subject–object–verb order of a
  language area is an areal feature.
]

#section("Reading the convergence")

`areal` holds your declared neighbourhood up against a language's own grammar:

```sh
inkhaven language areal Eldar      # one language's convergence overlay
inkhaven language areal            # the whole-region Sprachbund view
```

For each shared feature, the overlay reports where the language stands:
*converged* (it already has the feature, marked `✓`), *shift* (it answers
differently and would have to change, `→`), or *adopt* (it has no answer yet and
would gain one, `+`). This is strictly an *advisory overlay* — it shows you what
joining the area would mean and never rewrites your grammar. With no language
named, `areal` prints the regional view: every contact area, its members, and
each member's status on every shared feature. Contact also surfaces in the family
tree from Chapter 16, as *horizontal* edges (`Eldar ⇄ Sindar`) crossing the
vertical lines of descent — kinship and neighbourhood on one diagram.

#section("AI help for contact")

Two advisories round out the pillar, both keyed to your project's working
language and both write-nothing-without-`--yes`:

```sh
inkhaven language propose-loans Eldar --from Drake --topic seafaring --count 6
inkhaven language areal-check Eldar
```

`propose-loans` asks the AI what concepts a recipient would plausibly borrow from
a donor in a given domain — a seafaring people lending nautical words — and offers
donor forms for each; the *deterministic* borrowing engine then nativises every
one (so a donor *stɔrm* comes back as *sitarimi*), and you add the ones you like
with `borrow … --yes`. `areal-check` is the contact cousin of `realism-check`: it
judges whether a Sprachbund you have declared is typologically believable — the
kind of convergence real language areas actually show.

#recap((
  [Languages in contact *borrow words* and, over long enough, *converge in
   structure*.],
  [A loanword is a *phonotactic repair*: declare a `loan_phonology` block
   (`repair`, `epenthetic_vowel`, `substitutions`); `borrow` perceives then
   repairs the donor form (*tras → tulasu*).],
  [A *Sprachbund* is a region grown alike by contact; declare it with a `contact`
   block and read convergence with `areal` (✓ converged, → shift, + adopt).],
  [Contact shows as horizontal `⇄` edges in `family-tree`, beside the vertical
   lines of descent.],
  [`propose-loans` (AI) suggests borrowings the engine then nativises;
   `areal-check` (AI) judges a Sprachbund's plausibility. Both advisory,
   `--yes`-gated.],
))
