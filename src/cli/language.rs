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
}
