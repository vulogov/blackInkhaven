#import "../design.typ": *

#appendix(letter: "A", title: "Command Reference")

You never have to memorise these. Every command in this appendix was taught,
in context, in the chapter where you needed it — this page exists only for
browsing, for the moment at the keyboard when you remember *what* you want to do
but not the exact word for it. The commands are grouped by the part of the
process they belong to, in the same order the book teaches them: first defining
and growing the physical world, then deepening it with a past and a people, then
the author's deliberate hand, then working at the desk, and finally the bridges
that carry the world into your manuscript. All commands live under `realworld`
unless noted otherwise.

#section("Define & grow")

#chord_table((
  chord_row("realworld new <name>", "Scaffold a starter world.hjson with Earth-like defaults; never overwrites an existing one."),
  chord_row("realworld validate", "Compile every layer in turn and report each one ok — your proof the definition is sound before you build on it."),
  chord_row("realworld compile", "Compile the whole world — every layer, in order: astronomy → geology → climate → hydrology → demographics."),
  chord_row("… --layer <name>", "Compile just one named layer (or all) and read what it found on its own."),
  chord_row("… --materialize", "Write the compiled layers as chapters into the World system book."),
  chord_row("… --json", "Emit the result as structured data, for tools and scripts."),
  chord_row("realworld show", "Print the world definition; --json for structured form."),
  chord_row("Ctrl+B W", "Open the read-only World overview — every compiled layer, plus a \"This scene\" header when the cursor is in a scene."),
))

#section("Deepen — a past and a people")

#chord_table((
  chord_row("realworld history", "Infer a founding chronology, three epochs (Founding / Expansion / Present Age), and events (realm rise & fall, migrations), dated in years before the present."),
  chord_row("… --materialize", "Write the chronology as a History chapter in the World book."),
  chord_row("… --json", "Emit the whole chronology as structured data."),
  chord_row("realworld polities", "Cluster settlements into nations around their largest capitals — names, populations, seeded relations (allied / rival / neutral)."),
  chord_row("realworld culture", "Give each polity a culture — an ethos from its capital's biome, a belief, a language profile for the ConLang suite, and a naming sample."),
  chord_row("realworld ecology", "Generate flora and fauna archetypes, with a keystone animal per land biome."),
))

#section("Declare & the author's hand")

#chord_table((
  chord_row("realworld calendar", "Derive a story-Timeline calendar from the astronomy; prints lines you adopt into timeline.calendar."),
  chord_row("realworld magic", "Show and validate the magic ledger — the declared exceptions to physics."),
  chord_row("realworld propose", "The world proposes its settlements as Place entries for you to accept or reject."),
  chord_row("realworld proposals", "List the pending settlement proposals awaiting your decision."),
))

#section("At the desk")

#chord_table((
  chord_row("realworld scene --place <name> --day <N>", "A scene brief: season and weather at the place's latitude, its biome and climate, and the nearest realm's culture."),
  chord_row("realworld weather --day <N> --lat <deg>", "The local season and weather at a day-of-year and latitude."),
  chord_row("realworld travel --from <p> --to <p>", "Is a journey plausible? Checks real distance against the mode's pace; consults the magic ledger's travel_time rules."),
  chord_row("… --days <D> --mode <m>", "Set the days allowed and the mode: foot, horse, cart, or ship. (Also --from-x/--from-y coordinates.)"),
  chord_row("realworld fact-check --text \"…\"", "Check prose against the world — travel time, climate, date coherence — with declared magic exceptions suppressed."),
  chord_row("… --paragraph <id>", "Fact-check a manuscript paragraph by its id instead of literal text."),
))

#section("Bridges to your book")

#chord_table((
  chord_row("realworld gazetteer", "A consolidated Markdown world reference — calendar, sky, regions, landmarks, waters, settlements, economy, magic."),
  chord_row("… --output <path>", "Write the gazetteer to a chosen path."),
  chord_row("realworld map", "Render a map with the external plakat tool (optional)."),
  chord_row("inkhaven event add …", "Adopt a chosen history event onto the story Timeline — the lines realworld history prints for you."),
  chord_row("inkhaven language", "Realise a culture's language profile in the ConLang suite."),
))
