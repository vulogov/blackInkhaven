#import "../design.typ": *

#chapter(number: 13, title: "Rules and Magic")

The whole world you have built so far obeys its physics. Riders cross a continent
at the pace of a horse, not a thought. Winter arrives when the orbit says it
should. A character born a hundred years before the present is, by the calendar,
a hundred years old. The fact-checker holds your prose to all of this, and that is
exactly what makes the world worth having.

But you may be writing a world where some of that is *meant* to break. Messenger
birds that fly day and night, never resting, carry word faster than any horse
could run. A sorcerer keeps a garden in eternal summer while snow falls beyond its
wall. An order of monks lives ten normal lifetimes. These are not mistakes. They
are the point of your world — and if the fact-checker does not know about them, it
will flag every one as an error, and you will drown in false warnings.

The answer is not to switch the fact-checker off. A world with no laws catches no
real contradictions either. The answer is to write your magic *down*, as a short
list of precise exceptions the checker can consult.

#section("The magic ledger")

The `magic:` block is that list. It has two parts: `enabled`, which turns the
whole system on, and `rules`, an array of individual exceptions. Each rule is a
small, exact statement of *one* way your world departs from its physics.

#term("Magic ledger")[
  The `magic:` block of `world.hjson` — a declared, enumerated list of every way
  your world's magic overrides its physics. It is called a *ledger* because that
  is what it is: an account, kept openly, of the exceptions you have chosen. A
  reader never sees it, but its discipline is felt on every page that stays
  consistent.
]

#term("Rule")[
  One entry in the ledger — a single named exception. A rule carries a `kind` (what
  sort of magic it is), a `covers` list (which fact-check categories it may
  suppress), a `description` in plain words, and an `applicable_to` scope (the
  roles, regions, or seasons it applies to). One rule, one exception; the world's
  magic is the sum of them.
]

#term("Exception to physics")[
  A place where your world *knowingly* breaks a law it otherwise keeps. The word
  that matters is *knowingly*: an exception is written down and bounded, so both
  the fact-checker and the reader can trust that the rest of the world still holds.
]

#section("How a rule teaches the fact-checker")

The mechanism is simpler than it sounds. Each fact-checker warning belongs to a
*category* — `travel_time`, `climate_anomaly`, `character_age`, and the rest. A
rule's `covers` list names the categories it is allowed to silence, and its
`applicable_to` scope says where.

So you write a rule: *messenger birds fly day and night; their journeys break the
normal travel limit.* You give it `covers: [travel_time]` and scope it to the
courier role. From then on, when you fact-check a scene where a message crosses
the realm overnight, the checker sees the rule, recognises that this exact
exception was declared, and *stops flagging it*. A one-time act of declaration
turns an endless stream of false warnings into silence — while every travel-time
claim your rule does *not* cover is still checked as strictly as before.

#hjson[```
magic: {
  enabled: true
  rules: [
    {
      kind: "messenger_birds"
      covers: ["travel_time"]
      description: "Royal pelicans fly day and night, far faster than any rider."
      applicable_to: { roles: ["royal_messenger"], regions: ["any"] }
    }
  ]
}
```]

A second rule follows the same shape. Your order of long-lived monks breaks a
different law — age, not travel — so it `covers` a different category and scopes
itself to its own role:

#hjson[```
{
  kind: "long_lived_priests"
  covers: ["character_age"]
  description: "The monks of the Grey Cloister live ten normal lifetimes."
  applicable_to: { roles: ["cloister_monk"], regions: ["any"] }
}
```]

#note[
  A rule suppresses only the categories in its `covers` list, only within its
  `applicable_to` scope. Declaring that couriers outrun a horse does not excuse
  your foot-soldiers from the same physics. Magic narrows the checker exactly where
  you tell it to, and nowhere else.
]

#section("Validating the ledger")

`realworld magic` does two things: it *shows* the ledger, and it *validates* it. A
ledger is a promise to the fact-checker, and a broken promise is worse than none,
so the validator looks for the ways a ledger goes wrong. It flags a rule that
`covers` nothing — an exception that suppresses no category is dead weight,
probably a mistake. It flags an *unknown category* — a `covers` entry the checker
does not recognise, which would silence nothing and likely hides a typo. And it
flags a *duplicate* — two rules claiming the same ground, a sign your intentions
have started to overlap and drift.

#insight[
  Magic that means "anything can happen" is not worldbuilding — it is the absence
  of it, and a reader feels the floor drop out. A magic *system* is the opposite: a
  short, explicit list of consistent exceptions to laws that otherwise hold. The
  ledger is what makes the difference real rather than merely intended. The laws
  stay in force; the exceptions are named, bounded, and few. That is what lets a
  reader trust the impossible — because everything around it still obeys the rules.
]

#pitfall[
  The failure that hurts most is the *undeclared* exception: magic that quietly
  breaks a law you never wrote down. Your courier outruns a horse in chapter six
  because the plot needed it, but no rule says couriers can — so a reader who
  clocked the distances feels the cheat, even if they cannot name it. The fix is
  the ledger. If your world breaks a law, say so, once, in `magic:`. An exception
  you declared is wonder; an exception you smuggled is a plot hole.
]

#question[
  What are your world's *laws*, and what are their *precise* exceptions? Not "there
  is magic" — that answers nothing. Which specific law does each piece of magic
  break, for whom, and where does the exception stop? Answer that for every
  supernatural thing in your story, and you have written your ledger.
]

#tryit[
  Run `realworld magic`. Read your ledger back to yourself as the fact-checker
  reads it: each rule, the category it silences, the scope it silences it in. Then
  add a rule that covers a category not on the recognised list, run it again, and
  watch the validator catch you. That flag is the ledger keeping its promise.
]

#recap((
  [Magic is not the suspension of your world's physics but a set of *declared,
   consistent exceptions* to it, kept in the `magic:` block — `enabled` plus a list
   of `rules`.],
  [Each rule carries a `kind`, a `covers` list of fact-check categories it may
   suppress, a `description`, and an `applicable_to` scope. A rule teaches the
   fact-checker to stop flagging exactly the exception you declared — and nothing
   else.],
  [`realworld magic` shows *and* validates the ledger, flagging a rule that covers
   nothing, an unknown category, or a duplicate.],
  [A magic *system* is a short list of bounded exceptions to laws that still hold;
   "anything goes" is not worldbuilding. An exception you declare is wonder; one you
   smuggle past a law you never wrote down is a cheat the reader feels.],
))
