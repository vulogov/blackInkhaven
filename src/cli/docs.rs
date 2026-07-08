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
