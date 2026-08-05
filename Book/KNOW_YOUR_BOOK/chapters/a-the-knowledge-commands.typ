#import "../design.typ": *

#appendix(letter: "A", title: "The Knowledge Commands")

Every intelligence in this book has a command line and, where it lives in the editor,
a chord. This is the whole set on two pages, for when you know what you want and just
need the way to say it.

#section("From the command line")

#chord_table((
  chord_row("inkhaven factcheck", "Contradictions within the Facts book; SCHOLAR /contradict, /relate."),
  chord_row("inkhaven graph ask \"…\"", "Answer a plain question by walking the knowledge graph."),
  chord_row("inkhaven graph stats", "Node + edge counts, per kind. rebuild / neighbors / paths / contradicting."),
  chord_row("inkhaven continuity check", "The SENTINEL ledger. --only / --skip / --json; --coherence for the LLM pass."),
  chord_row("inkhaven knowledge", "The KEN epistemic check. --json; --deep for implied_irony. Non-zero exit on a break."),
  chord_row("inkhaven readthrough", "The LECTOR read-through. --deep for the synthetic first read; --json."),
  chord_row("inkhaven chorus", "Voice profiles + the distinctiveness matrix. scan / report / stylist."),
  chord_row("inkhaven chronicle", "The draft trend since the last mark. mark / list / diff / --json."),
))

#section("Inside the editor")

#chord_table((
  chord_row("Ctrl+B Shift+X", "Fact-check the open paragraph against the world."),
  chord_row("Ctrl+B z", "The graph hub — neighbourhood, the edge inbox, graph ask (F9 = the Graph scope)."),
  chord_row("Ctrl+B Shift+I", "The SENTINEL continuity ledger (k = the LLM coherence pass)."),
  chord_row("Ctrl+B Shift+Z", "The KEN knowledge dashboard — who knows what, when."),
  chord_row("Ctrl+B Shift+A", "The LECTOR read-through dashboard (k = the synthetic first read)."),
  chord_row("Ctrl+B J → Y", "The Inner Stylist — CHORUS's voice observations."),
  chord_row("Ctrl+B Shift+U", "The CHRONICLE draft-history dashboard (m = mark this draft)."),
  chord_row("Ctrl+V Shift+R", "The Editorial Pass — every reader's findings, one worklist, acted on."),
  chord_row("Ctrl+B Shift+C", "The unified review pass — the fast checks in one sweep."),
))

#section("The tags that declare knowledge (KEN)")

#chord_table((
  chord_row("secret:<topic>", "Marks <topic> a secret — a reference by someone ungranted is a leak."),
  chord_row("know:<topic>", "Grants the scene's pov: character knowledge of <topic> here."),
  chord_row("know:<topic>@<name>", "Grants <name> the knowledge explicitly."),
  chord_row("reveals:<topic>", "On an event's paragraph — binds the event to the fact it reveals."),
  chord_row("pov:<name>", "Declares a scene's viewpoint — used by CHORUS and KEN alike."),
))

#section("From a script (Bund)")

Every check reads out to Bund, read-only, for hooks and gates:
`ink.knowledge.{grants,findings,check}`, `ink.continuity.{findings,check}`,
`ink.readthrough.{report,curve,check}`, `ink.chronicle.{marks,trend,check}`, and the
graph words. Each `check` returns a `clean` flag — a one-line pass/fail gate for a
pre-submit script.
