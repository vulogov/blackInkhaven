#import "../design.typ": *

#appendix(letter: "C", title: "The Configuration Reference")

Every Inkhaven project carries one configuration file, `inkhaven.hjson`,
written verbatim by `inkhaven init` and read once at startup. It is
#link("https://hjson.github.io/")[HJSON] — strict JSON with comments,
unquoted keys, optional commas, and multiline strings — so every example
below pastes straight in. This appendix is the field-by-field reference
Chapter 29 points to: one block per subsection, each field with its type,
its compiled-in default, and a one-line meaning. Omit any block and you
get exactly these defaults, because every field is `#[serde(default)]`.

#callout(label: "How the layered load works")[
Precedence, lowest to highest: (1) compiled-in defaults, (2) the project's
`inkhaven.hjson`, (3) `~/.config/inkhaven/config.hjson`, (4) every
`~/.config/inkhaven/conf/*.hjson` in sorted filename order. A global file
*wins over the project* and may be *partial* — put in only the keys you
want to change. A malformed global file is skipped with a warning; only a
malformed project file is fatal. Unknown fields are ignored. See Chapter 29
for the narrative treatment.
]

#callout(label: "Defaults are from the code")[
Every default here is the value in `src/config.rs` `impl Default`, which is
authoritative where it and the prose docs disagree. Four such corrections
are flagged in place: `sync_interval_seconds` (600, not 60), `backup.out_dir`
(empty, not `backups`), `book_rag.exclude_system_books` (eight entries), and
`utopia.stage2_max_pairs` (present in code only).
]

#section("Top-level fields")

The keys that sit at the root of the file, beside the block tables.

#chord_table((
  chord_row("language", "string · english — primary writing language; drives Snowball stemmers + the F7 grammar prompt. Empty falls back to editor.stemming.languages"),
  chord_row("genre", "string · (none) — declared genre (literary_realism, fantasy, scifi, mystery, memoir, nonfiction, technical, academic, philosophy, theology, utopian, poetry, …); feeds genre-aware prompting"),
  chord_row("prompts_file", "path · prompts.hjson — the prompt library, resolved against the project root"),
  chord_row("inner_socrates_default_persona", "string? · null — the project's default reader persona until one is explicitly activated"),
  chord_row("artefacts_directory", "string · empty — where per-book build output lands; empty resolves to the OS cache dir, keeping artefacts out of the project tree"),
  chord_row("sync_interval_seconds", "u64 · 600 — seconds between background Store::sync() flushes of the HNSW index + DuckDB checkpoint; 0 disables the timer. (Code says 600; older prose said 60.)"),
))

#section("Everyday blocks")

#subsection("embeddings")

How paragraph bodies become vectors for semantic search (fastembed).

#chord_table((
  chord_row("model", "string · MultilingualE5Small — fastembed model; pick an E5 for any non-English writing"),
  chord_row("chunk_size", "int · 800 — approx characters per embedded chunk"),
  chord_row("chunk_overlap", "float · 0.15 — overlap fraction between adjacent chunks"),
  chord_row("pool_size", "int · 4 — r2d2 pool size per backing DuckDB file; clamped to a floor of 2 at open"),
))

Model names: `MultilingualE5Small` (default, 384-dim), `MultilingualE5Base`,
`MultilingualE5Large`, `BGEM3`, and the English-only `BGESmallENV15` /
`BGEBaseENV15` / `BGELargeENV15`. Switching models triggers a one-time
download; run `inkhaven reindex` afterward.

#subsection("llm")

AI providers and the default. The shipped defaults define five providers;
`default` picks one, `auto_fallback` lets a failed call retry a sibling.

#screen(caption: "llm — shipped defaults")[
```hjson
llm: {
  default: gemini
  auto_fallback: true
  providers: {
    gemini:   { model: gemini-2.5-pro,   api_key_env: GEMINI_API_KEY }
    claude:   { model: claude-sonnet-4-5, api_key_env: ANTHROPIC_API_KEY }
    openai:   { model: gpt-4o,            api_key_env: OPENAI_API_KEY }
    deepseek: { model: deepseek-chat,     api_key_env: DEEPSEEK_API_KEY }
    grok:     { model: grok-2-latest,     api_key_env: XAI_API_KEY }
  }
}
```
]

#chord_table((
  chord_row("default", "string · gemini — which provider is used with no --provider flag"),
  chord_row("auto_fallback", "bool · true — retry a failed call on another configured provider"),
  chord_row("providers.<name>.model", "string · varies — model id passed to genai (it picks the adapter)"),
  chord_row("providers.<name>.api_key_env", "string? · varies — env var holding the key; omit entirely for local providers like Ollama"),
))

If an `api_key_env` is set but unset at runtime, Inkhaven refuses the call
with a clean status message rather than crashing.

#subsection("editor — core")

Editor-pane behaviour. The visual look lives in `theme`.

#chord_table((
  chord_row("theme", "string · default — reserved; the visual theme is the top-level theme block"),
  chord_row("tab_width", "int · 2 — informational; tui-textarea inserts a literal tab"),
  chord_row("wrap", "bool · true — soft word-wrap; false gives horizontal scroll"),
  chord_row("autosave_seconds", "int · 5 — idle seconds before a dirty paragraph auto-saves; 0 disables idle autosave"),
  chord_row("startup_splash", "bool · true — 7-second launch splash with today's stats"),
  chord_row("mouse_captured", "bool · true — initial mouse-capture state; Ctrl+Shift+M toggles"),
  chord_row("confirm_quit", "bool · false — confirm modal on Ctrl+Q"),
  chord_row("pov_chip_enabled", "bool · true — status-bar POV/character chip"),
  chord_row("show_glossary_chip", "bool · true — the C·P·A worldbuilding-density chip"),
  chord_row("show_facts_chip", "bool · false — the ⚑N Facts-count chip"),
  chord_row("startup_haiku", "bool · true — emit a baked-in haiku at startup / new paragraph / Ctrl+Z p"),
  chord_row("continuation_anchor_count", "int · 3 — previous paragraphs sent as voice anchors for Ctrl+V d"),
  chord_row("footnote_style", "string · typst — typst (#footnote[…]) or markdown ([^id]) for Ctrl+V f"),
  chord_row("stemming.languages", "list · [english, russian] — legacy; superseded by top-level language when non-empty"),
))

#subsection("editor — prompt language")

Which language each AI prompt resolves against.

#chord_table((
  chord_row("prompt_language_mode", "string · book_defined — book_defined uses the language field; paragraph_detected runs whatlang on the live paragraph"),
  chord_row("prompt_language_detection_min_chars", "int · 50 — minimum characters before whatlang detection is attempted"),
))

#subsection("editor — durability")

Crash-safety and behaviour knobs (1.3.37).

#chord_table((
  chord_row("crash_mirror_seconds", "int · 2 — cadence of crash-rescue buffer mirrors"),
  chord_row("deleted_paragraph_history", "int · 10 — kill-ring depth for Ctrl+V Shift+U undelete"),
  chord_row("external_change_auto_reload", "bool · true — false warns instead of silently reloading a clean buffer changed on disk"),
  chord_row("fact_check_idle_seconds", "int · 5 — idle delay before the auto fact-check fires"),
  chord_row("visited_history_cap", "int · 0 — 0 is unbounded; caps the back/forward visit list"),
  chord_row("disk_warn_mb", "u64 · 100 — low-disk startup warning threshold (MiB); 0 disables"),
  chord_row("warn_uncommitted_on_exit", "bool · true — confirm on quit if the git repo has uncommitted changes"),
))

#subsection("editor — reading & echo")

Reading-pace chips (1.2.18) and the echo-repetition scan (1.2.19+).

#chord_table((
  chord_row("reading_time_chip", "bool · false — the 📖 remaining/total reading-length chip"),
  chord_row("reading_wpm", "u32 · 200 — words-per-minute for reading-time / pace / audiobook timing"),
  chord_row("paragraph_long_secs", "u32 · 180 — read-time threshold flagging a paragraph-too-long finding"),
  chord_row("echo_window", "usize · 5 — consecutive-paragraph window for the echo scan"),
  chord_row("echo_min_repeats", "usize · 3 — occurrences within the window required to flag"),
  chord_row("echo_max_global", "usize · 40 — distinctiveness ceiling; words used more often are common vocabulary"),
  chord_row("echo_overlay", "bool · false — default state of the live echo overlay (Ctrl+B Shift+K)"),
))

#subsection("editor.tts — read-aloud")

Text-to-speech (1.2.9 System backend + 1.2.17 Piper). Master switch off.

#chord_table((
  chord_row("enabled", "bool · false — master switch for Ctrl+B S read-aloud"),
  chord_row("voice", "string · Milena — case-insensitive voice-name fragment (prefers Enhanced/Premium)"),
  chord_row("speed", "f32 · 1.0 — rate multiplier over the engine's normal rate"),
  chord_row("greeting", "string · empty — spoken at startup; empty skips"),
  chord_row("goodbye", "string · empty — spoken at shutdown (blocks up to 5s); empty skips"),
  chord_row("engine", "string · auto — auto | piper | system backend selector"),
  chord_row("voices_dir", "string · .inkhaven/voices — Piper voice cache (sandboxed to project root)"),
  chord_row("auto_download", "bool · true — stream a missing Piper voice on first use"),
  chord_row("catalog_url", "string · piper-voices voices.json — Piper voice catalog URL"),
  chord_row("catalog_ttl_hours", "u32 · 24 — catalog cache freshness window"),
  chord_row("binary_path", "string? · null — explicit piper binary; null autoresolves via PATH then cache"),
  chord_row("auto_download_binary", "bool · true — the tts binary download CLI fetches Piper; the TUI never auto-fetches"),
  chord_row("cache_max_voices", "usize · 5 — LRU cap on the voices dir (models are 25–100 MB)"),
  chord_row("play_command", "string? · null — playback command override; {path} is the WAV"),
  chord_row("sample_rate_hz", "u32 · 22050 — Piper synthesis sample rate"),
  chord_row("auto_gitignore", "bool · true — append the voices dir to .gitignore on first download"),
))

#subsection("editor.style_warnings — inline overlays")

The amber inline overlays flagging weak prose (1.2.9+). Master switch off;
`Ctrl+B Shift+F` toggles per session.

#chord_table((
  chord_row("enabled", "bool · false — master switch for all inline style overlays"),
  chord_row("filter_words.enabled", "bool · true (if master on) — intensifier/hedge/sensory-verb detector"),
  chord_row("filter_words.use_stemming", "bool · true — match via Snowball stems so list entries are lemmas"),
  chord_row("filter_words.<lang>", "array · [] — per-language list; empty uses the built-in, non-empty REPLACES it"),
  chord_row("filter_words.extra_words", "array · [] — words added on top of the language default"),
  chord_row("repeated_phrases.enabled", "bool · true (if master on) — flags n-grams repeating within a paragraph"),
  chord_row("repeated_phrases.n", "int · 4 — window size (consecutive non-stop words)"),
  chord_row("repeated_phrases.threshold", "int · 3 — minimum occurrences before flagging"),
  chord_row("repeated_phrases.use_stemming", "bool · true — stem before n-gram comparison"),
  chord_row("repeated_phrases.<lang>_stop_words", "array · [] — closed-class words excluded from comparison; empty uses the built-in"),
  chord_row("show_dont_tell.enabled", "bool · true (if master on) — flags telling patterns (was angry, angrily, realised)"),
  chord_row("show_dont_tell.use_stemming", "bool · true — stem entries so inflections collapse"),
  chord_row("show_dont_tell.<lang>_linking_verbs / _emotion_adjectives / _manner_adverbs / _cognition_verbs", "array · [] — the four category lists; empty uses the built-in, non-empty REPLACES"),
  chord_row("anachronism.year", "int · (unset) — the setting year; the detector is OFF until this is set"),
  chord_row("anachronism.terms", "array · [] — additive term/earliest pairs on top of the ~35-term built-in lexicon"),
))

Any language can be curated through per-language *maps* under
`filter_words.languages.<lang>`, `show_dont_tell.languages.<lang>`, and
`repeated_phrases.languages.<lang>` (1.3.13); an uncurated language gets
empty built-ins, never the English lists. `inkhaven lang bootstrap <lang>`
populates them via the LLM.

#subsection("theme")

Every colour Inkhaven paints, as `#RRGGBB` (or `#RGB`) strings; empty or
unparseable falls back to the baked-in default. The shipped palette is
Catppuccin Mocha.

#screen(caption: "theme — pane, border, modal, lexicon")[
```hjson
theme: {
  pane_bg: "#1e1e2e",  pane_fg: "#cdd6f4"
  line_number_fg: "#6c7086",  current_line_bg: "#313244"
  border_focused: "#cba6f7",  border_unfocused: "#45475a"
  border_dirty: "#f9e2af",    border_saved: "#a6e3a1"
  border_readonly: "#94e2d5"
  modal_bg: "#181825", modal_fg: "#cdd6f4", modal_border: "#cba6f7"
  places_fg: "#89dceb",  characters_fg: "#f9e2af"
  language_word_fg: "#b4a8e1"   // italic, per foreign-inclusion convention
  search_match_bg: "#f38ba8",  search_current_bg: "#f5c2e7"
}
```
]

#screen(caption: "theme — tree, chips, overlays, syntax")[
```hjson
theme: {
  tree_open_marker: "#a6e3a1",  tree_book_fg: "#f5c2e7"
  tree_chapter_fg: "#89b4fa",   tree_subchapter_fg: "#94e2d5"
  tree_paragraph_fg: "#cdd6f4", editor_position_fg: "#89dceb"
  ai_scope_fg: "#fab387",  ai_infer_fg: "#94e2d5"
  grammar_change_fg: "#f38ba8"
  style_warning_anachronism_fg: "#eba672"
  style_warning_echo_fg: "#b48ead"
  style_warning_banned_synonym_fg: "#e05a5a"
  pov_chip_bg: "#8b1d88", pov_chip_fg: "#ffffff"
  syntax_heading: "#cba6f7", syntax_bold: "#f9e2af"
  syntax_italic: "#94e2d5", syntax_string: "#a6e3a1"
  syntax_number: "#fab387", syntax_comment: "#6c7086"
}
```
]

The per-detector `style_warning_*_modifier` keys (filter_word, repeated_phrase,
show_dont_tell, echo) default to the empty string, which maps to `underline`;
accept `bold`, `dim`, `reversed`, `italic`, `none`, or `+`-combined forms like
`underline+bold` for terminals where the default reads faint. The thirteen
`syntax_*` keys drive the tree-sitter Typst highlighter.

#subsection("keys")

The configurable global chords; everything else is hard-coded.

#chord_table((
  chord_row("save", "chord · Ctrl+s — save the current paragraph"),
  chord_row("search", "chord · Ctrl+/ — focus the top Search bar"),
  chord_row("ai_prompt", "chord · Ctrl+i — focus the AI prompt bar"),
  chord_row("next_pane", "chord · Tab — cycle focus forward"),
  chord_row("prev_pane", "chord · Shift+Tab — cycle focus back"),
  chord_row("page_up", "chord · PageUp — page up"),
  chord_row("page_down", "chord · PageDown — page down"),
  chord_row("meta_prefix", "chord · Ctrl+b — the Meta prefix (set Ctrl+g if tmux eats Ctrl+B)"),
  chord_row("bund_prefix", "chord · Ctrl+z — the Bund prefix"),
  chord_row("view_prefix", "chord · Ctrl+v — the View prefix"),
  chord_row("bindings", "array · [] — user overlay rebinding sub-chords (layer: meta_sub | bund_sub | view_sub | top_level)"),
))

#subsection("backup")

The `inkhaven backup` CLI and the auto-backup-on-exit hook.

#chord_table((
  chord_row("out_dir", "string · empty — snapshot destination; empty resolves to the OS data dir (not the literal backups/ the prose once implied). Empty-string never means disabled here — use auto_backup_on_exit"),
  chord_row("max_age", "humantime · 7d — max age of the last backup before the exit hook makes a fresh one; 0s disables the auto hook"),
  chord_row("wait_for_key_after_backup", "bool · true — hold the splash with a Press any key prompt"),
  chord_row("amber_threshold", "f32 · 0.5 — fraction of max_age at which the health chip turns amber; 0 disables the amber tier"),
  chord_row("auto_backup_on_exit", "bool · true — the clear toggle for the exit-hook backup"),
  chord_row("keep_last", "usize · 0 — 0 keeps all; else prune the oldest zips beyond this count"),
))

#subsection("goals & project")

Writing-progress goals feed the status bar and the Ctrl+V G / Ctrl+V Shift+G
modals. All optional; zero/empty disables a goal but still records events.

#chord_table((
  chord_row("goals.day_boundary", "enum · utc — utc | local; when the writing day rolls over for streaks + caps"),
  chord_row("goals.daily_words", "int · 0 — project-wide daily word target"),
  chord_row("goals.active_minutes_daily", "int · 0 — daily active-time target (save-to-save gaps capped at 5 min)"),
  chord_row("goals.streak_grace_per_week", "int · 0 — missed days forgiven per rolling 7-day window"),
  chord_row("goals.books.<slug>.target_words", "int · 0 — per-book total; 0 hides the pace line"),
  chord_row("goals.books.<slug>.deadline", "str · empty — YYYY-MM-DD target date"),
  chord_row("goals.status_ladder.<status>", "int · {} — trailing-7-day promotion targets keyed by lowercased status"),
  chord_row("goals.auto_promote_on_target", "bool · true — advance a paragraph's status when a save crosses its target_words"),
  chord_row("project.word_count_goal", "u64 · 0 — total across counted_books; 0 disables the goal display"),
  chord_row("project.target_date", "string · empty — ISO date for the projection verdict"),
  chord_row("project.counted_books", "array · [] — books that count (by title); empty means every user book"),
))

#subsection("ai")

AI-pane behaviour not tied to a provider (1.2.6+).

#chord_table((
  chord_row("per_paragraph_memory", "bool · false — record Paragraph-scope turns onto the paragraph's ai_memory"),
  chord_row("per_paragraph_memory_max_turns", "usize · 10 — cap on turns kept per paragraph; 0 disables"),
  chord_row("diff_review_on_apply", "bool · true — route r/g rewrites through a side-by-side diff modal first"),
  chord_row("reseed_prompt_examples", "bool · true — re-seed the Prompts book with the embedded .example prompts (idempotent)"),
))

#section("Retrieval & the semantic net")

#subsection("book_rag")

The AI pane's always-on Book-scope retrieval (BOOK_RAG-1). Tunes what a
Book-scope prompt grounds on.

#chord_table((
  chord_row("top_k", "int · 5 — direct semantic hits retrieved per query"),
  chord_row("context_expansion", "int · 1 — sibling paragraphs kept on each side of a hit"),
  chord_row("max_context_tokens", "int · 8000 — budget for the composed context (chars ÷ 4)"),
  chord_row("include_system_books", "list · notes, research, places, characters, artefacts, world, language — author-content books joined to the manuscript pool"),
  chord_row("exclude_system_books", "list · scripts, prompts, typst, help, intent, sources, glossary, snippets — meta books never searched (code default; the prose docs list only the first five)"),
))

#subsection("graph")

GRAPHMIND — the Graph AI scope + `graph ask` walk. Bounds the cost of a walk;
retrieval width is shared with `book_rag`.

#chord_table((
  chord_row("ask_max_steps", "int · 8 — max LLM turns a graph walk takes before a forced synthesis"),
  chord_row("ask_search_width", "int · 6 — seed nodes each search step returns"),
))

#subsection("continuity")

SENTINEL — the unified, zero-AI continuity ledger. On by default and free.

#chord_table((
  chord_row("enabled", "bool · true — master switch for the review-pass ledger"),
  chord_row("ambient", "bool · false — re-check the edit's scope on every save"),
  chord_row("ambient_cooldown_secs", "int · 30 — throttle floor between ambient re-checks"),
  chord_row("co_location", "bool · true — a character in two places at overlapping times"),
  chord_row("timeline", "bool · true — orphaned events / fuzzy overlaps (standalone check)"),
  chord_row("numeric", "bool · true — direction reversal / conflicting durations (EN/FR/ES)"),
  chord_row("char_facts", "bool · true — an established fact changed across chapters"),
  chord_row("introduce", "bool · true — an entity referenced before it is introduced"),
  chord_row("introduce_tolerance", "int · 0 — earlier-chapter references tolerated before introduce flags"),
))

#subsection("lector")

LECTOR — the read-through. Deterministic core on by default.

#chord_table((
  chord_row("enabled", "bool · true — master switch for the read-through review line"),
  chord_row("framework", "string? · null — structure framework the shape is read against (three_act, save_the_cat, story_circle, hero_journey, seven_point, kishotenketsu); null suggests one from genre"),
))

#subsection("prose")

NARR-1 — deterministic, zero-AI narrative-voice profiling per chapter.

#chord_table((
  chord_row("deep_metrics", "bool · false — include Tier-2 (sensory balance + active/passive ratio)"),
  chord_row("mattr_window", "int · 100 — MATTR sliding-window size (tokens)"),
  chord_row("baseline_chapter", "int · 1 — drift is measured against this chapter"),
  chord_row("language", "string? · null — override (en/ru/de/fr/es); null uses project language then EN"),
  chord_row("ambient", "bool · false — Ctrl+V Shift+V default; re-run after an editing pause"),
  chord_row("ambient_cooldown_secs", "int · 90 — floor between ambient runs"),
  chord_row("extra_modal_tokens", "array · [] — appended to the language's modal/hedging list"),
  chord_row("extra_interiority_phrases", "array · [] — appended to the language's FID phrase list"),
))

Its `thresholds` sub-block sets the per-metric crossing that raises a finding:
`sent_len_cv` 0.15, `burstiness_b` 0.15, `mattr` 0.05, `modal_density` 0.020,
`interiority_ratio` 0.10, `de_erlebte_rede_particle_density` 0.05 (DE only),
`sensory_channel_max` 0.15, `active_passive_ratio` 1.5.

#subsection("chorus & stylist")

CHORUS — voice and style at book scale, plus the Inner Stylist coach. All
advisory.

#chord_table((
  chord_row("chorus.distinct_threshold", "float · 0.5 — RMS z-distance below which two voices are flagged indistinguishable"),
  chord_row("chorus.distinct_ignore_pairs", "list · [] — pairs never flagged, as Name|Name (order/case-insensitive)"),
  chord_row("chorus.register_drift_threshold", "float · 0.08 — register-metric change vs chapter 1 that flags a drift"),
  chord_row("stylist.enabled", "bool · true — master switch for the Inner Stylist"),
  chord_row("stylist.session_budget", "float · 0.15 — informative daily USD budget for the coaching track"),
  chord_row("stylist.language", "string? · null — coaching-prompt language override"),
))

#subsection("dialogue")

DIALOG-1 — deterministic dialogue quality & attribution.

#chord_table((
  chord_row("attribution_window", "int · 60 — token window for the attribution name search"),
  chord_row("unattributed_run_threshold", "int · 8 — unattributed turns tolerated before the zero-attribution finding"),
  chord_row("talking_head_threshold", "int · 6 — dialogue-only paragraphs before the talking-head finding"),
  chord_row("beat_min_words", "int · 8 — minimum words for a non-speech sentence to count as an action beat"),
  chord_row("said_bookism_threshold", "float · 0.15 — said-bookism density delta above baseline that triggers a finding"),
  chord_row("fingerprint_min_utterances", "int · 5 — attributed utterances before a character fingerprint shows"),
  chord_row("language", "string? · null — override (en/ru/de/fr/es); null uses project then en"),
  chord_row("extra_neutral_verbs", "array · [] — verbs added to the neutral list (SF/fantasy speech verbs)"),
  chord_row("extra_said_bookisms", "array · [] — verbs added to the said-bookism list"),
))

#subsection("drift")

Semantic-drift retrieval (1.3.10) — divergent descriptions of one entity.

#chord_table((
  chord_row("top_k", "usize · 24 — vector hits pulled per entity before name-filtering"),
  chord_row("max_snippets", "usize · 8 — descriptions kept per entity (bounds the judge prompt)"),
  chord_row("pronouns.<lang>.{character,place,artefact}", "map · {} — per-language coref pronoun sets (1.3.13); written by lang bootstrap"),
))

#subsection("facts")

Series-shared canon (1.3.8). One field.

#chord_table((
  chord_row("shared_path", "string? · null — a directory of plain-text fact files layered under each book's Facts book (local wins on a clash)"),
))

#section("The Inner family & readers")

#subsection("inner_editor")

INNER_EDITOR-1 — the literary/stylistic companion. LLM-only, paragraph scope;
caps inform, never block.

#screen(caption: "inner_editor — the shape")[
```hjson
inner_editor: {
  enabled: true
  engagement: { idle_threshold_seconds: 60, cooldown_seconds: 120,
                max_findings_per_paragraph: 3 }
  context:    { preceding_paragraphs: 3, following_paragraphs: 0 }
  persona:    { tone: balanced, verbosity: concise,
                praise_frequency: moderate, genre_aware: true,
                belief_stance_enabled: true }
  output:     { severity_threshold: note, group_by_paragraph: true,
                always_show_persona_label: true }
}
```
]

#chord_table((
  chord_row("enabled", "bool · true — false fully disables (the manual chord then just informs)"),
  chord_row("engagement.idle_threshold_seconds", "int · 60 — ambient paragraph-pause wait before auto-engaging"),
  chord_row("engagement.cooldown_seconds", "int · 120 — same-paragraph cooldown; an edit re-arms it"),
  chord_row("engagement.max_findings_per_paragraph", "int · 3 — cap on findings per paragraph"),
  chord_row("context.preceding_paragraphs", "int · 3 — interpretation context sent before the paragraph"),
  chord_row("context.following_paragraphs", "int · 0 — context sent after"),
  chord_row("persona.tone", "enum · balanced — critical | balanced | encouraging"),
  chord_row("persona.verbosity", "enum · concise — concise | standard | detailed"),
  chord_row("persona.praise_frequency", "enum · moderate — rare | moderate | frequent"),
  chord_row("persona.genre_aware", "bool · true — fold the project genre into the prompt"),
  chord_row("persona.belief_stance_enabled", "bool · true — allow the does-the-prose-believe-itself category"),
  chord_row("persona.categories.*", "bool · true — the eight categories, each toggleable (literary_richness, tautology, style_observation, style_instability, dictionary_richness, belief_stance, craft_praise, editorial_suggestions)"),
  chord_row("output.severity_threshold", "enum · note — praise | note | concern visible floor; at note, Praise is persisted but not pushed to Output"),
  chord_row("output.group_by_paragraph", "bool · true — group findings by paragraph"),
  chord_row("output.always_show_persona_label", "bool · true — label every finding with the persona"),
  chord_row("llm.editor_engagement.max_calls_per_session", "int · 80 — session cap (warns, never blocks)"),
  chord_row("llm.editor_engagement.confirm_above_calls", "int · 40 — confirm once a session exceeds this"),
  chord_row("llm.editor_engagement.max_calls_per_day", "int · 200 — daily cap, shown in inkhaven cost"),
  chord_row("llm.editor_engagement.max_calls_per_month", "int · 4000 — monthly cap"),
  chord_row("llm.conversation.max_calls_per_session / _per_day", "int · 30 / 80 — conversation-mode caps"),
  chord_row("llm.backoff_max_retries", "int · 3 — retry count on transient failure"),
  chord_row("llm.backoff_initial_seconds", "int · 30 — initial backoff delay"),
))

#subsection("theologian")

INNER-THEOLOGIAN-1 — the tradition-neutral moral/theological reader. Eleven
lenses, never a verdict.

#chord_table((
  chord_row("enabled", "bool · true — master switch; false gates everything including the fast-track"),
  chord_row("on_paragraph_idle", "bool · true — fire a Category-1 question on paragraph idle"),
  chord_row("idle_threshold_seconds", "int? · null — idle wait; null uses Inner Socrates' Slow threshold"),
  chord_row("session_budget", "f32 · 0.15 — slow-track USD sub-budget; informs, never blocks"),
  chord_row("fast_track", "bool · true — run the deterministic fast-track in the review pass"),
  chord_row("moral_invisibility_window", "usize · 3 — paragraphs after a harm event checked for acknowledgment"),
  chord_row("consequence_gap_window", "usize · 5 — paragraphs after lethal violence checked for consequence"),
  chord_row("sacred_levity_signal", "bool · true — emit the sacred-vocabulary-in-levity signal"),
  chord_row("disabled_lenses", "array · [] — lens codes to exclude from slow-track hints"),
  chord_row("language", "string? · null — question/marker language override; null uses project then en"),
))

#subsection("rigor")

RIGOR — the deterministic, zero-AI reasoning-rigor reader.

#chord_table((
  chord_row("enabled", "bool · true — master switch"),
  chord_row("fast_track", "bool · true — run in the review pass / deep-refresh"),
  chord_row("language", "string? · null — marker-language override; null uses project then English"),
  chord_row("false_dichotomy", "bool · true — flag forced-binary framings"),
  chord_row("question_begging", "bool · true — flag unargued assertions (obviously, of course)"),
  chord_row("straw_man", "bool · true — flag dismissive characterizations"),
  chord_row("overgeneralization", "bool · true — flag strong absolutes (always, never)"),
  chord_row("non_sequitur", "bool · true — flag a conclusion connective with no warrant"),
  chord_row("equivocation", "bool · true — flag a Glossary watch_equivocation term used without pinning a sense"),
))

#subsection("myth")

MYTH-1 — the symbolic pattern library over the declared Mythology book.

#chord_table((
  chord_row("enabled", "bool · true — master switch for the review-pass findings + heatmap chord"),
  chord_row("heatmap_buckets", "usize · 8 — chapter buckets the heatmap collapses the book into"),
  chord_row("consistency_min_chapters", "u32 · 5 — chapters a symbol must span before the LLM consistency check"),
  chord_row("motif_min_occurrences", "u32 · 3 — occurrences a motif needs before the LLM completeness check"),
  chord_row("final_act_pct", "u32 · 25 — the final act is the last this-percent of chapters"),
  chord_row("check_cost_warn", "f32 · 0.08 — warn when an myth check LLM run exceeds this USD"),
))

#subsection("utopia")

WORLD-6 — utopian/dystopian coherence over declared premises.

#chord_table((
  chord_row("stage2_cost_warn", "f32 · 0.10 — Stage-2 cost-warning threshold (USD); informs, never blocks"),
  chord_row("stage2_max_pairs", "int · 200 — cap on premise pairs compared in Stage 2 (code-only; absent from the prose docs)"),
  chord_row("stage3_batch_size", "int · 5 — chapters per Stage-3 background pass"),
  chord_row("stage3_min_chapter_words", "int · 200 — minimum chapter words for the entailment scan"),
  chord_row("group_gap_threshold", "int · 1 — non-claim paragraphs that break a premise group; 0 makes all one group"),
))

#subsection("char")

CHAR-1 — character-arc tracking.

#chord_table((
  chord_row("stall_threshold", "int · 4 — unchanged chapters after the baseline before a stall fires"),
  chord_row("active_window_before", "int · 5 — tokens before a verb a name may sit and still be the actor"),
  chord_row("active_window_after", "int · 8 — tokens after a verb a name may sit and be the patient"),
  chord_row("min_chapters_for_check", "int · 3 — chapters of state before the LLM arc checks run"),
  chord_row("enrich_from_dialogue", "bool · true — enrich the state chain with DIALOG-1 signals"),
  chord_row("enrich_from_voice", "bool · true — enrich with NARR-1 chapter interiority"),
  chord_row("language", "string? · null — arc language override; null uses project then English"),
  chord_row("extra_action_verbs", "array · [] — genre verbs the agency scorer treats as action"),
  chord_row("extraction_cost_warn", "f32 · 0.20 — state-extraction cost-warning USD; informs, never blocks"),
))

#subsection("world")

WORLD-12 — the AI world-critique pass (`realworld critique`).

#chord_table((
  chord_row("critique_enabled", "bool · true — false runs the deterministic lints only, skipping the LLM"),
  chord_row("critique_max_tokens", "usize · 24000 — per-call soft token cap; 0 disables it"),
  chord_row("critique_cost_warn", "f32 · 0.10 — warn when a run exceeds this USD"),
))

#section("Timeline")

#subsection("timeline")

Story timeline (1.2.6+). Off by default; set `enabled: true` plus a calendar
to turn on event tracking.

#chord_table((
  chord_row("enabled", "bool · false — master switch; every chord/CLI/Bund hint no-ops when off"),
  chord_row("default_track", "string · main — track label when an event has none"),
  chord_row("calendar.preset", "string · custom — gregorian | sols | custom (custom honours every field)"),
  chord_row("display.show_orphans", "bool · true — synthetic orphan row at the bottom of the swim lane"),
  chord_row("display.swim_lane_max_rows", "u32 · 12 — truncate beyond this with a +N more row"),
  chord_row("display.default_zoom", "f32 · 1.0 — initial ticks-per-cell"),
  chord_row("display.grid_every_days", "u32 · 7 — faint vertical bar every N days; 0 disables (code-only field)"),
))

The `calendar` sub-block (preset `custom`) carries `base_unit`, a base-first
`units` stack (each with `per_parent` + optional `names`), `seasons`,
`epoch_label` / `epoch_before_label`, a `display_format` with `{year}`,
`{month}`, `{month-name}`, `{day}`, `{hour}` tokens, and `parse_aliases`.
`sols` and `gregorian` expand to preset stacks.

#subsection("timeline.critique")

The refactored timeline critique — orphan + fuzzy-overlap + optional LLM
elaboration. Gated by `timeline.enabled`.

#chord_table((
  chord_row("enabled", "bool · true — master switch for the critique"),
  chord_row("orphan.enabled", "bool · true — the orphaned-event check (code-only sub-block)"),
  chord_row("orphan.min_orphan_age_days", "i64 · 0 — suppress orphan findings younger than this; 0 emits immediately"),
  chord_row("orphan.min_significance", "string · low — lowest significance surfaced (low | moderate | high)"),
  chord_row("fuzzy_overlap.enabled", "bool · true — the fuzzy-precision overlap check"),
  chord_row("fuzzy_overlap.min_suspicion", "string · moderate — lowest suspicion surfaced (low | moderate | high)"),
  chord_row("fuzzy_overlap.cluster_min_size", "usize · 3 — minimum events for a cluster (vs pairwise) finding"),
  chord_row("elaboration.enabled", "bool · true — LLM elaboration when a provider exists; else pattern-only text"),
  chord_row("elaboration.max_calls_per_run", "usize · 20 — hard cap on elaboration calls per run"),
  chord_row("elaboration.confirm_above_calls", "usize · 10 — confirm once a run would exceed this"),
  chord_row("legacy_flag_deprecation.warn_on_use", "bool · true — warn on the retired event critique --legacy path"),
))

#subsection("scrivener")

Scrivener-importer behaviour (1.2.8+). Gated by `timeline.enabled`.

#chord_table((
  chord_row("date_fields", "array · [Date, Story Date, Event Date] — CustomMeta field names interpreted as event dates"),
))

#section("Research")

#subsection("research — the assistant")

RESRCH-1 — the separate `inkhaven research` TUI. Omit the block for these
defaults.

#chord_table((
  chord_row("default_thread", "string? · null — thread to open (null = picker / default)"),
  chord_row("rag_top_n", "usize · 5 — max Facts paragraphs prepended per query as RAG context"),
  chord_row("session_budget_warn", "f64 · 0.50 — per-session cost-cap warning (USD); informs, never blocks"),
  chord_row("max_pinned_nodes", "usize · 3 — max pinned Facts nodes (Ctrl+P)"),
  chord_row("show_keybind_hints", "bool · true — show the keybind hints bar by default"),
  chord_row("min_width", "u16 · 80 — minimum terminal width before a resize message shows"),
  chord_row("split_ratio", "u32 · 4 — Facts-tree columns out of 10 (4 = 40% tree)"),
  chord_row("diff_top_n", "usize · 3 — /diff: similar facts shown"),
  chord_row("verify_min_sentence_words", "usize · 8 — /verify: minimum sentence words for claim extraction"),
  chord_row("dedup_warn_score", "f64 · 0.92 — /fact: near-duplicate similarity that warns; informs, never blocks"),
  chord_row("triangulate_gate", "bool · false — /fact from model/web/document is triangulated across sources before commit (code-only; network-heavy)"),
  chord_row("refute_gate", "bool · false — /fact from model/document gets one adversarial refutation pass before commit (code-only)"),
  chord_row("import_chunk_chars", "usize · 1500 — /import: max characters per embedded chunk"),
))

The `research.agentic` block gates the autonomous deep-research loop:
`enabled` (bool, `true`), `max_subquestions` (usize, `6` — the total Facts a
run may emit), `max_rounds` (usize, `3` — gap-driven iterate rounds; `1` is a
single pass). The `research.web` block gates `/web`: `enabled` (`false`),
`provider` (`none` — tavily | searxng | none), `api_key` (empty), `endpoint`
(empty), `max_results` (`5`), `fetch` (`true`), `pipeline` (`chat` — chat |
ingest).

#subsection("research source blocks")

Each keyless source under `research`. `max_chars` bounds the embedded portion;
`auto_cite` mints a SOURCES-1 entry on `/fact`.

#chord_table((
  chord_row("geonames", "enabled true · endpoint http://api.geonames.org · username empty (empty keeps /geonames unavailable)"),
  chord_row("gutenberg", "enabled true · endpoint https://gutendex.com · max_chars 300000 · auto_cite true"),
  chord_row("archive", "enabled true · endpoint https://archive.org · max_chars 300000 · auto_cite true (Internet Archive; absent from the prose docs)"),
  chord_row("wikidata", "enabled true · endpoint https://www.wikidata.org · max_statements 24 (top of the trust ladder; skips the fact-check gate)"),
  chord_row("wikisource", "enabled true · default_lang en · max_chars 300000 · auto_cite true"),
  chord_row("scholarly", "enabled true · mailto empty · auto_cite true (OpenAlex + arXiv)"),
))

The `research.scripture` block (`/bible`, `/quran`, `/bookofmormon`) defaults:
`enabled` true, `bible_endpoint` `https://bolls.life`, `quran_endpoint`
`https://api.alquran.cloud/v1`, `bom_url` the bcbooks 1830 Book of Mormon JSON,
`bible_translation` null (en=WEB, ru=SYNOD, fr=FRLSG, de=LUT, es=RV1960),
`quran_translation` null, `max_chars` 200000, `auto_cite` true.

#section("Content authoring")

#subsection("sources")

SOURCES-1 — the bibliography engine over the Sources book.

#chord_table((
  chord_row("all", "bool · true — collect every Sources entry into every book; false scopes per book"),
  chord_row("bibliography_style", "string · ieee — CSL style passed to Typst's #bibliography (apa, chicago-author-date, mla, …)"),
  chord_row("auto_bibliography", "bool · true — append the #bibliography(...) line during assembly"),
))

#subsection("snippets")

Trigger-keyed editor text expansions (1.2.14+). A trigger followed by Space
expands. Placeholders: `{date}`, `{time}`, `{datetime}`, `{slug}`, `{book}`,
`{chapter}`, `{author}`, `{cursor}`, the picker forms `{char_lookup}` /
`{place_lookup}` / `{artefact_lookup}`, and a `bund:` body prefix.

#screen(caption: "snippets — a few bindings")[
```hjson
snippets: {
  "\\dt":   "{datetime}"
  "\\au":   "— {author}"
  "\\todo": "TODO ({date}): {cursor}"
}
```
]

#subsection("jinja")

STRUCT-1 — Jinja template paragraphs. Self-gating; one knob.

#chord_table((
  chord_row("continue_on_error", "bool · false — false aborts assembly on a render error; true writes a visible error block and continues"),
))

#subsection("images")

Image-node handling in the tree and editor.

#chord_table((
  chord_row("preview_enabled", "bool · true — render image previews"),
  chord_row("allowed_extensions", "list · png, jpg, jpeg, gif, webp, svg — extensions accepted for image nodes"),
  chord_row("max_size_bytes", "u64 · 33554432 (32 MiB) — a larger file is rejected with a status message"),
))

#section("Typst output & export")

These blocks feed the synthesised `settings.typ` / `globals.typ` prepended to
every assembled book. Empty/zero generally falls through to Typst's own default.

#subsection("typst_compile")

The compile engine and diagnostics (`Ctrl+B B` / `Ctrl+B O`).

#chord_table((
  chord_row("engine", "string · external — external (shells out to typst on PATH) | inprocess (typst as a library)"),
  chord_row("diagnostics", "bool · true — run typst-syntax on save/idle; parse errors on the status bar"),
  chord_row("diagnostics_idle_seconds", "int · 2 — idle debounce before the recheck; 0 runs every tick"),
  chord_row("semantic_diagnostics", "bool · false — full typst::compile on the open paragraph (inprocess only; false positives expected)"),
  chord_row("bundle_fonts", "bool · true — ship Computer Modern + Linux Libertine in the binary (inprocess)"),
  chord_row("use_system_fonts", "bool · true — also search host system fonts (inprocess)"),
  chord_row("packages_enabled", "bool · true — fetch @preview/<pkg> from packages.typst.org (inprocess)"),
  chord_row("wait_for_key_after_compile", "bool · true — hold the splash after compile finishes"),
  chord_row("error_system_prompt", "string · empty — override the AI compile-error prompt"),
))

#subsection("typst_page, typst_fonts, typst_layout")

Page geometry, face, and paragraph layout.

#chord_table((
  chord_row("typst_page.paper", "string · us-letter — anything Typst's paper: accepts"),
  chord_row("typst_page.margin_top / _bottom", "string · 2.5cm / 2.5cm — top and bottom margins"),
  chord_row("typst_page.margin_inside / _outside", "string · 3cm / 2cm — binding-edge and outside margins"),
  chord_row("typst_page.page_numbering", "string · 1 — page-number format; empty = none"),
  chord_row("typst_page.columns", "u32 · 1 — column count; 0/1 fall through to single-column"),
  chord_row("typst_fonts.body", "string · Linux Libertine — body typeface (emitted with a bundled fallback)"),
  chord_row("typst_fonts.body_size", "string · 11pt — body size"),
  chord_row("typst_fonts.monospace", "string · DejaVu Sans Mono — raw/code typeface"),
  chord_row("typst_fonts.language", "string · en — two-letter tag for #set text(lang:)"),
  chord_row("typst_layout.justify", "bool · true — justify body paragraphs"),
  chord_row("typst_layout.leading", "string · 0.7em — inter-line leading"),
  chord_row("typst_layout.paragraph_indent", "string · empty — first-line indent; empty = none"),
  chord_row("typst_layout.heading_numbering", "string · empty — #set heading(numbering:) arg; empty = unnumbered"),
))

#subsection("typst_templates")

The eight `wrap_*` functions written into each book's `globals.typ`. Each is a
string; an empty string falls back to the shipped default for that one function
(so you override just the wrapper you care about). Fields: `wrap_book`,
`wrap_chapter`, `wrap_subchapter`, `wrap_paragraph`, `wrap_image_book`,
`wrap_image_chapter`, `wrap_image_subchapter`, `wrap_image_inline` — all default
empty. The shipped bodies live in `default_wrap_*()` and are regenerated on
every assembly, so edits belong in the config, not the generated file.

#subsection("typst_universe")

Source for the `Ctrl+V #` package-import picker (1.6.15+).

#chord_table((
  chord_row("url", "string · orangex4 typst-universe-with-stars packages.json — the community package manifest"),
  chord_row("ttl_hours", "u32 · 24 — cache-freshness window in hours"),
))

#subsection("frontmatter")

Journal-article front matter (1.6.15+). All-empty renders nothing.

#chord_table((
  chord_row("abstract", "string · empty — a single plain-prose paragraph (Rust field abstract_text)"),
  chord_row("keywords", "list · [] — keyword list"),
  chord_row("authors", "list · [] — Author records: name, affiliation, orcid, email, corresponding (all empty/false)"),
  chord_row("funding", "string · empty — funding statement; identifying, dropped under --blind"),
  chord_row("data_availability", "string · empty — kept under --blind"),
  chord_row("code_availability", "string · empty — kept under --blind"),
))

#subsection("output")

Multi-format export for `Ctrl+B O`.

#chord_table((
  chord_row("extra_formats", "list · [] — formats built alongside the PDF (markdown, tex, epub, docx, imposed_pdf, cover_pdf)"),
  chord_row("extras_step_pause_ms", "u64 · 400 — ms the extras splash holds each format; 0 disables the pause (code-only)"),
  chord_row("extras_wait_for_key", "bool · false — hold the final splash frame for a keypress (code-only)"),
  chord_row("imposed_pdf_config", "string · default — the imposition profile used when imposed_pdf is listed"),
))

#subsection("docs")

TDOC — the technical-documentation blocks. Only `docs.html` appears in the
prose docs; `docs.verify`, `docs.variables`, and `docs.index` are code-only.

#chord_table((
  chord_row("verify.enabled", "bool · false — master switch for verified code blocks; nothing runs unless true"),
  chord_row("verify.timeout_seconds", "u64 · 30 — per-block wall-clock cap"),
  chord_row("verify.runners", "map · {} — language → shell command ({file}/{dir} substituted, run via sh -c)"),
  chord_row("verify.extensions", "map · seeded — language → temp-file extension (rust→rs, python→py, …); unknown falls back to .txt"),
  chord_row("variables", "map · {} — TDOC-3 {{key}} substitutions applied at assembly across every export"),
  chord_row("index.from_glossary", "bool · true — INDEX-1: include every Glossary term (synonyms as see-refs)"),
  chord_row("index.terms", "list · [] — extra index terms beyond the Glossary"),
))

The `docs.html` static-site export (`inkhaven export html`):

#chord_table((
  chord_row("html.site_title", "string? · null — null uses the exported book's title"),
  chord_row("html.theme", "string · default — bundled theme name (only default today)"),
  chord_row("html.template_dir", "string · html — project override root for functional/ and theme/ files"),
  chord_row("html.variables_file", "string · html.hjson — HJSON exposed to templates as site"),
  chord_row("html.search", "bool · true — build the client-side search index (accepted, wired later)"),
  chord_row("html.citation_style", "string · author-year — author-year | numeric (accepted, wired later)"),
  chord_row("html.include.sources / .glossary", "bool · true — fold Sources / Glossary into the site"),
  chord_row("html.include.characters / .places / .language / .world / .mythology / .notes / .index", "bool · false — the remaining companion books, off by default"),
))

#subsection("tex_export")

LaTeX export document class + preamble (1.6.15+). All-empty leaves tylax's
`article` output untouched.

#chord_table((
  chord_row("document_class", "string · empty — journal class (IEEEtran, elsarticle, article, …)"),
  chord_row("class_options", "string · empty — class options (conference, twocolumn, 11pt,a4paper)"),
  chord_row("extra_packages", "list · [] — extra usepackage lines (bare names or full lines)"),
  chord_row("preamble", "list · [] — raw preamble lines before begin{document}"),
))

#section("PDF production (1.3.0)")

Three blocks feed the `inkhaven pdf …` subsystem. `imposition.profiles` names
folding-signature recipes for `pdf impose` (six are built in — `default`,
`chapbook`, `us_perfect`, `us_chapbook`, `thick`, `a5_book`); a profile carries
`style` (perfect_bound | saddle_stitch | side_stab), `sheets_per_signature`,
`target_sheet_size`, `orientation`, `margins`, a `creep` sub-block, `marks`
toggles, and `blank_page_policy`. `cover` sets house cover/spine defaults —
`front_width_mm` 152.0, `front_height_mm` 229.0, `bleed_mm` 3.0,
`interior_stock` uncoated_80gsm, `cover_stock` cover_250gsm,
`spine_font_size_pt` 11.0, `image_fit` cover. `preflight` sets DPI targets —
`default_profile` hand_binding, `hand_binding_dpi` 300, `print_shop_dpi` 300.

#section("Environment, safety & audio")

#subsection("health")

Background health monitor (1.2.15+). Off by default.

#chord_table((
  chord_row("enabled", "bool · false — spawn the monitor task; the status chip stays hidden when off"),
  chord_row("auto_repair.rescue_orphans", "bool · false — auto-delete *.inkhaven-rescue files older than 30 days"),
))

#subsection("project_lock")

Single-instance advisory lock (1.3.36) — informs, never hard-blocks by default.

#chord_table((
  chord_row("enabled", "bool · true — acquire the lock; false disables guarding entirely"),
  chord_row("on_conflict", "string · prompt — prompt (interactive y/N) | warn (always proceed) | refuse (never open a second session)"),
))

#subsection("scripting")

The Bund sandbox policy. Destructive categories deny by default; writers opt in.

#chord_table((
  chord_row("trust_decision", "string · ask — ask (needs a .inkhaven/trust marker) | trust (run unconditionally) | deny (never run)"),
  chord_row("fs_unsandboxed", "bool · false — false confines ink.fs.* to the project root via path_safety"),
))

Category gates (`fs_write`, `net`, `shell`, `code_eval`, …) default to deny;
list the categories or individual words a project may use. See `SECURITY_WARNING.md`.

#subsection("shell")

The embedded nushell pane (1.2.8+, `Ctrl+Z o`).

#chord_table((
  chord_row("enabled", "bool · true — open the shell pane; false makes the chord a no-op"),
  chord_row("max_buffered_turns", "int · 50 — in-memory (command, output) pairs retained"),
  chord_row("max_output_lines", "int · 1000 — per-turn stdout/stderr lines kept; excess is truncated with a marker"),
  chord_row("blocked_externals", "list · ~40 basenames — full-screen TUI apps refused before spawn (vim, less, top, tmux, ssh, …); [] disables the guard"),
  chord_row("external_timeout_secs", "int · 30 — wall-clock budget per command before a watchdog interrupt"),
  chord_row("insert_template", "string · #raw(block: true, lang: shell, `{output}`) — Typst wrapping a Ctrl+Z h→i insert"),
))

#subsection("hierarchy")

#chord_table((
  chord_row("unbounded_subchapters", "bool · false — false keeps Book→Chapter→Subchapter→Paragraph; true nests subchapters arbitrarily"),
))

#subsection("sound")

TUI sound effects (opt-in; `Ctrl+B E` toggles).

#chord_table((
  chord_row("enabled", "bool · false — master switch, off so launch is silent"),
  chord_row("volume", "f32 · 0.6 — master volume 0.0–1.0, clamped at load"),
))

#subsection("oracle")

ORACLE (1.7.9+) — the conlang phonotactic guardian on save.

#chord_table((
  chord_row("enabled", "bool · true — master switch; false gates the on-save scan"),
  chord_row("on_save", "bool · true — run the guardian when a paragraph is saved"),
))

#subsection("cost")

The AI-cost dashboard (`Ctrl+B $` / `inkhaven cost`). Caps *inform*, never gate.

#chord_table((
  chord_row("world_daily_call_cap", "int · 200 — world fact-check slow-track ceiling"),
  chord_row("inner_socrates_daily_call_cap", "int · 150 — Inner Socrates slow-track ceiling"),
  chord_row("usage_retention_days", "int · 30 — days of per-category tallies kept before pruning"),
  chord_row("default_input_per_1m / default_output_per_1m", "f64 · 3.0 / 3.0 — fallback USD-per-1M-token prices"),
))

The `pricing` map ships list prices per model-name substring (gemini-2.5-pro
1.25/10.0, claude-sonnet 3.0/15.0, claude-opus 15.0/75.0, gpt-4o 2.50/10.0,
deepseek 0.27/1.10, and more); override an entry as prices move.

#callout(label: "The knowledge / KEN block")[
The 2.6.0 KEN feature (epistemic continuity) has an RFC but no config block
yet — there is no `knowledge:` key in `Config`. When it ships it will follow
the same `#[serde(default)]` contract as every block above, so an older config
keeps working unchanged.
]

Beyond these blocks, `inkhaven.hjson` is forward-compatible by construction:
old configs run on new releases (missing fields take the default), obsolete
fields stay parseable (so downgrading is safe too), and Inkhaven never rewrites
your file in place. To reset to shipping defaults, rename the file and run
`inkhaven init --force`, then re-merge your customisations from the annotated
template at `assets/default_project.hjson` — the same file `init` writes.
