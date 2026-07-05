# 12 — Composing From What You Know

Everything in this book so far has **built** something: a corpus of facts, grounded, cross-checked, and kept honest. But a corpus is not a book. At some point the research has to turn back into writing — and this is where most tools abandon you. You have the reading; now go and use it, somehow, in the other window. Inkhaven does not abandon you here. The corpus you built is **composable**: it can produce a cited overview, a working outline, and a list of what you still don't know — drawn entirely from the facts you verified.

There is one rule that governs all three commands in this chapter, and it is what makes them trustworthy: they compose from **your corpus only**. They do not go back to the model's free imagination; they work with the facts you established and checked, and they cite each one. This is the difference between "write me something about aqueducts" and "assemble what **I** have verified about aqueducts, and show your sources."

## A grounded overview: `/synthesize`

Point `/synthesize` at a topic, and it retrieves the facts you have kept that bear on it and weaves them into a coherent overview — citing each claim back to the fact it came from, and **flagging where your corpus is thin**.

```
/synthesize the Roman water supply
```

**Synthesis** — A **synthesis** draws together many separate facts into one coherent account of a topic. Here it is **grounded**: built only from facts you have verified, with each claim cited to its source, and honest about the gaps. It is not the model telling you about the topic — it is the model organising what **you** established about it.

The honesty about gaps matters as much as the overview itself. A synthesis that says "the corpus establishes the capacity and the route, but is silent on the construction dates" is telling you exactly where more research is needed before you write. It is a mirror held up to your own reading.

## The research-to-writing bridge: `/outline`

`/outline` takes the same grounded material and shapes it differently — into a **structured outline** for actually writing about the topic, where each point cites the facts that support it, and anything your corpus does **not** cover is marked plainly as `(needs research)`.

**The research-to-writing bridge** — The gap every writer knows: you have done the reading, and now face the blank page. An `/outline` is the plank across that gap — it turns your verified corpus into a shape you can write **into**, with the supporting fact sitting under each point and the holes marked. You are no longer starting from nothing; you are starting from what you know, arranged.

You copy the outline into your manuscript and write from it — and because each point is backed by a cited fact, you write **grounded**, without stopping to re-check. The `(needs research)` marks become your to-do list for the gaps.

## What you don't know yet: `/gaps`

The most useful thing a corpus can tell you is sometimes what is **missing** from it. `/gaps` asks exactly that: given everything you have on a topic, what are the open questions your corpus cannot yet answer?

```
/gaps the Roman water supply
```

It returns a list of specific, concrete questions — "you have the aqueduct's capacity but not its construction date; you have the route but not the gradient" — each one a piece of research you now know to go and do. And these questions are not a dead end: in the next part you will see them become the **input** to headless research, so the corpus can be told to go and fill its own gaps.

**For fiction —** Before you write a scene set in a place, `/synthesize` everything you have established about it into a brief, `/outline` the scene grounded in those facts, and let `/gaps` tell you the one detail you still need to look up. You draft from a foundation instead of improvising and hoping it holds.

**For non-fiction —** `/synthesize` produces a cited summary of the literature you have gathered — the backbone of a review section. `/outline` turns your sources into the skeleton of an argument, each claim already footnoted. `/gaps` is your literature-gap analysis, done from your own corpus.

> **It composes; you write:** These commands never write into your manuscript on their own, and they never invent to fill a hole — a gap is reported as a gap, not papered over. What they produce is a cited draft **of the research**, for you to lift, edit, and make your own. The words that end up in your book are still yours; the tool just makes sure you start from what you actually know.

## The corpus pays off

This is the moment the whole discipline pays for itself. Every fact you grounded, every source you cross-checked, every citation that filed itself — it was all so that, at **this** point, you could ask your own knowledge base to organise itself into something you can write from, honest about its own limits, cited throughout. The research was never the goal; this is.

One product of the corpus deserves its own chapter, because it is the one a non-fiction author often has to **ship**: the bibliography. All those citations that quietly filed themselves as you researched are about to become a formatted reference list, with a single command. That is next.

**Recap**

- The corpus is **composable**: `/synthesize`, `/outline`, and `/gaps` turn your verified facts back into writing — drawing on your corpus **only**, and citing each claim.
- `/synthesize <topic>` weaves your kept facts into a **grounded, cited overview** and flags where the corpus is thin.
- `/outline <topic>` is the **research-to-writing bridge**: a fact-citing outline you write into, with holes marked `(needs research)`.
- `/gaps <topic>` lists the **open questions** your corpus can't answer yet — which the next part turns into fresh research.
- These commands compose but never write your prose or invent to fill gaps — the payoff of all the grounding is a cited draft you make your own.
