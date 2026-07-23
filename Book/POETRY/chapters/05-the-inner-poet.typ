#import "../design.typ": *

#chapter(number: 5, title: "The Inner Poet")

The chapters so far handed you tools you point at one line at a time — scan this, rhyme
that. This one introduces a *reader*: a standing companion who takes in a whole stanza at
once and tells you how it sits against the form you declared. The Inner Poet is one of
Inkhaven's family of \"Inner\" readers — the Inner Editor, the Inner Socrates, the Inner
Theologian, and now the Inner Poet — each a second pair of eyes with a particular
expertise, each bound by the same iron rule: *observe, measure, report; never rewrite.*

#section("Two speeds of reading")

The Inner Poet reads at two speeds, and the distinction runs through the whole family.

The *fast track* is mechanical and instant. It needs no network, no model, no waiting — it
runs the scansion and rhyme engines of the last two chapters over every line of a stanza
and reports what it measures. It is deterministic: the same stanza yields the same findings
every time. This is the track you leave running as you write.

The *slow track* is reflective. It hands the poem to a language model with a carefully
bounded brief and asks for observations a ruler cannot make — where the imagery strains,
whether the volta turns, how the sound and the sense pull together or apart. It costs a
call and a moment's wait, and you invoke it deliberately, when you want a considered
second reading rather than a measurement.

#section("The fast track: findings")

On the command line the fast track is `poetry scan`; in the editor it is the Inner Poet's
instant read. Give it a stanza and its declared form, and it returns a list of *findings*:

```
$ inkhaven poetry scan --form heroic_couplet --language en \
    --text "The trees are green in summer and in spring
            The autumn leaves will fall upon the ground"
♪ Praise   [Metre]  Line 1 scans cleanly as iambic pentameter.
♪ Praise   [Metre]  Line 2 scans cleanly as iambic pentameter.
♪ Concern  [Rhyme]  Lines 1↔2 (A–A): “spring” / “ground” — do not rhyme.
```

Three findings, and they show the whole grammar of the fast track. Each carries a
*severity*, a *kind* (Metre or Rhyme), and a plain-language *message* tied to a line. The
couplet's two lines each keep their pentameter — earning Praise — but the form's rhyme
scheme said lines 1 and 2 must chime (A–A), and \"spring\" and \"ground\" do not. That is a
Concern, and it names exactly which promise was broken and where.

#subsection("The three severities")

The Inner Poet grades every finding on a three-step scale, and the scale is the whole
philosophy in miniature:

#table(
  columns: (auto, 1fr),
  stroke: none,
  inset: (x: 6pt, y: 3pt),
  align: (left, left),
  table.header(
    text(weight: "bold", size: 9pt)[Severity],
    text(weight: "bold", size: 9pt)[What it says — and does not say],
  ),
  table.hline(stroke: 0.5pt + ink_rule),
  [*Praise*],   [a line meets its declared target cleanly. The tool notices what *works*,
                 not only what fails — a reader that only ever complains is soon ignored.],
  [*Note*],     [a line departs from the target in a way worth seeing — a fit of 67%, a
                 line two syllables short. *Not* an error: a note is an observation you may
                 have meant, or may not.],
  [*Concern*],  [a promise is plainly broken — a declared rhyme that does not rhyme, a form
                 line count that cannot be right. The strongest word the tool will use, and
                 still only a flag, never a fix.],
)

#v(2mm)

Notice what is absent: there is no \"Error,\" no red, no imperative. The harshest thing the
Inner Poet will say is *Concern*, and even a Concern is a raised eyebrow, not a command.
Whether \"spring\" and \"ground\" ought to rhyme is your call — perhaps you are writing a
deliberately unrhymed couplet and the Concern is noise; perhaps you forgot the scheme and
it just saved you. The tool cannot know which, so it flags and stops.

#callout(label: "Praise is a feature, not politeness")[
  It would have been easy to build a tool that only reports problems. The Inner Poet
  reports successes too, and that is deliberate. A line that scans cleanly is *information*
  — it tells you the metre is holding, so that when a Note appears three lines later you
  know it is a real departure and not the tool finally waking up. Praise calibrates the
  silence around the warnings.
]

#section("Running the fast track in the editor")

Inside Inkhaven, the Inner Poet lives with the rest of the family under the Inner-reader
menu. Open it with `Ctrl+B J`, then press `P` for the Poet. On the poem paragraph under
your cursor, press:

- `F` — *fast check*. Runs the scan you just saw over the current stanza and posts its
  findings to the Output pane, each tied to its line, instantly and offline.
- `E` — *engage*. Starts the slow track (below).

The findings land in the Output pane under the `♪` mark, filterable like every other
reader's output, and they persist until you act on them — so a Concern does not scroll away
unaddressed. Nothing is written to your poem; the findings are notes *about* the text, set
beside it, never in it.

#section("The slow track: engaging the Poet")

Press `E`, and the Inner Poet reads reflectively. It sends the stanza — with its declared
form and the fast track's own findings for context — to a language model under a strict
system brief, and waits for a considered response. The brief is the constitution of the
feature, and it is worth knowing what it forbids:

- The model is told, in its opening instruction, that it is an *observer* — that it must
  describe and question, never rewrite, never hand back \"improved\" lines.
- It is asked for observations a ruler cannot make: whether an image earns its place,
  whether the argument of a sonnet actually turns at its volta, where sound reinforces
  sense and where it works against it.
- It answers in the poem's own language. Engage it on a Russian stanza and the observations
  come back in Russian, keyed to Russian prosody — the multilingual promise holds here as
  everywhere.

The response arrives as a *thought* in the Inner Poet's panel — prose for you to read and
weigh, headed `♪ Inner Poet`. It is advisory in the fullest sense: not a diff, not a
suggestion you accept or reject with a keystroke, but a reading you consider. The poem on
the page is untouched, and stays untouched, unless *you* change it.

#callout(label: "The line the Inner Poet will not cross")[
  This is the same principle that governs every AI feature in Inkhaven, and it is
  absolute: *the AI never edits your prose or your verse.* The slow track produces words
  about your poem, never a rewrite of it. There is no \"apply\" button, because there is
  nothing to apply — the Poet gives you a reading, and what you do with a reading is the
  one thing a workbench must always leave to the writer. If you want the poem changed, you
  change it, and then you can ask the Poet to read it again.
]

#recap((
  [The Inner Poet is one of Inkhaven's \"Inner\" readers — a standing companion that takes
   in a whole stanza and reports how it sits against the declared form. Same iron rule as
   the family: observe and report, never rewrite.],
  [It reads at two speeds: a *fast track* (deterministic, offline scansion + rhyme, run
   with `poetry scan` or `Ctrl+B J → P → F`) and a *slow track* (a bounded LLM reading, run
   with `E`).],
  [Fast-track *findings* carry a severity — *Praise* (a target met), *Note* (a departure
   worth seeing, not an error), *Concern* (a promise plainly broken). The harshest word is
   Concern, and even that is a flag, not a fix.],
  [Praise is deliberate: reporting what works calibrates the weight of the warnings.],
  [The slow track sends the poem to a model under a strict observer's brief, answers in the
   poem's own language, and returns a *reading* — never a rewrite. No AI feature in Inkhaven
   ever edits your verse.],
))
