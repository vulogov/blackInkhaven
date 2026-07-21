#import "../design.typ": *

#chapter(number: 5, title: "Syntax")

Sentences are built, not strung together, and syntax is the study of how. Russian is
a good place to see this, because its word order is *free* in a way English's is not:
the endings you met in the last chapter mark who is the subject and who the object, so
the words can be rearranged for emphasis without confusion. Underneath that freedom is
a stable structure, and the workbench can draw it.

#section("A baseline order")

Russian's neutral, unmarked order is subject–verb–object, the same as English:
*мать видит дом*, "mother sees house". You declare that baseline in the grammar
(`word_order: "SVO"`), and the syntax tools read it. The freedom comes from moving
pieces *out* of the baseline, which we come to below.

#section("The structure of a clause")

`tree` draws the X-bar phrase-structure tree of a clause from its verb and arguments,
placing heads and complements by the word order you declared:

```sh
inkhaven language tree Russian --verb видит --args "мать, дом"
```

```
CP
├─ C ∅
└─ TP
   ├─ NP
   │  └─ N мать
   └─ T'
      ├─ T ∅
      └─ VP
         └─ V'
            ├─ V видит
            └─ NP
               └─ N дом
```

The subject `мать` is the specifier of `TP`, sitting on the left; the object `дом` is
the complement of the verb inside the verb phrase. This is the same skeleton every
generative account of a clause assumes, drawn for a Russian sentence. Because Russian
is head-initial, the verb precedes its object; a head-final language would reverse the
lower branches, and asking for the tree is how you check that the order you declared
builds the constituents you meant.

#term("Constituent")[
  A group of words that behaves as a single unit — that can be moved, questioned, or
  replaced by one word together. The tree makes constituents visible: everything under
  one node is a constituent. Much of syntax is the study of which strings are
  constituents and which only look like they are.
]

#section("Moving a piece")

Russian's word-order freedom is *movement*: a constituent lifts out of its baseline
position to the front, for emphasis or to ask a question. `movement` performs that
over the tree and marks the gap with a trace:

```sh
inkhaven language movement Russian --verb видит --args "мать, дом" --move object
```

The object `дом` rises to the front of the clause as `NP₁`, leaving a coindexed trace
`t₁` where it came from — the structure behind *дом мать видит ⟨t⟩*, "the house,
mother sees". Because the case ending still marks `дом` as the object, the sentence is
perfectly clear even with the words rearranged; the trace records where it is
understood.

#section("Checking a clause")

The clause Oracle judges a sentence for two kinds of well-formedness at once —
argument structure and agreement:

```sh
inkhaven language check-clause Russian --verb видит --args "мать, дом"
```

It confirms the argument count matches the verb's valence (a transitive verb wants two
arguments, and here it has them), and — given the subject's features and the verb's
root — that the verb agrees with its subject. A mismatch on either is reported, so the
grammar you declared can be held to its own account of what a good Russian clause is.

#recap((
  [Russian's neutral order is SVO; its famous freedom is *movement* out of that
   baseline, made possible by the case endings that mark grammatical roles.],
  [`tree` draws a clause's X-bar structure (subject as specifier of `TP`, object as
   the verb's complement), placed by the declared word order; `movement` fronts a
   constituent and leaves a coindexed trace.],
  [`check-clause` judges a sentence's argument structure and subject–verb agreement
   together, testing the grammar against a real clause.],
))
