//! TDOC-1 — `inkhaven docs verify`. Runs the manuscript's `verify`-marked code
//! blocks through their configured runners and reports pass / fail / skip, exiting
//! non-zero when any block fails (for a pre-release / CI check).

use std::collections::BTreeSet;
use std::path::Path;

use anyhow::{bail, Result};
use uuid::Uuid;

use crate::cli::DocsCommand;
use crate::config::Config;
use crate::docs::{extract_verifiable, run_block, CodeBlock, VerifyOutcome};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;

pub fn run(project: &Path, cmd: DocsCommand) -> Result<()> {
    match cmd {
        DocsCommand::Verify { book_name, paragraph, dry_run, yes } => {
            verify(project, book_name.as_deref(), paragraph.as_deref(), dry_run, yes)
        }
        DocsCommand::Links { book_name, external } => links(project, book_name.as_deref(), external),
        DocsCommand::Review { book_name, floor, since } => {
            review(project, book_name.as_deref(), &floor, since.as_deref())
        }
    }
}

/// TDOC-5 — the canonical status ladder (lowest → highest).
const STATUS_NAMES: &[&str] = &["none", "napkin", "first", "second", "third", "final", "ready"];

/// TDOC-5 — a review/currency dashboard.
fn review(project: &Path, book_name: Option<&str>, floor: &str, since: Option<&str>) -> Result<()> {
    let (_cfg, _store, h) = open(project)?;
    let floor_idx = STATUS_NAMES
        .iter()
        .position(|n| n.eq_ignore_ascii_case(floor.trim()))
        .ok_or_else(|| anyhow::anyhow!("unknown --floor `{floor}`. Valid: {}", STATUS_NAMES.join(", ")))?;

    // `--since <ref>`: files changed since a git ref (relative to the project).
    let changed: Option<std::collections::HashSet<String>> = since.map(|r| git_changed_files(project, r));

    let mut total = 0usize;
    let mut ready = 0usize;
    let mut below: Vec<(String, String, bool)> = Vec::new(); // (loc, status, changed)
    let mut changed_count = 0usize;

    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        if let Some(name) = book_name {
            if !book.title.eq_ignore_ascii_case(name.trim()) {
                continue;
            }
        }
        println!("\ndocs review — {}", book.title);
        for chapter in h.children_of(Some(book.id)).into_iter().filter(|n| n.kind == NodeKind::Chapter) {
            let mut counts: std::collections::BTreeMap<usize, usize> = std::collections::BTreeMap::new();
            let mut ch_below = 0usize;
            for id in h.collect_subtree(chapter.id) {
                let Some(node) = h.get(id) else { continue };
                if node.kind != NodeKind::Paragraph || node.event.is_some() {
                    continue;
                }
                total += 1;
                let idx = crate::export::status_ladder_index(node.status.as_deref());
                *counts.entry(idx).or_default() += 1;
                if idx >= 6 {
                    ready += 1;
                }
                let is_changed = node
                    .file
                    .as_deref()
                    .zip(changed.as_ref())
                    .is_some_and(|(f, set)| set.contains(f));
                if is_changed {
                    changed_count += 1;
                }
                if idx < floor_idx {
                    ch_below += 1;
                    below.push((h.slug_path(node), STATUS_NAMES[idx].to_string(), is_changed));
                }
            }
            if counts.is_empty() {
                continue;
            }
            let breakdown: Vec<String> = counts
                .iter()
                .rev()
                .map(|(idx, n)| format!("{} {}", STATUS_NAMES[*idx], n))
                .collect();
            let flag = if ch_below > 0 { format!("   [{ch_below} below `{floor}`]") } else { String::new() };
            println!("  {}\n    {}{flag}", chapter.title, breakdown.join(" · "));
        }
    }

    if !below.is_empty() {
        println!("\nBelow `{floor}` (needs work):");
        for (loc, status, is_changed) in &below {
            let mark = if *is_changed { "   ← changed since ref" } else { "" };
            println!("  - {loc}  ({status}){mark}");
        }
    }

    let since_note = match since {
        Some(r) => format!(" · {changed_count} changed since {r}"),
        None => String::new(),
    };
    println!(
        "\ndocs review: {total} paragraphs · {ready} ready · {} below `{floor}`{since_note}",
        below.len()
    );
    if !below.is_empty() {
        bail!("{} paragraph(s) below `{floor}`", below.len());
    }
    Ok(())
}

/// Files changed since a git ref, relative to the project. Empty (with a note) when
/// the project is not a git repo or the ref is unknown.
fn git_changed_files(project: &Path, since: &str) -> std::collections::HashSet<String> {
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(project)
        .args(["diff", "--name-only", "--relative", since, "--", "."])
        .output();
    match out {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout)
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        _ => {
            eprintln!("docs review: --since needs a git repo and a valid ref; skipping change detection");
            std::collections::HashSet::new()
        }
    }
}

/// TDOC-2 — check link integrity: internal cross-references always, external URLs
/// with `--external`.
fn links(project: &Path, book_name: Option<&str>, external: bool) -> Result<()> {
    use std::collections::BTreeMap;
    let (_cfg, store, h) = open(project)?;

    // Internal cross-references (project-wide, deterministic).
    let internal = crate::docs::links::check_internal(&h);
    for f in &internal {
        println!("  ✗ {} → {} ({})", f.loc, f.target, f.reason);
    }

    // External URLs (opt-in, network) — one location per URL.
    let mut dead_external = 0usize;
    if external {
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        for book in h
            .children_of(None)
            .into_iter()
            .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        {
            if let Some(name) = book_name {
                if !book.title.eq_ignore_ascii_case(name.trim()) {
                    continue;
                }
            }
            for id in h.collect_subtree(book.id) {
                let Some(node) = h.get(id) else { continue };
                if node.kind != NodeKind::Paragraph {
                    continue;
                }
                let body = read_body(&store, id);
                let loc = h.slug_path(node);
                for url in crate::docs::links::extract_urls(&body) {
                    seen.entry(url).or_insert_with(|| loc.clone());
                }
            }
        }
        if seen.is_empty() {
            println!("docs links: no external URLs found.");
        } else {
            let rt = tokio::runtime::Runtime::new().map_err(|e| anyhow::anyhow!("tokio: {e}"))?;
            let dead: Vec<(String, String, String)> = rt.block_on(async {
                let client = crate::research::deadlinks::client().map_err(|e| anyhow::anyhow!("http client: {e}"))?;
                let mut out = Vec::new();
                for (url, loc) in &seen {
                    if let Some(reason) = crate::research::deadlinks::check_web(&client, url).await {
                        out.push((loc.clone(), url.clone(), reason));
                    }
                }
                Ok::<_, anyhow::Error>(out)
            })?;
            dead_external = dead.len();
            for (loc, url, reason) in &dead {
                println!("  ✗ {loc} → {url} ({reason})");
            }
            println!("  ({} external URL(s) checked)", seen.len());
        }
    }

    let broken = internal.len() + dead_external;
    let ext_note = if external { "" } else { " (internal only — pass --external to check URLs)" };
    println!("\ndocs links: {} internal · {dead_external} external broken{ext_note}", internal.len());
    if broken > 0 {
        bail!("{broken} broken link(s)");
    }
    Ok(())
}

fn open(project: &Path) -> Result<(Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;
    Ok((cfg, store, hierarchy))
}

/// A `para:code` paragraph and its verify-marked blocks.
struct Target {
    breadcrumb: String,
    blocks: Vec<CodeBlock>,
}

fn verify(
    project: &Path,
    book_name: Option<&str>,
    paragraph: Option<&str>,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let (cfg, store, h) = open(project)?;
    let vcfg = &cfg.docs.verify;

    // Gather manuscript `para:code` paragraphs (non-system books) and their
    // verify-marked blocks.
    let mut targets: Vec<Target> = Vec::new();
    for book in h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
    {
        if let Some(name) = book_name {
            if !book.title.eq_ignore_ascii_case(name.trim()) {
                continue;
            }
        }
        for id in h.collect_subtree(book.id) {
            let Some(node) = h.get(id) else { continue };
            if node.kind != NodeKind::Paragraph {
                continue;
            }
            if !node.tags.iter().any(|t| t == "para:code") {
                continue;
            }
            let breadcrumb = h.slug_path(node);
            if let Some(p) = paragraph {
                if breadcrumb != p {
                    continue;
                }
            }
            let body = read_body(&store, id);
            let blocks: Vec<CodeBlock> =
                extract_verifiable(&body).into_iter().filter(|b| b.verify).collect();
            if !blocks.is_empty() {
                targets.push(Target { breadcrumb, blocks });
            }
        }
    }

    let total_blocks: usize = targets.iter().map(|t| t.blocks.len()).sum();
    if total_blocks == 0 {
        println!("docs verify: no `verify`-marked code blocks found.");
        println!("  Mark a `para:code` listing's fence, e.g. ```rust verify```, and configure a runner.");
        return Ok(());
    }

    // Dry run — preview what would execute, never run anything.
    if dry_run {
        println!("docs verify --dry-run: {total_blocks} block(s) would run:\n");
        for t in &targets {
            for b in &t.blocks {
                let cmd = vcfg
                    .runners
                    .get(&b.lang)
                    .map(|c| c.replace("{file}", "<code>.tmp"))
                    .unwrap_or_else(|| format!("(no runner for `{}` — would skip)", b.lang));
                println!("  {} [{}]  →  {cmd}", t.breadcrumb, b.lang);
            }
        }
        return Ok(());
    }

    // Safety gates: the feature is opt-in, and executing configured commands
    // needs an explicit confirmation (so a freshly-cloned project can't run code).
    if !vcfg.enabled {
        bail!(
            "docs.verify is off. Enable it in inkhaven.hjson (`docs: {{ verify: {{ enabled: true }} }}`) \
             and configure runners, then re-run. Use `--dry-run` to preview without executing."
        );
    }
    if !yes {
        let langs: BTreeSet<&str> = targets.iter().flat_map(|t| &t.blocks).map(|b| b.lang.as_str()).collect();
        println!(
            "docs verify: {total_blocks} block(s) marked `verify` in language(s): {}.",
            langs.into_iter().collect::<Vec<_>>().join(", ")
        );
        println!("These runners will execute code with your privileges:");
        for (lang, cmd) in &vcfg.runners {
            println!("  {lang}: {cmd}");
        }
        bail!("re-run with --yes to execute, or --dry-run to preview.");
    }

    // Run.
    let (mut passed, mut failed, mut skipped) = (0usize, 0usize, 0usize);
    for t in &targets {
        for b in &t.blocks {
            match run_block(b, vcfg) {
                VerifyOutcome::Pass => {
                    passed += 1;
                    println!("  ✓ {} [{}]", t.breadcrumb, b.lang);
                }
                VerifyOutcome::Fail { detail } => {
                    failed += 1;
                    println!("  ✗ {} [{}]", t.breadcrumb, b.lang);
                    for line in detail.lines() {
                        println!("      {line}");
                    }
                }
                VerifyOutcome::Skipped { reason } => {
                    skipped += 1;
                    println!("  – {} [{}] — {reason}", t.breadcrumb, b.lang);
                }
                VerifyOutcome::Errored { reason } => {
                    failed += 1;
                    println!("  ! {} [{}] — {reason}", t.breadcrumb, b.lang);
                }
            }
        }
    }

    println!("\ndocs verify: {passed} passed · {failed} failed · {skipped} skipped");
    if failed > 0 {
        bail!("{failed} code block(s) failed verification");
    }
    Ok(())
}

fn read_body(store: &Store, id: Uuid) -> String {
    store
        .get_content(id)
        .ok()
        .flatten()
        .map(|b| String::from_utf8_lossy(&b).into_owned())
        .unwrap_or_default()
}
