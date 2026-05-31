//! 1.2.14+ Phase Q.1 — project templates for
//! `inkhaven init --template <name>`.
//!
//! Templates are pure data — embedded constants
//! describing the user-book structure + system-
//! book seed entries the template scaffolds on
//! top of the standard init machinery.  Walked
//! after the standard init returns so a template
//! that fails partway through still leaves a
//! functional empty project behind.
//!
//! Six templates ship:
//!
//! | Name | Use case |
//! |------|----------|
//! | `empty` | default — no extra scaffolding |
//! | `novel` | three-act manuscript + character stubs |
//! | `nonfiction` | intro/parts/conclusion + research methodology |
//! | `rpg-sourcebook` | setting/rules/adventures/appendices + worldbuilding seeds |
//! | `technical` | overview/reference/tutorials/index |
//! | `nanowrimo` | like `novel` with a 50K-word target |
//!
//! `inkhaven template list` enumerates the same
//! set with descriptions for at-the-terminal
//! reference.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store};

/// 1.2.14+ Phase Q.1 — one project template.
/// Captures the book structure + system-book seed
/// entries the template adds on top of the
/// standard init scaffolding.
#[derive(Debug, Clone, Copy)]
pub struct ProjectTemplate {
    pub name: &'static str,
    pub description: &'static str,
    pub manuscript_book: Option<ManuscriptBook>,
    pub seeds: &'static [SystemBookSeed],
    /// Plain-text guidance printed after init
    /// completes — typically the recommended
    /// `project.word_count_goal` and target-date
    /// pacing.  Multi-line; printed verbatim.
    pub post_init_message: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct ManuscriptBook {
    /// Display title for the user book the
    /// template creates (e.g. `"Manuscript"`,
    /// `"Sourcebook"`).
    pub title: &'static str,
    /// Chapters created under the book in
    /// canonical order.
    pub chapters: &'static [&'static str],
    /// Optional content-type override for every
    /// paragraph created under this book (e.g.
    /// `"markdown"` for the technical template).
    /// `None` keeps the default Typst.  Reserved
    /// for Q.1.1 — chapter scaffolding currently
    /// inherits the standard content-type.
    #[allow(dead_code)]
    pub paragraph_content_type: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
pub struct SystemBookSeed {
    /// System tag of the book that gets the seed
    /// paragraphs (e.g. `"characters"`, `"places"`,
    /// `"threads"`).
    pub system_tag: &'static str,
    /// (paragraph_title, body) tuples.  Empty body
    /// keeps `create_node`'s `= Title\n\n`
    /// skeleton; non-empty body overwrites.
    pub paragraphs: &'static [(&'static str, &'static str)],
}

/// Every template the CLI knows about.  Add new
/// templates here; the registry is consulted by
/// both `apply()` and `list_templates()`.
pub const TEMPLATES: &[ProjectTemplate] = &[
    EMPTY,
    NOVEL,
    NONFICTION,
    RPG_SOURCEBOOK,
    TECHNICAL,
    NANOWRIMO,
];

pub const EMPTY: ProjectTemplate = ProjectTemplate {
    name: "empty",
    description:
        "no extra scaffolding — system books only.  The current default \
         for hand-authored projects.",
    manuscript_book: None,
    seeds: &[],
    post_init_message: "",
};

pub const NOVEL: ProjectTemplate = ProjectTemplate {
    name: "novel",
    description:
        "three-act manuscript book (Act I / II / III) + Characters \
         seeded with protagonist / antagonist / confidant stubs.  \
         Recommended word-count goal: 80000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Act I — Setup",
            "Act II — Confrontation",
            "Act III — Resolution",
        ],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "characters",
        paragraphs: &[
            (
                "protagonist",
                "= protagonist\n\n\
                 The character whose arc the manuscript follows.\n\n\
                 // Edit this paragraph to capture: voice, want,\n\
                 // need, internal conflict, defining scenes.\n",
            ),
            (
                "antagonist",
                "= antagonist\n\n\
                 The force opposing the protagonist's want / need.\n\n\
                 // Doesn't have to be a person — could be a system,\n\
                 // an institution, a part of the protagonist's own\n\
                 // psyche.\n",
            ),
            (
                "confidant",
                "= confidant\n\n\
                 The character the protagonist confides in — and\n\
                 through whom the reader hears the protagonist's\n\
                 internal monologue made external.\n",
            ),
        ],
    }],
    post_init_message:
        "Recommended next steps:\n  \
         · Open the Manuscript book and start Act I\n  \
         · Edit Characters/protagonist (etc.) to capture voice + arc\n  \
         · Set `project.word_count_goal: 80000` in inkhaven.hjson \
            (1.2.14 Phase Q.4 will surface the projection modal)\n",
};

pub const NONFICTION: ProjectTemplate = ProjectTemplate {
    name: "nonfiction",
    description:
        "manuscript with Introduction / Part I / Part II / \
         Conclusion chapters + Research book seeded with a \
         methodology paragraph.  Recommended word-count goal: \
         60000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &["Introduction", "Part I", "Part II", "Conclusion"],
        paragraph_content_type: None,
    }),
    seeds: &[SystemBookSeed {
        system_tag: "research",
        paragraphs: &[(
            "methodology",
            "= methodology\n\n\
             How the research feeding this manuscript was conducted:\n\
             sources consulted, interviews held, archival visits,\n\
             criteria for inclusion / exclusion.\n\n\
             // Drives reviewer trust + makes a reproducibility\n\
             // statement easy to assemble when the manuscript ships.\n",
        )],
    }],
    post_init_message:
        "Recommended next steps:\n  \
         · Outline Introduction → state thesis, scope, audience\n  \
         · Edit Research/methodology before adding citation paragraphs\n  \
         · Set `project.word_count_goal: 60000` in inkhaven.hjson\n",
};

pub const RPG_SOURCEBOOK: ProjectTemplate = ProjectTemplate {
    name: "rpg-sourcebook",
    description:
        "Setting / Rules / Adventures / Appendices chapters + \
         Places / Artefacts / Threads seeded with one example \
         each.  Recommended word-count goal: 120000.",
    manuscript_book: Some(ManuscriptBook {
        title: "Sourcebook",
        chapters: &["Setting", "Rules", "Adventures", "Appendices"],
        paragraph_content_type: None,
    }),
    seeds: &[
        SystemBookSeed {
            system_tag: "places",
            paragraphs: &[(
                "example-locale",
                "= example-locale\n\n\
                 A starter Place entry.  Rename or duplicate as your\n\
                 setting grows.\n\n\
                 // Place entries light up in manuscript prose when\n\
                 // mentioned (cyan overlay via the lexicon walker).\n",
            )],
        },
        SystemBookSeed {
            system_tag: "artefacts",
            paragraphs: &[(
                "example-artefact",
                "= example-artefact\n\n\
                 A starter Artefact entry — for named items, magical\n\
                 objects, signature equipment, plot-bearing macguffins.\n",
            )],
        },
        SystemBookSeed {
            system_tag: "threads",
            paragraphs: &[(
                "example-arc",
                "{\n  \
                 title:         \"example-arc\"\n  \
                 status:        \"setup\"\n  \
                 weight:        \"major\"\n  \
                 opening:       \"What kicks the arc off — fill in.\"\n  \
                 midpoint:      \"\"\n  \
                 payoff:        \"\"\n  \
                 characters:    []\n  \
                 places:        []\n  \
                 artefacts:     []\n  \
                 related_threads: []\n  \
                 tension:       0\n  \
                 register:      \"\"\n  \
                 notes:         \"Starter Threads entry — see \
                 `inkhaven thread add` for the CLI shortcut.\"\n\
                 }\n",
            )],
        },
    ],
    post_init_message:
        "Recommended next steps:\n  \
         · Setting chapter first — establish geography + cosmology\n  \
         · Rules chapter — system + mechanics; use HJSON paragraphs\n   \
            for character classes / spells / monsters\n  \
         · Threads/example-arc — fill in (Ctrl+V Shift+H lists threads)\n  \
         · Set `project.word_count_goal: 120000`\n",
};

pub const TECHNICAL: ProjectTemplate = ProjectTemplate {
    name: "technical",
    description:
        "Overview / Reference / Tutorials / Index chapters.  No \
         word-count goal default (technical docs are bounded by \
         topic coverage, not length).",
    manuscript_book: Some(ManuscriptBook {
        title: "Documentation",
        chapters: &["Overview", "Reference", "Tutorials", "Index"],
        paragraph_content_type: None,
    }),
    seeds: &[],
    post_init_message:
        "Recommended next steps:\n  \
         · Overview/getting-started — what the system does, who for\n  \
         · Reference chapter — one paragraph per concept / API\n  \
         · Tutorials chapter — narrative, paragraph per task\n",
};

pub const NANOWRIMO: ProjectTemplate = ProjectTemplate {
    name: "nanowrimo",
    description:
        "NaNoWriMo manuscript scaffolding.  Same structure as \
         `novel` but with a 50000-word goal + recommended \
         1667-words/day pacing.",
    manuscript_book: Some(ManuscriptBook {
        title: "Manuscript",
        chapters: &[
            "Act I — Setup",
            "Act II — Confrontation",
            "Act III — Resolution",
        ],
        paragraph_content_type: None,
    }),
    seeds: NOVEL.seeds,
    post_init_message:
        "NaNoWriMo target: 50000 words by month-end.\n  \
         · 1667 words / day for 30 days\n  \
         · Set `project.word_count_goal: 50000` in inkhaven.hjson\n  \
         · Set `project.target_date: \"2026-11-30\"` (adjust to your year)\n  \
         · Daily streak heatmap: Ctrl+B Shift+G\n",
};

/// 1.2.14+ Phase Q.1 — apply the named template to
/// a freshly-initialised project.  Called by
/// `cli::init::run` after the standard
/// `Store::open` returns.  Errors are surfaced
/// upward but don't roll back the standard init —
/// a partial template scaffold is recoverable
/// (the author can `inkhaven add` the missing
/// nodes by hand) but a rolled-back init isn't.
pub fn apply(store: &Store, cfg: &Config, name: &str) -> Result<()> {
    let template = TEMPLATES
        .iter()
        .find(|t| t.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| {
            Error::Config(format!(
                "unknown template `{name}` — run `inkhaven template list` \
                 to see available templates"
            ))
        })?;
    if name.eq_ignore_ascii_case("empty") {
        // No-op fast path.  Caller still gets a
        // valid scaffold from the standard init.
        return Ok(());
    }
    if let Some(book) = template.manuscript_book.as_ref() {
        apply_manuscript_book(store, cfg, book)?;
    }
    for seed in template.seeds {
        apply_system_seed(store, cfg, seed)?;
    }
    if !template.post_init_message.is_empty() {
        eprintln!();
        eprintln!("Template `{}`:", template.name);
        for line in template.post_init_message.lines() {
            eprintln!("{line}");
        }
    }
    Ok(())
}

fn apply_manuscript_book(
    store: &Store,
    cfg: &Config,
    book: &ManuscriptBook,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let new_book = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Book,
        book.title,
        None,
        None,
        InsertPosition::End,
    )?;
    eprintln!("  · created book `{}`", book.title);
    // Standard Typst skeleton (index.typ / settings.typ /
    // globals.typ) — same path the tree-pane Add Book chord
    // calls.  Non-fatal: a partial provisioning is better
    // than aborting the whole template.
    if let Err(e) = store.provision_user_book(cfg, &new_book) {
        eprintln!(
            "    (warn: Typst skeleton provisioning failed: {e})"
        );
    }
    for chapter_title in book.chapters {
        let hierarchy = Hierarchy::load(store)?;
        store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Chapter,
            chapter_title,
            Some(&new_book),
            None,
            InsertPosition::End,
        )?;
        eprintln!("      · chapter `{chapter_title}`");
    }
    Ok(())
}

fn apply_system_seed(
    store: &Store,
    cfg: &Config,
    seed: &SystemBookSeed,
) -> Result<()> {
    let hierarchy = Hierarchy::load(store)?;
    let parent = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(seed.system_tag)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(format!(
                "system book `{}` missing — re-open the project to seed it",
                seed.system_tag
            ))
        })?;
    for (title, body) in seed.paragraphs {
        let hierarchy = Hierarchy::load(store)?;
        // Skip duplicates by title so re-running
        // init --template on top of an existing
        // project doesn't double-seed.
        if hierarchy
            .children_of(Some(parent.id))
            .iter()
            .any(|n| n.title.eq_ignore_ascii_case(title))
        {
            continue;
        }
        let mut node = store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Paragraph,
            title,
            Some(&parent),
            None,
            InsertPosition::End,
        )?;
        if !body.is_empty() {
            if let Some(rel) = &node.file {
                let abs = store.project_root().join(rel);
                std::fs::write(&abs, body.as_bytes())
                    .map_err(Error::Io)?;
            }
            store
                .update_paragraph_content(&mut node, body.as_bytes())
                .map_err(|e| {
                    Error::Store(format!("seed {title}: {e}"))
                })?;
        }
        eprintln!(
            "      · seeded {}/{}",
            seed.system_tag, title
        );
    }
    Ok(())
}

/// 1.2.14+ Phase Q.1 — `inkhaven template list`.
/// Prints a two-column table: name → description.
/// Column widths size to the longest name.
pub fn list_templates() {
    let max_name = TEMPLATES
        .iter()
        .map(|t| t.name.chars().count())
        .max()
        .unwrap_or(8);
    let name_w = max_name.max(8);
    println!(
        "  {:<width$}  description",
        "name",
        width = name_w,
    );
    println!("  {}", "-".repeat(name_w + 60));
    for t in TEMPLATES {
        let mut first_line = true;
        // Wrap description onto continuation lines
        // indented under the description column.
        let prefix_width = name_w + 4;
        for line in wrap_description(t.description, 70) {
            if first_line {
                println!(
                    "  {:<width$}  {}",
                    t.name,
                    line,
                    width = name_w,
                );
                first_line = false;
            } else {
                println!(
                    "  {:<width$}  {}",
                    "",
                    line,
                    width = name_w,
                );
            }
            let _ = prefix_width; // silence rustc until/if needed
        }
    }
    println!();
    println!("Use with: inkhaven init <path> --template <name>");
}

/// Word-wrap a description string to `width`
/// characters; never breaks inside a word.
fn wrap_description(s: &str, width: usize) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    for word in s.split_whitespace() {
        if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_every_named_template() {
        let names: Vec<&str> = TEMPLATES.iter().map(|t| t.name).collect();
        for required in
            ["empty", "novel", "nonfiction", "rpg-sourcebook", "technical", "nanowrimo"]
        {
            assert!(
                names.contains(&required),
                "missing template `{required}` in TEMPLATES"
            );
        }
    }

    #[test]
    fn empty_template_has_no_scaffolding() {
        let empty = TEMPLATES
            .iter()
            .find(|t| t.name == "empty")
            .unwrap();
        assert!(empty.manuscript_book.is_none());
        assert!(empty.seeds.is_empty());
    }

    #[test]
    fn novel_template_has_three_act_structure() {
        let novel = TEMPLATES
            .iter()
            .find(|t| t.name == "novel")
            .unwrap();
        let book = novel.manuscript_book.as_ref().unwrap();
        assert_eq!(book.chapters.len(), 3);
        assert!(book.chapters[0].contains("Act I"));
        assert!(book.chapters[1].contains("Act II"));
        assert!(book.chapters[2].contains("Act III"));
        // Seeds Characters with three stubs.
        let chars = novel
            .seeds
            .iter()
            .find(|s| s.system_tag == "characters")
            .unwrap();
        assert_eq!(chars.paragraphs.len(), 3);
    }

    #[test]
    fn rpg_template_seeds_places_artefacts_threads() {
        let rpg = TEMPLATES
            .iter()
            .find(|t| t.name == "rpg-sourcebook")
            .unwrap();
        let tags: Vec<&str> =
            rpg.seeds.iter().map(|s| s.system_tag).collect();
        assert!(tags.contains(&"places"));
        assert!(tags.contains(&"artefacts"));
        assert!(tags.contains(&"threads"));
    }

    #[test]
    fn nanowrimo_template_inherits_novel_seeds() {
        let nano = TEMPLATES
            .iter()
            .find(|t| t.name == "nanowrimo")
            .unwrap();
        let novel = TEMPLATES
            .iter()
            .find(|t| t.name == "novel")
            .unwrap();
        assert_eq!(nano.seeds.len(), novel.seeds.len());
    }

    #[test]
    fn wrap_description_handles_short_strings() {
        let lines = wrap_description("short", 70);
        assert_eq!(lines, vec!["short".to_string()]);
    }

    #[test]
    fn wrap_description_wraps_long_strings() {
        let s = "a ".repeat(50);
        let lines = wrap_description(s.trim(), 20);
        assert!(lines.len() > 1);
        for line in &lines {
            assert!(line.chars().count() <= 20);
        }
    }

    #[test]
    fn rpg_thread_seed_parses_as_hjson() {
        // The Threads seed body is HJSON; pin that
        // it parses so a future schema tweak can't
        // ship a stub the user can't open.
        let rpg = TEMPLATES
            .iter()
            .find(|t| t.name == "rpg-sourcebook")
            .unwrap();
        let threads = rpg
            .seeds
            .iter()
            .find(|s| s.system_tag == "threads")
            .unwrap();
        let (_, body) = threads.paragraphs[0];
        let _: serde_hjson::Value = serde_hjson::from_str(body)
            .expect("rpg-sourcebook threads seed must be valid HJSON");
    }
}
