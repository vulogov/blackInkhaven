#import "../design.typ": *

#v(1cm)
#text(font: body_family, size: 22pt, weight: "bold")[Following One Book]
#v(6mm)
#line(length: 100%, stroke: 0.5pt + ink_rule)
#v(8mm)

The companion volume, *The Inkhaven Manual*, is a reference: it takes each
feature in turn and tells you what it is and how to run it. This book does the
opposite. It picks *one manuscript* and follows it — from the empty project on a
Monday morning to a typeset PDF and a small web edition — reaching for each
feature exactly when a real author would, and no sooner.

If you learn best by watching something get built, start here. You can read this
book beside the manual (it points to the relevant chapter whenever it uses a
feature in passing) or entirely on its own.

#section("The book we are writing")

Our example is a short fantasy mystery — long enough to exercise a world, a cast,
and a secret; short enough to finish inside one volume.

#term("The Ninth Lantern")[
  Nine lanterns ring the harbour town of Saltmarch, and for three hundred years no
  keeper has let one go dark. On the morning the story opens, the ninth lantern is
  cold — and the keeper who tended it is gone. A short mystery: a small town, a
  handful of suspects, a magic that runs on light, and a truth that someone has
  worked very hard to keep.
]

It is deliberately a work of fiction with *secrets and a cast*, because that is
what exercises the most of Inkhaven — a world to keep consistent, characters who
must each sound like themselves, and a reveal that no one may act on too early.
If you write non-fiction, the *shape* of the journey is identical; where a step
is fiction-specific (the world, who-knows-what, the voices) the text notes the
non-fiction parallel.

#section("The shape of the journey")

#screen(caption: "From blank project to published book")[```
  I    The Foundation ..... init the project · sketch the world & cast
  II   Drafting ........... write the opening · keep the facts straight
  III  The Middle ......... plant the secret (KEN) · voices & threads
  IV   Revision ........... the read-through · the editorial pass ·
                            did it get better?
  V    Publishing ......... assemble the PDF · EPUB & the web edition
```]

Each part is a stage every book passes through. You will not use every Inkhaven
feature to write *your* book — few books need all of them — but by the end of
this one you will have seen where each fits in the arc of a manuscript, and be
able to reach for the right one at the right time.

#recap((
  [This book follows *one manuscript*, "The Ninth Lantern", from `inkhaven init`
  to a published PDF — learning by watching a book get written.],
  [It is a companion to *The Inkhaven Manual*: the manual is the reference, this
  is the journey. Read either alone or side by side.],
  [The example is fiction with a world and a secret because that exercises the
  most of Inkhaven; the *arc* is the same for non-fiction, and the text notes the
  parallels.],
))
