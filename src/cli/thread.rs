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

use super::{ThreadCommand, ThreadExportFormat};

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
        ThreadCommand::Doctor { json } => doctor(project, json),
        ThreadCommand::Export { format, output } => {
            export(project, format, output.as_deref())
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

/// public shim for the TUI's
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

/// full thread record used by
/// export + doctor.  Mirrors the proposal §3
/// schema fully (vs the lighter `ThreadSummary`
/// the `list` subcommand reads).
#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize)]
struct ThreadFull {
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    weight: String,
    #[serde(default)]
    opening: String,
    #[serde(default)]
    midpoint: String,
    #[serde(default)]
    payoff: String,
    #[serde(default)]
    characters: Vec<String>,
    #[serde(default)]
    places: Vec<String>,
    #[serde(default)]
    artefacts: Vec<String>,
    #[serde(default)]
    related_threads: Vec<String>,
    #[serde(default)]
    tension: i32,
    #[serde(default)]
    register: String,
    #[serde(default)]
    notes: String,
}

fn parse_thread_full(body: &str) -> Option<ThreadFull> {
    if body.trim().is_empty() {
        return None;
    }
    serde_hjson::from_str(body).ok()
}

/// `inkhaven thread doctor`.
fn doctor(project: &Path, json: bool) -> Result<()> {
    use crate::store::node::NodeKind;
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
                "Threads system book missing — re-open the project to seed it".into(),
            )
        })?;
    // Collect every thread + its reverse-link
    // count.  Mirrors the picker's tally in
    // tui::app::threads_impl.
    let mut threads: Vec<(String, ThreadFull, uuid::Uuid)> = Vec::new();
    for id in hierarchy.collect_subtree(threads_book.id) {
        if id == threads_book.id {
            continue;
        }
        let Some(node) = hierarchy.get(id) else { continue; };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue; };
        let body = std::str::from_utf8(&bytes).unwrap_or("");
        let parsed = parse_thread_full(body).unwrap_or_default();
        threads.push((node.title.clone(), parsed, id));
    }
    let mut link_tally: std::collections::HashMap<uuid::Uuid, usize> =
        std::collections::HashMap::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        for target in &node.linked_paragraphs {
            *link_tally.entry(*target).or_insert(0) += 1;
        }
    }
    // Distributions + blind spots.
    let mut status_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut weight_counts: std::collections::BTreeMap<String, usize> =
        std::collections::BTreeMap::new();
    let mut zero_links: Vec<&str> = Vec::new();
    let mut payoff_unfired: Vec<&str> = Vec::new();
    let mut dormant: Vec<&str> = Vec::new();
    let mut tension_avg_sum = 0i64;
    let mut tension_avg_n = 0usize;
    for (name, t, id) in &threads {
        let status_key = if t.status.is_empty() {
            "(empty)".to_string()
        } else {
            t.status.clone()
        };
        *status_counts.entry(status_key).or_insert(0) += 1;
        let weight_key = if t.weight.is_empty() {
            "(empty)".to_string()
        } else {
            t.weight.clone()
        };
        *weight_counts.entry(weight_key).or_insert(0) += 1;
        let links = link_tally.get(id).copied().unwrap_or(0);
        if links == 0 && !t.status.eq_ignore_ascii_case("setup") {
            zero_links.push(name.as_str());
        }
        if t.status.eq_ignore_ascii_case("payoff") && links == 0 {
            payoff_unfired.push(name.as_str());
        }
        // Dormant = status implies activity but
        // few links project-wide.  Heuristic: 0
        // or 1 link for a `develop` thread.
        if t.status.eq_ignore_ascii_case("develop") && links <= 1 {
            dormant.push(name.as_str());
        }
        tension_avg_sum += t.tension as i64;
        tension_avg_n += 1;
    }
    let tension_avg = if tension_avg_n > 0 {
        tension_avg_sum as f32 / tension_avg_n as f32
    } else {
        0.0
    };

    if json {
        use serde_json::json;
        let report = json!({
            "thread_count": threads.len(),
            "status_distribution": status_counts,
            "weight_distribution": weight_counts,
            "tension_avg": tension_avg,
            "blind_spots": {
                "zero_links": zero_links,
                "payoff_unfired": payoff_unfired,
                "dormant": dormant,
            },
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
        return Ok(());
    }

    println!("Thread doctor");
    println!();
    println!("  threads defined : {}", threads.len());
    println!("  avg tension     : {:.1}", tension_avg);
    println!();
    println!("  status:");
    for (k, v) in &status_counts {
        println!("    {k:<10} {v}");
    }
    println!();
    println!("  weight:");
    for (k, v) in &weight_counts {
        println!("    {k:<10} {v}");
    }
    println!();
    println!("Blind spots");
    if zero_links.is_empty() && payoff_unfired.is_empty() && dormant.is_empty() {
        println!("  (none detected)");
    } else {
        if !zero_links.is_empty() {
            println!("  ZERO LINKS — status past `setup` but no paragraph links:");
            for t in &zero_links {
                println!("    · {t}");
            }
        }
        if !payoff_unfired.is_empty() {
            println!("  PAYOFF UNFIRED — status `payoff` but no paragraph links:");
            for t in &payoff_unfired {
                println!("    · {t}");
            }
        }
        if !dormant.is_empty() {
            println!("  DORMANT — status `develop` but 0-1 links project-wide:");
            for t in &dormant {
                println!("    · {t}");
            }
        }
    }
    Ok(())
}

/// `inkhaven thread export`.
fn export(
    project: &Path,
    format: ThreadExportFormat,
    output: Option<&Path>,
) -> Result<()> {
    use crate::store::node::NodeKind;
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
                "Threads system book missing — re-open the project to seed it".into(),
            )
        })?;
    let mut threads: Vec<(String, ThreadFull)> = Vec::new();
    for id in hierarchy.collect_subtree(threads_book.id) {
        if id == threads_book.id {
            continue;
        }
        let Some(node) = hierarchy.get(id) else { continue; };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue; };
        let body = std::str::from_utf8(&bytes).unwrap_or("");
        let parsed = parse_thread_full(body).unwrap_or_default();
        threads.push((node.title.clone(), parsed));
    }
    threads.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let rendered: Vec<u8> = match format {
        ThreadExportFormat::Json => {
            let json: Vec<serde_json::Value> = threads
                .iter()
                .map(|(name, t)| {
                    serde_json::json!({
                        "name": name,
                        "title": t.title,
                        "status": t.status,
                        "weight": t.weight,
                        "opening": t.opening,
                        "midpoint": t.midpoint,
                        "payoff": t.payoff,
                        "characters": t.characters,
                        "places": t.places,
                        "artefacts": t.artefacts,
                        "related_threads": t.related_threads,
                        "tension": t.tension,
                        "register": t.register,
                        "notes": t.notes,
                    })
                })
                .collect();
            let mut out = serde_json::to_vec_pretty(&json)
                .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
            out.push(b'\n');
            out
        }
        ThreadExportFormat::Csv => {
            let mut s = String::new();
            s.push_str(
                "name,title,status,weight,tension,opening,midpoint,payoff,characters,places,artefacts,related_threads,register,notes\n",
            );
            for (name, t) in &threads {
                s.push_str(&format!(
                    "{},{},{},{},{},{},{},{},{},{},{},{},{},{}\n",
                    csv_field(name),
                    csv_field(&t.title),
                    csv_field(&t.status),
                    csv_field(&t.weight),
                    t.tension,
                    csv_field(&t.opening),
                    csv_field(&t.midpoint),
                    csv_field(&t.payoff),
                    csv_field(&t.characters.join(";")),
                    csv_field(&t.places.join(";")),
                    csv_field(&t.artefacts.join(";")),
                    csv_field(&t.related_threads.join(";")),
                    csv_field(&t.register),
                    csv_field(&t.notes),
                ));
            }
            s.into_bytes()
        }
        ThreadExportFormat::Markdown => {
            let mut s = String::new();
            s.push_str("# Thread inventory\n\n");
            for (name, t) in &threads {
                let title = if t.title.is_empty() {
                    name.clone()
                } else {
                    t.title.clone()
                };
                s.push_str(&format!("## {title}\n\n"));
                if !t.status.is_empty() || !t.weight.is_empty() {
                    s.push_str(&format!(
                        "* **status**: {} · **weight**: {} · **tension**: {}\n",
                        t.status, t.weight, t.tension
                    ));
                }
                if !t.opening.is_empty() {
                    s.push_str(&format!("* **opening**: {}\n", t.opening));
                }
                if !t.midpoint.is_empty() {
                    s.push_str(&format!("* **midpoint**: {}\n", t.midpoint));
                }
                if !t.payoff.is_empty() {
                    s.push_str(&format!("* **payoff**: {}\n", t.payoff));
                }
                if !t.characters.is_empty() {
                    s.push_str(&format!(
                        "* characters: {}\n",
                        t.characters.join(", ")
                    ));
                }
                if !t.places.is_empty() {
                    s.push_str(&format!(
                        "* places: {}\n",
                        t.places.join(", ")
                    ));
                }
                if !t.artefacts.is_empty() {
                    s.push_str(&format!(
                        "* artefacts: {}\n",
                        t.artefacts.join(", ")
                    ));
                }
                if !t.related_threads.is_empty() {
                    s.push_str(&format!(
                        "* related: {}\n",
                        t.related_threads.join(", ")
                    ));
                }
                if !t.register.is_empty() {
                    s.push_str(&format!("* register: {}\n", t.register));
                }
                if !t.notes.is_empty() {
                    s.push_str(&format!("\n{}\n", t.notes));
                }
                s.push('\n');
            }
            s.into_bytes()
        }
    };

    match output {
        Some(path) => {
            std::fs::write(path, &rendered).map_err(|e| {
                Error::Config(format!("write {}: {e}", path.display()))
            })?;
            eprintln!("wrote {} bytes to {}", rendered.len(), path.display());
        }
        None => {
            use std::io::Write;
            std::io::stdout()
                .write_all(&rendered)
                .map_err(|e| Error::Config(format!("stdout write: {e}")))?;
        }
    }
    Ok(())
}

/// CSV quoting for the export — RFC 4180.
fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
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

    #[test]
    fn csv_field_quotes_when_needed() {
        assert_eq!(csv_field("plain"), "plain");
        assert_eq!(csv_field("with, comma"), "\"with, comma\"");
        assert_eq!(csv_field("with \"quote\""), "\"with \"\"quote\"\"\"");
    }

    #[test]
    fn parse_thread_full_extracts_every_field() {
        let body = r#"{
  title: "Inheritance subplot"
  status: "develop"
  weight: "subplot"
  opening: "An heir arrives"
  midpoint: "Will is contested"
  payoff: "Truth revealed"
  characters: ["aerin", "filip"]
  places: ["marketplace"]
  artefacts: ["the seal"]
  related_threads: ["redemption-arc"]
  tension: 7
  register: "literary"
  notes: "Worldbuilding rationale"
}"#;
        let t = parse_thread_full(body).unwrap();
        assert_eq!(t.title, "Inheritance subplot");
        assert_eq!(t.status, "develop");
        assert_eq!(t.weight, "subplot");
        assert_eq!(t.opening, "An heir arrives");
        assert_eq!(t.midpoint, "Will is contested");
        assert_eq!(t.payoff, "Truth revealed");
        assert_eq!(t.tension, 7);
        assert_eq!(t.characters.len(), 2);
        assert_eq!(t.places.len(), 1);
        assert_eq!(t.artefacts, vec!["the seal".to_string()]);
        assert_eq!(t.related_threads, vec!["redemption-arc".to_string()]);
        assert_eq!(t.register, "literary");
        assert_eq!(t.notes, "Worldbuilding rationale");
    }

    #[test]
    fn parse_thread_full_returns_none_on_empty() {
        assert!(parse_thread_full("").is_none());
        assert!(parse_thread_full("  \n").is_none());
    }
}
