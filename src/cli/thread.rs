//! 1.2.14+ Phase A.1 — `inkhaven thread …`
//! subcommand family.  Manages plot-thread
//! paragraphs under the `Threads` system book.
//!
//! Threads are HJSON-fronted Paragraphs — same
//! content-type pattern as the 1.2.13 Language
//! dictionary entries.  Each thread captures one
//! named narrative arc (the inheritance subplot,
//! the redemption arc, the secret-society reveal)
//! with status / weight / arc shape /
//! character + place + artefact + related-thread
//! links / tension level / register / notes.
//!
//! Paragraphs in the manuscript link to threads
//! via the existing paragraph-link mechanism
//! (`Ctrl+V A` add outgoing link, `Ctrl+V I`
//! add incoming) — no new linking primitive.
//!
//! See `Documentation/PROPOSALS/1.2.14_PLAN.md`
//! for the full design including the thread weave
//! view (`Ctrl+V Shift+H`) and the AI thread
//! audit (`Ctrl+V Shift+A`) landing in phases A.2
//! and A.3.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store, SYSTEM_TAG_THREADS};

use super::ThreadCommand;

pub fn run(project: &Path, cmd: ThreadCommand) -> Result<()> {
    match cmd {
        ThreadCommand::Add {
            name,
            title,
            status,
            weight,
        } => add(project, &name, title.as_deref(), &status, &weight),
        ThreadCommand::List { status, weight } => {
            list(project, status.as_deref(), weight.as_deref())
        }
    }
}

/// Seed body for a freshly-added thread paragraph.
/// Pure HJSON (no Typst wrappers) so the editor
/// renders with syntax highlighting + the
/// paragraph status bar shows `[hjson]`.
/// Mirrors the proposal §3.1 schema.
///
/// `title` / `status` / `weight` are pre-filled
/// from the CLI flags so the resulting paragraph
/// is immediately useful without further editing.
fn seed_thread_body(
    name: &str,
    title: Option<&str>,
    status: &str,
    weight: &str,
) -> String {
    let title_value = title.unwrap_or(name);
    format!(
        "{{\n  \
         // ──────────────────────────────────────────\n  \
         // IDENTITY\n  \
         // ──────────────────────────────────────────\n  \
         \n  \
         title:         \"{title}\"\n  \
         \n  \
         // setup | develop | payoff | resolved |\n  \
         // abandoned.\n  \
         status:        \"{status}\"\n  \
         \n  \
         // major | subplot | runner | bridge.\n  \
         weight:        \"{weight}\"\n  \
         \n  \
         // ──────────────────────────────────────────\n  \
         // ARC SHAPE\n  \
         // ──────────────────────────────────────────\n  \
         \n  \
         // One-sentence opening hook — what kicks\n  \
         // the thread off.\n  \
         opening:       \"\"\n  \
         \n  \
         // One-sentence midpoint pivot — what\n  \
         // changes the trajectory.\n  \
         midpoint:      \"\"\n  \
         \n  \
         // One-sentence payoff — what the thread\n  \
         // resolves to.\n  \
         payoff:        \"\"\n  \
         \n  \
         // ──────────────────────────────────────────\n  \
         // CONNECTIONS — slug refs into the other\n  \
         // system books.  The thread weave view\n  \
         // (Ctrl+V Shift+H) renders the union as a\n  \
         // sidebar; the AI thread audit reads them\n  \
         // when scoring chapter relevance.\n  \
         // ──────────────────────────────────────────\n  \
         \n  \
         characters:    []\n  \
         places:        []\n  \
         artefacts:     []\n  \
         related_threads: []\n  \
         \n  \
         // ──────────────────────────────────────────\n  \
         // METADATA\n  \
         // ──────────────────────────────────────────\n  \
         \n  \
         // Tension on a 0-10 scale — drives the\n  \
         // height of this thread's swim-lane in the\n  \
         // weave view.\n  \
         tension:       0\n  \
         \n  \
         // Genre register: literary | thriller |\n  \
         // romance | horror | comedy | sacred |\n  \
         // (free-form; the LLM reads it as a\n  \
         // tonal hint when auditing).\n  \
         register:      \"\"\n  \
         \n  \
         // Author's notes — historical motivation,\n  \
         // alternative payoffs considered,\n  \
         // worldbuilding rationale.\n  \
         notes:         \"\"\n\
         }}\n",
        title = escape_hjson(title_value),
        status = escape_hjson(status),
        weight = escape_hjson(weight),
    )
}

/// Minimal HJSON string escape — backslash-quote +
/// backslash-backslash.  Same shape as the
/// Language CLI's escape; duplicated here to keep
/// the modules independent.
fn escape_hjson(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

/// 1.2.14+ Phase A.1 — public shim for the TUI's
/// `commit_add` to call when `+` fires under the
/// Threads system book.  The TUI typed-name
/// becomes the thread's `title` field; status
/// defaults to `setup` and weight to `major`, both
/// of which the author can edit immediately in
/// the seeded HJSON.
pub fn seed_thread_body_for_tui(name: &str) -> String {
    seed_thread_body(name, None, "setup", "major")
}

fn add(
    project: &Path,
    name: &str,
    title: Option<&str>,
    status: &str,
    weight: &str,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let threads_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_THREADS)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Threads system book missing — re-open the project to seed it"
                    .into(),
            )
        })?;

    // Reject duplicates by title (case-insensitive)
    // BEFORE create_node so the failure mode is a
    // friendly error rather than a silent `-2`
    // slug suffix.
    if hierarchy
        .children_of(Some(threads_book.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(name))
    {
        return Err(Error::Config(format!(
            "thread `{name}` already exists under Threads"
        )));
    }

    let hierarchy = Hierarchy::load(&store)?;
    let mut node = store.create_node(
        &cfg,
        &hierarchy,
        NodeKind::Paragraph,
        name,
        Some(&threads_book),
        None,
        InsertPosition::End,
    )?;
    let body = seed_thread_body(name, title, status, weight);
    node.content_type = Some("hjson".to_string());
    // Disk write FIRST — `update_paragraph_content`
    // is bdslib-only; the editor reads the .typ
    // file off disk so the on-disk content has to
    // match.  Same pattern as Language entries
    // (see `cli::language::create_dictionary_entry`).
    if let Some(rel) = &node.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write thread body: {e}")))?;
    }
    store
        .update_paragraph_content(&mut node, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed thread body: {e}")))?;
    eprintln!(
        "added thread `{name}` to Threads ({status} · {weight})"
    );
    eprintln!("  open Threads/{name} in the editor to fill opening / midpoint / payoff");
    Ok(())
}

/// Parsed thread summary — used by `list` to pull
/// the few fields we need without depending on a
/// full DictionaryEntry-style parser.  Phase A.2
/// extracts this into a `thread_entry` module
/// when the weave view needs deeper parsing.
#[derive(Debug, Default, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct ThreadSummary {
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    weight: String,
    #[serde(default)]
    tension: i32,
    #[serde(default)]
    characters: Vec<String>,
    #[serde(default)]
    places: Vec<String>,
    #[serde(default)]
    artefacts: Vec<String>,
    #[serde(default)]
    related_threads: Vec<String>,
}

fn parse_thread_summary(body: &str) -> Option<ThreadSummary> {
    if body.trim().is_empty() {
        return None;
    }
    serde_hjson::from_str(body).ok()
}

fn list(
    project: &Path,
    status_filter: Option<&str>,
    weight_filter: Option<&str>,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let threads_book = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_THREADS)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Threads system book missing — re-open the project to seed it"
                    .into(),
            )
        })?;

    // Collect every thread paragraph under the
    // Threads book (subtree walk so chapter
    // grouping the author may add later is
    // transparently supported).
    let mut rows: Vec<(String, ThreadSummary)> = Vec::new();
    for id in hierarchy.collect_subtree(threads_book.id) {
        if id == threads_book.id {
            continue;
        }
        let Some(node) = hierarchy.get(id) else { continue; };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue; };
        let Ok(body) = std::str::from_utf8(&bytes) else { continue; };
        let summary = parse_thread_summary(body).unwrap_or_default();
        if let Some(s) = status_filter {
            if !summary.status.eq_ignore_ascii_case(s) {
                continue;
            }
        }
        if let Some(w) = weight_filter {
            if !summary.weight.eq_ignore_ascii_case(w) {
                continue;
            }
        }
        rows.push((node.title.clone(), summary));
    }

    if rows.is_empty() {
        eprintln!("no threads defined — run `inkhaven thread add <name>`");
        return Ok(());
    }
    let max_name = rows.iter().map(|(n, _)| n.chars().count()).max().unwrap_or(8);
    let name_w = max_name.max(8);
    println!(
        "  {:<width$}  {:>8}  {:>8}  {:>7}  {:>3}  {:>3}  {:>3}",
        "name", "status", "weight", "tension", "ch", "pl", "art",
        width = name_w,
    );
    println!("  {}", "-".repeat(name_w + 42));
    for (name, s) in &rows {
        println!(
            "  {:<width$}  {:>8}  {:>8}  {:>7}  {:>3}  {:>3}  {:>3}",
            name,
            s.status,
            s.weight,
            s.tension,
            s.characters.len(),
            s.places.len(),
            s.artefacts.len(),
            width = name_w,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_thread_body_includes_core_fields() {
        let body =
            seed_thread_body("inheritance subplot", None, "develop", "subplot");
        assert!(body.contains("title:"));
        assert!(body.contains("status:"));
        assert!(body.contains("weight:"));
        assert!(body.contains("opening:"));
        assert!(body.contains("midpoint:"));
        assert!(body.contains("payoff:"));
        assert!(body.contains("characters:"));
        assert!(body.contains("places:"));
        assert!(body.contains("artefacts:"));
        assert!(body.contains("related_threads:"));
        assert!(body.contains("tension:"));
        assert!(body.contains("register:"));
        assert!(body.contains("notes:"));
        // CLI-supplied fields land pre-populated.
        assert!(body.contains("\"inheritance subplot\""));
        assert!(body.contains("\"develop\""));
        assert!(body.contains("\"subplot\""));
    }

    #[test]
    fn seed_thread_body_uses_name_when_title_omitted() {
        let body =
            seed_thread_body("redemption arc", None, "setup", "major");
        assert!(body.contains("title:         \"redemption arc\""));
    }

    #[test]
    fn seed_thread_body_prefers_explicit_title() {
        let body = seed_thread_body(
            "redemption arc",
            Some("The Long Way Home"),
            "setup",
            "major",
        );
        assert!(body.contains("title:         \"The Long Way Home\""));
        // Slug-derived name doesn't appear in `title:`
        // even though it stays the paragraph's
        // identifier.
        assert!(!body.contains("title:         \"redemption arc\""));
    }

    #[test]
    fn seed_thread_body_parses_as_valid_hjson() {
        // The verbose commented template is easy to
        // typo into invalid HJSON; pin it here so
        // a future field addition can't ship a
        // template the user can't open.
        let body = seed_thread_body("test", None, "setup", "major");
        let _: serde_hjson::Value = serde_hjson::from_str(&body)
            .expect("seeded thread body must parse as HJSON");
    }

    #[test]
    fn parse_thread_summary_extracts_filter_fields() {
        let body = r#"{
  title: "Inheritance subplot"
  status: "develop"
  weight: "subplot"
  tension: 7
  characters: ["aerin", "filip"]
  places: ["marketplace"]
  artefacts: []
  related_threads: ["redemption-arc"]
}"#;
        let s = parse_thread_summary(body).unwrap();
        assert_eq!(s.title, "Inheritance subplot");
        assert_eq!(s.status, "develop");
        assert_eq!(s.weight, "subplot");
        assert_eq!(s.tension, 7);
        assert_eq!(s.characters.len(), 2);
        assert_eq!(s.places.len(), 1);
        assert_eq!(s.artefacts.len(), 0);
        assert_eq!(s.related_threads.len(), 1);
    }

    #[test]
    fn parse_thread_summary_returns_none_on_empty() {
        assert!(parse_thread_summary("").is_none());
        assert!(parse_thread_summary("   \n  ").is_none());
    }

    #[test]
    fn escape_hjson_handles_quotes_and_backslashes() {
        assert_eq!(escape_hjson(r#"he said "hi""#), r#"he said \"hi\""#);
        assert_eq!(escape_hjson(r"a\b"), r"a\\b");
    }
}
