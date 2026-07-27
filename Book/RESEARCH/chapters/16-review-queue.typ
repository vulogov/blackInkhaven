#import "../design.typ": *

#chapter(number: 16, title: "The Review Queue")

Chapter 15 gave the assistant permission to research on its own — to plan its
sub-questions, answer them, and write down what it found. But it left something deliberately
unfinished. Every fact an agentic run emits lands in your Facts book *untrusted*: it carries
the `·` model tier, and nothing about it has passed under your eye. A run that produces
twenty facts has produced twenty small claims you have not yet decided to believe.

That is by design — the machine widens your reach, you decide what's true — but a promise you
never collect on is worthless. If untrusted facts simply pile up, they rot: you stop
distinguishing the ones you vetted from the ones a model invented at 2 a.m. This chapter is
about the tool that lets you *collect* — a single command that gathers everything the
autonomous loop left for you and walks you through it, one claim at a time.

#section("`/review` — the triage queue")

Open the Research Assistant (`inkhaven research`) and type `/review` in the query prompt. If
the current thread has any untrusted, agentic-emitted facts, a queue opens over the panes:

#screen(caption: "/review — triaging the facts an agentic run emitted")[```
╭──────────────────────────────────────────────────────────╮
│ Review — 3 untrusted fact(s), 1 ≠ · 0 triaged            │
├──────────────────────────────────────────────────────────┤
│ ▸ ≠ The 1918 pandemic killed ~50 million people…         │
│      The "Spanish flu" name came from wartime press…     │
│      Young adults died at unusually high rates…          │
├──────────────────────────────────────────────────────────┤
│ source: model · from query: How many died?               │
│ ≠ in a recorded contradiction — check /contradict        │
│   before trusting                                        │
│                                                          │
│ The 1918 pandemic killed ~50 million people              │
│ worldwide.                                               │
├──────────────────────────────────────────────────────────┤
│ j/k move · a accept · d delete · u ※ · Esc close         │
╰──────────────────────────────────────────────────────────╯
```]

The top pane is the queue; the cursor `▸` marks the fact you're on. The lower pane shows that
fact in full, with its recorded provenance — where it came from and which sub-question the
loop was answering when it wrote it. Move with `j`/`k` (or the arrows), and act with a single
key.

#subsection("The three decisions")

Every fact in the queue is waiting for exactly one of three verdicts, and each key gives it:

#chord_table((
  chord_row("a — accept", "You believe it. The fact is tagged reviewed and leaves the queue for good — it never comes back."),
  chord_row("d — delete", "It's wrong, redundant, or noise. The fact is removed from the Facts book entirely."),
  chord_row("u — undisputed", "It's an authorial premise, not a checkable claim. Tagged ※ undisputed, excluded from `/factcheck` (Chapter 11)."),
))

Whichever you press, the fact drops out of the queue and the cursor advances to the next one,
so a run of triage is just a rhythm: read, decide, read, decide. When the last fact is
handled the queue closes itself and tells you how many you worked through. Press `Esc` any
time to stop — the facts you haven't reached yet stay in the queue for next time.

Note the asymmetry between *accept* and *delete*. Accepting does not raise a fact's tier — a
model claim you've read is still a model claim, and if you want it grounded harder you still
reach for `/upgrade` (Chapter 9). What accepting buys you is *triage state*: the fact is
marked as seen, so it stops nagging. The queue is about attention, not about truth tiers.

#section("What lands in the queue — and what doesn't")

The queue is deliberately narrow. A fact appears only when all three of these hold:

- its provenance *thread* is `agentic` — it was emitted by an autonomous run (Chapter 15),
  not written by you, not promoted from a Note, not pulled from a structured source;
- it has not already been *accepted* — once you tag a fact reviewed, it's gone from the queue
  permanently, even across sessions;
- it is not marked *undisputed* — authorial premises were never in scope for triage.

Everything you authored, every fact you promoted or grounded by hand, every Wikidata triple —
none of it clutters the queue, because none of it is the thing `/review` exists to catch:
*machine-emitted claims you have not yet looked at*. The queue is the exact set of "the robot
said this; have you checked?"

#subsection("The `≠` flag — contradictions first")

Look again at the first fact in the screen: it carries a `≠`. That flag means the fact appears
in a contradiction the in-loop gate recorded during the agentic run (Chapter 15) — some other
fact in your corpus disagrees with it. The lower pane spells it out and points you at
`/contradict` to see the clash in full.

This is the queue earning its keep. Of twenty emitted facts, nineteen may be unremarkable and
one may quietly contradict something you already established — and that one is exactly the fact
you must not rubber-stamp. The `≠` flag drags it into view so your attention lands where the
risk is, instead of being spread evenly over claims that don't need it.

#section("Where it sits in the trust ladder")

`/review` is the human end of the autonomous loop. The chain now reads cleanly: `--agentic`
*emits* untrusted facts and *flags* the contradictions among them; `/review` *gathers* those
facts, *surfaces* the flags, and *records your verdict* on each. The loop widened your reach
without ever deciding what you believe; the queue is where you decide, quickly, and move on.

Run it whenever an agentic session ends — or at the top of a writing day, to clear whatever the
robot left overnight. A Facts book with an empty review queue is one you can trust at a glance,
because everything in it is either something you wrote or something you *chose* to keep.

#recap((
  [`/review` opens a triage queue over the untrusted facts an agentic run emitted — the exact
   set of "the machine said this; have you checked?"],
  [Each fact takes one of three one-key verdicts: `a` accept (tagged reviewed, gone for good),
   `d` delete, or `u` mark ※ undisputed. The cursor advances; `Esc` stops without losing your
   place.],
  [Only agentic-thread facts that are neither already accepted nor undisputed appear — your own
   work, promoted facts, and structured sources never clutter it.],
  [A `≠` flag marks a fact caught in a recorded contradiction, pulling the claims that most need
   scrutiny to the front so your attention lands where the risk is.],
  [Accepting records *triage state*, not trust tier — a reviewed model fact is still a model
   fact; reach for `/upgrade` when you want it grounded harder.],
))
