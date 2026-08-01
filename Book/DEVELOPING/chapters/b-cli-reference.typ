#import "../design.typ": *

#appendix(letter: "B", title: "The command reference")

Every command this book put to work, gathered in one place and grouped by the part
of the process it serves. This is a reference, not a tutorial — for the full
treatment of any subsystem, the sibling books go deep; here you have the whole
surface at a glance, so you can find the command you half-remember. Add `--help` to
any command for its exact flags.

#section("Starting and shaping a project")

#list(
  [`inkhaven init <path> [--template T]` — create a project. Templates: `empty`, `novel`, `nonfiction`, `technical`, `rpg-sourcebook`, `nanowrimo`.],
  [`inkhaven tui` — launch the full editor. Where most of the work happens.],
  [`inkhaven add <book|chapter|subchapter|paragraph> <title>` — add a node.],
  [`inkhaven list` — print the project tree; `inkhaven outline` — an indented outline.],
  [`inkhaven mv <path> up|down` — reorder a node; `inkhaven delete <path> --yes` — remove one.],
  [`inkhaven paragraph copy|move` — move a paragraph across parents.],
  [`inkhaven search <query>` — full-text and semantic search; `inkhaven stats` — per-paragraph stats.],
  [`inkhaven reindex [--prune] [--adopt]` — re-index hand-edited files back into the store.],
)

#section("Worldbuilding")

#subsection("The world model")

#list(
  [`inkhaven realworld new "<name>"` — create a `world.hjson` world definition.],
  [`inkhaven realworld validate` — check the definition is well-formed.],
  [`inkhaven realworld compile [--materialize]` — grow the world; `--materialize` writes it into the World book.],
  [`inkhaven realworld proposals` — list what the world offers (settlements, calendar, history); `… accept <id>` to take one.],
  [`inkhaven realworld propose-myth | propose-rulers | propose-language` — AI proposals seeded from the world.],
)

#subsection("Reading the world into the manuscript")

#list(
  [`inkhaven realworld scene --place <name> --day <N>` — a scene brief: season, weather, biome, nearest feature.],
  [`inkhaven realworld weather --day <N> --lat <deg>` — local weather at a day and latitude.],
  [`inkhaven realworld travel --from <p> --to <p>` — is a journey plausible for its distance and mode?],
  [`inkhaven realworld places` — the Place ↔ world cross-references.],
  [`inkhaven realworld set-coords <name> --lat <d> --lon <d>` — position a Place on the grid so the map draws it.],
  [`inkhaven realworld gazetteer [--output <f>]` — a consolidated Markdown world reference.],
  [`inkhaven realworld map` — render a map with the external plakat tool.],
)

#subsection("The world's people, past, and checks")

#list(
  [`inkhaven realworld history | chronicle` — the world's history, as events / as a state trajectory.],
  [`inkhaven realworld polities | culture | trade | ecology` — nations, cultures, trade routes, and life.],
  [`inkhaven realworld name` — propose settlement names in each realm's style.],
  [`inkhaven realworld co-location` — flag a character in two places at overlapping times.],
  [`inkhaven realworld coherence <node>` — an LLM pass over a container for cross-paragraph contradictions.],
  [`inkhaven realworld critique` — an AI reading of the whole world for consistency and realism.],
  [`inkhaven world utopia-check` — read the declared premises as a chain of claims and find where the society cheats.],
  [`inkhaven world utopia-suppress <id>` — mark a tension as intended; `inkhaven world utopia-refresh` — re-check.],
  [`inkhaven character arc | check | refresh | plan` — declare and test character arcs and agency.],
  [`inkhaven myth scan | check | profile | refresh | suppress` — the declared symbol / motif / archetype layer.],
  [`inkhaven event add | list | show | critique` — the story timeline; `inkhaven export-timeline [typst|svg|png]`.],
)

#section("Research, grounding, and continuity")

#list(
  [`inkhaven research` — the Research Assistant. Inside it, slash-commands do the work: `/fact`, `/note`, `/web`, `/wikidata`, `/geonames`, `/openalex`, `/arxiv`, `/gutenberg`, `/triangulate`, `/factcheck`, `/undisputed`, `/synthesize`, `/outline`, `/gaps`, `/upgrade`, `/stale`, `/deadsources`, `/sources`, `/calc`, `/world`, `/rag`, `/chain`.],
  [`inkhaven facts init --genre G` — seed a Facts book's continuity categories (`general`, `fantasy`, `scifi`, `mystery`, `historical`).],
  [`inkhaven facts scan | check | list | import | extract` — find, audit, and manage continuity facts.],
  [`inkhaven fact-check` — audit the manuscript against the world and the kept facts.],
  [`inkhaven continuity extract | list` — the continuity bible.],
  [`inkhaven drift scan | list` — style and continuity drift across the book.],
  [`inkhaven terms suggest | check` — propose and enforce canonical terminology.],
  [`inkhaven thread doctor | add | list | export` — plot threads: setups, developments, and payoffs.],
)

#section("Citations and sources")

#list(
  [`inkhaven sources list` — every citation in the Sources book.],
  [`inkhaven sources import <file.bib>` — bring citations in from BibTeX (e.g. Zotero).],
  [`inkhaven sources export --format bibtex|csl-json [--out <f>]` — write citations out.],
  [`inkhaven sources check` — validate entries; exits non-zero on a problem (fits CI).],
)

#section("The semantic net")

The knowledge graph (2.0, RFC SEMNET-1) — a typed-edge layer over every node,
connecting what you already have into one interrogable whole. It starts empty:
`graph rebuild` derives its structural edges from your project, `graph lexical`
imports the WordNet bridge, and the editor's `Ctrl+V ?` confront persists judged
stance edges as you work. Its own book, this manual's sibling reference
`Documentation/GRAPH.md`, covers the edge model in full.

#list(
  [`inkhaven graph rebuild` — (re)derive the structural edges: paragraph links, timeline event involvements, fact provenance, `/factcheck` verdicts, and `@key[locus]` citations. Idempotent.],
  [`inkhaven graph lexical` — import the WordNet lexical bridge for the project language (run `inkhaven wordnet fetch <lang>` first).],
  [`inkhaven graph stats` — node + edge counts and a per-kind breakdown.],
  [`inkhaven graph neighbors <node>` — the node's one-hop neighbourhood as a tree (links, contradictions, sources, citations, senses).],
  [`inkhaven graph contradicting <node>` — the recorded stance clashes touching a node.],
  [`inkhaven graph loci <node>` — the primary-source loci a node cites; `inkhaven graph paths <from> <to>` — a bounded citation / link path.],
  [`inkhaven graph promote <edge> | dismiss <edge>` — accept a judged stance edge (kept across rebuilds), or delete one.],
)

#section("Constructed languages")

The ConLang suite is large (its own book, _Constructed Language Development_,
covers it in full). The families, each `inkhaven language <action> <lang>`:

#list(
  [*Lexicon* — `init`, `add-word`, `import`, `generate-lexicon`, `dictionary`, `gaps`, `audit`, `doctor`, `query`.],
  [*Phonology* — `generate-word`, `syllabify`, `ipa`, `stress`, `romanize`, `tone`, `sound-change`, `reconstruct`.],
  [*Grammar* — `grammar`, `define-rule`, `derive`, `gloss`, `paradigm`, `sentence`, `agree`, `grammar-book`.],
  [*Translation* — `translate`, `reverse`, `cross`, `remember`, `memory`, `corpus`, `eval`, `export-translation`.],
  [*Varieties & contact* — `varieties`, `lect`, `dialects`, `borrow`, `areal`, `cognates`, `family-tree`.],
  [*Writing systems* — `font-build`, `glyph-draft`, `glyph-lint`, `transliterate`, `spatial-typst`.],
  [*World links* — `link-place`, `link-character`, `speakers`, `ecology`, `idiolect`; and `export` to a portable format.],
)

#section("The AI readers")

#list(
  [`inkhaven inner-socrates check | persona | findings | ledger | suggestions` — the Socratic interrogator and its personas.],
  [`inkhaven inner-editor engage | findings | intent | suggestions | config` — the craft reader.],
  [`inkhaven theologian scan | session | suppress` — the moral / theological reader.],
  [`inkhaven show-dont-tell …`, `inkhaven prose profile|refresh|suggest`, `inkhaven dialogue …`, `inkhaven editorial` — the craft passes.],
)

#section("Producing the book")

#list(
  [`inkhaven build` — assemble the chapters and compile (the headless twin of `Ctrl+B B`).],
  [`inkhaven export typst|pdf|epub|docx [--status S] [--tag T]` — render the manuscript, scoped; companion books are never exported.],
  [`inkhaven pdf …` — a finishing workshop: `impose`, `booklet`, `cover`, `barcode`, `watermark`, `preflight`, `merge`, `split`, and more.],
  [`inkhaven submission …` / `inkhaven submissions …` — generate a synopsis / logline / comparables, and track where the book has gone.],
)

#section("Housekeeping")

#list(
  [`inkhaven backup [--out <f>]` / `inkhaven restore <archive> --to <dir>` — archive and recover a project.],
  [`inkhaven doctor [--scan] [--autofix]` — health check and repair.],
  [`inkhaven cost` — what the AI features have spent; `inkhaven config` — read or set configuration.],
  [`inkhaven ai "<prompt>"` — a one-shot LLM inference; `inkhaven bund "<code>"` — evaluate a Bund expression.],
)

#note[
  This is the surface the book touches, not the whole of Inkhaven — there is more,
  and `inkhaven --help` (and `inkhaven <command> --help`) is the always-current
  authority for your version. When a command here feels thin, that is the sign to
  open the sibling book that treats its subsystem in full.
]
