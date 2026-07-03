# 6 — The Web, Earned

The structured sources and the scholarly indexes are authoritative by construction — a Q-id, a DOI, a catalogued book. The open web is not. It is vast and immediate and often exactly what you need, but its authority is uneven: a careful reference page and a careless blog post look the same until you read them. So Inkhaven treats a web fact differently from every other source. It lets you use the web freely — and then makes a web fact **earn** its place at the gate.

## Asking the web

The command is `/web`:

```
/web how wide was a standard Roman road
```

The Assistant searches, fetches the actual pages, and grounds an answer on what they say — cited by URL. This is a real step up from the model's unaided memory: there is now a **page** behind the claim, one you (or a reader) can open. On the ladder, a cited web page sits above a guess but below the structured and scholarly rungs — because a URL proves **where** a claim was found, not that it is **true**.

## The fact-check gate

Here is what makes the web safe to lean on. When you take a `/fact` from a web-grounded answer, the confirmation gate does something it did not do for a structured source: it **checks the claim before letting you keep it.**

**The fact-check gate** — For a fact drawn from the web (or from the model), the confirmation gate runs a single-claim accuracy check **before** the fact commits. An `ACCURATE` verdict lets it through; a `DUBIOUS` or `INACCURATE` verdict shows you the reasoning and asks you to confirm again — deliberately, with your eyes open. It informs; it never silently blocks. You always get the final say.

This is the same gate from Chapter 3, doing more work. For a Wikidata datum there was nothing to check. For a web page — which might be wrong — the gate pauses, weighs the claim, and reports back. If it is confident the claim is accurate, the fact lands normally. If it has doubts, it does not throw the fact away; it **tells you why** and asks whether you want to keep it anyway. A shaky claim can still be kept — sometimes you know better than the check — but you keep it knowingly, and its provenance records the verdict.

> **Inform, never block:** Every gate in Inkhaven follows the same principle: it surfaces what it found and hands you the decision. It will warn, it will double-check, it will ask you to confirm twice — but it will never refuse to record a fact you insist on. You are the author; the tool is your research assistant, not your gatekeeper.

## Two ways to use a page

`/web` has two modes, and knowing which you want is the whole skill.

The default is what we just described: **ground a cited answer** on the fetched pages, and let you `/fact` from it through the checking gate. Use this when you want an **answer** — a specific claim, checked and kept.

The other mode, `/web --ingest`, **embeds the pages themselves** into your corpus — the same ingestion you met with `/gutenberg`, but from the live web. No model answers; the pages simply become searchable, quotable research material. Use this when you want the **material**, not a single answer — when you are gathering a body of reading to draw on repeatedly.

**For fiction —** Chase texture and detail quickly — the smell of a trade, the layout of a period street — grounding on real pages, keeping only the details that survive the check. The reader never sees the URLs; they feel the specificity.

**For non-fiction —** Pull a claim from a reputable page **with the check as a first filter**, then — in the next part — cross-check the survivors against the structured and scholarly rungs before the claim goes into your argument. The web is where a non-fiction claim **starts**, not where it rests.

## A note on the low rungs

It is worth saying plainly: there is nothing wrong with using the web, or even the model. The ladder is not a rule that forbids the lower rungs — it is a way of being **honest** about where a fact stands. A novelist grounding the feel of a city on a good web page has done real, legitimate work. The check at the gate, and the provenance on the fact, simply keep that work truthful about itself.

What the ladder **does** insist on is this: for a load-bearing claim — the kind a sharp reader or a reviewer would test — a single web page is a starting point, not an ending. The next part is about turning a starting point into an ending: holding a claim up against several independent sources at once, and even arguing against it, so that what you finally keep has been tested from more than one side.

**Recap**

- `/web` searches and grounds a **cited** answer on real pages — above a guess, but below the structured and scholarly rungs, because a URL proves **where**, not **whether**.
- Taking a `/fact` from a web answer runs the **fact-check gate**: `ACCURATE` passes; `DUBIOUS`/`INACCURATE` shows its reasoning and asks you to confirm again.
- Every gate **informs but never blocks** — you can always keep a fact you insist on, knowingly, with the verdict recorded in its provenance.
- `/web --ingest` embeds whole pages into your corpus as searchable material, instead of answering a single question.
- The low rungs are legitimate; the ladder just keeps a fact honest about where it stands — and load-bearing claims deserve the cross-checking of Part III.
