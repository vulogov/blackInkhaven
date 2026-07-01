# Tutorial 103 — The Research Assistant

*Inkhaven 1.5.0*

Your manuscript stands on a foundation of facts: the geography that must stay consistent, the history a
character half-remembers, the physics of an invented world. In Inkhaven those live in the **Facts**
system book — the ground truth that the world-checker (WORLD-4/5/6), the Book-scope RAG chat, the Inner
family, and the Mythology library all draw on. The trouble was always *populating* it: you had to leave
the writing environment, search, read, decide, and hand-type each entry.

**`inkhaven research`** closes that loop. It is a **separate, full-screen TUI** — its own layout and
keymap — where you conduct AI-assisted research and transfer **verified** findings straight into the
Facts (or Notes) corpus, with a confirmation step on every insertion. The output of a session is not a
chat log; it is a richer, audited, *yours* knowledge base that every other feature can immediately use.

This tutorial walks the whole tool top to bottom.

---

## 1. Launching

```sh
inkhaven research                       # opens the thread picker, or your one/default thread
inkhaven research --thread rome         # open (or create) a named, resumable session
inkhaven research --list-threads        # name · last-active · turns · cost   (--format json)
inkhaven research --export-thread rome  # the session as Markdown   (--format json, --out FILE)
```

No `--project` is needed inside a project directory. The screen needs **≥ 80 columns** (it shows a
resize hint below that). Quit any time with **`q`** (outside a text field), or **`Ctrl+Q`** / `Ctrl+C`
from anywhere — the terminal is always restored.

If you forget a key, press **`Ctrl+B h`** for a full quick reference of every chord and `/command`.
**`?`** toggles the one-line context hints at the bottom.

---

## 2. The layout

```
┌─ Facts ───────────────┬─ Research · thread: rome ─────────────────┐
│ (the Facts book tree) │ (streaming RAG chat)                      │
│  navigate · pin · edit │  ❯ query 1                                │
│                        │  your question (accent colour)            │
│                        │  the model's answer …                     │
├────────────────────────┴───────────────────────────────────────── ┤
│  context-sensitive hint line  (? toggles)                          │
├──────────────────────────────────────────────────────────────────┤
│ Query  /fact "extract the capacity figure" → Facts/Rome           │
├──────────────────────────────────────────────────────────────────┤
│ [RAG: Facts+Full]  [~$0.031]  [⬡ Rome/Engineering]  [?:help q:quit]│
└──────────────────────────────────────────────────────────────────┘
```

Three focus targets — the **Facts tree** (left ~40 %), the **chat** (right ~60 %), and the **query
prompt**. **`Tab`** / **`Shift+Tab`** cycle them; the active pane's border lights up in the theme's
focus colour. Colours come from your project / global `theme:` config, so the assistant matches the
editor.

The bottom status bar shows the live **RAG mode**, the running **session cost** estimate, your **pinned
nodes**, and any transient message.

---

## 3. Asking — retrieval-augmented chat

Type a question in the query prompt and press **`Enter`**. The answer streams into the chat, **grounded
by your Facts book**: before each query the assistant retrieves the most relevant facts and prepends
them as context, so the model reasons over *your* world, not just its training data.

**`F10`** cycles the RAG mode (also `/rag`):

- **Facts+Full** — your facts *plus* the model's general knowledge (default);
- **Facts only** — answer strictly from your corpus ("if it's not in the provided context, say so");
- **Full only** — no retrieval; the model's knowledge alone.

The query prompt is a real editable field: **`←` `→` `Home` `End`** and `Backspace`/`Delete` edit;
**`↑` `↓`** move between lines and recall **prompt history** at the top/bottom edge; **`Alt+Enter`**
inserts a newline for a multi-line query. The chat pane scrolls with **`j`/`k`/`g`/`G`** (focus it with
`Tab`), and **`Ctrl+F`** searches it (`n`/`N` jump between matches). Queries render in the accent
colour, answers in the default — easy to tell apart while scrolling.

---

## 4. Pinning context

The retrieval is automatic, but you can **force** a fact into every query's context. In the Facts tree,
**`Ctrl+P`** pins (or unpins) the cursor node — up to three, marked `⬡`, shown in the status bar. Pinned
text is always prepended (ahead of the semantic results), and pins persist with the thread. Use it to
keep a load-bearing fact — a timeline, a core rule of your magic system — in front of the model for a
whole line of questioning.

---

## 5. Turning research into facts — the confirmation step

Every insertion is a deliberate, reviewed act. Nothing reaches your corpus without you seeing and
approving it.

### `/fact "instruction" [→ path]`

Extracts **one** titled fact from the **last response**:

```
/fact "extract the daily capacity figure"
/fact "the founding date" → Facts/Rome/History
```

A second LLM call distils the response into `{title, fact}` — **in your project's language** (it never
silently drops to English) — and opens the **editable confirmation overlay**:

```
┌ Confirm insertion → Facts ───────────────────────────┐
│ Title: Aqua Claudia Daily Capacity                   │
│ ──────────────────────────────────────────────────── │
│ The Aqua Claudia carried approximately 190,000 m³ of │
│ water per day, the largest aqueduct in Rome.         │
│ → facts/rome/history                                 │
│ [Tab: field]  [Ctrl+S / Ctrl+Enter: confirm]  [Esc]  │
└──────────────────────────────────────────────────────┘
```

`Tab` switches between **Title** and **Body**; edit freely (full arrow/Home/End editing); **`Ctrl+S`**
(or `Ctrl+Enter`) inserts; **`Esc`** discards. An empty body is refused — you'll be told to type it or
cancel, so you never get a title-only stub.

The `→ path` is optional: with it, the fact lands at that slug path; without it, at the Facts-tree
cursor (a branch hosts it as a child; a paragraph as a sibling). Inserted facts are **re-embedded
immediately**, so they're instantly retrievable by `/diff`, the writing-mode RAG, and every Facts
consumer — no rebuild.

**Provenance (1.5.1).** Every inserted fact records where it came from — `model` (with the originating
research query), `manual`, or `promoted` — in `.inkhaven/fact-sources.json`. The confirmation overlay
shows the pending `src:`; **`/sources`** lists every fact with its origin. **Near-duplicate guard:** if
a `/fact` body closely matches an existing fact, the overlay warns once (`⚠ similar to facts/…`); press
`Ctrl+S` again to insert anyway. It only informs — it never blocks.

### `/promote [notes/path] [→ facts/path]`

Turn a speculative **Note** into a verified **Fact**: re-runs the extraction + confirmation over the
note's text into Facts (provenance `promoted`). With no argument it promotes the thread's most recent
`/note`. Non-destructive — the note stays unless you delete it.

### `/note "instruction" [→ path]`

The same flow, but into the **Notes** book and preserving a **speculative** voice ("this *might*
connect…"). Facts are verified ground truth; Notes are your own thinking. Keeping them separate protects
the integrity of the corpus other features depend on.

### `n` — type a fact by hand

In the Facts tree, **`n`** opens a two-step inline entry (title, then `Ctrl+S` body). The research mode
is the single home for all fact entry — whether the source is the model, a `/fact` extraction, or your
own knowledge.

---

## 6. Checking before — and after — you commit

- **`/diff`** — embed the last response and show the **most similar facts already in your corpus**, with
  similarity scores, so you spot a near-duplicate *before* adding one.
- **`/verify`** — pull the specific, checkable claims out of the last response (years, quoted titles,
  quantities, named entities) and have the model self-assess each as **HIGH / MEDIUM / ⚠ LOW**
  confidence. Guidance for what to investigate before you `/fact` it.
- **`/factcheck`** — a post-hoc audit of the **whole Facts corpus** (multiple LLM calls): every fact is
  judged for real-world accuracy (`ACCURATE` / `DUBIOUS` / `INACCURATE`), then the corpus is checked for
  facts that **contradict each other**. The accuracy pass and the consistency pass both **stream live**
  (dim tokens) so you see progress, and the consistency pass is **chunked by Facts chapter** (plus a
  cross-branch pass) instead of one unbounded call — it scales to a large corpus. The report lands in the
  chat in two sections — factual accuracy and mutual consistency — with a `✓N ?N ✗N` tally. Read-only.

### Verdict flags in the Facts tree + `/whatswrong` (1.5.3)

After a `/factcheck`, each fact in the **Facts tree** carries a verdict glyph so problems are visible at a
glance:

- **✓** (green) — accurate · **?** (yellow) — questionable (`DUBIOUS`) · **✗** (red) — inaccurate.

Navigate to a flagged fact (the marks persist in `.inkhaven/fact-verdicts.json` until the next audit),
then run **`/whatswrong`** — it streams an AI explanation of *what* is inaccurate or questionable and what
the correct information is, seeded with the recorded verdict reason and written in the project language.
`/whatswrong` targets the **selected fact** by default, or an explicit `/whatswrong facts/path`. This is
the loop: `/factcheck` flags → the tree shows where → `/whatswrong` explains → you fix it by hand.

### Undisputed (authorial) facts + `/undisputed` (1.5.6)

Some Facts are your **creative invention** — an invented god, a fictional aqueduct's capacity, a rule of
your magic system. They are *not* claims about the real world, so real-world fact-checking is meaningless
(and they'd fail it anyway). Mark such a fact **undisputed** by selecting it in the Facts tree and pressing
**`u`** (toggle) — it gains a magenta **`※`** glyph. (You can also add the tag `fact:undisputed` from the
editor's tag picker.)

- An undisputed fact is **excluded from `/factcheck`** — but the report notes how many were skipped
  (`※ N undisputed fact(s) excluded (authorial)`).
- **`/undisputed`** runs a *separate*, read-only pass over **only** the undisputed facts, with a different
  question: not "is this real?" but "does it make **internal common sense**?" Each is judged
  `PLAUSIBLE / ODD / INCOHERENT` **within its own fictional frame** — self-consistent, free of obvious
  contradiction — in your project language. It **never rewrites** anything; it just reports (with a
  `✓N ?N ✗N` tally). After the pass, the tree **colours each `※` glyph** by its verdict (green plausible ·
  yellow odd · red incoherent). Undisputed facts live *outside* the trust ladder — they're authorial axioms.

### Cost (1.5.3)

The status bar's session cost is now priced from a **per-model table** (`cost.pricing`, USD per million
tokens, input and output separately) against the provider's **real reported token usage** when available
— shown as `$0.012` (exact) or `~$0.012` (the char-length estimate, when a provider reports no usage).
It still only *informs*; `research.session_budget_warn` never blocks.

### Tab-completion (1.5.3)

In the query prompt, **Tab** completes a Facts slug path after `/goto ` or after a `→` insertion arrow
(e.g. `/fact "…" → facts/ro⇥`): one match completes and descends, several splice the common prefix and
list the options in the status bar. Elsewhere, Tab still cycles panes.

---

## 6½. Importing documents (1.5.1)

Ground the assistant on **your own sources**, not just the model's knowledge. **`/import <path>`**
(or `inkhaven research --import <path>`) ingests a **Markdown / text / PDF** file: it chunks the text and
embeds the chunks into the shared vector store as *research sources*. From then on, those chunks are
retrieved alongside your Facts and prepended (cited as `[source: name]`) to ground answers — and a
`/fact` taken from a source-grounded answer records provenance **`document`** with the source name.

- **`/import`** (bare) lists your imported sources; **`/forget <name>`** removes one.
- **`/import <folder>`** imports a **whole directory** (recursively): every `.md` / `.txt` / `.pdf`
  inside becomes a research source — point it at an Obsidian-style vault or a folder of notes and the
  status line reports how many files and chunks landed. Re-importing replaces a same-named source.
- PDFs are text-extracted on import; scanned/image-only PDFs yield little text (no OCR).
- Imported chunks are retrieved whenever RAG is on (any mode except *Full only*).
- **`/import <file.bib>`** / **`<file.json>`** (and `inkhaven research --import`) imports a **BibTeX** or
  **CSL-JSON** file: each entry becomes a **SOURCES-1 citation** under a *Research* chapter of your Sources
  book (deduped by cite key) — your existing bibliography (or a Zotero export), brought straight into the
  corpus (R3-D).
- **`inkhaven research --sync <folder>`** registers a folder for **re-import-on-change**: it imports the
  folder now, and every subsequent `inkhaven research` launch re-imports it **if its files changed**
  (mtime-gated) — so a working vault stays in sync without a manual `/import` each time.

This is the first step of *grounded research*: your corpus can now stand on real documents, with every
derived fact citing where it came from.

## 6¾. Web search & fetch (1.5.2)

Bring **live web sources** in — with the safety posture you choose. Configure a provider in
`research.web` (Tavily with an API key, or your own SearXNG instance), then:

- **`/web <query>`** (default) searches + fetches and **grounds an LLM answer** on the results, cited by
  URL. A **`/fact`** taken from that web-grounded answer is **fact-checked before it commits**: a
  single-fact accuracy check runs in the confirmation overlay — `ACCURATE` inserts, `DUBIOUS` /
  `INACCURATE` shows the verdict and asks you to confirm again (`Ctrl+S`). Provenance is recorded as
  `web` with the source URL(s) and the verdict.
- **`/web --ingest <query>`** instead embeds the fetched pages directly as cited research sources (like
  `/import`, but from the web) — no LLM in the loop.

Set the default with `research.web.pipeline` (`chat` or `ingest`); `--chat` / `--ingest` override it per
call. Web search needs network and a provider; without one, `/web` simply reports it's unavailable.

## 6.8. Structured facts — `/wikidata` (1.5.5)

Where `/web` brings in *prose*, **`/wikidata <query>`** brings in **structured triples** — the top of the
trust ladder. It looks up the best-matching Wikidata entity and shows its **statements** (property →
value), each citable by the entity's **Q-ID**:

```
/wikidata Albert Einstein
  Albert Einstein (Q937) — German-born theoretical physicist (1879–1955)
  • occupation: theoretical physicist
  • date of birth: 1879-03-14
  • field of work: physics
  …
  Source: Wikidata Q937 · https://www.wikidata.org/wiki/Q937
```

- **Keyless** (no API key, no config needed — on by default; tune `research.wikidata`), and it uses your
  **project language** for labels and descriptions.
- A **`/fact`** taken from a `/wikidata` result records provenance **`wikidata`** with the Q-ID, and
  **skips the fact-check gate** — a Q-ID-backed triple is already a verifiable fact, not a model's guess.
- **Wikipedia is deliberately excluded.** Its narrative carries well-documented editorial bias
  (politics / economics / history); Wikidata's per-statement triples don't. We ground on the facts, not
  the story. (External-identifier properties like catalog IDs are filtered out to keep the view factual.)

## 6.85. Scholarly sources — `/openalex` & `/arxiv` (1.5.5)

Bring in **peer-reviewed and preprint literature**, cited by **DOI / arXiv-ID** — the scholarly tier of
the trust ladder. Both are **keyless**:

- **`/openalex <query>`** — the top matching work from [OpenAlex](https://openalex.org) (title, authors,
  year, abstract, DOI). Set `research.scholarly.mailto` to your email to join OpenAlex's *polite pool*
  (recommended — avoids the anonymous rate limit).
- **`/arxiv <query>`** — the top matching [arXiv](https://arxiv.org) preprint (title, authors, year,
  abstract, arXiv-ID).

```
/arxiv attention is all you need
  Attention Is All You Need
  Ashish Vaswani, Noam Shazeer, … · 2017
  arxiv:1706.03762v5
  <abstract…>
  Source: arxiv · http://arxiv.org/abs/1706.03762v5
```

**Auto-citation.** A **`/fact`** taken from an `/openalex` or `/arxiv` result records provenance
**`openalex`** / **`arxiv`** (with the DOI/ID) **and auto-creates a SOURCES-1 `BibEntry`** under a
**Research** chapter in your Sources book — the verified fact and a real bibliography entry land together
(deduped by cite key). The metadata is authoritative, so the fact-check gate is skipped; you still review
the extracted claim in the confirmation overlay as always. Disable with `research.scholarly.auto_cite`.

## 6.9. Triangulation — `/triangulate` (1.5.5)

Once you have **several independent sources**, a claim can be checked against *all* of them — a far
stronger test than the model grading its own output. **`/triangulate <claim>`** (bare → the last
response) queries the **structured sources concurrently** — Wikidata, OpenAlex, arXiv — gathers what each
says, then has the model judge **each source** against the claim:

```
/triangulate the aqueduct carried 1 million cubic metres per day
  Wikidata: SILENT — no capacity statement for this entity
  OpenAlex: SUPPORTS — the cited work gives a comparable daily figure
  arXiv:    SILENT — the preprint is about flow modelling, not capacity
  Agreement: 1/3 support
```

Crucially, the model judges **independent external evidence**, not its own answer — so the verdict means
something. Use it before you `/fact` a shaky claim: if the sources don't corroborate, investigate first.
(Triangulation needs at least Wikidata or the scholarly sources enabled.)

**As the `/fact` gate.** Set **`research.triangulate_gate: true`** and triangulation runs *automatically*
before a `model` / `web` / `document` fact commits — cross-source agreement **replaces** the single-source
self-check. On the confirmation, `SUPPORTS` with no `CONTRADICTS` inserts; a contradicted or uncorroborated
verdict shows the agreement and asks you to `Ctrl+S` again to insert anyway (like the dedup guard —
informs, never blocks). Deterministic / structured facts (`computed`, `wikidata`, `openalex`, `arxiv`)
are already authoritative and skip the gate. It's **off by default** (network-heavy).

## 6⅞. Deterministic calculation — `/calc` (1.5.2)

Not every fact comes from a model or the web — some you just **compute**. **`/calc <expr>`** evaluates a
deterministic expression in the in-tree Bund VM, with a prelude of **physical constants** and
**unit-conversion words**. The computation *is* its own proof: a `/fact` taken from a `/calc` result
records provenance **`computed`** and **skips the fact-check gate** entirely — nothing can be fabricated.

```
/calc 100 mi2km        → = 160.9344
/calc 4.2 ly2km        → = 3.97e13
/calc 20 c2f           → = 68
/calc calc.pi          → = 3.14159265358979
```

- **Conversions** (pop a number, push the result): `mi2km` `km2mi` `m2ft` `ft2m` `m2mi` `mi2m`
  `c2f` `f2c` `c2k` `k2c` `kg2lb` `lb2kg` `kmh2mph` `mph2kmh` `au2km` `km2au` `ly2km` `km2ly`
  `ly2au` `au2ly` `pc2ly` `ly2pc` `deg2rad` `rad2deg`.
- **Constants** (push a value, SI base units): `calc.pi` `calc.tau` `calc.e` `calc.c` (speed of light)
  `calc.grav` (G) `calc.gee` (g₀) `calc.au` `calc.ly` `calc.pc` `calc.year` `calc.day` `calc.hour`
  `calc.minute`.
- Because it's Bund, the full stack language is available too: `/calc 3 4 + 5 *` → `= 35`. Each word
  also has a `calc.`-prefixed form if a short alias collides with a builtin.

### Scientific & astronomy words (1.5.4)

`/calc` also carries a **scientific substrate** (`sqrt cbrt pow exp ln log10 log2 sin cos tan asin acos
atan atan2 hypot abs floor ceil round`) and **astronomy/planetology formulas** in convenient units:

| Word | Stack | Meaning |
|---|---|---|
| `kepler_period` | `( a M -- T )` | orbital period; a [AU], M [M☉] → T [yr] (`√(a³/M)`) |
| `surface_gravity` | `( M R -- g )` | M [M⊕], R [R⊕] → g [m/s²] |
| `escape_velocity` | `( M R -- v )` | M [M⊕], R [R⊕] → v [m/s] |
| `insolation` | `( L d -- S )` | L [L☉], d [AU] → flux [W/m²] (inverse-square) |
| `synodic_period` | `( T1 T2 -- Tsyn )` | `1/Tsyn = |1/T1 − 1/T2|` |
| `angular_size` `hill_sphere` `roche_limit` `tidal_accel` | … | (see the [RFC](../PROPOSALS/RESRCH-4_RFC.md) formula table) |

```
/calc 4 1 kepler_period      → = 8         (a=4 AU, M=1 M☉ → 8 years)
/calc 1 1 surface_gravity    → = 9.80665   (Earth)
```

**Climate, geography & economy (1.5.6).** `lapse_rate` (altitude→ΔT), `dewpoint`, `insolation_at_lat`,
`heat_index`; **`haversine`** (two lat/lon → great-circle km), `bearing`, `destination_point` (pushes
`lat2 lon2`), `slope`; and finance/population: `compound`, `cagr`, `inflation_adjust`, `annuity`,
`malthus` (exponential) / `logistic` (carrying-capacity). Plus math `trunc sign factorial gcd lcm`.

```
/calc 51.5 -0.13 40.71 -74.0 haversine   → = 5570.2   (London → New York, km)
/calc 1000 0.05 1 10 compound            → = 1628.89  (1000 at 5%/yr for 10 yr)
/calc 5000 100000 0.03 50 logistic       → projected population toward K=100000
```

### Grounding `/calc` in your World book (1.5.4)

`/calc` can read **this project's own World-book facts** (the materialized WORLD-4 simulation), so a
calculation is grounded in *your* world, not generic constants:

```
/calc "Astronomy/year_length_planet_days" world.get   → world: Astronomy/year_length_planet_days = 412.3
                                                         = 412.3
/calc world.year                                       → the same, via a convenience word
```

- **`world.get <path>`** — the value at a `Chapter/field[/index|key…]` path in the World book (e.g.
  `"Climate/zones/0/annual_mean_c"`); pushes a number, or `NODATA` when the path doesn't resolve (it
  never invents a value). **`world.has <path>`** tests a path; **`world.dict <Chapter>`** pushes a whole
  layer as a dict.
- **Convenience words** for the common astronomy figures: `world.year`, `world.declared_year`,
  `world.tilt`, `world.star_mass`, `world.orbit_days`, `world.divergence`.
- Every World read is **echoed** in the result (`world: <path> = <value>`), and a `/fact` taken from a
  World-grounded `/calc` records provenance **`computed · world:<path>`** — the deterministic, *self-citing*
  tier. It reads the **materialized** book, or (if you haven't run `realworld compile --materialize` yet)
  **recomputes** the layer from `world.hjson` on the fly — same numbers either way.
- **List reducers:** `[ 1 2 3 4 ] sum` → `10`; also `mean`, `min`, `max` over a Bund list literal.

### Browsing the world — `/world` (1.5.6)

**`/world`** lists your World simulation's layers (Astronomy · Geology · Climate · Hydrology ·
Demographics); **`/world <layer>`** shows that layer's facts (the materialized JSON, or recomputed from
`world.hjson`). A `/fact` from a `/world` result records provenance **`simulation`** and **skips the
gate** — it's your own deterministic, internally-consistent world, not an outside claim.

## 6.95. Producing output — `/synthesize` & `/bibliography` (1.5.8)

The corpus is finally a source you can *compose from*, not just add to.

- **`/synthesize <topic>`** retrieves the facts related to a topic and streams a **grounded synthesis**
  that uses *only* your verified facts, **cites each by its breadcrumb**, and flags where the corpus is
  thin — a mini overview built from your own knowledge base, in your project language. It never invents
  facts; `y` copies it, or `/fact` a distilled point. (RESRCH-5 / R5-A.)
- **`/outline <topic>`** produces a **structured, fact-citing outline** — nested points, each citing its
  supporting fact by `[breadcrumb]`, with anything the corpus doesn't cover marked `(needs research)`.
  The research → writing bridge: copy it into the manuscript. (R5-B.)
- **`/gaps <topic>`** lists the **open questions** the corpus *doesn't* answer — concrete things to go
  find — which you can drop into a file to seed **`--batch`**, closing the loop research → gaps → batch →
  facts. (R5-C.)
- **`/bibliography`** emits the **Sources book's Research chapter** — every citation auto-filed by
  `/openalex`/`/arxiv` and `/import <.bib>` — as **BibTeX** (`y` to copy). Headless:
  **`inkhaven research --bibliography [--out refs.bib]`** writes it to a file. The citations you accrued
  become a real manuscript bibliography. (R5-D.)

## 6.97. Keeping the corpus healthy — `/upgrade` & `/stale` (1.5.8)

A knowledge base decays; these tend it.

- **`/upgrade [facts/path]`** (bare → the selected fact) tries to **re-ground a `model`-origin fact on a
  structured source** — it gathers evidence from Wikidata / OpenAlex / arXiv about the claim and asks the
  model whether any source **corroborates** it. If one does (and none contradicts), the fact's
  **provenance tier is raised** to that source (its tree tier glyph updates) — a speculative claim becomes
  a cited one. **The fact text is never changed**; only the provenance. Already-structured facts are
  skipped. (RESRCH-5 / R5-E.)
- **`/stale [days]`** lists your **`model` / `web`** facts older than *N* days (default 90) — the ones
  whose grounding may have drifted — so you can re-verify or `/upgrade` them. Read-only. (R5-F.)

## 7. Deeper research

- **`/chain q1 → q2 → q3`** — a sequential pipeline: each step's answer becomes context for the next.
  Good for "what were the challenges → what solutions emerged → which are archaeologically verified".
- **`/goto facts/path/slug`** — jump the Facts tree to a node (and focus it). Handy before a `/fact`
  when you know exactly where it should land.
- **`/rag [facts+full|facts|full]`**, **`/clear`** (empty the chat), **`/save [name]`** (rename the
  thread) round out the namespace.

---

## 8. Organising the corpus — tree editing

The Facts tree is a first-class editor, on par with the main Tree pane (and the Outline's copy/move):

| Key | Action |
|:---|:---|
| `j`/`k` · `g`/`G` | Navigate · top/bottom |
| `h`/`l`/`Enter` | Collapse / expand / step |
| `R` | Rename the cursor node |
| `c` / `s` | New **chapter** (under the Facts book) / **subchapter** (under the enclosing chapter) |
| `-` / `D` | Delete a **paragraph** / **branch** (with a `y`·N confirmation) |
| `K` / `J` | Move the node up / down among its siblings |
| `y` / `x` / `p` | **Copy** / **cut** / **paste** a node across parents |
| `Ctrl+P` | Pin / unpin for RAG context |
| `n` | New fact (manual entry) |

Use chapters and subchapters to group facts by domain (Geography, History, Magic-System) so retrieval
and navigation stay sharp as the corpus grows.

---

## 9. Threads — persistent, resumable sessions

Every session is a **named thread** stored at `.inkhaven/research-threads/<slug>.json`: its queries,
fact/note insertions, pinned nodes, and RAG mode all persist and **resume** exactly where you left off.
`↑`/`↓` in the prompt recall your history (queries *and* the `/fact` commands you ran).

- **Launch rules:** 0 threads → a `default` thread; 1 → opened directly; 2+ → a **picker** (`Enter`
  open, `n` new, `d` delete, `Esc` quit).
- **`inkhaven research --list-threads`** prints every thread (name · last-active · turns · cost), and
  **`--export-thread <name>`** writes its full history as Markdown or JSON (`--out FILE`, default
  stdout) — a clean record of how a fact came to be.

## 9½. UI niceties (1.5.6)

- **Command completion + live hints.** Type `/` and press **Tab** to complete a command name (one match
  completes, several list in the status bar); the **hints bar** tracks what you're typing — matching
  commands while you type the word, then the recognised command's usage (`/web [--ingest|--chat]
  <query>`). Tab still completes Facts paths after `/goto ` or a `→` arrow.
- **Trust made visible.** Each chat turn shows a **source-tier badge** — `[computed]` `[◆ Q937]`
  `[§ arxiv]` `[⚠ web]` `[document]` `[? model]` — and each fact in the **Facts tree** carries a
  permanent **tier glyph** by its recorded provenance (≡ deterministic · ◆ structured · § scholarly ·
  ◇ web · ▪ imported · · model), so the trust ladder is legible at a glance.
- **Async liveness.** A braille **spinner + elapsed timer** appears in the status bar whenever a fetch,
  stream, or gate is in flight (`⠙ (3s) Querying Wikidata…`).
- **Quick-view a fact.** In the Facts tree, **`Enter`** on a fact opens a **scrollable modal** with its
  full body (`j/k`/arrows scroll · `Y` copy · `Esc` close); `Enter` on a branch folds/expands it.
- **Layout control.** **`<` / `>`** (in the tree or chat) resize the split; **`Ctrl+Z`** zooms the
  focused pane full-screen (toggle). *(macOS note: `Ctrl+←/→` is Mission Control, so resize uses `<`/`>`.)*
- **Copy to clipboard.** **`y`** in the chat copies the last response; **`Y`** in the tree copies the
  selected fact's body. A dim **`▼ more`** marks the chat when you're scrolled up, and a rule separates
  turns.
- **Light markdown** in responses — **bold**, *italic*, `inline code`, headings, bullets and links are
  styled inline, so structured answers (Wikidata triples, outlines) read more clearly.
- **A richer confirmation.** When the triangulation gate or fact-check runs, the confirmation overlay
  shows the **per-source verdict** (green `SUPPORTS` / red `CONTRADICTS` / dim `SILENT`, with the
  `Agreement: n/m` tally) instead of a status flash; the near-duplicate guard shows the **similar fact's
  text** beside the pending one; and the location row previews the exact **`will record: <tier> ·
  <citation>`** provenance, with Title/Body char counts.

### Headless batch research (1.5.6)

**`inkhaven research --batch questions.txt`** researches a **question list** non-interactively (one
question per line; `#` comments ignored). For each question it grounds an answer on your Facts corpus,
distils one candidate fact, and scores the model's confidence — then writes a **Markdown report**
(`--out FILE`, default stdout). By default it only *proposes* (the interactive rule — confirm every
insertion — still holds). Add **`--auto-confirm`** (and optionally **`--confidence 0.8`**, default 0.7)
to **insert** the facts that clear the bar, each with `model` provenance. Good for seeding a corpus from a
research outline — the relaxation happens *only* behind the explicit flag + threshold.

---

## 10. The Inner family already knows your facts

Because inserted facts live in the real Facts book and the shared vector index, they immediately enrich
everything else: the **Book-scope** RAG chat (`Ctrl+B J`), the **Inner Socrates "Facts"** scope, the
**world-coherence** checks, and the **Mythology** library all read the same corpus you just grew. No
coupling, no rebuild — that's the whole point.

---

## 11. Configuration

The optional `research:` block (see [CONFIGURATION.md](../CONFIGURATION.md)) tunes everything; omit it
for the defaults:

```hjson
research: {
  default_thread: null          // null = picker / `default`
  rag_top_n: 5                  // max Facts prepended per query
  session_budget_warn: 0.50     // cost note (informs, never blocks)
  max_pinned_nodes: 3
  show_keybind_hints: true
  min_width: 80
  split_ratio: 4                // 4 = 40% tree, 60% chat
  diff_top_n: 3
  verify_min_sentence_words: 8
  triangulate_gate: false        // R3-E: cross-source-agreement /fact gate (network-heavy)
  wikidata: { enabled: true }    // R3-A (keyless)
  scholarly: { enabled: true, mailto: "you@example.org", auto_cite: true }  // R3-B
}
```

Colours follow your `theme:` block. Cost is priced from the **`cost.pricing`** table (per-model USD per
million tokens, input/output separately) against the provider's real token usage when reported — `$`
exact, `~$` estimated. It still only informs, never blocks.

```hjson
cost: {
  pricing: {
    "gemini-2.5-pro": { input_per_1m: 1.25, output_per_1m: 10.0 }
    "claude-sonnet":  { input_per_1m: 3.0,  output_per_1m: 15.0 }
    // … keyed by a model-name substring; longest match wins
  }
  default_input_per_1m: 3.0       // fallback for unlisted models
  default_output_per_1m: 3.0
}
```

---

## 12. What it is not (yet)

Web search and document/PDF import are **RESRCH-2**. Today the assistant works from the configured
model's knowledge and your existing corpus — and turns them into a verified one, in your language, that
you control entirely. Every insertion is yours; nothing is written without your confirmation; nothing
edits your prose.

> **The minute loop:** ask → `/diff` to check for duplicates → `/verify` the shaky claims → `/fact` the
> keepers (review, edit, confirm) → `/factcheck` the corpus now and then. That's the whole discipline.
