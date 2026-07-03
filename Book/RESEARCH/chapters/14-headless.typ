#import "../design.typ": *

#chapter(number: 14, title: "Research at Scale")

Everything so far has been hands-on: you ask, you read, you confirm, one fact at a
time. That is exactly right when you are thinking — research is a conversation, and
a conversation wants a person in it. But some research is not thinking; it is
*fetching*. A list of forty questions you already know you need answered. A folder
of PDFs to bring in. A book to ingest. For that kind of work you do not want to sit
and confirm forty times — you want to set it running and come back to a report.
Inkhaven can do all of this *headlessly*, from the command line, with no interface
at all.

#term("Headless")[
  A tool runs *headless* when it works without its interactive interface —
  driven from the command line, unattended, its results written to a file. The
  same Research Assistant you have been using in its two-pane screen can also be
  invoked as `inkhaven research …` with flags, to do in bulk and in the background
  what you would otherwise do by hand.
]

#section("Answering a list: `--batch`")

The headline is `--batch`. Give it a file of questions — one per line — and it
researches each one, distils a candidate fact, scores its own confidence, and
writes a report:

```
inkhaven research --batch questions.txt --out findings.md
```

By default it is conservative: it *proposes* facts and reports them, but leaves the
confirming to you — the gate is still yours, just deferred to when you read the
report. If you want it to actually insert the high-confidence findings unattended,
you say so explicitly, and set the bar:

```
inkhaven research --batch questions.txt --auto-confirm --confidence 0.8
```

Now facts whose confidence clears the threshold are kept automatically, and the
rest are reported for your judgement. The threshold is the dial between "do it all
for me" and "show me your work" — and it is opt-in, because unattended insertion is
exactly the kind of thing that should never happen by surprise.

#term("Batch research")[
  *Batch research* answers a whole list of questions in one unattended run, writing
  a report of what it found and how confident it was. It trades the interactive
  gate for a confidence threshold you set — a way to do the fetching kind of
  research at volume while keeping the thinking kind for the screen.
]

#section("Closing the loop from `/gaps`")

Remember where a list of questions comes from. In Part VI, `/gaps` handed you
exactly that — the open questions your corpus could not yet answer. Feed those
questions to `--batch`, and the loop closes: the corpus tells you what it is
missing, you point a batch run at the holes, and it comes back with candidate facts
to fill them.

#batch_loop()

This is the corpus filling its own gaps. You `/gaps` a topic, drop the questions
into a file, run `--batch` over them while you do something else, and return to a
report of proposed facts — each one still crossing your judgement before it is
kept. Research that used to be a day of tab-juggling becomes a run you start and
walk away from.

#section("Bringing material in: `--import` and `--sync`")

The other headless jobs are about *material* rather than answers.

`--import <path>` ingests a document — a PDF, a text file, a whole folder — into
your corpus from the command line, the same ingestion you met interactively, now
scriptable.

`--sync <folder>` goes one step further: it *registers* a folder so that whenever
its contents change, the material is re-imported automatically. Point it at the
directory where your reading accumulates, and your corpus stays current with your
reading without a manual step.

```
inkhaven research --import ~/reading/aqueducts.pdf
inkhaven research --sync ~/reading/
```

And `--gutenberg` brings a public-domain book in headlessly, the command-line twin
of the `/gutenberg` you already know.

#two_track(
  [Batch-research the texture questions for a whole act at once — "what did X look
   like, sound like, cost?" — overnight, and skim the report in the morning for the
   details worth keeping. `--sync` a folder of period sources so anything you drop
   in is searchable by the time you sit down to write.],
  [This is where a large project becomes tractable. Turn a literature-gap list into
   a batch run; `--sync` the folder where your references land so the corpus never
   drifts from your reading; script `--bibliography` into your build so the
   references section regenerates on every draft. The whole research apparatus
   becomes part of your pipeline.],
)

#callout(label: "Automation with the safety kept")[
  Headless mode relaxes the *interactive* gate, but not the *principle*. By default
  nothing is inserted without your review; unattended insertion is opt-in and
  threshold-gated; provenance is still recorded on every fact; and your manuscript
  is never touched. Scale changes how much you can do at once — it does not change
  who decides what is true.
]

#section("The tools are all in your hands")

You now have the entire workflow, at every scale: one fact at a time in the
interactive screen, and whole lists and folders at once from the command line.
Acquire, cross-check, maintain, compose — by hand when you are thinking, headless
when you are fetching. There is nothing left to introduce.

What is left is to see it all *move together*. The final chapter is a single worked
example — one novelist and one non-fiction author, each taking a real claim from a
first question to a grounded, cited fact and on into their book — so the pieces you
have met one at a time run, at last, as one workflow.

#recap((
  [Interactive research is for *thinking*; *headless* research (`inkhaven research
   …`) is for *fetching* — bulk, unattended, written to a report.],
  [`--batch <file>` answers a list of questions; it proposes by default, and only
   inserts unattended with `--auto-confirm` above a `--confidence` threshold you
   set.],
  [`/gaps` → a questions file → `--batch` closes the loop: the corpus fills its own
   holes while you do something else, every fact still crossing your judgement.],
  [`--import` ingests material from the CLI; `--sync` keeps a folder's contents
   auto-imported; `--gutenberg` and `--bibliography` have headless twins too.],
  [Scale relaxes the interactive gate but keeps the principle: review by default,
   provenance always, prose never touched — you still decide what is true.],
))
