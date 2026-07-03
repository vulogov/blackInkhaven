# 3 — Your First Fact

Enough groundwork. Let us put one real fact into your book, from question to kept claim, and watch what the Research Assistant does at each step. The whole thing takes about a minute; understanding it takes a chapter, because every stage is teaching you something you will lean on for the rest of the project.

Here is the shape of what we are about to do:

Ask a question. Ground the answer on what you already trust. Cross a confirmation step — the **gate** — where you get the final say. Keep the result as a fact that remembers where it came from. Four steps, and you will do them thousands of times.

## Ask

Open the Research Assistant, and in the conversation pane type a plain question — whatever your book actually needs to know. Not a command, not a keyword; a real question.

**For fiction —** `How far could a Roman legion march in a single day on a good road?` You need this because your protagonist has to reach the frontier "in time," and **in time** has to mean something.

**For non-fiction —** `What was the average daily marching distance of a Roman legion, and what is the evidence for it?` You ask for the **evidence** too, because your reader will expect a citation, not just a number.

The Assistant answers in prose. Behind the scenes it did something important before replying: it looked through the facts you have **already** kept for this project and offered them to the model as context, so the answer is consistent with your own established world — not a generic answer that might contradict what you decided three chapters ago.

**Grounding on your corpus** — Before answering, the Assistant **retrieves** the most relevant facts you have already kept and hands them to the model along with your question. This is why its answers get better as your corpus grows: it is not guessing in a vacuum, it is reasoning over what **you** have established. Early on, with an empty corpus, it leans on the model's own knowledge — which is exactly why the next steps matter.

## Turn the answer into a candidate fact

Suppose the answer says a legion covered roughly twenty Roman miles a day, more under forced march. That is useful — but right now it is just words in a chat, sitting on the bottom rung of the ladder: a model's guess. To keep it, you turn it into a candidate fact with a command:

```
/fact a Roman legion marched about 20 Roman miles (30 km) per day
```

You do not have to retype the whole answer; you can point `/fact` at the claim you want and let it distil a clean, single statement. What you are doing is **promoting** a line from the disposable conversation into something the book will remember.

> **Fact or Note?:** If the claim is solid enough to build on, `/fact` puts it in your trusted Facts book. If it is a promising lead you are not sure about yet, its sibling `/note` puts it in Notes instead — the same gesture, a different shelf. You can promote a Note to a Fact later, once it has earned it. When in doubt, take a Note; the Facts book is worth keeping clean.

## The gate: you get the last word

Here is the step that makes the whole tool trustworthy. `/fact` does not silently write to your book. It opens a **confirmation** — the gate — showing you exactly what is about to be kept: the fact's wording (which you can edit), where it will be filed, and where it says it came from.

**The confirmation gate** — The **gate** is the mandatory pause between "the Assistant proposed a fact" and "the fact is in your book." Nothing reaches your Facts without passing it. You can edit the wording, accept it with `Ctrl+S`, or discard it entirely. The Assistant proposes; you dispose.

For a plain model-derived claim like this one, the gate may also offer to **check** the fact before you commit — and for facts drawn from the web or from structured sources, later chapters show the gate doing real cross-checking here. For now the important thing is the principle: **a fact becomes yours only when you say so.** If the wording is off, fix it in the gate. If you have changed your mind, discard it and nothing is written. If it is right, confirm — and it lands on the tree to your left.

## What you just kept

Look at the new fact in the Facts tree. It is not just a sentence. Attached to it, quietly, is its **provenance** — a small record that this fact came from the model (the bottom rung), together with the question that produced it. You can call that record up any time.

This matters more than it seems. That provenance is honest: it says, in effect, "this is currently only a guess." Later chapters will show you how to **climb the ladder** with this exact fact — re-grounding it on a real source with a single command, at which point its provenance updates to say so, and the fact grows firmer without you rewriting a word of it.

**For fiction —** Your legion's march is now a kept fact your story can lean on — and one you have flagged (via its provenance) as still needing a real source before you fully trust the timeline that depends on it.

**For non-fiction —** Your marching-distance claim is kept, but its provenance says **model** — which for your purposes is not good enough yet. You now know precisely which of your facts still need a citation, because each one tells you.

## The habit under the habit

That is the entire core loop, and it never gets more complicated than this: ask, `/fact`, confirm. Everything else in this book makes the **middle** richer — better places to draw the answer from, real cross-checking at the gate, ways to keep the corpus healthy — but the gesture stays the same. Ask a question; keep what survives; let it remember where it came from.

In the next part we start climbing. Instead of grounding on the model's memory, you will draw facts from authoritative sources — structured knowledge bases, real places, scholarly papers, whole public-domain books — so that the facts you keep start their life much higher on the ladder.

**Recap**

- The core loop is **ask → `/fact` → confirm**, and it never gets harder than that.
- A plain question is answered **grounded on the facts you already kept**, so answers improve as your corpus grows.
- `/fact` keeps a trusted claim; `/note` keeps a speculative one — when in doubt, take a Note and promote it later.
- The **gate** is a mandatory pause: you edit, confirm, or discard, and nothing reaches your Facts without your say-so.
- Every kept fact records its **provenance** — even "just a model's guess" — which is what lets you climb the ladder later without rewriting anything.
