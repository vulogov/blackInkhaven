#import "../design.typ": *

#chapter(number: 1, title: "The Facts Beneath")

Every book rests on things that must stay true. In a novel it is the world's own
law — the queen's name, the distance between two cities, the year the plague came.
In non-fiction it is the borrowed fact — the population, the date, the finding a
reader could look up. Get one wrong once and a reader shrugs; get it wrong *twice,
differently*, and the trust is gone. Inkhaven gives these must-stay-true things a
home of their own: the *Facts* book.

#term("Fact")[
  A *fact* is a specific, checkable claim your book rests on — "the war ended in
  412," "Vienna sat on the Danube," "she is the elder sister." It lives in the Facts
  system book, apart from your prose, so the tool can hold it steady while you write
  around it.
]

#section("The bible your book checks itself against")

The Facts book is your story bible made *operational*. It is not a document you
read; it is a set of claims the tool can test your manuscript against. You add to it
as you go — a name here, a date there — and Inkhaven keeps them organised the way a
book is, in foldable sections.

#screen(caption: "The Facts book — the world's must-stay-true claims")[```
▾ Facts
  ▾ People
    ✓ Mara is the elder of the two sisters
    ✓ Joren served in the northern war
  ▾ Chronology
    ✓ The war ended in the year 412
    ✓ The coronation follows the war by three years
  ▾ Geography
    ✓ Rillmark stands where the two rivers meet
```]

#section("The fact-check — is the page true to the bible?")

Once a fact is in the book, `Ctrl+B Shift+X` checks the *open paragraph* against it
— and against your compiled world. It is looking for the moment your prose and your
bible disagree: a journey too fast for its distance, a season that cannot follow the
last, a claim that contradicts something you already fixed as true.

#screen(caption: "Ctrl+B Shift+X — the paragraph, against the world")[```
Fact-check · ch. 14 ¶ the-crossing
  ⚠ "they reached Rillmark by nightfall" — the bible puts Rillmark
    six days' ride away; a single day is not plausible.
  ● no other conflicts with the Facts book.
```]

#section("When the facts disagree with each other")

The subtler danger is not prose-versus-bible but *bible-versus-bible*: two claims
you fixed as true, months apart, that cannot both hold. `inkhaven factcheck` reads
the Facts book for these internal contradictions, and the SCHOLAR tools go further —
`/contradict` surfaces the source-attributed clashes, `/relate` grades a new claim
against what you already believe (does it *contradict*, *qualify*, *agree*, or say
nothing).

#two_track(
  [For fiction, the Facts book is your continuity bible — the invented truths the
  rest of the machinery in this book checks against. It is where "who is the elder
  sister" stops being something you hope you remembered.],
  [For non-fiction, every kept fact remembers *where it came from*. The Research
  Assistant (its own companion book) gathers and cross-checks them; here they become
  the ground your argument — and every other check — stands on.],
)

The Facts book is the floor of everything that follows. The knowledge graph draws
its edges over these facts; SENTINEL's continuity watch reads them; KEN asks who
*knows* them. Get this floor solid, and the rest of the house has something to stand
on.

#recap((
  [The *Facts book* is your story bible made operational — the must-stay-true claims
  your manuscript can be checked against.],
  [`Ctrl+B Shift+X` fact-checks the open paragraph against the bible and the world;
  `inkhaven factcheck` finds contradictions *within* the bible.],
  [SCHOLAR's `/contradict` and `/relate` grade how a new claim stands against what
  you already hold true.],
))
