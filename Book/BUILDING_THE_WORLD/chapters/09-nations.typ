#import "../design.typ": *

#chapter(number: 9, title: "Nations")

Your world now has a place and a past. This part gives it a *people* — and the
first thing a people needs is not a language or a religion but a *map of power*.
Who rules this stretch of coast? Whose banner flies over that cluster of river
towns? Where does one realm end and the next begin, and along which border do
the two of them glare at each other? Before your world has a single named
character, it has *powers* — and they are already latent in the map you built.

Look again at your settlements. They are not scattered evenly. They gather:
a great port with a ring of smaller towns leaning on it, a fertile basin with
its market city and the villages that feed it. Those clusters are not an
accident of the rendering — they are where the land concentrates people. And a
concentration of people around a dominant city is very nearly the definition of
a realm. The `realworld polities` command reads those clusters and draws the
nations your geography already implies.

#two_hands()

#section("From cities to countries")

`polities` works outward from the biggest cities. It finds the largest
settlements — the natural seats of power — and gathers the smaller settlements
around each one into a group, by which capital they sit nearest and lean toward.
Each group becomes a nation.

#term("Polity")[
  An organised body of people under one rule — a kingdom, a republic, a
  city-state, a confederation. In this command a polity is a *cluster* of
  settlements gathered around one dominant city. The word stays deliberately
  neutral about the *form* of rule: the command gives you the shape of the power
  (these cities, together, under that seat); you decide whether it is a crown, a
  council, or a merchant league.
]

#term("Capital")[
  The dominant settlement a polity forms around — the largest city in the
  cluster, the seat its towns and villages lean toward. The command picks the
  capital first, by size, and then builds the realm around it. When you rename a
  polity, its capital's name is usually the place to start: realms are very often
  named for their chief city, or their chief city for the realm.
]

For each polity the command gives you three things. A *generated name*, so the
realm is something you can say aloud rather than "the cluster around city 4." A
*summed population* — every settlement in the cluster added up — so you know at a
glance whether this is a great power or a minor one. And its *sphere of
influence*: the reach of the capital, the ground its pull covers.

#term("Sphere of influence")[
  The territory a capital's pull effectively covers — the settlements that fall
  to it rather than to a rival seat. Spheres are how the command decides where
  one realm ends and the next begins: a town on the seam between two capitals is
  claimed by the nearer, stronger pull. Where two spheres press against each
  other is exactly where you will want a contested border in the story.
]

#section("Who hates whom")

A collection of nations sitting politely side by side is a map, not a politics.
What turns neighbours into a *world* is that they have *opinions* about one
another. So `polities` seeds each pair of realms with a relation — *allied*,
*rival*, or *neutral* — deterministically from the world seed, so the same world
always has the same web of friendships and grudges.

These relations are not arbitrary decoration; they are the raw material of every
conflict your story might use. An alliance is a promise that can be broken at the
worst moment. A rivalry is a war waiting for a pretext, or a cold trade dispute,
or a marriage that would end it. Even neutrality is a fact with weight: the realm
that stays out is the one both sides are courting. Read the relations as a list
of the tensions your world hands you for free.

#insight[
  A nation is not a coloured shape on a map. It is settlements *plus a story of
  who rules whom and who resents it* — the population that gives it weight, the
  capital that gives it a will, and the relations that give it something to want
  and something to fear. Draw the borders and you have a map; name the grudges
  and you have a plot.
]

#question[
  Who are the powers of your world — and who hates whom, and *why*? Not "these
  two are rivals," but the reason: an old war never quite settled, a river both
  need, a throne two families both claim, a faith one exported and the other
  refused. The command hands you *that* they are rivals; the richest work you
  will do is deciding the *because*.
]

#tryit[
  Run `realworld polities`. Read the realms first — their names, their capitals,
  their populations — and get a feel for which are the great powers and which are
  the minnows. Then read the relations: the allies, the rivals, the neutrals.
  Find the one rivalry that most makes you want to know its history, and write a
  sentence explaining why those two realms cannot stand each other.
]

#section("A first draft of a political map")

Keep this grounded. The realms `polities` gives you are not a decree about your
world's politics — they are the political map your *geography* implies, drawn
consistently from where the cities actually are. That makes them a strong first
draft and a poor final word.

So treat them as clay. The generated names are placeholders for names in your
own tongue — and when you reach the culture chapter, the language each people
speaks will give you far better ones. The borders follow population, but your
story may need a realm that punches above its size, a capital in exile, a
federation the map would have split in two. Redraw them. Merge two clusters the
history joined; split one a civil war tore apart.

#note[
  The world proposes the realms; you rule them. Nations are *generated* first:
  `realworld polities` reads them out of where your cities already sit and seeds
  their relations from the world seed, and it writes nothing into your manuscript.
  But you may also *declare* realms of your own, in a `nations:` block. Each
  declared realm pins a named country to a capital cell; the world clusters the
  rest of the settlements around your seats just as it does around the generated
  ones, and where a declaration and the inference disagree, your declared *names*
  and *relations* win. You still rename, redraw, and rewrite the grudges as the
  story needs, exactly as you accept or reject the places the world proposes. The
  map is the world's; the politics is yours.
]

#section("Declaring your own nations")

Redrawing the generated realms by hand is one way to get the politics you want.
Declaring them outright is another, and it is the one to reach for when a realm
matters enough to fix in the world itself — a capital your plot has already
chosen, an old enmity the story turns on. You write realms into `world.hjson`, in
a `nations:` block, and the world builds around them:

#hjson[
```
nations: [
  {
    name: "Karon"
    capital: [48, 19]
    relations: [
      { with: "Serai", stance: "rival" }
    ]
  }
]
```
]

The `capital` is a *map cell* — an `[x, y]` coordinate on the world grid, not a
city name — and the world seats the realm at the settlement nearest that cell,
then clusters the surrounding towns around it exactly as `polities` does for a
generated seat. The `relations` list overrides the seeded web pair by pair: each
`{ with, stance }` names another realm and the stance this one takes toward it —
`rival`, `allied`, `neutral` — replacing whatever the seed rolled for that pair.
Everything you do not declare is still inferred, so a single named realm can sit
inside a map of otherwise generated neighbours.

#note[
  The coordinate is checked against the land. If a `capital` cell sits far from
  any settlement — out in the wilderness, where no city stands to be its seat —
  the world warns you: a realm needs people to rule, and a capital in empty
  country is almost always a coordinate typed a few cells wrong. As ever the
  warning does not overrule you; it points at the seat with no city under it and
  lets you move it.
]

#recap((
  [Your settlements already cluster around dominant cities; `realworld polities`
   reads those clusters into *nations*, each a *polity* built around its
   *capital*.],
  [Each realm gets a generated name, a summed population, and a *sphere of
   influence* — and where two spheres meet is where a contested border belongs.],
  [The command seeds every pair of realms with a relation — *allied, rival, or
   neutral* — deterministically, handing you the tensions your story can use.],
  [A nation is settlements *plus a story of who rules whom and who resents it*.
   The realms emerge from where the cities are; the author renames and re-draws
   as the story demands.],
))
