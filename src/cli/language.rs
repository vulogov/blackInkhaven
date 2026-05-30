//! 1.2.13+ Phase A — `inkhaven language …`
//! subcommand family.  Currently hosts `init`,
//! which scaffolds a per-language sub-book under
//! the top-level `Language` system book with the
//! five standard chapters (`Meta`, `Dictionary`,
//! `Grammar`, `Phonology`, `Sample texts`) and a
//! seeded `Meta/overview.typ` HJSON paragraph.
//!
//! See `Documentation/PROPOSALS/LANGUAGE_BOOK.md`
//! for the full design including the dictionary
//! entry HJSON schema, grammar-rule schema, and the
//! AI text-to-text translation flow that Phases B-D
//! will add on top of this foundation.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{
    InsertPosition, NodeKind, Store, SYSTEM_TAG_LANGUAGES,
};

use super::{LanguageCommand, LanguageExportFormat};

pub fn run(project: &Path, cmd: LanguageCommand) -> Result<()> {
    match cmd {
        LanguageCommand::Init { name } => init(project, &name),
        LanguageCommand::AddWord {
            language,
            word,
            r#type,
            translation,
            example,
            import,
        } => {
            if let Some(csv_path) = import {
                import_dictionary_csv(project, &language, &csv_path)
            } else {
                // Single-add mode requires word + type +
                // translation positionals/flags.
                let word = word.ok_or_else(|| {
                    Error::Config(
                        "missing <WORD> — pass a word argument OR use --import <PATH>"
                            .into(),
                    )
                })?;
                let pos = r#type.ok_or_else(|| {
                    Error::Config(
                        "missing --type — pass a part-of-speech OR use --import".into(),
                    )
                })?;
                let translation = translation.ok_or_else(|| {
                    Error::Config(
                        "missing --translation — pass a working-language gloss OR use --import"
                            .into(),
                    )
                })?;
                add_word(
                    project,
                    &language,
                    &word,
                    &pos,
                    &translation,
                    example.as_deref(),
                )
            }
        }
        LanguageCommand::Doctor { language, json } => doctor(project, &language, json),
        LanguageCommand::Export {
            language,
            format,
            output,
        } => export(project, &language, format, output.as_deref()),
        LanguageCommand::List => list(project),
        LanguageCommand::RemoveWord { language, word } => {
            remove_word(project, &language, &word)
        }
    }
}

/// The five standard chapters every language book
/// gets at scaffold time.  Order matches the order
/// authors are most likely to fill them in:
///
///   * Meta — language metadata HJSON (alphabet,
///     language_kind, family, reading_direction).
///     Author fills first because every other
///     chapter depends on the alphabet.
///   * Dictionary — entries land here under
///     alphabet subchapters (auto-created on first
///     entry in each letter).
///   * Grammar — HJSON rules the AI translator
///     consumes.
///   * Phonology — sound rules kept separate so
///     they don't bloat every translation prompt.
///   * Sample texts — few-shot examples for the AI
///     plus author re-immersion material.
const STANDARD_CHAPTERS: &[&str] = &[
    "Meta",
    "Dictionary",
    "Grammar",
    "Phonology",
    "Sample texts",
];

/// Seed body for `Meta/overview` — pure HJSON so the
/// editor renders with HJSON syntax highlighting.
/// The paragraph's `content_type` is set to `"hjson"`
/// at create time; the body is just the metadata
/// object (no Typst headings, no markdown fences).
///
/// 1.2.13+ Phase D.1 hotfix — switched FROM the Typst-
/// with-fenced-HJSON format to pure HJSON because the
/// Typst editor mode rendered the body as a heading +
/// opaque code fence instead of as structured config.
/// The translation prompt composer + parser handle
/// both formats; new entries use pure HJSON, legacy
/// Typst-wrapped entries still parse via the fence
/// extractor.
const META_OVERVIEW_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Display name for the language.
  name: \"\"

  // Sibling languages (e.g. Elvish, Romance, Slavic).
  // Phase D.2 will use this for cross-language family
  // browsing in the sidebar.
  family: \"\"

  // \"constructed\" | \"natural\" — drives default
  // assumptions in the AI translator.  Constructed
  // languages get stricter adherence to the explicit
  // rules below; natural languages let the LLM lean
  // more on its pretraining.
  language_kind: constructed

  // Optional ISO 639-3 code (e.g. \"qya\" for Quenya).
  // Used by the multilingual prompt resolver when
  // mixing this language with the project's working
  // language flow.
  iso_code: \"\"

  // ──────────────────────────────────────────────────
  // ORTHOGRAPHY
  // ──────────────────────────────────────────────────

  // Alphabet entries in canonical order.  For non-
  // Latin orthographies, override with the author's
  // declared groupings:
  //   * paired-case Latin: [\"Aa\", \"Bb\", \"Cc\"]
  //   * Hebrew letter names: [\"Aleph\", \"Beth\", \"Gimel\"]
  //   * Greek: [\"Α\", \"Β\", \"Γ\"]
  //   * Cyrillic: [\"А\", \"Б\", \"В\"]
  //   * Polish digraphs: [\"A\", \"Cz\", \"Dz\", \"Sz\"]
  // Drives Dictionary bucket auto-creation in
  // `inkhaven language add-word` and the in-TUI `+`
  // chord.
  alphabet: [\"A\", \"B\", \"C\", \"D\", \"E\", \"F\", \"G\", \"H\", \"I\",
             \"J\", \"K\", \"L\", \"M\", \"N\", \"O\", \"P\", \"Q\", \"R\",
             \"S\", \"T\", \"U\", \"V\", \"W\", \"X\", \"Y\", \"Z\"]

  // \"ltr\" (default) | \"rtl\" | \"ttb\" (top-to-bottom)
  reading_direction: ltr

  // Script / writing system name (Latin, Cyrillic,
  // Tengwar, Devanagari, …).  Free-form; informational.
  script: \"\"

  // ──────────────────────────────────────────────────
  // LINGUISTIC SHAPE — quick-reference summary the
  // AI translator reads before composing prompts.
  // ──────────────────────────────────────────────────

  // Word order: SVO | SOV | VSO | VOS | OSV | OVS | free
  word_order: \"\"

  // Morphological type: isolating | agglutinative |
  // fusional | polysynthetic | mixed
  morphology: \"\"

  // Tonal: true | false (informational only).
  tonal: false

  // Has grammatical case (declension)?
  has_cases: false

  // Has grammatical gender?
  has_gender: false

  // ──────────────────────────────────────────────────
  // RUNTIME / TOOLING
  // ──────────────────────────────────────────────────

  // Optional Snowball stemmer algo name (\"english\",
  // \"russian\", \"french\", \"spanish\", \"german\").
  // Rare for conlangs — leave empty to let the
  // lexicon overlay rely on the dictionary
  // `inflection` paradigm fields instead.
  stemmer: \"\"

  // Free-form citation for the canonical sample
  // corpus the LLM should treat as authoritative
  // (Tolkien's Etymologies, Klingon Dictionary, etc.).
  example_corpus_ref: \"\"

  // ──────────────────────────────────────────────────
  // NOTES
  // ──────────────────────────────────────────────────

  // Worldbuilding context — who speaks the language,
  // where, in what era, what register.  Read by the
  // human author; the LLM only consumes the
  // structured fields above when composing
  // translation prompts.
  notes: \"\"
}
";

fn init(project: &Path, name: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    let lang_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it"
                    .into(),
            )
        })?;

    // Reject duplicate before the create so the
    // failure mode is a friendly error, not a
    // silent `-2` slug suffix on the second
    // attempt.
    if hierarchy
        .children_of(Some(lang_book.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(name))
    {
        return Err(Error::Config(format!(
            "language `{name}` already exists under Language"
        )));
    }

    let hierarchy = Hierarchy::load(&store)?;
    let per_lang = store.create_node(
        &cfg,
        &hierarchy,
        NodeKind::Book,
        name,
        Some(&lang_book),
        None,
        InsertPosition::End,
    )?;
    eprintln!(
        "created language book `{name}` at {}",
        hierarchy.slug_path(&per_lang),
    );

    scaffold_language_chapters(&store, &cfg, &per_lang, |chapter_title| {
        eprintln!("  · {chapter_title}");
    })?;

    eprintln!("\nNext steps:");
    eprintln!(
        "  · edit `Language/{name}/Meta/overview` to set the alphabet + metadata"
    );
    eprintln!(
        "  · add dictionary entries under `Language/{name}/Dictionary` (Phase B: `inkhaven language add-word`)"
    );
    eprintln!(
        "  · add grammar rules under `Language/{name}/Grammar` for the Phase C AI translation flow"
    );

    Ok(())
}

/// 1.2.13+ Phase D.1 — shared scaffold helper.
/// Creates the 5 standard chapters under an already-
/// existing per-language book + seeds
/// `Meta/overview` with the starter HJSON.  Used by
/// both the CLI `init` path and the in-TUI tree-pane
/// commit path (see `App::provision_language_book`)
/// so the two entry points produce identical
/// scaffolds.
///
/// `on_chapter` is called for each chapter at create
/// time so the caller can emit progress (CLI prints
/// `· Meta`; the TUI updates the status bar).
pub(crate) fn scaffold_language_chapters(
    store: &Store,
    cfg: &Config,
    per_lang: &crate::store::node::Node,
    mut on_chapter: impl FnMut(&str),
) -> Result<()> {
    for title in STANDARD_CHAPTERS {
        // Reload between creates so each subsequent
        // create sees the previous create's slug +
        // order.
        let hierarchy = Hierarchy::load(store)?;
        let chapter = store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Chapter,
            title,
            Some(per_lang),
            None,
            InsertPosition::End,
        )?;
        on_chapter(title);
        if *title == "Meta" {
            let hierarchy = Hierarchy::load(store)?;
            let mut overview = store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Paragraph,
                "overview",
                Some(&chapter),
                None,
                InsertPosition::End,
            )?;
            // Switch to HJSON content type so the editor
            // renders with syntax highlighting + the
            // paragraph status bar shows `[hjson]` to
            // match the rest of the project's HJSON
            // configuration paragraphs.  Mutating
            // `node.content_type` before
            // `update_paragraph_content` lets the
            // metadata write inside that call persist
            // the change.
            overview.content_type = Some("hjson".to_string());
            // `update_paragraph_content` only writes
            // to bdslib — the on-disk `.typ` file
            // (already created with the default
            // `= overview\n\n` template by
            // `create_node`) needs an explicit
            // overwrite so the editor (which reads
            // from disk) sees the seeded body.  Same
            // pattern `ensure_system_books` uses for
            // its seeded paragraphs.
            if let Some(rel) = &overview.file {
                let abs = store.project_root().join(rel);
                std::fs::write(&abs, META_OVERVIEW_BODY.as_bytes())
                    .map_err(|e| Error::Store(format!("write overview: {e}")))?;
            }
            store
                .update_paragraph_content(&mut overview, META_OVERVIEW_BODY.as_bytes())
                .map_err(|e| Error::Store(format!("seed overview: {e}")))?;
        }
    }
    Ok(())
}

/// 1.2.13+ Phase B — `inkhaven language add-word`.
/// Resolves the target language sub-book by case-
/// insensitive title; finds its Dictionary chapter;
/// derives the alphabet bucket for the new word from
/// the first character (auto-creates the subchapter
/// when missing); rejects duplicate words.
fn add_word(
    project: &Path,
    language: &str,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    let hierarchy = Hierarchy::load(&store)?;
    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it"
                    .into(),
            )
        })?
        .clone();

    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    let (entry, bucket) = add_dictionary_entry_impl(
        &store,
        &cfg,
        &lang_book,
        word,
        pos,
        translation,
        example,
    )?;
    let _ = entry;
    eprintln!(
        "added `{word}` to `{language}/Dictionary/{bucket}` ({pos} · {translation})"
    );
    Ok(())
}

/// 1.2.13+ Phase D.1 hotfix — shared "add dictionary
/// entry" implementation used by:
///   * the CLI `add-word` subcommand (above);
///   * the in-TUI tree-pane Add Paragraph (`+`) commit
///     handler when the cursor sits anywhere under
///     `Language/<lang>/Dictionary`.
///
/// Caller supplies the per-language Book; we look up
/// the Dictionary chapter, derive the alphabet bucket
/// (consulting Meta/overview first, first-char
/// uppercase as fallback), find-or-create the bucket
/// subchapter, reject duplicates, create the entry
/// paragraph, and seed its body with the HJSON
/// template (POS / translation / example fields are
/// left empty in the TUI flow — the author fills them
/// in by editing the paragraph).
///
/// Returns `(entry_node, bucket_name)` so callers can
/// surface a status message or move the tree cursor.
pub(crate) fn add_dictionary_entry_impl(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> Result<(crate::store::node::Node, String)> {
    let body = seed_dictionary_entry_body(word, pos, translation, example);
    create_dictionary_entry(store, cfg, lang_book, word, &body)
}

/// 1.2.13+ Phase D.1 — fully-populated entry record
/// used by the CSV import path.  Distinct from the
/// `language_entry::DictionaryEntry` parser type
/// because we own this one (mutable builder) and the
/// parser one is immutable (deserialised view).
#[derive(Debug, Default, Clone)]
pub(crate) struct ImportEntry {
    pub word: String,
    pub pos: String,
    pub translation: String,
    pub example: String,
    pub pronunciation: String,
    pub etymology: String,
    pub related: Vec<String>,
    pub inflection: std::collections::BTreeMap<String, String>,
    pub examples: Vec<String>,
    pub register: String,
    pub era: String,
    pub notes: String,
}

/// Add a fully-populated dictionary entry from an
/// import row.  Bypasses the verbose commented seed
/// template and writes compact HJSON with only the
/// populated fields.  Shares the bucket-derivation +
/// duplicate-check + persistence machinery with the
/// interactive `add_dictionary_entry_impl`.
pub(crate) fn add_imported_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    entry: &ImportEntry,
) -> Result<(crate::store::node::Node, String)> {
    let body = build_imported_entry_body(entry);
    create_dictionary_entry(store, cfg, lang_book, &entry.word, &body)
}

/// Shared bucket-derivation + duplicate-check +
/// node-creation + disk/bdslib persistence for both
/// the interactive and bulk-import paths.  Body is
/// passed verbatim — callers pick whether they want
/// the verbose commented template or a compact
/// concrete entry.
fn create_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    body: &str,
) -> Result<(crate::store::node::Node, String)> {
    let hierarchy = Hierarchy::load(store)?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{}` has no `Dictionary` chapter — likely scaffolded with a pre-Phase-A inkhaven",
                lang_book.title
            ))
        })?;
    let bucket = derive_alphabet_bucket(store, &hierarchy, lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;
    let dictionary_kids = hierarchy.children_of(Some(dictionary.id));
    let subchapter = match dictionary_kids
        .iter()
        .find(|n| n.kind == NodeKind::Subchapter && n.title == bucket)
        .cloned()
    {
        Some(existing) => existing.clone(),
        None => {
            let hierarchy = Hierarchy::load(store)?;
            store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Subchapter,
                &bucket,
                Some(&dictionary),
                None,
                InsertPosition::End,
            )?
        }
    };
    let hierarchy = Hierarchy::load(store)?;
    if hierarchy
        .children_of(Some(subchapter.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(word))
    {
        return Err(Error::Config(format!(
            "word `{word}` already defined under `{}/Dictionary/{bucket}`",
            lang_book.title
        )));
    }
    let hierarchy = Hierarchy::load(store)?;
    let mut entry = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        word,
        Some(&subchapter),
        None,
        InsertPosition::End,
    )?;
    entry.content_type = Some("hjson".to_string());
    if let Some(rel) = &entry.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write entry: {e}")))?;
    }
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed entry: {e}")))?;
    Ok((entry, bucket))
}

/// 1.2.13+ Phase D.1 — compact concrete HJSON for an
/// imported entry.  Emits ONLY the fields the import
/// row actually populated; skips empty optional
/// fields entirely so the resulting paragraph reads
/// cleanly when the author opens it.
fn build_imported_entry_body(entry: &ImportEntry) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  word:         \"{}\"\n", escape_hjson(&entry.word)));
    out.push_str(&format!("  type:         \"{}\"\n", escape_hjson(&entry.pos)));
    out.push_str(&format!(
        "  translation:  \"{}\"\n",
        escape_hjson(&entry.translation)
    ));
    if !entry.example.is_empty() {
        out.push_str(&format!(
            "  example:      \"{}\"\n",
            escape_hjson(&entry.example)
        ));
    }
    if !entry.examples.is_empty() {
        out.push_str("  examples: [\n");
        for ex in &entry.examples {
            out.push_str(&format!("    \"{}\"\n", escape_hjson(ex)));
        }
        out.push_str("  ]\n");
    }
    if !entry.pronunciation.is_empty() {
        out.push_str(&format!(
            "  pronunciation: \"{}\"\n",
            escape_hjson(&entry.pronunciation)
        ));
    }
    if !entry.etymology.is_empty() {
        out.push_str(&format!(
            "  etymology:    \"{}\"\n",
            escape_hjson(&entry.etymology)
        ));
    }
    if !entry.related.is_empty() {
        let items: Vec<String> = entry
            .related
            .iter()
            .map(|r| format!("\"{}\"", escape_hjson(r)))
            .collect();
        out.push_str(&format!("  related:      [{}]\n", items.join(", ")));
    }
    if !entry.inflection.is_empty() {
        out.push_str("  inflection: {\n");
        for (k, v) in &entry.inflection {
            out.push_str(&format!(
                "    {}: \"{}\"\n",
                k,
                escape_hjson(v)
            ));
        }
        out.push_str("  }\n");
    }
    if !entry.register.is_empty() {
        out.push_str(&format!(
            "  register:     \"{}\"\n",
            escape_hjson(&entry.register)
        ));
    }
    if !entry.era.is_empty() {
        out.push_str(&format!("  era:          \"{}\"\n", escape_hjson(&entry.era)));
    }
    if !entry.notes.is_empty() {
        out.push_str(&format!(
            "  notes:        \"{}\"\n",
            escape_hjson(&entry.notes)
        ));
    }
    out.push_str("}\n");
    out
}

/// 1.2.13+ Phase D.1 hotfix — seed body for a grammar
/// rule paragraph created in the TUI.  Mirrors the
/// proposal §4 schema so future Phase D.2 work
/// (`--format grammar` exporter, `language define-rule`
/// CLI) can parse it the same way the dictionary entry
/// parser handles entries today.  Authors edit the
/// HJSON to fill in `category`, `applies_when`, etc.
pub(crate) const GRAMMAR_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier the AI translation prompt references
  // in applied-rules lists.  Lowercase + hyphens.
  // Example: \"noun-case-system\",
  // \"verb-tense-aspect\", \"reduplication\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 grammar export
  // sectioning AND the in-prompt grouping.
  //   morphology   — word-formation, inflection
  //   syntax       — clause structure, word order
  //   phonology    — sound rules
  //   orthography  — spelling conventions
  //   semantics    — meaning relationships
  //   pragmatics   — usage / discourse rules
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // Plain text inside an HJSON multi-line string;
  // tabular layouts work fine.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  This text is fed
    verbatim to the AI translator at translation
    time, so be explicit:

      * State the input → output transformation.
      * Show the morpheme boundaries (- or .).
      * Show ALL exceptions inline so the LLM
        doesn't have to guess.

    Example layout for a case system:

      NOM: zero suffix.   aran     (king)
      ACC: -n.             aran → aranin
      DAT: -en.            aran → aranen
      GEN: -o.             aran → arano
  '''

  // ──────────────────────────────────────────────────
  // FEW-SHOT EXAMPLES — bundled into the translation
  // prompt envelope so the LLM sees the rule applied.
  // ──────────────────────────────────────────────────

  examples: [
    // { source: \"\",  target: \"\",  gloss: \"\" }
    // { source: \"\",  target: \"\",  gloss: \"\" }
  ]

  // ──────────────────────────────────────────────────
  // RAG TRIGGERING — when this rule should be
  // included in the translation prompt envelope.
  // ──────────────────────────────────────────────────

  // Plain-language condition the LLM evaluates
  // against the source sentence.  Tight applies_when
  // keeps the prompt focused (Phase C envelope
  // includes only matching rules; default cap is 6).
  applies_when: \"\"

  // Sibling rules this one builds on, by rule_id.
  // The RAG layer pulls dependent rules
  // automatically.  Example: a verb-conjugation
  // rule depends on the stem-formation rule.
  depends_on:   []

  // Rules that conflict with this one — only one
  // should fire per translation pass.  Phase D.2
  // `language doctor` will surface conflicting
  // pairs that lack an `applies_when` disambiguator.
  conflicts_with: []

  // ──────────────────────────────────────────────────
  // METADATA / NOTES
  // ──────────────────────────────────────────────────

  // Productivity — how broadly the rule applies.
  // \"core\"        — fires on most sentences
  // \"common\"      — fires on a recognisable
  //                  subset of constructions
  // \"specialised\" — narrow / register-bound
  // \"vestigial\"   — historical residue only
  productivity: \"\"

  // Register / style restrictions, if any:
  // formal | informal | literary | sacred | archaic.
  register:     \"\"

  // Author's notes — historical motivation,
  // worldbuilding rationale, comparison to natural-
  // language analogues.  Not read by the LLM.
  notes:        \"\"
}
";

/// 1.2.13+ Phase D.1 hotfix — seed body for a
/// phonology rule paragraph.  Lighter than the
/// grammar template because phonology rules tend to
/// be more declarative (allowed onsets, vowel
/// harmony patterns) than triggered.
pub(crate) const PHONOLOGY_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier — lowercase + hyphens.  Referenced by
  // grammar rules' `depends_on` field and by the
  // phonotactic generator (`Ctrl+B Shift+W` in the
  // Language book — Phase D.2).
  // Examples: \"consonant-inventory\",
  // \"vowel-harmony\", \"syllable-template\",
  // \"intervocalic-voicing\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 phonology export
  // sectioning AND the phonotactic generator's
  // weighting.
  //   consonants     — IPA inventory of consonants
  //   vowels         — IPA inventory of vowels
  //   phonotactics   — allowed onset / nucleus / coda
  //   syllable       — syllable template (CV, CVC, …)
  //   stress         — stress placement rule
  //   tone           — tonal system / pitch rules
  //   sound-changes  — historical or allophonic shifts
  //   prosody        — intonation / rhythm patterns
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  Use IPA inside
    /slashes/ for phonemic and [brackets] for
    phonetic.

    Example layouts:

      Phonotactic template:
        ONSET: zero | C | CC (only stop+liquid)
        NUCLEUS: V | VV (long vowels)
        CODA: zero | C | CC (limited to /s, n, r, l/)

      Sound change:
        /s/ → [z] / V_V (intervocalic voicing)

      Vowel harmony:
        Front vowels {i, e} co-occur in roots;
        back vowels {a, o, u} co-occur in roots;
        suffixes harmonise with the root.
  '''

  // ──────────────────────────────────────────────────
  // INVENTORIES — for consonants / vowels categories.
  // ──────────────────────────────────────────────────

  // List of phonemes (IPA strings).  Optional; used
  // by the phonotactic generator to constrain output.
  // phonemes:     []

  // Allophonic variants by environment.  Map of
  // phoneme → list of (environment, realisation).
  // allophones:   {}

  // ──────────────────────────────────────────────────
  // ENVIRONMENT — for sound-changes / allophony.
  // ──────────────────────────────────────────────────

  // Where the rule applies (LLM evaluates against the
  // source's phonetic context).
  // environment:  \"\"

  // ──────────────────────────────────────────────────
  // EXAMPLES — IPA pairs showing the rule in action.
  // ──────────────────────────────────────────────────

  examples: [
    // { input: \"\", output: \"\", gloss: \"\" }
  ]

  // Known exceptions — words / morphemes where the
  // rule does NOT apply.
  exceptions: []

  // ──────────────────────────────────────────────────
  // NOTES
  // ──────────────────────────────────────────────────

  // Register / style restrictions, if any.
  register:     \"\"

  // Author's notes — historical motivation, source
  // dialect, comparison to natural-language analogues.
  notes:        \"\"
}
";

/// Derive the alphabet-bucket subchapter name for a
/// word.  Uses the first non-whitespace character,
/// uppercased.  Returns `None` only if the input is
/// entirely whitespace — alphanumeric, Cyrillic,
/// Greek, hyphen / apostrophe-prefix all map to
/// their leading letter or symbol.
fn alphabet_bucket(word: &str) -> Option<String> {
    let ch = word.chars().find(|c| !c.is_whitespace())?;
    Some(ch.to_uppercase().to_string())
}

/// Consult the language sub-book's `Meta/overview`
/// HJSON for the alphabet-bucket name.  The author's
/// declared groupings override the naive first-char
/// uppercase (Phase B's fallback).  Returns:
///   * `Ok(Some(bucket))` — declared alphabet covers
///     the word's first character.
///   * `Ok(None)` — Meta chapter missing, overview
///     paragraph missing, HJSON block absent, alphabet
///     list empty, or first char not in any declared
///     entry.  Caller falls back to `alphabet_bucket`.
///   * `Err` — HJSON parse failure or store IO error.
///     Surfaced rather than swallowed so a malformed
///     overview is noisy enough to fix.
fn derive_alphabet_bucket(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    word: &str,
) -> Result<Option<String>> {
    let Some(meta_chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(overview) = hierarchy
        .children_of(Some(meta_chapter.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case("overview")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(bytes) = store.get_content(overview.id)? else {
        return Ok(None);
    };
    let body = std::str::from_utf8(&bytes).map_err(|e| {
        Error::Config(format!("Meta/overview body is not UTF-8: {e}"))
    })?;
    let meta = match crate::language_entry::parse_meta_overview(body)
        .map_err(Error::Config)?
    {
        Some(m) => m,
        None => return Ok(None),
    };
    Ok(meta.bucket_for_word(word).map(|s| s.to_string()))
}

/// Build the seeded body for a freshly-added
/// dictionary entry.  Pure HJSON — no Typst wrappers
/// — so the editor renders with HJSON syntax
/// highlighting.  The paragraph's `content_type` is
/// set to `"hjson"` at create time.
///
/// 1.2.13+ Phase D.1 hotfix — switched FROM Typst-
/// with-fenced-HJSON to pure HJSON.  The translation
/// prompt composer + parser handle both formats; new
/// entries use pure HJSON.
fn seed_dictionary_entry_body(
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> String {
    let example_value = example.unwrap_or("").trim();
    format!(
        "{{\n  \
         // ──────────────────────────────────────────────────\n  \
         // CORE — required for the entry to function as a\n  \
         // lexicon-overlay target + translation-prompt source.\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         word:         \"{word}\"\n  \
         \n  \
         // Part of speech.  Free-form string; the\n  \
         // proposal suggests: noun | verb | adjective |\n  \
         // adverb | pronoun | preposition | conjunction |\n  \
         // interjection | particle.  Language-specific\n  \
         // categories (\"classifier\", \"evidential\",\n  \
         // \"applicative\") are fine.\n  \
         type:         \"{pos}\"\n  \
         \n  \
         // Working-language gloss — what this word\n  \
         // means in the project's `language` (the value\n  \
         // the AI translator maps to/from).\n  \
         translation:  \"{translation}\"\n  \
         \n  \
         // Canonical sample sentence the author wants\n  \
         // frozen into the entry.  Becomes few-shot\n  \
         // anchor data in the translation prompt.\n  \
         example:      \"{example}\"\n  \
         \n  \
         // ──────────────────────────────────────────────────\n  \
         // OPTIONAL — uncomment and fill the ones you need.\n  \
         // Each is consumed by either the translation\n  \
         // prompt envelope (Phase C) or the future\n  \
         // dictionary card renderer (Phase D.2).\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         // Additional example sentences beyond the\n  \
         // canonical one.  Phase C translation flow\n  \
         // uses every example as few-shot data.\n  \
         // examples:     [\n  \
         //   \"\"\n  \
         //   \"\"\n  \
         // ]\n  \
         \n  \
         // IPA transcription (between slashes for\n  \
         // phonemic, brackets for phonetic).\n  \
         // pronunciation: \"\"\n  \
         \n  \
         // Etymology / derivation.  Plain text or\n  \
         // [[wikilink]] style cross-reference to a\n  \
         // proto-form entry.\n  \
         // etymology:    \"\"\n  \
         \n  \
         // Cross-references to sibling entries — other\n  \
         // words in this language that share roots,\n  \
         // contrast in register, or commonly co-occur.\n  \
         // related:      []\n  \
         \n  \
         // Paradigm forms.  Every VALUE here gets\n  \
         // added to the lexicon overlay so inflected\n  \
         // words light up in prose alongside the\n  \
         // lemma.  KEY names are free-form and feed\n  \
         // the translation prompt as paradigm hints.\n  \
         // inflection:   {{\n  \
         //   plural:     \"\"\n  \
         //   genitive:   \"\"\n  \
         //   accusative: \"\"\n  \
         //   dative:     \"\"\n  \
         //   ablative:   \"\"\n  \
         // }}\n  \
         \n  \
         // Register / style: formal | informal |\n  \
         // archaic | literary | colloquial | sacred.\n  \
         // register:     \"\"\n  \
         \n  \
         // Era — when the word entered the language.\n  \
         // Useful for historical-fiction projects.\n  \
         // era:          \"\"\n  \
         \n  \
         // Auto-tracked count of mentions in the\n  \
         // manuscript.  Phase D.2 `language doctor`\n  \
         // updates this; leave 0 for now.\n  \
         // frequency:    0\n  \
         \n  \
         // Free-form usage notes — register cues,\n  \
         // taboos, mnemonic etymology, whatever\n  \
         // helps you remember the word.\n  \
         notes:        \"\"\n\
         }}\n",
        word = escape_hjson(word),
        pos = escape_hjson(pos),
        translation = escape_hjson(translation),
        example = escape_hjson(example_value),
    )
}

/// Minimal HJSON string escape — backslash-quote +
/// backslash-backslash.  Sufficient for the
/// dictionary-entry seed body, which never sees
/// control characters in practice.
fn escape_hjson(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 1.2.13+ Phase D — health report for a language
/// sub-book.  Walks every chapter, counts entries +
/// rules + samples, computes coverage metrics, and
/// emits a human-readable summary on stdout.  Exit
/// code 0 always — informational, not a gate.
///
/// Coverage gap analysis (§13 of the proposal):
///   * count manuscript words (working language) that
///     don't appear as translations in this language's
///     dictionary.  Surfaces vocabulary the author has
///     written in prose but hasn't yet defined a
///     translation for.
///   * count dictionary entries that lack examples —
///     half-finished work.
///   * count entries that lack inflection paradigms —
///     hint that the lexicon overlay won't catch
///     inflected forms for those words.
fn doctor(project: &Path, language: &str, json: bool) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    // Walk each chapter's paragraphs.  We don't reach
    // for the in-memory TUI helpers because doctor /
    // export need to run from a headless CLI process.
    let chapters = hierarchy.children_of(Some(lang_book.id));
    let mut dict_entries: Vec<(String, crate::language_entry::DictionaryEntry)> =
        Vec::new();
    let mut dict_unparseable = 0usize;
    let mut grammar_count = 0usize;
    let mut phonology_count = 0usize;
    let mut sample_count = 0usize;
    let mut meta: Option<crate::language_entry::MetaOverview> = None;
    for chapter in &chapters {
        let title_lc = chapter.title.to_lowercase();
        let paragraphs: Vec<_> = hierarchy
            .collect_subtree(chapter.id)
            .into_iter()
            .filter_map(|id| hierarchy.get(id))
            .filter(|n| n.kind == NodeKind::Paragraph)
            .cloned()
            .collect();
        match title_lc.as_str() {
            "dictionary" => {
                for p in &paragraphs {
                    let Ok(Some(bytes)) = store.get_content(p.id) else {
                        continue;
                    };
                    let Ok(body) = std::str::from_utf8(&bytes) else {
                        continue;
                    };
                    match crate::language_entry::parse(body) {
                        Ok(Some(e)) => dict_entries.push((p.title.clone(), e)),
                        Ok(None) => dict_unparseable += 1,
                        Err(_) => dict_unparseable += 1,
                    }
                }
            }
            "grammar" => grammar_count = paragraphs.len(),
            "phonology" => phonology_count = paragraphs.len(),
            "sample texts" => sample_count = paragraphs.len(),
            "meta" => {
                for p in &paragraphs {
                    if p.title.eq_ignore_ascii_case("overview") {
                        let Ok(Some(bytes)) = store.get_content(p.id) else {
                            continue;
                        };
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            if let Ok(Some(m)) =
                                crate::language_entry::parse_meta_overview(body)
                            {
                                meta = Some(m);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let total_entries = dict_entries.len();
    let with_examples = dict_entries
        .iter()
        .filter(|(_, e)| !e.example.trim().is_empty())
        .count();
    let with_inflection = dict_entries
        .iter()
        .filter(|(_, e)| !e.inflection.is_empty())
        .count();
    let missing_examples = total_entries.saturating_sub(with_examples);
    let missing_inflection = total_entries.saturating_sub(with_inflection);

    // Coverage-gap analysis: which working-language
    // words in the manuscript have no dictionary
    // translation?  Walk every paragraph in user
    // books (skip system books — Notes / Places /
    // Characters / Artefacts / Prompts / Language /
    // Typst are reference material, not manuscript
    // prose) and collect their words.
    use unicode_segmentation::UnicodeSegmentation;
    let dictionary_translations: std::collections::HashSet<String> = dict_entries
        .iter()
        .filter_map(|(_, e)| {
            let t = e.translation.trim().to_lowercase();
            if t.is_empty() { None } else { Some(t) }
        })
        .collect();
    let mut manuscript_words: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        // Skip system-book content.
        let mut cursor = Some(node.id);
        let mut is_system = false;
        while let Some(id) = cursor {
            if let Some(n) = hierarchy.get(id) {
                if n.system_tag.is_some() {
                    is_system = true;
                    break;
                }
                cursor = n.parent_id;
            } else {
                break;
            }
        }
        if is_system {
            continue;
        }
        if let Ok(Some(bytes)) = store.get_content(node.id) {
            if let Ok(body) = std::str::from_utf8(&bytes) {
                for w in UnicodeSegmentation::unicode_words(body) {
                    let lc = w.to_lowercase();
                    // Stop-word-ish filter: drop
                    // 1-letter "words" (a, I) — most
                    // are noise; the rest are too
                    // common to be worth flagging.
                    if lc.chars().count() < 2 {
                        continue;
                    }
                    manuscript_words.insert(lc);
                }
            }
        }
    }
    let manuscript_word_count = manuscript_words.len();
    let undefined_words: Vec<String> = manuscript_words
        .difference(&dictionary_translations)
        .cloned()
        .collect();

    // 1.2.13+ Phase D.1 — JSON mode emits the same
    // numbers in a structured form so CI pipelines
    // can gate on `coverage.with_example_pct < 80`
    // etc.  Returns early; the text render below
    // stays unchanged.
    if json {
        use serde_json::{json, Map, Value};
        let mut sorted_undefined: Vec<String> =
            undefined_words.iter().take(50).cloned().collect();
        sorted_undefined.sort();
        let example_pct = if total_entries > 0 {
            with_examples * 100 / total_entries
        } else {
            0
        };
        let inflection_pct = if total_entries > 0 {
            with_inflection * 100 / total_entries
        } else {
            0
        };
        let coverage_pct = if manuscript_word_count > 0 {
            manuscript_word_count.saturating_sub(undefined_words.len()) * 100
                / manuscript_word_count
        } else {
            0
        };
        let mut report = Map::new();
        report.insert("language".into(), Value::String(lang_book.title.clone()));
        report.insert(
            "meta".into(),
            meta.as_ref()
                .map(|m| json!({
                    "name": m.name,
                    "language_kind": m.language_kind,
                    "family": m.family,
                    "iso_code": m.iso_code,
                    "alphabet_count": m.alphabet.len(),
                    "reading_direction": m.reading_direction,
                }))
                .unwrap_or(Value::Null),
        );
        report.insert(
            "chapters".into(),
            json!({
                "dictionary_parseable": total_entries,
                "dictionary_unparseable": dict_unparseable,
                "grammar": grammar_count,
                "phonology": phonology_count,
                "sample_texts": sample_count,
            }),
        );
        report.insert(
            "coverage".into(),
            json!({
                "with_example": with_examples,
                "with_example_pct": example_pct,
                "with_paradigm": with_inflection,
                "with_paradigm_pct": inflection_pct,
                "missing_example": missing_examples,
                "missing_paradigm": missing_inflection,
            }),
        );
        report.insert(
            "manuscript_gap".into(),
            json!({
                "unique_words": manuscript_word_count,
                "uncovered_count": undefined_words.len(),
                "coverage_pct": coverage_pct,
                "uncovered_sample": sorted_undefined,
            }),
        );
        let s = serde_json::to_string_pretty(&Value::Object(report))
            .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
        println!("{s}");
        return Ok(());
    }

    // Emit the human-readable report.
    println!("Language doctor — `{}`", lang_book.title);
    println!();
    if let Some(m) = meta.as_ref() {
        if !m.name.is_empty() {
            println!("  name           : {}", m.name);
        }
        if !m.language_kind.is_empty() {
            println!("  kind           : {}", m.language_kind);
        }
        if !m.family.is_empty() {
            println!("  family         : {}", m.family);
        }
        if !m.iso_code.is_empty() {
            println!("  iso_code       : {}", m.iso_code);
        }
        if !m.alphabet.is_empty() {
            println!("  alphabet       : {} entries", m.alphabet.len());
        }
        if !m.reading_direction.is_empty() {
            println!("  direction      : {}", m.reading_direction);
        }
        println!();
    } else {
        println!("  Meta/overview  : MISSING or unparseable");
        println!();
    }
    println!("Chapters");
    println!("  Dictionary     : {total_entries} parseable entries");
    if dict_unparseable > 0 {
        println!(
            "                   {dict_unparseable} unparseable (no HJSON block — pre-Phase-B authoring)"
        );
    }
    println!("  Grammar        : {grammar_count} rules");
    println!("  Phonology      : {phonology_count} rules");
    println!("  Sample texts   : {sample_count} samples");
    println!();
    println!("Dictionary coverage");
    if total_entries > 0 {
        let example_pct = with_examples * 100 / total_entries;
        let inflection_pct = with_inflection * 100 / total_entries;
        println!(
            "  with example   : {with_examples}/{total_entries} ({example_pct}%)"
        );
        println!(
            "  with paradigm  : {with_inflection}/{total_entries} ({inflection_pct}%)"
        );
        if missing_examples > 0 {
            println!("  missing example: {missing_examples}");
        }
        if missing_inflection > 0 {
            println!(
                "  missing paradigm: {missing_inflection} (overlay won't catch inflected forms)"
            );
        }
    } else {
        println!("  no dictionary entries yet — try `inkhaven language add-word`");
    }
    println!();
    println!("Manuscript gap analysis");
    println!("  unique words (≥2 chars) in manuscript prose: {manuscript_word_count}");
    let undefined_count = undefined_words.len();
    if total_entries > 0 {
        let covered = manuscript_word_count.saturating_sub(undefined_count);
        let pct = if manuscript_word_count > 0 {
            covered * 100 / manuscript_word_count
        } else {
            0
        };
        println!("  covered by dictionary: {covered}/{manuscript_word_count} ({pct}%)");
        if undefined_count > 0 {
            println!("  uncovered words (sample, max 15):");
            let mut sample: Vec<&String> = undefined_words.iter().take(15).collect();
            sample.sort();
            for w in sample {
                println!("    · {w}");
            }
            if undefined_count > 15 {
                println!("    ... and {} more", undefined_count - 15);
            }
        }
    } else {
        println!("  (skipping — no dictionary entries to compare against)");
    }
    Ok(())
}

/// 1.2.13+ Phase D — export a language's content
/// to a portable artefact.  Three formats land in
/// Phase D; `grammar` and `phrasebook` from the
/// proposal §12 are deferred to D.2.
fn export(
    project: &Path,
    language: &str,
    format: LanguageExportFormat,
    output: Option<&Path>,
) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found"
            ))
        })?;

    // Collect data once; per-format renderers fan
    // out from a single walk.
    let chapters = hierarchy.children_of(Some(lang_book.id));
    let mut entries: Vec<(String, crate::language_entry::DictionaryEntry)> = Vec::new();
    let mut meta: Option<crate::language_entry::MetaOverview> = None;
    let mut grammar_bodies: Vec<(String, String)> = Vec::new();
    let mut phonology_bodies: Vec<(String, String)> = Vec::new();
    let mut sample_bodies: Vec<(String, String)> = Vec::new();
    for chapter in &chapters {
        let title_lc = chapter.title.to_lowercase();
        // For Dictionary, walk the subtree (entries
        // live one level deeper, under the alphabet
        // subchapter).  For the flat chapters
        // (Grammar / Phonology / Sample texts / Meta),
        // a children_of(chapter) is enough.
        match title_lc.as_str() {
            "dictionary" => {
                for id in hierarchy.collect_subtree(chapter.id) {
                    let Some(n) = hierarchy.get(id) else { continue; };
                    if n.kind != NodeKind::Paragraph {
                        continue;
                    }
                    let Ok(Some(bytes)) = store.get_content(n.id) else { continue; };
                    let Ok(body) = std::str::from_utf8(&bytes) else { continue; };
                    if let Ok(Some(e)) = crate::language_entry::parse(body) {
                        entries.push((n.title.clone(), e));
                    }
                }
            }
            "grammar" | "phonology" | "sample texts" => {
                let bucket = match title_lc.as_str() {
                    "grammar" => &mut grammar_bodies,
                    "phonology" => &mut phonology_bodies,
                    _ => &mut sample_bodies,
                };
                for n in hierarchy
                    .children_of(Some(chapter.id))
                    .into_iter()
                    .filter(|n| n.kind == NodeKind::Paragraph)
                {
                    if let Ok(Some(bytes)) = store.get_content(n.id) {
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            bucket.push((n.title.clone(), body.to_string()));
                        }
                    }
                }
            }
            "meta" => {
                if let Some(overview) = hierarchy
                    .children_of(Some(chapter.id))
                    .into_iter()
                    .find(|n| {
                        n.kind == NodeKind::Paragraph
                            && n.title.eq_ignore_ascii_case("overview")
                    })
                {
                    if let Ok(Some(bytes)) = store.get_content(overview.id) {
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            if let Ok(Some(m)) =
                                crate::language_entry::parse_meta_overview(body)
                            {
                                meta = Some(m);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Sort entries by lemma so every format renders
    // in a stable order.
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let rendered: Vec<u8> = match format {
        LanguageExportFormat::Json => render_json(
            &lang_book.title,
            meta.as_ref(),
            &entries,
            &grammar_bodies,
            &phonology_bodies,
            &sample_bodies,
        )?,
        LanguageExportFormat::Anki => render_anki(&entries)?,
        LanguageExportFormat::DictionaryTwocol => render_dictionary_twocol(
            &lang_book.title,
            meta.as_ref(),
            &entries,
        ),
    };

    match (output, format) {
        (Some(path), _) => {
            std::fs::write(path, &rendered).map_err(|e| {
                Error::Config(format!("write {}: {e}", path.display()))
            })?;
            eprintln!("wrote {} bytes to {}", rendered.len(), path.display());
        }
        (None, LanguageExportFormat::DictionaryTwocol) => {
            return Err(Error::Config(
                "dictionary-twocol export needs --output <path.typ> — \
                 the Typst renderer doesn't stream to stdout"
                    .into(),
            ));
        }
        (None, _) => {
            use std::io::Write;
            std::io::stdout()
                .write_all(&rendered)
                .map_err(|e| Error::Config(format!("stdout write: {e}")))?;
        }
    }
    Ok(())
}

fn render_json(
    language_name: &str,
    meta: Option<&crate::language_entry::MetaOverview>,
    entries: &[(String, crate::language_entry::DictionaryEntry)],
    grammar: &[(String, String)],
    phonology: &[(String, String)],
    samples: &[(String, String)],
) -> Result<Vec<u8>> {
    use serde_json::{json, Map, Value};
    let mut root = Map::new();
    root.insert("language".into(), Value::String(language_name.to_string()));
    if let Some(m) = meta {
        root.insert("meta".into(), json!({
            "name": m.name,
            "language_kind": m.language_kind,
            "family": m.family,
            "iso_code": m.iso_code,
            "alphabet": m.alphabet,
            "reading_direction": m.reading_direction,
            "stemmer": m.stemmer,
            "example_corpus_ref": m.example_corpus_ref,
        }));
    }
    let entries_json: Vec<Value> = entries
        .iter()
        .map(|(title, e)| {
            json!({
                "title": title,
                "word": e.word,
                "type": e.pos,
                "translation": e.translation,
                "example": e.example,
                "inflection": e.inflection,
            })
        })
        .collect();
    root.insert("dictionary".into(), Value::Array(entries_json));
    root.insert(
        "grammar".into(),
        Value::Array(
            grammar
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    root.insert(
        "phonology".into(),
        Value::Array(
            phonology
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    root.insert(
        "sample_texts".into(),
        Value::Array(
            samples
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    let mut buf = serde_json::to_vec_pretty(&Value::Object(root))
        .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
    buf.push(b'\n');
    Ok(buf)
}

fn render_anki(
    entries: &[(String, crate::language_entry::DictionaryEntry)],
) -> Result<Vec<u8>> {
    // CSV columns: word, translation, type, example,
    // inflection.  Anki / SuperMemo / Mochi all parse
    // comma-separated; quoting handled by the
    // standard escape rules.  Header row included so
    // the user can map columns in the import wizard.
    let mut out = String::new();
    out.push_str("word,translation,type,example,inflection\n");
    for (_, e) in entries {
        let infl: String = e
            .inflection
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_field(&e.word),
            csv_field(&e.translation),
            csv_field(&e.pos),
            csv_field(&e.example),
            csv_field(&infl),
        ));
    }
    Ok(out.into_bytes())
}

/// Standard RFC 4180-style CSV quoting: wrap the
/// field in `"…"` and double any embedded `"` when
/// the field contains comma / newline / quote;
/// otherwise emit verbatim.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_dictionary_twocol(
    language_name: &str,
    meta: Option<&crate::language_entry::MetaOverview>,
    entries: &[(String, crate::language_entry::DictionaryEntry)],
) -> Vec<u8> {
    // Group entries by alphabet bucket.  Use the
    // first character of the entry's title
    // (uppercased) as the bucket key — same logic as
    // the add-word fallback.  Authors with non-
    // Latin alphabets get sensible grouping for free.
    let mut by_bucket: std::collections::BTreeMap<String, Vec<&(String, crate::language_entry::DictionaryEntry)>> =
        std::collections::BTreeMap::new();
    for entry in entries {
        let bucket = entry
            .0
            .chars()
            .find(|c| !c.is_whitespace())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into());
        by_bucket.entry(bucket).or_default().push(entry);
    }

    let mut s = String::new();
    s.push_str(&format!("#set page(paper: \"a4\", columns: 2)\n"));
    s.push_str("#set text(font: \"New Computer Modern\", size: 10pt)\n");
    s.push_str("#set par(justify: true)\n");
    s.push('\n');
    s.push_str(&format!("#align(center)[= {} dictionary]\n", language_name));
    if let Some(m) = meta {
        if !m.language_kind.is_empty() || !m.family.is_empty() {
            s.push_str("#align(center)[#text(style: \"italic\")[");
            if !m.language_kind.is_empty() {
                s.push_str(&m.language_kind);
            }
            if !m.family.is_empty() {
                if !m.language_kind.is_empty() {
                    s.push_str(" · ");
                }
                s.push_str(&m.family);
            }
            s.push_str("]]\n");
        }
    }
    s.push('\n');
    for (bucket, group) in &by_bucket {
        s.push_str(&format!(
            "#align(center)[#text(size: 14pt, weight: \"bold\")[— {bucket} —]]\n"
        ));
        s.push('\n');
        for (title, e) in group {
            s.push_str(&format!(
                "*{title}*  #text(style: \"italic\")[{}]  {}\n",
                typst_escape(&e.pos),
                typst_escape(&e.translation),
            ));
            if !e.example.trim().is_empty() {
                s.push_str(&format!(
                    "  #pad(left: 2em)[#text(style: \"italic\")[{}]]\n",
                    typst_escape(e.example.trim()),
                ));
            }
            if !e.inflection.is_empty() {
                let pretty: Vec<String> = e
                    .inflection
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect();
                s.push_str(&format!(
                    "  #pad(left: 2em)[#text(size: 8pt)[forms — {}]]\n",
                    typst_escape(&pretty.join(", ")),
                ));
            }
            s.push('\n');
        }
    }
    s.into_bytes()
}

/// Minimal Typst-content escape: `*`, `_`, `#`, `[`,
/// `]`, `\` are the only markup-bearing
/// characters in body-text context.  Sufficient for
/// dictionary-entry content; authors with
/// adversarial input (raw Typst inside translations)
/// should use the `json` format instead.
fn typst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '_' | '#' | '[' | ']' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 1.2.13+ Phase D.1 — `inkhaven language list`.
/// Walks the `Language` system book and emits one
/// row per language with summary counts.  Quick
/// at-a-glance complement to `language doctor`.
/// 1.2.13+ Phase D.1 — `inkhaven language add-word
/// <lang> --import <path.csv>`.  Bulk-load a CSV
/// dictionary.  Format described in the CLI variant
/// docstring; mechanically:
///   * RFC 4180 quoting (`"…"` for fields with
///     commas / quotes / newlines; `""` for embedded
///     quotes).
///   * Header row maps column NAMES to row positions
///     so the CSV's columns can appear in any order
///     and any subset.
///   * Complex fields parsed inside the row:
///       - `inflection`: `;`-separated `key=value` pairs
///       - `examples`:   `|`-separated sentences
///       - `related`:    `;`-separated word slugs
///   * Skip rules: empty `word` cell + `word` starting
///     with `#` both treated as skip-this-row; duplicate
///     `word` (already in the dictionary) skipped with
///     warning.
///   * Tally printed at end (imported / skipped /
///     failed counts).
fn import_dictionary_csv(
    project: &Path,
    language: &str,
    csv_path: &Path,
) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?
        .clone();
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    let raw = std::fs::read_to_string(csv_path).map_err(|e| {
        Error::Config(format!(
            "could not read CSV file {}: {e}",
            csv_path.display()
        ))
    })?;
    let rows = parse_csv(&raw)
        .map_err(|e| Error::Config(format!("CSV parse error: {e}")))?;
    let mut rows = rows.into_iter();
    let header = rows
        .next()
        .ok_or_else(|| Error::Config("CSV is empty (no header row)".into()))?;
    let columns = resolve_csv_columns(&header)?;

    let mut imported = 0usize;
    let mut skipped_blank = 0usize;
    let mut skipped_comment = 0usize;
    let mut skipped_duplicate = 0usize;
    let mut failed = 0usize;

    for (row_idx, row) in rows.enumerate() {
        // Row 1 in user terms = header; data starts at row 2.
        let display_row = row_idx + 2;
        let entry = match build_import_entry_from_row(&columns, &row) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("row {display_row}: {e} — skipped");
                failed += 1;
                continue;
            }
        };
        let trimmed = entry.word.trim();
        if trimmed.is_empty() {
            skipped_blank += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            skipped_comment += 1;
            continue;
        }
        match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
            Ok((_, bucket)) => {
                eprintln!("imported `{}` → {language}/Dictionary/{bucket}", entry.word);
                imported += 1;
            }
            Err(e) => {
                let msg = e.to_string();
                // The duplicate-detect message comes from
                // `create_dictionary_entry`; surface as a
                // skip rather than a failure so an
                // idempotent re-import doesn't tally the
                // pre-existing entries as errors.
                if msg.contains("already defined") {
                    eprintln!("row {display_row}: `{}` already exists — skipped", entry.word);
                    skipped_duplicate += 1;
                } else {
                    eprintln!("row {display_row}: import `{}` failed: {msg}", entry.word);
                    failed += 1;
                }
            }
        }
    }

    eprintln!();
    eprintln!("Import summary for `{language}`");
    eprintln!("  imported:        {imported}");
    if skipped_blank > 0 {
        eprintln!("  skipped (blank): {skipped_blank}");
    }
    if skipped_comment > 0 {
        eprintln!("  skipped (#):     {skipped_comment}");
    }
    if skipped_duplicate > 0 {
        eprintln!("  skipped (dup):   {skipped_duplicate}");
    }
    if failed > 0 {
        eprintln!("  failed:          {failed}");
    }
    Ok(())
}

/// Column-name → index mapping.  Built from the
/// CSV's header row so columns can appear in any
/// order and any subset (required columns enforced
/// here).
struct CsvColumns {
    word: usize,
    pos: usize,
    translation: usize,
    example: Option<usize>,
    pronunciation: Option<usize>,
    etymology: Option<usize>,
    related: Option<usize>,
    inflection: Option<usize>,
    examples: Option<usize>,
    register: Option<usize>,
    era: Option<usize>,
    notes: Option<usize>,
}

fn resolve_csv_columns(header: &[String]) -> Result<CsvColumns> {
    let lookup = |name: &str| -> Option<usize> {
        header.iter().position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let word = lookup("word").ok_or_else(|| {
        Error::Config("CSV missing required column `word`".into())
    })?;
    let pos = lookup("type").ok_or_else(|| {
        Error::Config("CSV missing required column `type`".into())
    })?;
    let translation = lookup("translation").ok_or_else(|| {
        Error::Config("CSV missing required column `translation`".into())
    })?;
    Ok(CsvColumns {
        word,
        pos,
        translation,
        example: lookup("example"),
        pronunciation: lookup("pronunciation"),
        etymology: lookup("etymology"),
        related: lookup("related"),
        inflection: lookup("inflection"),
        examples: lookup("examples"),
        register: lookup("register"),
        era: lookup("era"),
        notes: lookup("notes"),
    })
}

fn build_import_entry_from_row(
    cols: &CsvColumns,
    row: &[String],
) -> std::result::Result<ImportEntry, String> {
    let get = |idx: usize| -> String {
        row.get(idx).cloned().unwrap_or_default()
    };
    let opt = |maybe_idx: Option<usize>| -> String {
        maybe_idx.map(get).unwrap_or_default()
    };
    let inflection_raw = opt(cols.inflection);
    let inflection = parse_inflection_field(&inflection_raw);
    let examples_raw = opt(cols.examples);
    let examples = split_pipe(&examples_raw);
    let related_raw = opt(cols.related);
    let related = split_semicolon(&related_raw);
    Ok(ImportEntry {
        word: get(cols.word).trim().to_string(),
        pos: get(cols.pos).trim().to_string(),
        translation: get(cols.translation).trim().to_string(),
        example: opt(cols.example).trim().to_string(),
        pronunciation: opt(cols.pronunciation).trim().to_string(),
        etymology: opt(cols.etymology).trim().to_string(),
        related,
        inflection,
        examples,
        register: opt(cols.register).trim().to_string(),
        era: opt(cols.era).trim().to_string(),
        notes: opt(cols.notes).trim().to_string(),
    })
}

/// `nominative=atal;genitive=atale;plural=atatal`
/// → BTreeMap.  Bad entries (no `=`) are silently
/// skipped — the import is best-effort row-by-row.
fn parse_inflection_field(
    raw: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for pair in raw.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        if let Some(eq) = pair.find('=') {
            let key = pair[..eq].trim().to_string();
            let value = pair[eq + 1..].trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                out.insert(key, value);
            }
        }
    }
    out
}

fn split_pipe(raw: &str) -> Vec<String> {
    raw.split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn split_semicolon(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Minimal RFC 4180-style CSV reader.  Handles:
///   * Quoted fields with embedded `,`, `\n`, `"`
///     (`""` doubles to a single `"`).
///   * Unquoted fields with neither.
///   * CRLF + bare LF row separators.
/// Returns `Vec<Vec<String>>` — one Vec per row.
/// Errors only on truly malformed input (unclosed
/// quote at end of file).
fn parse_csv(raw: &str) -> std::result::Result<Vec<Vec<String>>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quoted = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quoted {
            match c {
                '"' => {
                    // `""` inside a quoted field = one literal quote.
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quoted = false;
                    }
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quoted = true,
                ',' => {
                    row.push(std::mem::take(&mut field));
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if in_quoted {
        return Err("unclosed quote at end of file".into());
    }
    // Flush the trailing field/row when the file
    // doesn't end with a newline.
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}

fn list(project: &Path) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let languages = hierarchy.children_of(Some(lang_root.id));
    if languages.is_empty() {
        eprintln!("no languages defined — run `inkhaven language init <name>`");
        return Ok(());
    }
    // Compute counts up-front so the column widths
    // can size to the data.  Tuple shape:
    // (name, entries, grammar, phonology, samples).
    let mut rows: Vec<(String, usize, usize, usize, usize)> =
        Vec::with_capacity(languages.len());
    for lang in &languages {
        let chapters = hierarchy.children_of(Some(lang.id));
        let mut entries = 0usize;
        let mut grammar = 0usize;
        let mut phonology = 0usize;
        let mut samples = 0usize;
        for chapter in &chapters {
            let title_lc = chapter.title.to_lowercase();
            let paragraph_count = hierarchy
                .collect_subtree(chapter.id)
                .into_iter()
                .filter_map(|id| hierarchy.get(id))
                .filter(|n| n.kind == NodeKind::Paragraph)
                .count();
            match title_lc.as_str() {
                "dictionary" => entries = paragraph_count,
                "grammar" => grammar = paragraph_count,
                "phonology" => phonology = paragraph_count,
                "sample texts" => samples = paragraph_count,
                _ => {}
            }
        }
        rows.push((lang.title.clone(), entries, grammar, phonology, samples));
    }
    let max_name = rows.iter().map(|r| r.0.chars().count()).max().unwrap_or(8);
    let name_w = max_name.max(8);
    println!(
        "  {:<width$}  {:>6}  {:>7}  {:>9}  {:>7}",
        "name", "words", "grammar", "phonology", "samples",
        width = name_w,
    );
    println!(
        "  {}",
        "-".repeat(name_w + 36)
    );
    for (name, entries, grammar, phonology, samples) in &rows {
        println!(
            "  {:<width$}  {:>6}  {:>7}  {:>9}  {:>7}",
            name, entries, grammar, phonology, samples,
            width = name_w,
        );
    }
    Ok(())
}

/// 1.2.13+ Phase D.1 — `inkhaven language
/// remove-word <language> <word>`.  Mirror of
/// `add-word`: resolves the language sub-book by
/// case-insensitive title; finds the Dictionary
/// chapter; locates the bucket subchapter via the
/// same alphabet-bucket derivation
/// (`Meta/overview.alphabet` consultation first,
/// first-char fallback); deletes the entry
/// paragraph.  Errors when the entry doesn't
/// exist rather than silently no-op-ing so the
/// caller knows their `remove-word foo` against
/// an already-removed entry needs no follow-up
/// action.
fn remove_word(project: &Path, language: &str, word: &str) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?
        .clone();
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!("language `{language}` not found"))
        })?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter
                && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` has no Dictionary chapter"
            ))
        })?;
    // Same bucket derivation as add-word.
    let bucket = derive_alphabet_bucket(&store, &hierarchy, &lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;
    let subchapter = hierarchy
        .children_of(Some(dictionary.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Subchapter
                && n.title.eq_ignore_ascii_case(&bucket)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "no bucket subchapter `{bucket}` under `{language}/Dictionary` — `{word}` isn't defined"
            ))
        })?;
    let entry = hierarchy
        .children_of(Some(subchapter.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph
                && n.title.eq_ignore_ascii_case(word)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "word `{word}` not found under `{language}/Dictionary/{bucket}`"
            ))
        })?;
    let ids = hierarchy.collect_subtree(entry.id);
    // Entry is a Paragraph — its on-disk path lives
    // in `entry.file` (no children to walk for the
    // fs path).
    let fs_rel = entry
        .file
        .as_ref()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    store
        .delete_subtree(&fs_rel, &ids)
        .map_err(|e| Error::Store(format!("delete entry: {e}")))?;
    eprintln!(
        "removed `{word}` from `{language}/Dictionary/{bucket}`"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_chapters_match_proposal() {
        // Locks the chapter shape against the
        // proposal §1 hierarchy diagram — adding
        // or renaming a chapter requires updating
        // both the constant + the proposal.
        assert_eq!(
            STANDARD_CHAPTERS,
            &["Meta", "Dictionary", "Grammar", "Phonology", "Sample texts"]
        );
    }

    /// 1.2.13+ Phase D.1 hotfix — the verbose seed
    /// templates use HJSON multi-line strings (`'''`)
    /// and a generous amount of commented-out
    /// optional fields.  A typo or unbalanced bracket
    /// in any of them would silently break every new
    /// language sub-book the user scaffolds.  Parse
    /// each template through serde_hjson directly to
    /// catch syntax regressions at test time, not at
    /// the user's first `+` press.
    #[test]
    fn csv_parser_handles_quoted_fields() {
        let csv = "word,type,translation\n\
                   atal,noun,river\n\
                   sora,verb,\"to flow, swiftly\"\n\
                   nan,pronoun,\"\"\"you\"\"\"\n";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 4);
        assert_eq!(rows[0], vec!["word", "type", "translation"]);
        assert_eq!(rows[1], vec!["atal", "noun", "river"]);
        assert_eq!(rows[2], vec!["sora", "verb", "to flow, swiftly"]);
        // Embedded "" doubles to one literal quote.
        assert_eq!(rows[3], vec!["nan", "pronoun", "\"you\""]);
    }

    #[test]
    fn csv_parser_handles_newlines_in_quoted_fields() {
        let csv = "word,notes\natal,\"line1\nline2\"\n";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1], vec!["atal", "line1\nline2"]);
    }

    #[test]
    fn csv_parser_handles_crlf_and_missing_trailing_newline() {
        let csv = "a,b\r\nc,d";
        let rows = parse_csv(csv).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0], vec!["a", "b"]);
        assert_eq!(rows[1], vec!["c", "d"]);
    }

    #[test]
    fn csv_parser_errors_on_unclosed_quote() {
        assert!(parse_csv("word\n\"unclosed").is_err());
    }

    #[test]
    fn inflection_parser_extracts_pairs() {
        let m = parse_inflection_field("nominative=atal;genitive=atale;plural=atatal");
        assert_eq!(m.len(), 3);
        assert_eq!(m.get("nominative"), Some(&"atal".to_string()));
        assert_eq!(m.get("genitive"), Some(&"atale".to_string()));
        assert_eq!(m.get("plural"), Some(&"atatal".to_string()));
    }

    #[test]
    fn inflection_parser_tolerates_whitespace_and_skips_malformed() {
        let m = parse_inflection_field(" plural = atatal ; bad-no-equals ; genitive=atale ");
        assert_eq!(m.len(), 2);
        assert!(m.contains_key("plural"));
        assert!(m.contains_key("genitive"));
    }

    #[test]
    fn split_helpers_filter_empty_tokens() {
        assert_eq!(
            split_pipe("a|b||c"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
        assert_eq!(
            split_semicolon("a;b;;c"),
            vec!["a".to_string(), "b".to_string(), "c".to_string()]
        );
    }

    #[test]
    fn resolve_csv_columns_requires_word_type_translation() {
        let header = vec!["word".into(), "type".into(), "translation".into()];
        let cols = resolve_csv_columns(&header).unwrap();
        assert_eq!(cols.word, 0);
        assert_eq!(cols.pos, 1);
        assert_eq!(cols.translation, 2);
        assert!(cols.example.is_none());
    }

    #[test]
    fn resolve_csv_columns_errors_on_missing_required() {
        let header = vec!["word".into(), "type".into()];
        assert!(resolve_csv_columns(&header).is_err());
    }

    #[test]
    fn resolve_csv_columns_is_case_insensitive_and_order_independent() {
        let header = vec![
            "Notes".into(),
            "Translation".into(),
            "TYPE".into(),
            "Word".into(),
            "inflection".into(),
        ];
        let cols = resolve_csv_columns(&header).unwrap();
        assert_eq!(cols.word, 3);
        assert_eq!(cols.pos, 2);
        assert_eq!(cols.translation, 1);
        assert_eq!(cols.notes, Some(0));
        assert_eq!(cols.inflection, Some(4));
    }

    #[test]
    fn imported_entry_body_skips_empty_optionals() {
        let entry = ImportEntry {
            word: "atal".into(),
            pos: "noun".into(),
            translation: "river".into(),
            ..Default::default()
        };
        let body = build_imported_entry_body(&entry);
        assert!(body.contains("word:"));
        assert!(body.contains("type:"));
        assert!(body.contains("translation:"));
        // Empty optionals must be absent — no `example:`,
        // `pronunciation:`, `notes:` etc. in the body
        // when the import didn't populate them.
        assert!(!body.contains("example:"));
        assert!(!body.contains("pronunciation:"));
        assert!(!body.contains("notes:"));
        assert!(!body.contains("inflection:"));
    }

    #[test]
    fn imported_entry_body_emits_inflection_and_examples() {
        let mut entry = ImportEntry {
            word: "atal".into(),
            pos: "noun".into(),
            translation: "river".into(),
            ..Default::default()
        };
        entry.inflection.insert("plural".into(), "atatal".into());
        entry.inflection.insert("genitive".into(), "atale".into());
        entry.examples = vec!["Atal sora-mi.".into(), "Atal kima.".into()];
        let body = build_imported_entry_body(&entry);
        assert!(body.contains("inflection: {"));
        assert!(body.contains("plural: \"atatal\""));
        assert!(body.contains("genitive: \"atale\""));
        assert!(body.contains("examples: ["));
        assert!(body.contains("\"Atal sora-mi.\""));
        // Round-trips through the parser.
        let parsed: serde_hjson::Value =
            serde_hjson::from_str(&body).expect("imported entry body must parse");
        let _ = parsed;
    }

    #[test]
    fn meta_overview_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(META_OVERVIEW_BODY)
            .expect("META_OVERVIEW_BODY must be valid HJSON");
    }

    #[test]
    fn dictionary_entry_seed_parses() {
        let body = seed_dictionary_entry_body(
            "aiya", "interjection", "hail", Some("Aiya!"),
        );
        let _: serde_hjson::Value = serde_hjson::from_str(&body)
            .expect("dictionary entry seed must be valid HJSON");
    }

    #[test]
    fn grammar_rule_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(GRAMMAR_RULE_SEED_BODY)
            .expect("GRAMMAR_RULE_SEED_BODY must be valid HJSON");
    }

    #[test]
    fn phonology_rule_seed_parses() {
        let _: serde_hjson::Value = serde_hjson::from_str(PHONOLOGY_RULE_SEED_BODY)
            .expect("PHONOLOGY_RULE_SEED_BODY must be valid HJSON");
    }

    #[test]
    fn meta_overview_body_contains_alphabet_field() {
        // The `alphabet` field is the load-bearing
        // metadata key — drives Dictionary
        // subchapter auto-creation in Phase B.
        // Lock its presence in the seeded body so
        // a future seed-body edit can't silently
        // drop it.
        assert!(META_OVERVIEW_BODY.contains("alphabet:"));
        assert!(META_OVERVIEW_BODY.contains("language_kind:"));
    }

    #[test]
    fn alphabet_bucket_uppercases_first_char() {
        assert_eq!(alphabet_bucket("aiya"), Some("A".to_string()));
        assert_eq!(alphabet_bucket("Bran"), Some("B".to_string()));
        assert_eq!(alphabet_bucket("  zeta"), Some("Z".to_string()));
    }

    #[test]
    fn alphabet_bucket_handles_non_latin() {
        // Cyrillic 'я' uppercases to 'Я'.
        assert_eq!(alphabet_bucket("ярости"), Some("Я".to_string()));
        // Greek 'α' uppercases to 'Α'.
        assert_eq!(alphabet_bucket("αυτός"), Some("Α".to_string()));
    }

    #[test]
    fn alphabet_bucket_returns_none_for_whitespace() {
        assert_eq!(alphabet_bucket(""), None);
        assert_eq!(alphabet_bucket("   "), None);
    }

    #[test]
    fn seed_dictionary_entry_includes_core_fields() {
        let body = seed_dictionary_entry_body(
            "aiya",
            "interjection",
            "hail",
            Some("Aiya Eärendil!"),
        );
        // The four core HJSON fields land in the
        // body.  Locking presence stops a future
        // schema rename from silently breaking the
        // seed.
        assert!(body.contains("word:"));
        assert!(body.contains("type:"));
        assert!(body.contains("translation:"));
        assert!(body.contains("example:"));
        assert!(body.contains("aiya"));
        assert!(body.contains("interjection"));
        assert!(body.contains("hail"));
        assert!(body.contains("Aiya Eärendil!"));
    }

    #[test]
    fn csv_field_quotes_when_needed() {
        // Plain field — emit verbatim.
        assert_eq!(csv_field("aiya"), "aiya");
        // Comma triggers quoting.
        assert_eq!(csv_field("hail, friend"), "\"hail, friend\"");
        // Embedded quote doubles + wraps.
        assert_eq!(csv_field("he said \"hi\""), "\"he said \"\"hi\"\"\"");
        // Newline triggers quoting too.
        assert_eq!(csv_field("line1\nline2"), "\"line1\nline2\"");
    }

    #[test]
    fn typst_escape_handles_markup_chars() {
        // Markup-bearing characters get backslashed
        // so the renderer doesn't apply emphasis /
        // code / link semantics to dictionary
        // content.
        assert_eq!(typst_escape("plain"), "plain");
        assert_eq!(typst_escape("a*b"), "a\\*b");
        assert_eq!(typst_escape("[bracket]"), "\\[bracket\\]");
        assert_eq!(typst_escape("#hash"), "\\#hash");
        assert_eq!(typst_escape("with_under"), "with\\_under");
        // Non-Latin / Unicode passes through.
        assert_eq!(typst_escape("ñ'olor"), "ñ'olor");
    }

    #[test]
    fn render_anki_emits_header_row() {
        let out = render_anki(&[]).unwrap();
        let s = String::from_utf8(out).unwrap();
        assert!(s.starts_with("word,translation,type,example,inflection\n"));
    }

    #[test]
    fn render_anki_renders_entry_row() {
        let mut entry = crate::language_entry::DictionaryEntry::default();
        entry.word = "aiya".into();
        entry.translation = "hail".into();
        entry.pos = "interjection".into();
        entry.example = "Aiya Eärendil!".into();
        let out = render_anki(&[("aiya".into(), entry)]).unwrap();
        let s = String::from_utf8(out).unwrap();
        // Header on line 1, entry on line 2.
        let lines: Vec<&str> = s.lines().collect();
        assert_eq!(lines.len(), 2, "got: {s:?}");
        assert!(lines[1].contains("aiya"));
        assert!(lines[1].contains("hail"));
        assert!(lines[1].contains("interjection"));
        assert!(lines[1].contains("Aiya Eärendil!"));
    }

    #[test]
    fn render_dictionary_twocol_groups_by_alphabet() {
        let mut a_entry = crate::language_entry::DictionaryEntry::default();
        a_entry.word = "aiya".into();
        a_entry.pos = "interj.".into();
        a_entry.translation = "hail".into();
        let mut b_entry = crate::language_entry::DictionaryEntry::default();
        b_entry.word = "bara".into();
        b_entry.pos = "noun".into();
        b_entry.translation = "fire".into();
        let out = render_dictionary_twocol(
            "Quenya",
            None,
            &[("aiya".into(), a_entry), ("bara".into(), b_entry)],
        );
        let s = String::from_utf8(out).unwrap();
        // Bucket headers for both A and B sections.
        assert!(s.contains("— A —"), "got: {s}");
        assert!(s.contains("— B —"), "got: {s}");
        // Page setup + entries appear.
        assert!(s.contains("#set page(paper: \"a4\", columns: 2)"));
        assert!(s.contains("*aiya*"));
        assert!(s.contains("*bara*"));
        // Title shows the language name.
        assert!(s.contains("Quenya dictionary"));
    }

    #[test]
    fn escape_hjson_handles_quotes_and_backslashes() {
        assert_eq!(escape_hjson(r#"he said "hi""#), r#"he said \"hi\""#);
        assert_eq!(escape_hjson(r"a\b"), r"a\\b");
    }
}
