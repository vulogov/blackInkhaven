#import "../design.typ": *

#chapter(number: 7, title: "Revising the prose")

The readers of Chapter 5 tested the argument — whether a claim met its sources,
whether a sentence carried its theological weight. But an argument is delivered in
prose, and prose has a second set of demands the interrogators do not touch: it must
read well, it must be grammatical, and — because revising means changing what you
have already written — it must be _safe to change_. This chapter is the polish pass
and the safety net beneath it. Its three tools are older than anything in the SCHOLAR
family, and on this track they do the quiet, unglamorous work that lets the argument
be heard.

#section("Snapshot before you revise")

Revision is destruction with intent: to improve a paragraph you must first be willing
to break it, and a study rewritten under a reviewer's questions can go through a
dozen versions of its hardest page. Inkhaven's snapshots make that safe. Press `F5`
in the editor and the current paragraph is captured — a frozen version you can always
return to — before you touch a word:

#tryit[
  Put the cursor in the "asymptote versus arrival" paragraph the Inner Theologian
  flagged in Chapter 5. Press `F5` to snapshot it. Now rewrite it freely — narrow the
  "never", name the stake, cite the tension with Matthew 5:48. If the rewrite is worse
  than the original, press `F6` to open the snapshot picker and restore the version
  you started from. You have lost nothing by being bold.
]

`F5` creates a snapshot; `F6` opens the picker for the open paragraph, where you can
read the earlier versions and restore one; and `Ctrl+F6` opens the project-wide
snapshot browser, the whole manuscript's version history in one place. Because a
snapshot records _which_ draft a paragraph was in, the readers can even attribute a
finding to the version it was made against — so you can see that the Theologian's
objection was answered by the rewrite, not merely forgotten.

#insight[
  Snapshots change how boldly you revise. The writer who cannot cheaply undo a change
  edits timidly — tinkering at the edges of a paragraph they are afraid to break. The
  writer who can snapshot in one keystroke rewrites the whole thing, reads it, and
  reverts if it failed. On a track where the hardest paragraphs are rewritten again
  and again under adversarial reading, that difference — timid tinkering versus
  fearless revision — is much of the difference in the finished prose.
]

#section("Grammar-check the passage")

With the draft safe to change, check that it is _correct_. Press `F7` on a paragraph
and Inkhaven runs a grammar check over its prose. The check keys off the project's
`language`: an English essay is read against English grammar, and a Russian, French,
German, or Spanish study is read against its own — the same setting from Chapter 1
that chose the scripture translations now choosing the grammar the prose is held to.

Grammar on this track has one wrinkle worth naming. A study of Kant is thick with
technical terms — _noumenal_, _apodictic_, the Latin _a priori_ — and with quoted
scripture in an older register. The check reads these as prose, so treat its verdict
as advice, not law: it will catch the genuine slip — the subject that lost its verb
across a long subordinate clause, the number that stopped agreeing — and it will
occasionally query a term of art that is exactly right. Take the first; wave off the
second.

#section("Read for craft — the Inner Editor")

Grammar asks whether a sentence is correct. The *Inner Editor* asks whether it is
_good_. It is the prose-craft reader — the one that notices the buried subject, the
sentence carrying three clauses that wanted to be two, the paragraph whose best line
is its third rather than its first — and it reads your style the way an attentive
line-editor would.

Open its overview with `Ctrl+V O` (O for _Observe_, the editor's defining act), and
from there `E` engages the open paragraph; its observations land in the Output pane
under the ✎ glyph. Turn it on the essay's dense third movement, and:

#transcript("Ctrl+V O → E  — the Inner Editor reads the open paragraph", [
  ✎ The sentence beginning "The differences are at least as important" opens the
  movement on an abstraction; the concrete claim — three axes of divergence — arrives
  only in the next clause. Consider leading with the three. \
  ✎ "Ontologically continuous with humanity" is doing heavy work in a subordinate
  position; a reader meets the essay's key term where the sentence is already tired.
  ✎ Three sentences in this paragraph open with "For Kant"; the parallelism is
  deliberate, but the third lands as repetition rather than rhythm.
])

None of that is about whether the argument is _right_ — the Theologian and the
confront chord already asked that. It is about whether the argument is well _made_:
whether the key term arrives where the reader is fresh, whether the rhythm is
intended or accidental, whether the concrete claim leads or trails. The Inner Editor
is genre-aware, so with `genre: "philosophy"` set it reads for the virtues this kind
of prose wants — clarity under abstraction, a controlled sentence, a claim placed
where it can be seen — rather than for a novelist's scene-craft.

#pitfall[
  The Inner Editor observes; it never rewrites. Nothing it says changes a word of
  your prose — it emits an observation and leaves the decision to you, exactly as the
  Theologian and confront do. This is deliberate, and it is the same principle across
  every reader Inkhaven carries: the tools read, and the author writes. Do not wait
  for a reader to fix a sentence. It will only ever tell you the sentence could be
  better; the fix is yours, and so is the credit.
]

#section("The order of the pass")

The three tools have a natural order. Snapshot first (`F5`), so the revision is safe.
Then read for craft (`Ctrl+V O → E`) and correctness (`F7`), and make the changes the
readings earn. If a change goes wrong, revert (`F6`) and try again. The argument was
settled in Chapter 5; this pass settles the prose that carries it — and settles it
without fear, because the version you started from is one keystroke away the whole
time.

#recap((
  [*Snapshot before you revise:* `F5` captures the open paragraph, `F6` opens the picker to restore an earlier version, `Ctrl+F6` browses the whole manuscript's history — so you can rewrite a hard paragraph boldly and revert if the rewrite fails.],
  [*Grammar-check* with `F7` — it reads against the project's `language`, so the same setting that chose the scripture translations chooses the grammar; take its catches on genuine slips, wave off its queries on terms of art.],
  [*The Inner Editor* (`Ctrl+V O` → `E`) reads for *craft*, not correctness — the buried subject, the mislaid key term, the rhythm that became repetition — genre-aware, emitting ✎ observations to Output.],
  [Every reader *observes and never rewrites*: the tools read, the author writes. Snapshot first, read for craft and correctness, act on what the readings earn, and revert fearlessly when a change goes wrong.],
))
