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

use super::LanguageCommand;

pub fn run(project: &Path, cmd: LanguageCommand) -> Result<()> {
    match cmd {
        LanguageCommand::Init { name } => init(project, &name),
        LanguageCommand::AddWord {
            language,
            word,
            r#type,
            translation,
            example,
        } => add_word(
            project,
            &language,
            &word,
            &r#type,
            &translation,
            example.as_deref(),
        ),
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

/// Seed body for `Meta/overview` — a starter HJSON
/// paragraph the author edits to populate the
/// language's metadata.  Defaults assume a Latin-
/// alphabet constructed language; non-Latin
/// authors override `alphabet`.
const META_OVERVIEW_BODY: &str = "\
= overview

```hjson
{
  // Language metadata.  Edit before adding entries —
  // `alphabet` drives the Dictionary's subchapter
  // auto-creation.
  name: \"\"
  language_kind: constructed     // \"constructed\" | \"natural\"
  family:                         // sibling languages (e.g. Elvish)
  iso_code:                       // optional ISO 639-3
  alphabet: [\"A\", \"B\", \"C\", \"D\", \"E\", \"F\", \"G\", \"H\", \"I\",
             \"J\", \"K\", \"L\", \"M\", \"N\", \"O\", \"P\", \"Q\", \"R\",
             \"S\", \"T\", \"U\", \"V\", \"W\", \"X\", \"Y\", \"Z\"]
  reading_direction: ltr         // \"ltr\" | \"rtl\"
  stemmer:                        // optional Snowball algo (rare for conlangs)
  example_corpus_ref:             // free-form citation
}
```

# Free-form notes

Worldbuilding context for this language: who speaks
it, where, in what era, what register.  This block
is read by the human author; the LLM only consumes
the HJSON above when composing translation
prompts.
";

fn init(project: &Path, name: &str) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;

    // Locate the top-level Language system book.
    // `ensure_system_books` (called inside
    // `Store::open`) seeds it on every project
    // open so the lookup never fails on a healthy
    // project.
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

    // Reject duplicate.  Slug collision is what
    // `create_node` would normally catch, but
    // explicit rejection is friendlier than a
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

    // Per-language book — child of Language.
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

    // Five standard chapters under the per-language
    // book.  Reload hierarchy between creates so
    // each subsequent create sees the previous
    // create's slug + order.
    for title in STANDARD_CHAPTERS {
        let hierarchy = Hierarchy::load(&store)?;
        let chapter = store.create_node(
            &cfg,
            &hierarchy,
            NodeKind::Chapter,
            title,
            Some(&per_lang),
            None,
            InsertPosition::End,
        )?;
        eprintln!("  · {title}");
        // Seed `Meta/overview` with the starter
        // HJSON.  Other chapters stay empty —
        // they fill with entries via Phase B's
        // `add-word` and the in-TUI tree-pane
        // workflow.
        if *title == "Meta" {
            let hierarchy = Hierarchy::load(&store)?;
            let mut overview = store.create_node(
                &cfg,
                &hierarchy,
                NodeKind::Paragraph,
                "overview",
                Some(&chapter),
                None,
                InsertPosition::End,
            )?;
            store
                .update_paragraph_content(&mut overview, META_OVERVIEW_BODY.as_bytes())
                .map_err(|e| Error::Store(format!("seed overview: {e}")))?;
        }
    }

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

    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` has no `Dictionary` chapter — likely scaffolded with a pre-Phase-A inkhaven"
            ))
        })?;

    // Alphabet bucket — consult the language's
    // `Meta/overview.alphabet` first so authors with
    // non-Latin orthographies (Hebrew letter names,
    // paired-case Latin, Greek) get bucket subchapters
    // titled by their declared groupings rather than
    // the naive first-char uppercase.  Falls back to
    // first-char uppercase when:
    //   * the language has no Meta chapter (pre-Phase-A
    //     scaffold);
    //   * the Meta chapter has no overview paragraph;
    //   * the overview body has no HJSON block;
    //   * the alphabet list is empty;
    //   * the word's first char isn't covered by any
    //     declared entry.
    let bucket = derive_alphabet_bucket(&store, &hierarchy, &lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;

    // Find or create the bucket subchapter.
    let dictionary_kids = hierarchy.children_of(Some(dictionary.id));
    let subchapter = match dictionary_kids
        .iter()
        .find(|n| {
            n.kind == NodeKind::Subchapter && n.title == bucket
        })
        .cloned()
    {
        Some(existing) => {
            eprintln!("using existing subchapter `{bucket}`");
            existing.clone()
        }
        None => {
            let hierarchy = Hierarchy::load(&store)?;
            let created = store.create_node(
                &cfg,
                &hierarchy,
                NodeKind::Subchapter,
                &bucket,
                Some(&dictionary),
                None,
                InsertPosition::End,
            )?;
            eprintln!("created subchapter `{bucket}`");
            created
        }
    };

    // Reject duplicate.
    let hierarchy = Hierarchy::load(&store)?;
    if hierarchy
        .children_of(Some(subchapter.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(word))
    {
        return Err(Error::Config(format!(
            "word `{word}` already defined under `{language}/Dictionary/{bucket}`"
        )));
    }

    // Create the entry paragraph + seed its HJSON
    // body.
    let hierarchy = Hierarchy::load(&store)?;
    let mut entry = store.create_node(
        &cfg,
        &hierarchy,
        NodeKind::Paragraph,
        word,
        Some(&subchapter),
        None,
        InsertPosition::End,
    )?;
    let body = seed_dictionary_entry_body(word, pos, translation, example);
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed entry: {e}")))?;

    eprintln!(
        "added `{word}` to `{language}/Dictionary/{bucket}` ({pos} · {translation})"
    );
    Ok(())
}

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
/// dictionary entry.  Title line + HJSON
/// frontmatter block + a free-form notes section
/// the author fills in.
fn seed_dictionary_entry_body(
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
) -> String {
    let example_line = match example {
        Some(s) if !s.trim().is_empty() => {
            format!("  example:      \"{}\"\n", escape_hjson(s))
        }
        _ => "  example:      \"\"\n".to_string(),
    };
    format!(
        "= {word}\n\
         \n\
         ```hjson\n\
         {{\n\
         {core}\
         {example_line}\
         }}\n\
         ```\n\
         \n\
         # Free-form notes\n\
         \n\
         Usage, register, etymology, related entries.\n",
        word = word,
        core = format!(
            "  word:         \"{}\"\n  type:         \"{}\"\n  translation:  \"{}\"\n",
            escape_hjson(word),
            escape_hjson(pos),
            escape_hjson(translation),
        ),
        example_line = example_line,
    )
}

/// Minimal HJSON string escape — backslash-quote +
/// backslash-backslash.  Sufficient for the
/// dictionary-entry seed body, which never sees
/// control characters in practice.
fn escape_hjson(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
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
    fn escape_hjson_handles_quotes_and_backslashes() {
        assert_eq!(escape_hjson(r#"he said "hi""#), r#"he said \"hi\""#);
        assert_eq!(escape_hjson(r"a\b"), r"a\\b");
    }
}
