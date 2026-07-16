//! `inkhaven language` scaffolding: creating a language sub-book (`init`),
//! its standard chapters, the shared book-access front-half (`open_lang_book`,
//! `find_language_book`), and `language list`. Split out of the flat handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// Open a project and resolve a language sub-book under the `Language`
/// system book. The shared front-half of every conlang command — returns the
/// open `Store` (kept alive for the DuckDB lock), the loaded `Hierarchy`, and
/// the language's `Book` node.
pub(crate) fn open_lang_book(
    project: &Path,
    language: &str,
) -> Result<(Store, Hierarchy, crate::store::node::Node)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?
        .clone();
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;
    Ok((store, hierarchy, lang_book))
}

// ── Store-based API for the Bund `ink.lang.*` stdlib (1.3.21) ─────────────
//
// The CLI handlers above re-open the store from a `&Path`; a Bund script
// already holds the active project store, so these variants take `&Store`
// (and an already-loaded `&Hierarchy`) instead, reusing the same engine
// loaders and node-creation paths.

/// Find a language sub-book by name in an already-loaded hierarchy.
pub(crate) fn find_language_book(
    hierarchy: &Hierarchy,
    name: &str,
) -> Result<crate::store::node::Node> {
    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?;
    hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(name))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{name}` not found — create it first (e.g. `ink.lang.init`)"
            ))
        })
}

/// Create a language sub-book plus its scaffold chapters, using the active
/// store. Errors if the name already exists. Returns the new book node.
pub(crate) fn init_language(
    store: &Store,
    cfg: &Config,
    name: &str,
) -> Result<crate::store::node::Node> {
    let hierarchy = Hierarchy::load(store)?;
    let lang_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("Language system book missing — re-open the project to seed it".into())
        })?;
    if hierarchy
        .children_of(Some(lang_book.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(name))
    {
        return Err(Error::Config(format!(
            "language `{name}` already exists under Language"
        )));
    }
    let hierarchy = Hierarchy::load(store)?;
    let per_lang = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Book,
        name,
        Some(&lang_book),
        None,
        InsertPosition::End,
    )?;
    scaffold_language_chapters(store, cfg, &per_lang, |_| {})?;
    Ok(per_lang)
}

/// Create a paragraph with an HJSON `body` under a named chapter of a language
/// sub-book — the primitive behind `ink.lang.define`, which lets a Bund script
/// write a phonology / grammar / morphology / typology / sample block exactly as
/// it would appear in the book. Returns the created paragraph node.
pub(crate) fn create_chapter_paragraph(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    chapter_title: &str,
    paragraph_title: &str,
    body: &str,
) -> Result<crate::store::node::Node> {
    let hierarchy = Hierarchy::load(store)?;
    let chapter = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case(chapter_title))
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{}` has no `{chapter_title}` chapter (try Phonology, Grammar, \
                 Sample texts, or Meta)",
                lang_book.title
            ))
        })?;
    let hierarchy = Hierarchy::load(store)?;
    let mut entry = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        paragraph_title,
        Some(&chapter),
        None,
        InsertPosition::End,
    )?;
    entry.content_type = Some("hjson".to_string());
    if let Some(rel) = &entry.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write block: {e}")))?;
    }
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed block: {e}")))?;
    Ok(entry)
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
pub(crate) const STANDARD_CHAPTERS: &[&str] = &[
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
/// switched FROM the Typst-
/// with-fenced-HJSON format to pure HJSON because the
/// Typst editor mode rendered the body as a heading +
/// opaque code fence instead of as structured config.
/// The translation prompt composer + parser handle
/// both formats; new entries use pure HJSON, legacy
/// Typst-wrapped entries still parse via the fence
/// extractor.
pub(crate) const META_OVERVIEW_BODY: &str = "{
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

pub(crate) fn init(project: &Path, name: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
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
        "  · add dictionary entries under `Language/{name}/Dictionary` (`inkhaven language add-word`)"
    );
    eprintln!(
        "  · add grammar rules under `Language/{name}/Grammar` for the AI translation flow"
    );

    Ok(())
}

/// shared scaffold helper.
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
pub(crate) fn list(project: &Path) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
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
