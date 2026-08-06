#import "../design.typ": *

#chapter(number: 3, title: "Writing the First Chapters")

The world is sketched and the cast has names (Chapter 2), which means the part
of the work no tool can do for you is finally due: putting one word after
another. This chapter is a single morning of it. We draft the opening of *The
Ninth Lantern* — Mira Fenn walking out to the end of the Long Mole in the fog
and finding the ninth lantern cold — and, as we go, we meet the three surfaces
you will live inside for the whole draft: the *Editor* where the prose lands,
the *Search bar* that finds a line again a week later, and the *AI assistant*
you can lean on for a stubborn sentence. None of them writes the book. They keep
the writing frictionless, and they get out of the way.

#section("One paragraph, open in front of you")

You start where every scene starts: an empty paragraph. In the Tree you move the
cursor to the slot that will hold the opening — call it *The Cold Lantern* — and
press `Enter`. Focus jumps to the Editor, the (still empty) `.typ` file loads,
and a line-number gutter appears down the left. From here you type.

#callout(label: "The Editor holds one paragraph")[
  The Editor is not a scroll of the whole chapter — it holds a *single
  paragraph*, one `.typ` file, because a paragraph is Inkhaven's unit of prose,
  of snapshot, and of search. You move *between* paragraphs in the Tree; you
  write *within* one here. Opening the next one saves this one first, so the
  boundary costs you nothing.
]

An hour in, the opening looks like this:

#screen(caption: "The Editor — the opening of The Ninth Lantern taking shape")[```
┌─ Editor · The Cold Lantern [modified] · L5 C27 ─────┐
│  1  The ninth lantern had gone out in the night,   │
│  2  and Mira knew it before she reached the end    │
│  3  of the Mole, because the fret was already      │
│  4  ashore — a cold grey wall over the quay, the   │
│  5  way it never came while the light held.▏       │
└─────────────────────────────────────────────────────┘
```]

The pane is a running readout of where the words stand. The *border colour*
speaks while the Editor has focus: *green* means the buffer matches the file on
disk, *yellow* means you have unsaved edits. Unfocused, the border goes plain
and hands the signal to two always-on marks — the `[modified]` chip in the title
and the red `●` in the status bar — so you can tell a dirty paragraph from
anywhere in the window. The title also carries a live cursor read-out,
`L<row> C<col>`, the quiet coordinate you glance at when a reading tells you to
"jump to line 40."

Moving around is entirely conventional, which is the point — your fingers
already know it. Arrows move a cell or a line; `Ctrl+←` and `Ctrl+→` jump by
word; `Home` and `End` snap to the ends of the line; `Ctrl+Home` and `Ctrl+End`
leap to the top and bottom of the paragraph. When a scene runs several beats
separated by a break line — `* * *` between them — `Ctrl+B >` hops the cursor
forward to the next break and `Ctrl+B <` back to the previous one, so you walk
the beats without hunting for them. There is nothing here to unlearn.

#section("Watching the draft change")

Here is the first small kindness the Editor does you, and you will come to rely
on it without noticing. As you write, Inkhaven renders in *bold* every character
you have added since the paragraph was last saved. It is a running visual diff
against the version on disk: at a glance you see exactly what *this* pass has
touched. Say you had saved after "…the fret was already ashore," and then, still
picking at the sentence, you kept going. The clause you just added stands out:

#screen(caption: "Bold marks what is new since the last save")[```
┌─ Editor · The Cold Lantern [modified] · L5 C27 ─────┐
│  1  The ninth lantern had gone out in the night,   │
│  2  and Mira knew it before she reached the end    │
│  3  of the Mole, because the fret was already      │
│  4  ashore — a cold grey wall over the quay, the   │
│  5  way it never came while the light held.▏       │
│                                                     │
│  bold on screen (new since save): from the em      │
│  dash on line 4 to the cursor.                     │
└─────────────────────────────────────────────────────┘
```]

The bolding *clears the moment you save* — bold means "new since disk," and a
save makes buffer and disk agree, so there is nothing left to mark. Watching the
bold dissolve on `Ctrl+S` is the simplest confirmation your work has landed.

You will reach for `Ctrl+S` less than you expect, because Inkhaven also saves for
you at three natural moments: after a few seconds idle, when you open another
paragraph, and the instant focus leaves the Editor. Moving around the window
therefore costs you no work — the deliberate save is there for the moments you
want the confirmation. And every save does one more thing you will cash in
shortly: it re-embeds the paragraph into the semantic index, so a line you write
this morning is *findable* this afternoon.

#section("A snapshot before a risky change")

You reread the opening and the first line nags. *"The ninth lantern had gone out
in the night"* is fine, but you wonder whether starting on the fog — the thing
the reader can feel — lands harder. This is a real rewrite, the kind you might
regret, so before you touch it you take a *snapshot*.

#term("Snapshot")[
  A *snapshot* is a point-in-time bookmark of a paragraph. `F5` takes a fresh
  one; `F6` opens the picker of every snapshot for this paragraph, to load,
  compare, or restore. Snapshots are how you experiment without fear: capture
  the version you have, try something bolder, and if the bolder thing is worse,
  the earlier one is one keystroke away.
]

Press `F5` and the current opening is safe. Now you rewrite the first two lines
outright. To see where you came from while you work, `F4` splits the Editor: your
live, editable buffer sits *above*, and a frozen copy of the pre-`F4` version sits
*below* in dim grey.

#screen(caption: "F4 split-edit — the new opening above, the old frozen below")[```
┌─ Editor · The Cold Lantern [modified] ──────────────┐
│  1  The fret was ashore before the light failed —  │
│  2  Mira felt it on the Mole, and knew.▏           │
├─ snapshot · line 1/5 · Ctrl+H/J scroll ─────────────┤
│  1  The ninth lantern had gone out in the night,   │
│  2  and Mira knew it before she reached the end    │
└─────────────────────────────────────────────────────┘
```]

You read the two side by side and decide the new version is too clever — it
gives away the fog's menace before the lantern has even earned it. So you throw
it back: `Ctrl+F4` *accepts the snapshot*, replacing the live buffer with the
frozen original wholesale. (Had you liked the rewrite, you would simply have
pressed `F4` again to close the split and keep it.) Nothing was lost either way,
because you bookmarked before you leapt — and even without the split, `F6` would
have offered that same snapshot back. This is the habit worth building: `F5`
before any change you are not sure of, and experiment freely.

#section("Finding that line about the fog, later")

A week of drafting later, you are three chapters on and you want *that line about
the fog rolling in cold off the water* — you can feel the shape of it, but you
have written a dozen foggy mornings since and you cannot remember which paragraph
holds this one, or how you actually phrased it. This is the job semantic search
was built for, and it is one keystroke away: `Ctrl+/` (or `Ctrl+4`) focuses the
Search bar.

You do not type keywords. You *describe the passage* — the fuller the query, the
sharper the result:

#screen(caption: "A query by feeling, not by exact words")[```
┌─ Search ───────────────────────────────────────────────────┐
│ the morning the fret came ashore and the ninth lantern     │
│ was found cold▏                                            │
└─────────────────────────────────────────────────────────────┘
```]

Press `Enter`, and a ranked overlay drops down — the nearest paragraphs across
the *whole* project, best match first:

#screen(caption: "Results ranked by meaning — the prose, and the note that fed it")[```
┌─ Results · "…the ninth lantern was found cold" (4) ────────┐
│  0.912  [paragraph] Lantern › One › the-cold-lantern       │
│         The Cold Lantern                                   │
│         The ninth lantern had gone out in the night…       │
│                                                            │
│  0.861  [paragraph] Lantern › One › out-along-the-mole     │
│         Out Along the Mole                                 │
│         She went past the eighth pillar, where the fret…   │
│                                                            │
│  0.834  [note]      Notes › the-sea-fret                   │
│         What the fret is                                   │
│         A cold fog off the deep water; the town believes…  │
├────────────────────────────────────────────────────────────┤
│ ↑↓ select · Enter open · Esc close                         │
└────────────────────────────────────────────────────────────┘
```]

Look at what just happened. Your query said *fret came ashore*; the top
paragraph says *"had gone out in the night."* Your query said *found cold*; the
paragraph names no such thing. The words are strangers, yet it is unmistakably
the passage you meant — because semantic search matches by *meaning*, not by
characters, and the meanings are neighbours. It even surfaces the worldbuilding
note on the sea-fret you took days ago, alongside the scene it informed, because
both were indexed the same way. `↑` and `↓` move the selection; `Enter` on a hit
*opens that paragraph in the Editor* and repositions the Tree onto it, so one
keystroke takes you from "the scene where the fog comes in" to the cursor
blinking inside it.

#callout(label: "Semantic search, not literal — know which you want")[
  The Search bar finds passages by *sense*. When instead you want a *specific
  string* — a character's exact name, a coined term, a phrase you are hunting to
  replace — that is a different tool: `Ctrl+F` runs a literal regex find inside
  the open paragraph, and `rg` over your `books/` folder searches the files on
  disk. Meaning-based for "the passage about a thing"; literal for "every place
  I typed these exact letters."
]

#section("Steering the assistant to tighten a line")

There is a third party you can call to the desk without leaving the window: a
language model in the AI pane. Inkhaven's stance on it is one line — *AI is a
co-author you steer* — and every part of the pane is built around that word. The
assistant never acts on its own, never sees more of your book than you hand it,
and *never changes a word of your prose until you press the key that says so.*

Back in the opening paragraph, one sentence still sags. *"— a cold grey wall over
the quay, the way it never came while the light held"* is two ideas wearing one
sentence. You could worry it yourself; this once, you ask for a second pair of
eyes. Steering the model is three small dials.

First, *scope* — what the model is allowed to see. By default it sees only your
prompt; you attach a slice of the manuscript with `F9`, which cycles a ring of
scopes. You select the sagging sentence in the Editor (`Shift`+arrows), then press
`F9` until the pane reads `scope=Selection` — now the next prompt carries exactly
that sentence and nothing more.

Second, *mode* — how much of its own training the model may lean on. `F10`
toggles between `Full` (its general knowledge is fair game) and `Local` (use
*only* what I have shown you). For "tighten *this* sentence," you want `Local`:
the job is to reshape your words, not to reach for someone else's.

Then you type. `Ctrl+I` drops focus to the prompt bar; you ask plainly and press
`Enter`. The answer streams in live — you watch it arrive:

#screen(caption: "A scoped, local-mode ask — the model reshaping your own words")[```
┌─ AI · ollama · streaming… · infer=Local · scope=Selection ──┐
│  you  Tighten this to one clean line. Keep the word "cold".  │
│  ai   The fret was already ashore, cold off the deep        │
│       water, and it never came while the light ▏            │
└─────────────────────────────────────────────────────────────┘
```]

When it finishes, the title flips to `done` and a row of *destination chips*
appears in the footer. Focus is still on the prompt bar — handy for a follow-up
question — but the chip keys are live only *inside the AI pane*. Press `Esc` to
step from the prompt bar into the pane. Nothing has touched your buffer yet — the
answer is a proposal, inert until you choose:

#screen(caption: "Done — the scope has reset, and the proposal waits for a key")[```
┌─ AI · ollama · done · infer=Local ──────────────────────────┐
│  ai  The fret was already ashore, cold off the deep         │
│      water, and it never came while the light held.         │
│                                                             │
│  r replace · i insert · t top · b bottom · c copy           │
└─────────────────────────────────────────────────────────────┘
```]

You read it against your own. It is tighter, it keeps *cold*, and it is
genuinely better — so you accept it. Now in the pane, `r` *replaces the
selection* with the model's text, converting its markdown to Typst on the way
in, and jumps focus back to the Editor so you land where the change did. And here the two threads of
this chapter meet: because an apply marks the buffer dirty, the new sentence
arrives *bold* — the same "new since save" diff you watched earlier — so you can
read exactly what changed against what is still on disk:

#screen(caption: "Back in the Editor — the accepted line, bold as a change you can still undo")[```
┌─ Editor · The Cold Lantern [modified] · L4 C52 ─────┐
│  1  The ninth lantern had gone out in the night,   │
│  2  and Mira knew it before she reached the end    │
│  3  of the Mole, because the fret was already      │
│  4  ashore, cold off the deep water, and it never  │
│  5  came while the light held.▏                    │
│                                                     │
│  bold on screen (new since save): the replaced     │
│  sentence — Ctrl+U to revert, Ctrl+S to keep.      │
└─────────────────────────────────────────────────────┘
```]

That is the whole contract. Had the proposal been wrong, `Ctrl+B C` would have
cleared it unapplied, or you would simply have never pressed `r`; and even after
accepting, `Ctrl+U` walks the change straight back out — the edit is yours to
approve or reject, always. The model advised; you decided. Note too that
`Local` mode governs only what the model *draws on*, not where the call runs — on
an external provider your prompt still travels to their servers; if you want the
whole exchange to stay on your machine, that is a *provider* choice (run Ollama),
which is exactly what the pane above is doing.

#callout(label: "The assistant advises; you hold the key")[
  This is the one habit to internalise about AI in Inkhaven, and it holds
  everywhere the assistant touches prose: the pane only ever *shows* you
  something. A separate, deliberate keystroke — `r`, `i`, `t`, `b`, or the `g` of
  a grammar-check — is what lets it land, and the change then arrives as a visible
  diff you can undo. No word of your manuscript ever changes behind your back.
]

#two_track(
  [Reach for the assistant the way you just did: a *scoped, local* ask to reshape
  a line you have already written — tighten a sentence, vary a repeated verb, cut
  a clause. It is a sharpening stone for your own prose, never a ghostwriter; the
  words that land are the ones you accept.],
  [The same move serves an argument: select a clause and ask, in `Local` mode, to
  make it *precise* rather than merely shorter — collapse a hedged claim into one
  clean statement, or surface a buried qualifier. You steer, you read the diff,
  you accept only what sharpens the point.],
)

#recap((
  [The *Editor* holds *one paragraph* — a plain `.typ` file — and shows its save
  state three ways: a green (saved) or yellow (dirty) focused border, the
  `[modified]` title chip, and the red `●` in the status bar. Movement is
  conventional; `Ctrl+B >` / `Ctrl+B <` hop between scene breaks.],
  [Inkhaven renders in *bold* every character added since the last save — a
  running diff against disk that clears the moment you `Ctrl+S`. It also autosaves
  on idle, on paragraph switch, and on focus loss, re-embedding as it goes.],
  [Before a risky change, `F5` takes a *snapshot*; `F4` splits the Editor to edit
  against the frozen version (`Ctrl+F4` accepts it back), and `F6` reopens any
  snapshot later. Experiment freely — the earlier version is one key away.],
  [`Ctrl+/` (or `Ctrl+4`) opens *semantic search*: describe the passage by feeling
  and it finds the paragraph even when your query shares no words with it. Use
  `Ctrl+F` or `rg` instead when you want a specific literal string.],
  [The *AI assistant* is a co-author you steer with three dials: `F9` sets *scope*
  (what it sees — here `Selection`), `F10` sets *mode* (`Local` = only your
  context), and — once you `Esc` from the prompt into the pane — a *destination
  chip* (`r` to replace) lands the answer. It advises in the pane and changes
  nothing until you press the key — and the result arrives as a bold, undoable
  diff.],
))
