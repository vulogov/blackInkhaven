#import "../design.typ": *

#chapter(number: 2, title: "The sound of a line")

Everything else in this book stands on one operation: cutting a word into syllables and
finding which one is stressed. Metre is a pattern of stressed and unstressed syllables;
rhyme is an agreement of sounds from the stressed syllable onward; a haiku is a count of
syllables and nothing else. Get the syllables wrong and every measurement above them is
wrong too. So we begin at the bottom, with the humblest and most consequential tool in
the workbench: `poetry syllabify`.

#section("Counting syllables")

Ask Inkhaven to break a word:

```
$ inkhaven poetry syllabify extensive
  extensive          ex·ˈten·sive  (3 syl)
```

Three things happened. The word was *cut* into syllables at `ex·ten·sive`; the *count*
came back as three; and the stressed syllable was marked with `ˈ` before `ten`. Each of
those is a separate act of judgement, and it is worth knowing how each is made, because
each has failure modes a poet needs to see coming.

#subsection("Where the cuts fall")

The syllable boundaries come from #emph[hypher], the same hyphenation engine Typst uses
to break words at the end of a line — which means Inkhaven inherits a mature, per-language
hyphenation dictionary without adding a single dependency. Hyphenation and syllabification
are not quite the same thing (hyphenation is conservative — it would rather miss a break
than put one in the wrong place), but for showing you the shape of a word it is
excellent, and it already knows the rules of all five of Inkhaven's languages.

#term("Hyphenation dictionary")[
  A per-language set of rules (originally Liang's algorithm, as used in TeX) for finding
  the points where a word may be broken across lines. Inkhaven borrows Typst's bundled
  dictionaries — English, Russian, German, French, Spanish — to place syllable
  boundaries. Because the rules are conservative, the *cuts* can under-count; the
  *count* is done separately, below.
]

#subsection("Where the count comes from")

Here is a subtlety that trips up naïve tools: the number of syllable *cuts* is not
always the number of *syllables*. Hyphenation is cautious and sometimes declines to break
a word it nonetheless knows has two syllables. So Inkhaven does not count cuts — it counts
*vowel nuclei*, the syllabic peaks, because every syllable has exactly one. \"Fire\" may
resist hyphenation, but its vowels tell the true story.

Two language-specific rules ride on top:

- *English silent final-e.* English spells syllables it does not say. \"Extensive\" ends
  in a written `e` that is no syllable at all; \"make\" is one syllable, not two.
  Inkhaven drops the mute final `e` before counting, so English counts come out right
  where a pure vowel-counter would inflate them.
- *Every language keeps its own vowel set.* What counts as a nucleus differs by language
  — Russian's vowels are not English's, and `y` is a vowel in some positions and not
  others. The counter consults the right set for the declared language.

#callout(label: "Does it work in Russian?")[
  It works *better* in Russian. English orthography hides its sounds — silent letters,
  the same spelling for different vowels — so English syllable counts are the roughest of
  the five. Russian spelling is close to phonemic: a written vowel is a spoken vowel.
  Ask for `poetry syllabify --language ru стихотворение` and the answer —
  `сти·хот·ˈво·ре·ние (5 syl)` — is exact, because there is nothing hidden to guess at.
]

#section("Finding the stress")

A syllable count with no stress is half a measurement — you know how *long* the word is
but not its *shape*, and metre is all shape. Inkhaven marks the stressed syllable with
`ˈ`, and it finds it by a different route in each language, because stress lives in a
different place in each:

#table(
  columns: (auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (left, left),
  table.header(
    text(weight: "bold", size: 9pt)[Language],
    text(weight: "bold", size: 9pt)[Default stress rule],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [English],  [the penultimate syllable (a rough default — English stress is lexical and
               truly unpredictable from spelling)],
  [Russian],  [penult in two-syllable words, antepenult in longer ones — a heuristic, since
               Russian stress is famously mobile and unmarked],
  [German],   [the initial syllable — the Germanic default, right far more often than not],
  [Spanish],  [penult, unless the spelling (a final consonant, or a written accent) moves it],
  [French],   [phrase-final — French stress is not a property of the word but of the phrase,
               falling on its last full syllable],
)

#v(2mm)

These are *defaults*, and defaults are guesses. The honest ones — English and Russian —
are marked as heuristics above, because no rule reads lexical stress off spelling alone.
This is the roughest edge in the whole poetry layer, and the book will not pretend
otherwise. There are two recourses: the accent mark (next), and — for English — a
*pronouncing dictionary* you can install, which makes the guess exact (see the callout below).

#subsection("Overriding stress with an accent mark")

When the default is wrong — and in English or Russian it sometimes will be — you can tell
Inkhaven the truth by writing an acute accent over the stressed vowel in your text:
`presént` for the verb, `présent` for the noun. The engine sees the mark and takes it as
gospel, overriding whatever the default rule would have guessed. In Russian verse, where
poets and editors already mark stress on ambiguous words for exactly this reason, the
convention is native — write `стои́т` and the stress is fixed. The mark is for you and for
the tool at once: it disambiguates the poem *and* corrects the scan.

#callout(label: "The one honest limitation — and how to close it")[
  Read from spelling alone, English stress is a guess and English syllable counts are
  approximate. Russian, German, and Spanish are far more reliable; French, being
  phrase-timed, sidesteps the problem. Two recourses for English: mark your stresses with
  accents (above), or install a *pronouncing dictionary* — `inkhaven poetry phonemes import
  <cmudict>` loads the CMU Pronouncing Dictionary, after which syllable counts, stress, and
  rhyme are read from a word's actual phonemes rather than its spelling, exactly. Words not
  in the dictionary fall back to the heuristic, so nothing breaks. (The other four languages
  don't need it — their spelling already tracks their sound.)
]

#section("Scanning a whole line")

Feed `syllabify` a line rather than a word with `--line`, and it breaks each word in turn
— the raw material every later chapter builds on:

```
$ inkhaven poetry syllabify --line "Shall I compare thee to a summer's day"
  Shall              ˈShall  (1 syl)
  I                  ˈI  (1 syl)
  compare            ˈcom·pare  (2 syl)
  thee               ˈthee  (1 syl)
  to                 ˈto  (1 syl)
  a                  ˈa  (1 syl)
  summer's           ˈsum·mer's  (2 syl)
  day                ˈday  (1 syl)
```

Ten syllables, as a line of iambic pentameter should have. Two things to notice, both
honest. Every word carries a stress mark, monosyllables included — the standalone
syllabifier reports each word's *default* stress in isolation, with nothing to lean on but
the language rule, so \"compare\" comes back `ˈcom·pare` (the English penult default),
though in context the stress is really on \"-pare.\" And that is the deeper point: counting
is not yet scanning. Knowing there are ten syllables does not tell you they alternate
weak–strong in five neat feet, and the isolated per-word stress is only a first guess. The
scanner of the next chapter reads stress *in context*, and gets \"compare\" right — which
is where the poem starts to answer back.

#recap((
  [Syllabification underlies everything: metre, rhyme, and syllabic forms all stand on
   it. `poetry syllabify` is the tool.],
  [Cuts come from Typst's bundled #emph[hypher] hyphenation dictionaries (no new
   dependency); the *count* comes separately from vowel nuclei, so a cautious
   hyphenator's missed break never costs a syllable.],
  [English drops its silent final-e before counting; each language uses its own vowel set.
   Russian counts are near-exact because its spelling is near-phonemic; English is the
   roughest.],
  [Stress is found per language — penult, antepenult, initial, phrase-final — but English
   and Russian defaults are honest *heuristics*, not lexical truth.],
  [Write an acute accent over a vowel to override the guess and fix the stress — native
   convention in Russian verse, and your recourse in English until a phoneme dictionary
   lands.],
))
