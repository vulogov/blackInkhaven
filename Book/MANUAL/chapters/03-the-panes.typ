#import "../design.typ": *

#chapter(number: 3, title: "The Four Panes")

Everything in the last two chapters happened somewhere on one screen. Now we
name the places. Inkhaven fills your terminal edge to edge with a single
full-screen window, and the whole of writing a book takes place inside it —
you never open a second app, never reach for a mouse you do not need, never
lose your place. This chapter is the map of that window: the four regions it is
divided into, how you move your attention between them, and the two-key *chord*
language that reaches every command the tool has. Learn this chapter and the
rest of the manual becomes a matter of looking things up; skip it and every
later chapter will feel like a room you entered through the wrong door.

#section("One window, four regions")

Open a project and the terminal divides into four working areas. On the left, a
narrow *Tree* holds the shape of your book — its parts, chapters, and
paragraphs, folded and unfolded like an outline. Filling the centre is the
*Editor*, where the open paragraph's words live. To its right sits a region
that shows one of three things at a time — most often the *AI* pane, a
conversation with your assistant. Across the bottom run two thin input bars — a
Search bar and the AI prompt — and beneath them a single *status bar* summing
up the moment. That is the whole instrument.

#term("Pane")[
  A *pane* is one of the window's working regions — the Tree, the Editor, the
  AI pane, and the others. Exactly one pane has *focus* at any moment: the pane
  your keystrokes are talking to. Everything you type is read as input to the
  focused pane (or, when a chord is in flight, as a command).
]

The book you are reading calls this "the four panes" for the shape you meet
first — Tree, Editor, AI, and the region beside the Editor — but the window is
really a small family of surfaces, and the right-hand region alone can show
three different panes. Hold the four in mind as the frame; the rest hang off it.

#screen(caption: "The whole window — Tree, Editor, and the right region")[```
┌─ Tree ────────────┬─ Editor · the-quay [modified] ──────────────┐
│ ▾ Rain            │  1  The rain came sideways off the harbour, │
│   ▸ 1 Arrival     │  2  and Mara counted the lamps as they went │
│   ▾ 2 The City    │  3  dark, one by one, the length of the     │
│     ► the-quay    │  4  quay. Six days' ride still lay ahead.   │
│     ¶ the-inn 88w │                                             │
│   ▸ 3 Departure   ├─ AI · llama · done · scope=Paragraph ───────┤
│ ▸ Facts           │  you  Is the pacing of this opening too     │
│ ▸ Notes           │       slow for a first page?                │
│                   │  ai   The image is strong, but four beats   │
│                   │       land before any question is raised…   │
├───────────────────┴─────────────────────────────────────────────┤
│ search: ▏                                  AI prompt · scope …  │
├─────────────────────────────────────────────────────────────────┤
│ 2 The City ▸ the-quay   ● 214w   scope=None   infer=Local       │
└─────────────────────────────────────────────────────────────────┘
```]

Read it top to bottom. The Tree names the book; the green `►` marks the
paragraph currently loaded in the Editor, wherever your tree cursor happens to
be. The Editor holds that paragraph's lines with a number gutter down the left.
The right region — here showing the AI pane — carries the conversation. The two
bottom bars wait for a search query and an AI prompt. The status bar, last of
all, tells you where you stand: the breadcrumb to the open paragraph, its word
count, the red `●` that means unsaved edits, and the current AI scope and
inference mode.

#callout(label: "Terminal-first")[
  Because it is only text, this window runs anywhere a terminal does — over
  SSH, inside `tmux`, on a bare tiling window manager, on a laptop on a train
  with the lid half-closed. There is no separate GUI, no browser tab, nothing
  to install beyond the one binary. The frames in this book are faithful to
  what you will actually see.
]

#section("The right-hand region — three panes in one place")

The region to the right of the Editor is not a single pane but a slot that
holds *one of three* at a time. This is the part of the layout newcomers find
surprising, so it is worth naming plainly.

#term("The right region")[
  The right-hand slot cycles between three panes: the *AI pane* (a
  conversation you drive), the *Output pane* (a notice board subsystems post
  to), and the *Thoughts pane* (a quiet home for long reflective text). Only
  one is visible at a time; switching between them is a single chord.
]

The three are three different *kinds* of surface, which is why they are three
panes and not one. The AI pane is a *dialogue* — you ask, the model answers,
the history scrolls. The Output pane is a *notice board* — structured, one-way
messages that the fact-checker, the continuity watch, the translation engine,
a finished background job, or a Bund script post for you to read, act on, and
dismiss; each carries a severity glyph (`●` info, `⚠` warning, `⊗`
contradiction, `↻` still-running) and expires on its own so the board never
becomes a junk drawer. The Thoughts pane is a *reading* surface — a scrollable,
read-only place where the longer, essayistic output lands, such as the Inner
Theologian's questions or a developmental brief. Because you read there rather
than act, its keys are few: `y` copies the whole reflective transcript to the
system clipboard — to carry a session out into a journal or a note — while `c`,
the one destructive key, clears the pane.

#screen(caption: "The right region showing the Output pane — a notice board")[```
┌─ Output · 3/4 · fact-check ─────────────────────────┐
│ ⊗ fact_conflict                                     │
│   ▌"they reached Rillmark by nightfall" — the bible │
│    puts Rillmark six days' ride away.               │
│ ● review_complete                                   │
│   3 checks clean · 1 contradiction · 0 warnings     │
│ ⚠ uncovered_words                                   │
│   2 word(s) uncovered: sword, sun                   │
├─────────────────────────────────────────────────────┤
│ ↑↓ select  o expand  Enter jump  a ask-AI  d dismiss │
└─────────────────────────────────────────────────────┘
```]

Which pane the slot shows when you launch is remembered: on a project's very
first start it opens on the Output pane, and after that Inkhaven restores
whichever of the three you last left active, exactly as it restores which
paragraph was open. When fresh content arrives for a pane it will quietly
surface that pane for you — unless you are actively working in a right pane at
the time, in which case it holds still rather than steal your place mid-read.

Findings arrive newest-first, at the top of the board, and the Output pane is
built so that a stream of them never moves the ground under your feet. Your
selection is anchored to the *message itself*, not to a row number: as new
findings push in above, the cursor stays on the very finding you were reading,
wherever it slides to. While the cursor rests on the top row the pane *follows
the newest* arrival — a small `⟳follow` marker lights in its title bar — so a
running check keeps the freshest finding under your cursor; move the selection
down and following stops, return to the top and it resumes. Dismissing a
finding with `d` keeps you on the row that shifts up into its place rather than
flinging the cursor back to the top. And `Enter` on *any* finding that carries a
source paragraph — a fact-check warning, a continuity note, a Socratic question —
opens that paragraph in the Editor, which is what turns the board from a log into
a worklist you can act down. Findings with their own primary action (accept a
lexicon proposal, insert a translation, jump to a timeline event) still do that
on `Enter`.

#section("Moving your attention — focus and navigation")

Only one pane listens to your keys at a time. Moving that attention around is
the most common thing you will do, so Inkhaven gives you both a cycle and a set
of direct jumps.

#subsection("The plain cycle — Tab")

The everyday key is `Tab`. It walks focus through a short ring — the Tree, the
Editor, and *whichever right pane is currently shown* — then back to the Tree.
It is a three-stop loop, not a tour of every surface: the right region counts
as one stop, so if the slot is on the Output pane, `Tab` visits the Output
pane; if it is on the AI pane, `Tab` visits the AI pane. `Shift+Tab` walks the
same ring in reverse. You will reach for `Tab` far more than any other
navigation key, and because Inkhaven intercepts it before the text editor sees
it, pressing `Tab` never inserts a literal tab into your prose.

#screen(caption: "Tab cycles a three-stop ring")[```
        Tab →           Tab →              Tab →
  ┌──────────┐    ┌──────────┐    ┌──────────────────┐
  │   Tree   │ →  │  Editor  │ →  │  right pane      │ →  (Tree)
  └──────────┘    └──────────┘    │ (AI/Output/…)    │
        ← Shift+Tab                └──────────────────┘
```]

#subsection("Cycling the right region — Ctrl+B Tab")

To change *which* pane the right slot shows, you cycle the region itself with
`Ctrl+B Tab`. Each press advances the slot through the three panes in order —
Output, then AI, then Thoughts — and moves focus onto it; `Ctrl+B Shift+Tab`
steps backward. This is a different motion from plain `Tab`: `Tab` moves your
focus *to* the region, `Ctrl+B Tab` changes *what the region is*. The chord
works from anywhere — even mid-edit — because the `Ctrl+B` prefix (which you
will meet properly in a moment) swallows the keystroke before the editor could
read `Tab` as a tab.

#subsection("Jumping straight to a pane")

When you know exactly where you want to be, skip the cycle and jump. Each of
the main surfaces has a direct key, so no pane is ever more than one chord away.

#chord_table((
  chord_row("Tab", "Cycle focus: Tree → Editor → current right pane → Tree."),
  chord_row("Shift+Tab", "Cycle focus in reverse."),
  chord_row("Ctrl+B Tab", "Cycle the right region Output → AI → Thoughts, and focus it."),
  chord_row("Ctrl+B Shift+Tab", "Cycle the right region backward."),
  chord_row("Ctrl+T", "Focus the Tree pane."),
  chord_row("Ctrl+1", "Focus the Editor pane."),
  chord_row("Ctrl+3", "Focus the AI pane."),
  chord_row("Ctrl+/", "Focus the Search bar (top). Ctrl+4 does the same."),
  chord_row("Ctrl+I", "Focus the AI prompt bar (bottom). Ctrl+5 does the same."),
))

#subsection("How you know which pane has focus")

Focus is never a guess — every pane shows it. The Editor's border carries the
signal most vividly: *green* when the pane is focused and saved, *yellow* when
focused with unsaved edits, and plain *white* when it is not focused at all. In
the Tree, the open paragraph keeps its green `►` marker whether or not the Tree
has focus, so you can always find your place. And when you have started a chord
but not finished it, a yellow *META* chip lights up in the status bar to tell
you the tool is waiting for your next key. Between the border, the marker, and
the chip, the window always tells you what it is listening to.

#callout(label: "Focus-loss autosave")[
  Whenever focus leaves the Editor — by `Tab`, by a direct jump, by `Esc` from
  another input — the open paragraph is saved for you if it was dirty. You can
  shift your attention mid-sentence without a thought for losing work; the words
  are on disk before the next pane has your keystrokes.
]

#section("The chord system — the heart of the interface")

Inkhaven has more commands than a keyboard has keys, and it refuses to bury the
useful ones behind menus. Its answer is the *chord*: a short, deliberate
two-key sequence, led by a prefix that tells the tool "a command is coming."
Once the idea clicks, the whole interface opens up, because every serious
action in the program is one chord away from wherever you are.

#term("Chord")[
  A *chord* is a two-key command: a *prefix* key, then an *action* key. You
  press the prefix, release it, then press the second key — they are struck in
  sequence, not held down together. Between the two presses the tool is in a
  brief *pending* state, waiting for the action key and showing you it is
  waiting.
]

There are two prefixes, and the difference between them is a genuine
distinction of *kind*, not an accident of which keys were free.

#subsection("Ctrl+B — the meta prefix")

`Ctrl+B` is the *meta* prefix: it introduces commands that *act on your book* —
add a chapter, take a snapshot, run a check, tag a paragraph, translate a line.
Press `Ctrl+B` and release it, and the tool enters *meta mode*: the status bar
lights its yellow *META* chip and prints the actions available for the pane you
are in. The next key you press selects one of them. `Esc` backs out of a
pending chord without doing anything, and any key the table does not recognise
cancels with a hint naming which pane's table it consulted.

#term("Meta prefix")[
  `Ctrl+B` — the key that opens *meta mode*. The single most important thing to
  learn about it is that the action table is *pane-specific*: `Ctrl+B S` means
  *read the paragraph aloud* in the Editor, *add a subchapter* in the Tree, and
  *clear the inference* is one of the AI pane's few actions. The same second
  key, read against a different pane, is a different command.
]

#screen(caption: "Meta mode pending — the status bar prompts for the second key")[```
┌─ Editor · the-quay [modified] ──────────────────────┐
│  1  The rain came sideways off the harbour, and…    │
│                                                     │
├─────────────────────────────────────────────────────┤
│ META  S read-aloud · N snapshot · R cycle-status ·  │
│       T retitle · P place-RAG · C char-RAG · H help │
└─────────────────────────────────────────────────────┘
```]

That the table is pane-specific is not a complication to memorise but a
simplification to lean on. You do not learn one enormous list of chords; you
learn the handful that belong to the pane you are working in, and the same
letters mean the natural thing in each place. In the Tree, `Ctrl+B C`, `S`, `P`
add a *chapter*, a *subchapter*, and a *paragraph* — the second key is the
word's first letter — while adding a *book* is the bare Tree key `B` (it has no
meta-chord, since `Ctrl+B B` builds). In the Editor, `Ctrl+B N` takes a *new*
snapshot
and `F6` opens its *history* (`Ctrl+B R` cycles the paragraph's status). A last tier of meta chords — the ones that
run whole-book intelligences, like `Ctrl+B Shift+C` to run every fast checker at
once — work from *any* pane, because they belong to the book rather than to a
place in it.

#subsection("Ctrl+V — the view prefix")

`Ctrl+V` is the *view* prefix, and it introduces a different kind of command:
things that *show you a view* or *reach a tool* rather than change the
manuscript's structure — export this paragraph to markdown, open the fuzzy
paragraph picker, float a rendered preview, jump a bookmark, run the Editorial
Pass, open the command palette. Where `Ctrl+B` is about *acting on the book*,
`Ctrl+V` is about *looking through it* and reaching the pickers and panels that
help you look. It reads the same way — press and release `Ctrl+V`, then the
action key — and like the meta layer it is fully rebindable.

The mnemonic is worth stating out loud, because it is what lets you *guess* a
chord correctly instead of looking it up: `Ctrl+B` for the *book* (things you
do to it), `Ctrl+V` for the *view* (things you do to see it). Nearly every
chord you meet in this manual sits under one of these two prefixes, and the
prefix tells you which half of your mind the command lives in.

#subsection("Overlays — the modal stack on top of it all")

Some chords do not run and return; they *open something* — a picker, a
confirmation, a full-screen editor for your config, a floating preview. These
are *overlays*, and while one is up it sits on top of the panes and takes your
keys for itself. The pane beneath keeps its visual focus but does not see input
until the overlay closes, and `Esc` is the near-universal way to close the top
overlay and drop back to what was underneath. Overlays stack: a picker can open
a confirm on top of itself, and closing the confirm returns you to the picker.
This is why `Esc` is the key you can always press when you feel lost — it peels
one layer off the top and never does anything destructive.

#section("The status bar and the title bars")

The bottom line and the little labels along the tops of panes are not
decoration; they are a continuous, quiet readout of state. Learning to glance at
them is most of what "knowing where you are" amounts to.

#subsection("The status bar")

The status bar is the window's single most information-dense line. In calm
moments it shows the breadcrumb path to your cursor, the open paragraph's word
count, a red `●` when there are unsaved edits, and the current AI scope and
inference mode. In busy moments it is also where transient messages land — a
search reporting `match 1 / N`, a diagnostic jump reading `diag 2/5 line 40:12`,
a `marked N` count while you multi-select in the Tree, the yellow *META* prompt
while a chord is pending, or a notice that a file changed on disk and was
reloaded. It is the first place to look when you wonder what just happened.

#screen(caption: "The status bar — a running readout of the moment")[```
 2 The City ▸ the-quay    ● 214w    scope=None    infer=Local
 └── breadcrumb ──┘   dirty ┘   │        │             └ Local / Full
                     word count ┘        └ AI scope (F9)
```]

Optional chips ride the same line when you turn them on — a POV chip naming the
paragraph's viewpoint character, a `lang=` chip for the prompt language, a live
syllable readout while a verse paragraph is open. They are quiet by default and
each has its own toggle; the point is that the status bar is the one place
Inkhaven speaks to you without being asked.

#subsection("The title bars")

Every pane wears a thin title along its top, and each title earns its space. The
Editor's names the open paragraph and appends `[modified]` when it is dirty. The
Tree's is simply the pane's name. The right region's title tells you which of
the three panes you are looking at and its live state — the AI pane's reads like
`AI · llama · done · infer=Full · scope=Paragraph`, folding in the provider, the
stream state, the inference mode, and the scope; the Output pane's shows a
`shown/total · filter` count; the AI prompt bar's title echoes the scope as
`AI prompt · scope: Paragraph`. You rarely read them word for word, but they are
the reason you are never guessing which mode a pane is in.

#section("Finding a chord — the palette and the quick reference")

Two prefixes and a pane-specific table is a system you can *reason* about, but
nobody memorises every chord, and Inkhaven does not ask you to. Three surfaces
exist precisely so you can find a command without remembering its keys.

#subsection("The command palette — Ctrl+V Space")

`Ctrl+V Space` opens the *command palette*: a fuzzy search over the entire
keybind registry. Start typing any part of a command's name, its chord, or its
description, and the list narrows to match; `↑↓` moves the selection, `Enter`
runs it, `Esc` closes. This is the fast way to reach *any* command without
knowing its chord — type "snapshot", or "fact", or "outline", and run what comes
up. It is also how you *learn* the chords: the palette shows each command's key
beside its name, so the thing you searched for by word today you will strike by
chord tomorrow.

#screen(caption: "Ctrl+V Space — fuzzy-find any command in the registry")[```
┌─ Command palette ───────────────────────────────────┐
│ > snap▏                                             │
├─────────────────────────────────────────────────────┤
│ ▌New snapshot of the buffer            Ctrl+B N / F5 │
│  Snapshot history picker               Ctrl+B R / F6 │
│  Split-edit against a snapshot         Ctrl+B F / F4 │
├─────────────────────────────────────────────────────┤
│ type to filter · ↑↓ select · Enter run · Esc close  │
└─────────────────────────────────────────────────────┘
```]

#subsection("The quick reference — Ctrl+B H, and ? from the Tree")

Where the palette is for *running* a command you can half-name, the *quick
reference* is for *browsing* what is available from where you stand. `Ctrl+B H`
opens it from any pane — a floating, scrollable panel of the chords that apply,
pane-aware so it shows you the ones that matter here first. Scroll with the
arrows or `PgUp` / `PgDn`; `Esc` closes it. When the Tree has focus you can also
open it with a bare `?`, since the Tree is the one pane where `?` is not a
character you would type; in the Editor and the input bars `?` stays a literal
question mark, and `Ctrl+B H` is the way in.

#chord_table((
  chord_row("Ctrl+V Space", "Command palette — fuzzy-find and run any command by name, chord, or description."),
  chord_row("Ctrl+B H", "Quick reference — the pane-aware chord panel, from any pane."),
  chord_row("?", "Quick reference, but only when the Tree pane has focus."),
  chord_row("Ctrl+B V", "Version / credits pane — the running version, author, licence, and dependencies."),
))

Between them the two surfaces cover both ways a person forgets a chord. When you
know *what you want* but not the keys, reach for the palette. When you want to
*see what is possible* from where you are, open the quick reference. Neither
asks you to leave the window or read this book.

#section("Fullscreen and focus modes")

The four-pane layout is the working default, but sometimes you want less on the
screen — the prose alone while you draft, or one right pane blown up while you
read a long result. Inkhaven has a mode for each, and each is a toggle: press to
enter, press again to return.

#subsection("Focus mode — Ctrl+B Shift+W")

`Ctrl+B Shift+W` enters *focus mode* — the distraction-free layout. It hides the
Tree, the AI pane, the Search bar, and the AI prompt, and gives the whole window
to the Editor, forcing focus there as it opens. It is the mode for the moment
you have decided to stop tending the book and simply write it: nothing on screen
but your paragraph. Press `Ctrl+B Shift+W` again to restore the four panes. (The
setting is called "typewriter mode" in a few older strings and in the config
file, for backward compatibility — same mode, the friendlier name won.)

#screen(caption: "Focus mode — the Editor alone, everything else hidden")[```
┌─ Editor · the-quay ─────────────────────────────────┐
│  1  The rain came sideways off the harbour, and     │
│  2  Mara counted the lamps as they went dark, one   │
│  3  by one, the length of the quay. Six days' ride  │
│  4  still lay ahead, and the city behind her had    │
│  5  already forgotten her name.                     │
│                                                     │
│                                                     │
│                                                     │
└─────────────────────────────────────────────────────┘
```]

#subsection("Fullscreen panes — Ctrl+Z f and Ctrl+B K")

When it is a *result* you want to sink into rather than the prose, fullscreen
the right pane instead. `Ctrl+Z f` blows up the currently shown right pane — the
Output pane or the Thoughts pane — to fill the window, for reading a long notice
board or a developmental brief without the Editor crowding it. The AI pane has
its own dedicated fullscreen, `Ctrl+B K`, which lays out the AI pane, its
scrollable chat history, and the prompt across the whole window for a long
conversation. Focus mode and AI-fullscreen are mutually exclusive — you are
either writing alone or talking at length, not both — and each toggle returns
you to the four-pane layout.

#chord_table((
  chord_row("Ctrl+B Shift+W", "Focus mode — hide everything but the Editor. Press again to restore."),
  chord_row("Ctrl+Z f", "Fullscreen the current right pane (Output / Thoughts)."),
  chord_row("Ctrl+B K", "Fullscreen the AI pane with its chat history and prompt."),
))

#section("A note on the mouse")

Inkhaven is a keyboard instrument, but it does not ignore the mouse. It captures
mouse input on start: a left-click moves focus to the pane you click, and inside
a pane the click lands where you expect — the row cursor in the Tree, the
character cursor in the Editor (clicks in the number gutter are ignored). The
scroll wheel scrolls whatever it is over — three rows per tick in the Tree,
three lines in the Editor, the chat history in the AI pane, the message list in
the Output pane. It is enough to point and scroll when a hand is already on the
mouse, and it is entirely optional — nothing in this book requires it.

#callout(label: "Selecting text through the terminal")[
  Because Inkhaven captures the mouse, dragging to select does not reach your
  terminal's own copy by default. Hold `Shift` (or `Option`, depending on the
  terminal) while you drag to bypass the capture and select through the
  terminal, or toggle capture off entirely with `Ctrl+Shift+M` when you want the
  terminal's native clipboard for a while.
]

The window you have just mapped does not change from here. Every later chapter —
the editor's depths, the AI's scopes, the world and its facts, the reading
intelligences — plays out on these same four panes, reached by these same
chords, read off this same status bar. You will spend the rest of the book
learning what to *do* in this window; you now know what the window *is*.

#recap((
  [The window is *four regions*: the *Tree* (left), the *Editor* (centre), and
  a *right slot* that shows one of the *AI*, *Output*, or *Thoughts* panes, over
  a Search bar, an AI prompt, and a status bar.],
  [Plain `Tab` cycles focus *Tree → Editor → current right pane*; `Ctrl+B Tab`
  cycles *which* pane the right slot shows (*Output → AI → Thoughts*); `Ctrl+T`,
  `Ctrl+1`, and `Ctrl+3` jump straight to a pane.],
  [A *chord* is a two-key command: `Ctrl+B` (the *meta* prefix — *act on the
  book*) or `Ctrl+V` (the *view* prefix — *reach a view or tool*), then a
  *pane-specific* action key. `Esc` cancels a pending chord or closes the top
  overlay.],
  [The *status bar* and *title bars* are a running readout — breadcrumb, word
  count, the red `●` dirty chip, AI scope and inference mode, and the yellow
  *META* prompt while a chord waits.],
  [You never have to memorise a chord: `Ctrl+V Space` fuzzy-finds and runs any
  command, `Ctrl+B H` (or `?` in the Tree) opens the pane-aware quick reference.],
  [`Ctrl+B Shift+W` gives the window to the Editor alone; `Ctrl+Z f` fullscreens
  the current right pane and `Ctrl+B K` the AI pane — each a toggle back to the
  four panes.],
))
