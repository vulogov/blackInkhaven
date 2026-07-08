#import "../design.typ": *

#chapter(number: 7, title: "The scenarios track")

A scenario is not a story; it is a _space for stories_. The game module, the RPG
sourcebook, the interactive fiction — each hands a reader-player a place, a cast of
people, a set of situations, and the branches they may take, and then gets out of
the way. The discipline of the track is _usability under pressure_: a referee is
flipping to your city in the middle of a session with three players waiting, and
whatever they need must be there, findable, and complete. This chapter is the loop
for the writer who builds worlds others will play in.

#section("Frame — the sourcebook template")

Start from the template built for it:

```
inkhaven init "the-drowned-coast" --template rpg-sourcebook
```

Set the genre to match the fiction underneath — usually `fantasy` or `scifi` — so
the readers frame the prose correctly. But the structure is the point here: a
scenario is _reference-organised_. Where a novel is read once front to back, a
sourcebook is read a hundred times in fragments, so build it as a tree a referee
can jump through — Places at the top level, each self-contained, nothing that
requires reading the chapter before it.

#section("Gather — a world that is usable, not just consistent")

The world simulation serves this track well, but for a different reason than
fiction's. You are less concerned that the sun rises correctly and more that the
_places are real and reachable_. Grow the world, then lean on the parts that make it
navigable:

- *Places* (`Ctrl+B p`) is the book you live in — every location a player might go,
  described enough to run cold.
- `inkhaven realworld gazetteer` produces a consolidated reference — regions,
  landmarks, waters, settlements — the referee's quick-look document.
- `inkhaven realworld map` renders an actual map, so the space is not only described
  but seen.

#subsection("Situations, not plot")

A scenario's equivalent of plot is _situations_ — tensions set up and left for the
players to resolve. The Threads book (`Ctrl+V Shift+H`) is where you track them:
each thread a hook with a setup and a range of possible payoffs, rather than a
single fixed outcome. Populate the Characters book with the people who drive them —
the faction leader, the informant, the monster — as NPCs a referee can voice on
sight.

#term("Situation")[
  A hook prepared for play but not resolved by the author — a tension, a faction at
  odds, a secret about to surface — with its setup fixed and its outcome left open
  for the players. Where a novel's thread has one payoff the author chose, a
  scenario's has many the table will discover. The Threads book holds both kinds; on
  this track you deliberately leave the ending unwritten.
]

#section("Read — hunt for the gap, not the contradiction")

Fiction's readers ask whether the prose is alive; the scenario's overriding question
is whether anything is _missing_. The failure mode of the track is the gap: a place
named on the map but never described, a faction with a leader who has no
motivation, a thread with a setup and no possible payoff. So the reading pass is a
completeness sweep:

- `inkhaven thread doctor` checks your threads for dangling setups and unreachable
  payoffs — the promise a player will chase that leads nowhere.
- The Outline (`Ctrl+2`) and the tree let you scan for the named-but-empty node.
- The status marks (`Ctrl+B r`) matter more here than anywhere: at a glance, which
  locations are `ready` to run and which are still a stub?

#note[
  The AI readers still help, but you point them at coverage. Ask the Inner Editor
  whether a location description gives a referee enough to improvise from; ask the
  Inner Socrates what a situation _presupposes_ that the text never states — the
  motive left implicit, the exit not mentioned. On this track a good question is
  usually "what would a referee need here that isn't on the page?"
]

#insight[
  A scenario is judged the way software is judged, not the way a novel is: by
  whether every path a user takes leads somewhere real. That is why structure and
  completeness matter more than voice here, and why the tools you lean on — Places,
  the gazetteer, the map, the thread doctor, the status marks — are the tools of
  _reference_, not of narrative. Write it as a thing to be used, and it will be.
]

#section("Produce")

Sourcebooks are physical objects more often than novels are. The same `export pdf`
that renders a manuscript carries a full finishing workshop — imposition, booklets,
covers, even a barcode — under the `pdf` command, for when the module is going to a
printer rather than a screen.

#section("Hands-on: two procedures")

#subsection("Make a place drawable and reachable")

+ Grow the world (`inkhaven realworld compile --materialize`) so its settlements exist, and accept the ones your module uses (`inkhaven realworld proposals`, then accept).
+ Give any hand-authored location a position so it appears on the map: `inkhaven realworld set-coords "The Drowned Chapel" --lat 41 --lon 8`.
+ Produce the referee's quick-look reference: `inkhaven realworld gazetteer --output gazetteer.md` — regions, landmarks, waters, and settlements in one document.
+ Render the map itself: `inkhaven realworld map`. The space is now both described and seen.

#subsection("Set up a situation and check for gaps")

+ Add a hook to the Threads book (`Ctrl+V Shift+H`): a setup with a range of possible payoffs rather than a single fixed outcome.
+ Populate the people who drive it in the Characters book (`Ctrl+B c`) — the faction leader, the informant — with enough for a referee to voice on sight.
+ Hunt for the gap: `inkhaven thread doctor` reports dangling setups and unreachable payoffs — the promise a player will chase that leads nowhere.
+ Scan the tree for named-but-empty nodes with the Outline (`Ctrl+2`), and use the status marks (`Ctrl+B r`) to see at a glance which locations are `ready` to run.

#recap((
  [A scenario is a *space for stories*, judged by *usability under pressure* — build it reference-organised (`rpg-sourcebook` template) so a referee can jump to any self-contained piece mid-session.],
  [*Gather* a navigable world: Places (`Ctrl+B p`) as the home book, `realworld gazetteer` for the quick-look, `realworld map` to see the space.],
  [Track *situations, not plot*, in the Threads book — hooks with fixed setups and open payoffs — and populate Characters with runnable NPCs.],
  [*Read for the gap*: `thread doctor` for dangling setups and unreachable payoffs, the Outline for named-but-empty nodes, and status marks for what is `ready` to run — then `export pdf` with its print-finishing workshop.],
))
