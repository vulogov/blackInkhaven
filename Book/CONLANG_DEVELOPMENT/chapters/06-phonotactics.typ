#import "../design.typ": *

#chapter(number: 6, title: "Phonotactics: the rules of combination")

Templates say what shape a syllable has. *Phonotactics* says which actual
combinations of sounds are allowed inside that shape. Every language forbids
certain sequences: English allows *spr-* at the start of a word (*spring*) but
never *tlp-*; Japanese allows almost no consonant clusters at all. These rules
are a big part of why languages sound the way they do.

#term("Phonotactics")[
  The rules governing which sequences of sounds are permitted in a language —
  which consonants may cluster, which sounds may end a syllable, and so on. They
  are constraints layered on top of your syllable templates.
]

#section("Constraints")

You express these rules as a list of *constraints* in the phonology block, under
a `constraints` field. Each constraint has a `kind`, and some take extra
details. The most common kinds:

#term("Constraint")[
  A single phonotactic rule that rejects certain words. Constraints are checked
  whenever Inkhaven generates or validates a word; a word that breaks any
  constraint is not a legal word of the language.
]

#subsection("Limit cluster length")

A *cluster* is two or more consonants in a row. To cap how long clusters may be,
use `max_cluster_size`:

```hjson
{ kind: "max_cluster_size", value: 2 }
```

This forbids three-consonant pileups while still allowing pairs like *tr* or
*sk*. Setting the value to `1` forbids all clusters, giving a smooth,
open-syllabled language.

#subsection("Forbid doubled consonants")

A *geminate* is the same consonant written twice, like the *tt* in Italian
*gatto*. If you do not want them:

```hjson
{ kind: "no_geminate" }
```

#term("Geminate")[
  A doubled or lengthened consonant, as in Italian *notte* ("night"). Some
  languages use them meaningfully; many forbid them. `no_geminate` rules them out.
]

#subsection("Restrict what may end a syllable")

Often a language allows many consonants at the start of a syllable but only a
few at the end. Use `forbid_in_coda` (or its mirror `forbid_in_onset`) with a
class name. This is *syllable-aware* — Inkhaven works out the coda for you:

```hjson
{ kind: "forbid_in_coda", classes: ["Stop"] }
```

(This assumes you have declared a class named `Stop`; you can name a class for
any group of sounds you like, just as you named `C` and `V`.)

#subsection("Follow the sonority rule")

Real consonant clusters are not random: they tend to rise in *sonority* (roughly,
loudness or openness) toward the vowel and fall away after it. That is why *pla-*
feels natural and *lpa-* does not. Turn on this natural tendency with:

```hjson
{ kind: "sonority_sequencing" }
```

#term("Sonority")[
  How open and resonant a sound is. Vowels are most sonorous; then glides (w, y),
  liquids (l, r), nasals (m, n), and finally the quiet stops (p, t, k). The
  *sonority sequencing principle* says syllables tend to rise in sonority up to
  the vowel and fall afterward — a strong ingredient of a natural sound.
]

#section("Putting it together")

A complete phonotactics section for Eldar might read:

```hjson
constraints: [
  { kind: "max_cluster_size", value: 2 }
  { kind: "no_geminate" }
  { kind: "sonority_sequencing" }
]
```

Add it to the phonology block, reindex, and generate a fresh batch of words with
`generate-word`. You will notice the output is now cleaner — no triple clusters,
no doubled letters, and clusters that "lean" the natural way.

#callout(label: "Constraints are a sound designer's dials")[
  There is no single correct set. Tight constraints (short clusters, restricted
  codas) give a smooth, melodic language; loose ones give a rugged, clattering
  one. Generate words after each change and let your ear decide. The constraints
  are also the gatekeeper later: when you coin or import words, anything that
  breaks them is flagged automatically (Chapter 10).
]

#recap((
  [*Phonotactics* are the rules for which sound sequences are allowed.],
  [You write them as a list of `constraints`, each with a `kind`.],
  [Useful kinds: `max_cluster_size`, `no_geminate`, `forbid_in_coda` /
   `forbid_in_onset`, and `sonority_sequencing`.],
  [Tighter constraints sound smoother; looser ones sound rougher — tune by ear
   using `generate-word`.],
))
