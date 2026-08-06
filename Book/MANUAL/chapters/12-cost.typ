#import "../design.typ": *

#chapter(number: 12, title: "Watching the Cost")

Every tool that talks to a large language model spends money doing it, and most
of them are quiet about it — the meter runs somewhere you cannot see, and the
bill arrives at the end of the month with no way to trace which feature ate what.
Inkhaven takes the opposite stance. It keeps almost everything free, makes the
handful of paid touchpoints explicit, records every model call the moment it
happens, and gives you one screen that shows the day's spend at a glance. This
chapter is about that visibility: the philosophy behind it, the dashboard that
surfaces it, the daily caps that warn you before a big pass, and exactly which
features cost anything at all. Read it once and you will never again wonder where
your model budget went.

#section("Cost informs, it never blocks")

The single principle that governs money in Inkhaven is the one that governs
everything else in the tool: it is a *permissive* program built for one writer
who owns their own decisions. It informs; it does not gate. There is no feature
Inkhaven will refuse to run because you have "used too much" today, and no
hidden ceiling that silently drops a request. When you cross a budget, the tool
tells you plainly and then does the thing you asked for anyway.

This is a deliberate reversal of how quotas usually work. A daily cap in
Inkhaven is not a wall — it is a *speed bump*. It exists so that a runaway loop,
or a habit you did not notice forming, becomes visible before it becomes
expensive. The number in the config is a threshold for a warning, not a limit on
your agency.

#callout(label: "The permissive principle")[
  Cost caps *inform*, they never *block*. Past a daily budget the model-using
  passes print a one-line notice and continue. The only things Inkhaven ever
  truly refuses are matters of safety — a sandbox boundary, a destructive
  operation — never a matter of spend. Your money is your business.
]

Two consequences follow, and they run through the rest of the chapter. First,
because nothing is blocked, the caps can afford to be honest rather than
conservative — they are set where a normal day of writing never reaches them, so
that touching one actually means something. Second, because the tool never
surprises you, it has to be scrupulous about *showing* you: every inference is
counted, every counted call is visible, and the dashboard is one keystroke away.

#section("Almost everything is free")

The reason the bill stays small is architectural, not accidental. The
overwhelming majority of what Inkhaven does for you is *deterministic* — it runs
locally on your machine, from your text and your data, with no network call and
no model behind it. Deterministic work is free, it is instant, it works with no
network, and — the quiet virtue — it is *reproducible*: run it twice on the same
input and you get the same answer, because there is no model in the loop to drift.

#term("Deterministic")[
  A computation whose output is fixed by its input — the same paragraph in gives
  the same result out, every time, computed on your machine with no model and no
  network. The opposite of a *model call*, which asks a language model for a
  judgement and costs money to make. Inkhaven's default posture is deterministic;
  the model is reached only where a determinate answer is genuinely impossible.
]

Look across the reading intelligences and you find the same pattern again and
again: a free deterministic *core* that runs all the time, and an optional model
*pass* you invoke by hand when you want the deeper reading the patterns cannot
give. The continuity ledger checks co-location, timeline, numbers, and
character-fact drift with no model at all; its cross-paragraph coherence pass is
a separate key you press only when you want it. The read-through measures your
prose's intensity curve and walks the book for confusion and info-dumps
deterministically; its synthetic first-read is opt-in. The voice profiler, the
poetry scanner, the theologian's fast signals — all free, all local, all
reproducible.

So the honest one-sentence summary of Inkhaven's cost is this: *the tool is free
until you deliberately ask a model a question.* The rest of this chapter is about
those deliberate asks — where they are, what they cost, and how to keep them in
view.

#section("Where the money goes")

There is no long list to memorise, because the paid surfaces are few and each
one announces itself. Every model touchpoint in Inkhaven is one of two shapes: a
*slow track* you engage on a paragraph or a book, or a *conversation* you type
into. Here is the whole territory.

#subsection("The slow tracks")

A slow track is the model pass that sits behind a deterministic reader and
supplies the judgement the patterns cannot. Each is opt-in — you press its
engage key or pass its flag — and each is capped and counted.

- *World fact-check (slow)* — reads a paragraph against your world's declared
  facts for contradictions a rule cannot catch. Default cap: 200 calls a day.
- *Inner Socrates (slow)* — the `J → E` engage pass, which surfaces the
  assumptions, tensions, and framings a paragraph rests on as questions. Default
  cap: 150 calls a day.
- *Inner Editor engagement* — the editor's own coaching pass. Default cap: 200
  engagement calls a day (and a session and monthly ceiling besides).
- *The coherence and synthetic passes* — the continuity ledger's `k` coherence
  pass, the read-through's `k` synthetic first-read, the Inner Stylist's `E`
  coaching, the Inner Theologian's session, and a `--deep` flag on the CLI where
  one exists. Each is a small number of model calls you asked for by name.

Every one of these is a single short model call per paragraph — one request, one
response. With the default caps, an ordinary writing day never comes near the
ceiling; you would have to engage the slow track on a hundred and fifty separate
paragraphs to reach the Inner Socrates cap, and the tool would still let you.

#subsection("The conversations")

The other paid surface is anything you type into and get an answer back from: the
AI pane's chat, chat-with-your-book, the research assistant's session, grammar
and explain and continuation on a selection, the graph's `ask`. These are not
capped by a daily ceiling — they are metered instead, counted by *category* so
you can see how many of each you have run today. The research assistant goes one
step further and shows a running dollar estimate in its status bar as you work,
priced from the model you are actually using.

#callout(label: "What a model call costs")[
  Inkhaven ships a per-model price table (USD per million tokens) so the research
  assistant can turn tokens into dollars — a Sonnet-class model is priced around
  \$3 in / \$15 out per million, a Haiku-class one far less, an Opus-class one
  more. A single slow-track paragraph pass is a few hundred tokens; the arithmetic
  is pennies. The caps exist to catch a *runaway*, not a normal session. Prices
  are informative and live in config — adjust them as the market moves.
]

#section("The cost dashboard")

Everything counted lands in one place. There are two doors to it — a command and
a keystroke — and both render the exact same view, because the CLI and the modal
share a single aggregator under the hood.

From a shell, `inkhaven cost` prints today's tally and exits. Inside the editor,
the chord `Ctrl+B $` opens the same report as a scrollable modal titled
*AI cost · Ctrl+B \$*. The report is strictly read-only: it changes no caps,
enforces nothing, and computes its numbers fresh each time you open it.

#chord_table((
  ("Ctrl+B \$", "Open the AI cost dashboard modal (read-only)."),
))

The report is in three bands. First the *daily budgets* — the capped slow
tracks, each shown as calls-used over its cap with a twenty-cell usage bar and a
percentage. Then *other AI calls* — every uncapped conversation, listed by
category with a count and no bar, because there is nothing to measure against.
Last a *total* line and the standing reminder that the budgets inform rather than
limit.

#screen(caption: "inkhaven cost — a day's tally")[```
AI cost — LLM calls today (2026-08-05)

  daily budgets (informative):
    world fact-check (slow)    12 / 200  [█░░░░░░░░░░░░░░░░░░░] 6%
    inner socrates (slow)       3 / 150  [░░░░░░░░░░░░░░░░░░░░] 2%

  other AI calls today:
    chat                        7
    explain                     2
    grammar                     1

  total AI calls today         25

  Budgets are informative, not limits — past a budget the slow
  tracks warn and continue. The tally resets per goals.day_boundary.
```]

A few things are worth reading off that screen. The bar fills proportionally and
clamps at full, but the percentage always tells the truth — cross a cap and you
will see the bar solid with, say, `150%` beside it, which is exactly the signal
you want. The capped rows and the counted rows never double-count: the slow
tracks record only against their own budget, so they do not also appear in the
per-category list below. And the dashboard is *extensible by design* — any new
analytical thread that records its calls under its own sub-budget key shows up
here automatically, with no change to the dashboard itself. The Inner Socrates
and Inner Editor entries you see are exactly that mechanism: sub-budgets keyed by
feature, surfaced dynamically.

#callout(label: "One aggregator, two faces")[
  `inkhaven cost` and `Ctrl+B $` are the same code. The CLI is for a quick check
  from a shell or a script; the modal is for a glance without leaving your
  paragraph. Neither can change anything — to move a cap you edit config (below),
  and the next report reflects it.
]

#section("The daily caps")

The caps are the numbers the dashboard measures against, and understanding what
they do — and do not — is the heart of the chapter. A cap is a per-day ceiling on
one slow track's model calls. It is *informative*: the tool checks it before a
call, and if you are over it, it warns and proceeds.

#term("Daily call cap")[
  A per-day threshold on a slow track's model calls — 200 for world fact-check,
  150 for Inner Socrates, 200 for Inner Editor engagement by default. Crossing it
  triggers a one-line warning, not a refusal. "Today" is defined by
  `goals.day_boundary`, so the caps, the streak, and the usage tallies all agree
  on when the day rolls over.
]

Before a slow-track call, a *preflight* estimates the request's size and checks
your standing against the cap. When you are under budget, it prints a quiet line
naming the model, the rough token count, and where you stand — then reads:

#screen(caption: "A preflight, under budget")[```
inner socrates (slow) · model: claude-sonnet · ~420 tokens
· 3/150 calls today · reading…
```]

When you are *over* budget, the preflight does not stop. It prints the notice and
continues into the very same call — the permissive principle made literal:

#screen(caption: "A preflight, past the cap — it continues")[```
inner socrates (slow): past today's slow-track budget
(150/150 calls) — continuing (the cap is informative,
see `inkhaven cost`).
```]

That is the whole behaviour: a warning, a pointer to the dashboard, and the work
done regardless. There is a second, separate guard worth knowing about — a
per-call *soft cap* on estimated tokens, which catches a single pass that is
unusually large (a very long paragraph, say) and asks you to confirm with
`--force`. That one can decline a call, because its job is to stop a single
accidental monster request, not to ration your day; but it too yields the moment
you insist.

#callout(label: "The one exception, and it is not a wall")[
  The per-call soft cap is the only place a model pass will pause rather than
  proceed, and it pauses for size, not for your daily total. Re-run with `--force`
  and it goes through. The daily caps never pause at all.
]

#subsection("The `cost` config block")

The caps and the retention window live in one small config block. Edit your
project's config to move any of them; the change takes effect on the next report
and the next preflight.

#screen(caption: "The cost block — defaults shown")[```
cost: {
  world_daily_call_cap: 200            // world slow-track ceiling
  inner_socrates_daily_call_cap: 150   // Inner Socrates ceiling
  usage_retention_days: 30             // days of tallies kept in
                                       // .inkhaven/ai_usage.json
}
```]

The Inner Editor keeps its own richer set of ceilings under its own block — a
per-session cap, a "confirm above" threshold, a per-day cap of 200, and a monthly
cap — because a coaching conversation has more shapes to bound than a one-shot
paragraph pass. All of them are informative in the same way.

Two of these numbers deserve a note. `usage_retention_days` governs how much
history the per-category tally file keeps before the oldest days are pruned — it
is a housekeeping bound on a small JSON file, not a spend control. And the day
boundary that all the caps share is `goals.day_boundary`: `utc` by default, or
`local` so an evening session far from UTC is attributed to the right day and the
caps reset at your midnight, not Greenwich's.

#section("How usage is tracked")

The dashboard can only show what the tool bothered to record, so the recording is
deliberate and complete. Every inference Inkhaven makes — chat, grammar, explain,
continuation, critique, the retrieval-augmented answers, all of it — passes
through a single chokepoint that records one tally under a `(day, category)` key
the instant the call is made. Nothing that reaches a model goes uncounted.

#term("The usage ledger")[
  A small JSON file at `<project>/.inkhaven/ai_usage.json`, keyed
  `day → category → calls`. Every model call increments its category's count for
  the current day. The dashboard reads it; the slow tracks keep their capped
  counts in their own stores so they are not double-counted here.
]

The file is plain and human-readable — you can open it and see exactly what ran:

#screen(caption: ".inkhaven/ai_usage.json")[```
{
  "2026-08-05": {
    "chat": 7,
    "explain": 2,
    "grammar": 1
  }
}
```]

Three properties keep this trustworthy. It is counted by *calendar day* on the
same clock the caps use, so a day's tally resets exactly when the caps do. It is
written *atomically* and under a lock, so two inferences racing to record at once
never lose a tally — the count is exact, not approximate. And it is *pruned on
write* to the retention window, so the file never grows without bound; the oldest
day falls off once you pass `usage_retention_days` of history.

One boundary is worth stating plainly: the ledger counts *calls*, not tokens or
dollars, because a call count is exact and free to keep while a token count would
need the model to tell it the truth. For dollars you have the research
assistant's live session estimate and the price table behind it; for volume you
have this. Together they answer the two questions a writer actually asks — "how
much am I leaning on the model today?" and "what is this session costing me?" —
without either one pretending to a precision it does not have.

#callout(label: "Before a project is open")[
  The tracker is pointed at your project at startup, so a call made headless or
  before any project is open is simply not recorded — there is nowhere to write
  it. In practice every call you make from inside a project lands in that
  project's ledger, which is exactly the scope you want: cost is accounted
  per-book, not globally.
]

#section("A field guide to free versus paid")

To close, the practical map — what you can lean on all day at no cost, and what
to reach for deliberately. When in doubt, remember the rule: if you did not press
an engage key or pass a `--deep`-style flag and type into a conversation, you did
not spend anything.

*Free, always — the deterministic core:* the editor and the tree; snapshots,
search, and the semantic index; the continuity ledger's structural checks; the
read-through's measured intensity curve and forward walk; the prose voice
profiler; the poetry scanner; the theologian's fast signals; the graph's
structural queries; assembly to PDF, EPUB, and the web. None of it touches a
model.

*Paid, on purpose — the deliberate asks:* the world and Socratic and editor slow
tracks; the continuity coherence pass; the synthetic first-read; the stylist and
theologian engagements; the research assistant's session; chat and
chat-with-your-book; grammar, explain, and continuation on a selection. Each is
capped or counted, each shows in the dashboard, and each is one keystroke or one
flag you chose to press.

That is the whole of watching the cost: a tool that stays free until you ask it
not to, tells you the moment you do, and never once stands between you and the
work.

#recap((
  [Cost *informs, never blocks* — the daily caps are speed bumps, not walls; past
  a budget the slow tracks warn and continue.],
  [Almost everything is *deterministic and free* — the model is reached only where
  a determinate answer is impossible, and only when you engage it by hand.],
  [`inkhaven cost` and `Ctrl+B $` render the *same read-only dashboard*: capped
  slow tracks with usage bars, uncapped calls by category, and a daily total.],
  [The default caps are *200* (world), *150* (Inner Socrates), and *200* (Inner
  Editor engagement) calls a day; they and the `usage_retention_days` window live
  in the `cost` config block.],
  [Every inference is recorded by category in `.inkhaven/ai_usage.json`, keyed by
  day on the `goals.day_boundary` clock, written atomically and pruned to the
  retention window.],
))
