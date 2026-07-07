#import "../design.typ": *

#chapter(number: 10, title: "Cultures and Tongues")

In the last chapter you clustered your settlements into nations — polities with
capitals, populations, and seeded relations of alliance and rivalry. But a nation
is not only a border on a map and a count of heads. A people has a *character*:
things it holds sacred, a way of carrying itself that its homeland taught it, and
a tongue in which it names its children and its towns. Two realms with identical
populations can be nothing alike — a desert khanate and a forest confederacy are
different peoples before either of them has done a single thing in your story.

The command that gives each of your polities that character is `realworld culture`. It reads the nations you already have and, for each one, proposes three
things at once: an *ethos* drawn from the land its capital stands on, a *belief*
its people live by, and a *language profile* — a compact sketch of the tongue
they speak. Together these turn a shaded region into a people you could sit down
and share a meal with.

#term("Culture")[
  The shared character of a people — how they see the world, what they honour,
  and how they speak. In the World system a culture belongs to a *polity*: one
  nation, one culture, generated from where that nation lives and grown into
  something you can name and describe. It is the human layer that sits atop the
  physical world, the way demographics sits atop climate.
]

#section("An ethos grows from the ground")

The first thing `realworld culture` gives a people is an *ethos*, and it does not
pull it from nowhere. It reads the biome of the capital — the land your largest
city actually stands on — and lets that land shape the temperament. This is the
oldest idea in worldbuilding, made mechanical: people are marked by the country
that fed them.

#term("Ethos")[
  The characteristic temperament and values of a people — how they meet the
  world. A desert people, schooled by scarcity, may prize hospitality, endurance,
  and the sacred trust of water shared; a forest people, hemmed in and sheltered,
  may prize kinship, patience, and a wariness of what the trees hide. The ethos
  is the biome's fingerprint on the human heart.
]

Because the ethos follows from the capital's biome, it is not arbitrary and it is
not detachable. Move a nation's heart from the savanna to the taiga and its ethos
would move with it. That is the point: your peoples differ from one another for
the *same reason your climates do* — they grew in different places. You built
those places in Part II without thinking about anyone living in them; now the land
pays you back by telling you who its people became.

#insight[
  A desert people and a forest people are not two colours on a legend — they are
  two answers to the question "what does this land ask of the people who live
  here?" Let the biome you already built decide the ethos, and your peoples will
  differ the way real peoples do: because their homelands differed first.
]

#section("A belief they live by")

On top of the ethos, `realworld culture` proposes a *belief* — the frame through
which a people explains the world and orders its life. It might be reverence for
ancestors, a pantheon tied to the seasons your astronomy already fixed, a single
distant sky-god, a cult of the great river that feeds the capital, or a
philosophy with no gods in it at all. The belief gives you the thing your
characters swear by, build temples to, go to war over, and quietly stop believing
in — the material of a hundred scenes.

#question[
  For each of your peoples: what do they hold *sacred* — and can you trace it back
  to their homeland? A people of the river mouth might worship the flood; a people
  of the long dark winter might reckon time by the return of the light. If a
  belief could belong to any people anywhere, it is not yet rooted. Ask what *this*
  land would teach *these* people to fear and to honour.
]

#section("A language profile — a proposal, not a language")

The third thing a culture carries is a *language profile*: a short typological
sketch of the tongue the people speak. You will see it written in a compact line
like `SOV · agglutinative · tonal` — three coordinates that place a language in
the space of all human languages.

#term("Language typology")[
  A description of a language by its structural type rather than its words, along
  three axes: *word order* (the default arrangement of Subject, Object, and Verb —
  SOV, SVO, VSO, and so on), *morphology* (how words are built — *isolating*, one
  morpheme per word; *agglutinative*, morphemes glued in transparent chains;
  *fusional*, morphemes fused so one ending carries many meanings), and *sound*
  (the phonological flavour — *tonal*, where pitch distinguishes words, versus
  non-tonal, and the general texture of the consonants and vowels). Three
  coordinates, and you already know a great deal about how a tongue would feel.
]

Here is the crucial thing, and it is the reason this chapter exists at all: the
language profile is *not a language*. It is a *proposal* — a specification, a
brief handed to a builder. `realworld culture` also gives you a small *naming
sample*, a handful of names in the proposed style so you can hear its music, but
it does not invent a phonology, a lexicon, or a grammar. It tells you what kind of
tongue this people speaks. It does not speak it.

#note[
  The naming sample is illustrative, not canonical. It shows you the flavour of
  the profile — the shape of the syllables, whether the tongue runs to long
  agglutinative chains or short isolating stems — so you can decide whether that
  is the language you want this people to have before you build it for real.
]

The sample is one name; a realm has many towns. `realworld name` extends the
sample into a whole gazetteer: it proposes a name for every settlement in its
realm's phonic style, so a realm's towns share a family sound — a Karon coast of
Torvelras and Velkaeths, a Serai valley that sounds nothing like it — instead of
the generic placeholder names. They are proposals in the world's style; adopt one
when you accept its Place, or, once you have realised the realm's tongue in the
ConLang suite, name in the real language instead.

#section("The bridge to the ConLang suite")

To turn the profile into a real language you cross from the World system into
Inkhaven's *ConLang suite* — its constructed-language workshop — with `inkhaven language`. That is where the profile stops being a sketch and becomes a tongue: a
sound inventory, rules for how syllables combine, a lexicon that actually contains
words, a grammar that inflects them. You hand the ConLang suite the profile as its
starting brief — `SOV · agglutinative · tonal` — and it helps you build the
phonology, the vocabulary, and the grammar that profile describes. Then, and only
then, does the culture *speak* an invented tongue rather than merely gesture at
one.

#propose_accept()

You do not have to carry the profile across by hand. Run `realworld propose-language` and the world reads every culture's profile and naming sample and
proposes a *language* per people — waiting in the same queue as your Places and
rulers. Accept one and the world scaffolds a fresh language book in the ConLang
suite for you: its Meta, Phonology, Grammar, Dictionary, and Sample-texts
chapters, with a `world-profile` brief already written in — the word order, the
morphology, the sound, and the naming sample the world proposed. The handoff is
done; what remains is the making, which was always yours.

This is the *World × ConLang bridge*, and it is the reason the two systems exist
side by side rather than as one. The World system is very good at *where a
language sits* — it can read the biome, the isolation of a mountain valley, the
trade contacts of a river port, and propose a plausible typology for a people who
grew up there. It is not in the business of *inventing the language*; that is
craft of a different kind, and the ConLang suite is built for exactly it. So the
world proposes the profile, and the ConLang suite realises it. Neither could do
the whole job alone; together they take you from "these people live in a cold
forest" all the way to a named child in a spoken tongue.

#insight[
  A people is three things at once: *where they live*, *what they believe*, and
  *how they speak* — and the world proposes all three. The ethos comes from the
  land you built, the belief from the ethos, and the language profile is a brief
  the ConLang suite turns into a real tongue. Build a people the way you built a
  climate: let each layer follow from the one beneath it.
]

Remember, too, that all of this obeys the same authority discipline as the rest of
the world: `realworld culture` *proposes* an ethos, a belief, and a profile. You
accept what rings true, rewrite what does not, and discard what belongs to some
other people. The world is handing you a strong first draft of a nation's soul —
never the last word. The last word, here as everywhere, is yours.

#tryit[
  Run `realworld culture` and read one nation's card end to end — its ethos, its
  belief, its language profile, and the naming sample beneath it. Trace the ethos
  back to the capital's biome: does it fit the land you built? Then take the
  profile line — say `SOV · agglutinative · tonal` — and open `inkhaven language`
  with it as your brief. You have just crossed the bridge from a world that knows
  *where* a language lives to a suite that will help you *speak* it.
]

#section("Pinning a culture")

`realworld culture` generates a culture for every nation, and much of the time its
proposal is the right one — the land it read is the land you built. But now and
then you have a people already alive in your head, with a character no biome would
have guessed, and you want to hold it fast against anything the generator might
say. You can *pin* your own culture with a `cultures:` block, matched to a nation
by name. Whatever you declare — the *ethos*, the *belief*, the *language* profile —
overrides the generated one for that nation.

#hjson[```
cultures: [
  {
    nation: "Karon"
    ethos: "mercantile and open"
    belief: "the tide-mother"
    language: "SOV · agglutinative · tonal"
  }
]
```]

You do not have to pin all three; declare only the fields you mean to fix, and the
generator still fills the rest. A pinned culture is you taking the last word early
— telling the world *this* people is settled, build the others around it.

#note[
  The world still checks a pinned culture for plausibility. It warns if a
  *seafaring* ethos is pinned to a dry, inland capital that touches no coast — a
  people of the tides with no tide to speak of — or if the `nation` you named does
  not exist among the polities you clustered. The warning does not overrule you;
  the author always wins. It only asks whether the people you pinned truly belongs
  to the land you gave them.
]

#section("The realm's ruler, into your cast")

A realm implies a person at its head, and that is the one individual the world
can hand you without inventing a life out of nothing. `realworld propose-rulers`
reads your polities and proposes, for each, a *ruler* — a Character named in the
same style the world uses for its towns, rooted in that realm's ethos and belief:

```
inkhaven realworld propose-rulers
```

Each proposal waits in the same queue as your Places and Mythology entries. Accept
one and it becomes a paragraph in the Characters book — a short, factual stub: who
they rule, from what capital, over how many people, and what their people hold
sacred. It is a starting point, not a character. The world will not give you their
wound, their want, or the choice that breaks them; those are the work only you can
do. What it gives you is a name to rename and a throne to fill, so your cast begins
already standing on the map you built.

#note[
  The world proposes *one* ruler per realm and nothing more — it does not model
  individuals, so it will not populate your book with courtiers, rivals, or kin.
  That restraint is deliberate: a generated crowd of empty names is a burden, not
  a gift. One rooted figure per realm, offered for you to accept or ignore, is the
  honest limit of what a deterministic world can know about people.
]

#recap((
  [`realworld culture` gives each polity a *culture*: an *ethos* drawn from the
   capital's biome, a *belief* the people live by, and a *language profile*.],
  [The *ethos* follows from the land — a desert people and a forest people differ
   for the same reason their climates do, because their homelands differed first.],
  [A *language profile* (`SOV · agglutinative · tonal`) is a typological sketch —
   word order, morphology, and sound — plus a naming sample. It is a *proposal*,
   not a finished language.],
  [You realise the profile in Inkhaven's *ConLang suite* with `inkhaven language`,
   which builds the real phonology, lexicon, and grammar — the *World × ConLang
   bridge*, and the reason the two systems exist together.],
  [As always, the world *proposes* a people's character; the author decides. A
   people is where they live, what they believe, and how they speak.],
  [`realworld propose-rulers` offers one ruler per realm as a *Character* stub,
   named in style and rooted in the culture — a name to rename and a throne to
   fill. The world models no one else; the rest of the cast is yours.],
))
