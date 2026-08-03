# Tutorial 105 — Chat with Your Graph

*Inkhaven 2.0.1 (GRAPHMIND)*

Tutorial 104 built the knowledge graph and queried it by verb. GRAPHMIND gives
that graph a *mind*: it fills itself as you research, and you can **talk to it**.
Ask how your book connects — what contradicts what, what grounds a claim, how a
scene is sourced — and the model answers from the graph's *relations*, not just
the prose.

## The graph builds itself

Deep research now leaves a connected subgraph behind. When `research --agentic`
detects contradictions among its facts, it persists them as `contradicts` edges.
And you can ask the graph to connect a fact to its neighbours:

```sh
inkhaven graph link <fact-node-id>
inkhaven graph pending
```

`graph link` proposes stance edges from a fact to its nearest related facts (it
grades each relation with the LLM). They land as advisory `judged` edges in the
**edge inbox** — `graph pending` on the CLI, or `Ctrl+B z → i` in the editor —
where you promote the ones that hold and dismiss the rest.

## The Graph AI scope (F9)

In the editor, cycle the AI-pane scope with **F9** to **Graph** (the last stop).
A prompt in Graph scope retrieves the passages relevant to your question and
folds in the graph edges touching each — what it contradicts, is sourced from,
links to — so the answer is grounded in how your book *connects*. It's a sticky
conversation scope; press **`p`** in the AI pane to see the retrieved passages
and their relations. The same citation contract as Book scope applies — every
claim cites a `[location/path]`, and invented labels are flagged.

## Walk the graph — `graph ask`

Where the Graph scope reads one hop, `graph ask` lets the model *walk* the graph:

```sh
inkhaven graph ask "which of my claims about the harbour contradict each other?"
```

```
» graph ask: which of my claims about the harbour contradict each other?
· search "harbour claims" (5 node(s))
· neighbours n1 — 003. Quiet hour
· contradicting n2 — 007. The lantern
· answering…

Your claim in [act-two/quiet-hour] that the harbour was dark at dusk
contradicts [act-two/the-lantern], where the lantern is described as lit…
```

It searches for seed nodes, then queries neighbours / contradictions / loci /
paths turn by turn until it can answer — honest about the graph's limits ("the
graph doesn't record that" rather than inventing a link). The exploration
transcript prints to stderr; the answer to stdout, so you can pipe it.

## The same walk, live in the editor

Type a question in the AI prompt, then press **`Ctrl+B z → w`**. The AI pane
streams the walk as it happens — each step (search → neighbours → contradictions
→ paths) — then the grounded prose answer lands as a chat turn. **`Esc`** stops
the walk; the status bar shows `turn k/N`. A walk is several model calls, so it's
an explicit action — you choose the depth per question.

## Bounded and tunable

```hjson
graph: {
  ask_max_steps: 8      // max LLM turns before a forced answer
  ask_search_width: 6   // seed nodes per search
}
```

Per the permissive principle these cap cost, they never block. Multilingual
throughout — a Russian project gets Russian grounding and tool prompts.

---

**See also:** [GRAPH.md → "Chat with your graph"](../GRAPH.md) · Tutorial 104
(the graph) · Tutorial 63 (facts & fact-checking) · `inkhaven graph ask --help`.
