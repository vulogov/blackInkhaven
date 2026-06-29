//! WORLD-6 (W-P6) — `inkhaven world utopia-*` handlers: the utopian/dystopian
//! coherence checker CLI. Stage 1 runs by default; Stage 2 (pairing) and Stage 3
//! (entailment) run on demand. Findings are advisory; the cap informs, never
//! blocks. `utopia-check` exits 1 on any chain-logic finding, 2 on any
//! entailment violation (pre-submission gate).

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::world::utopia::{
    FindingType, UtopiaStore, detect_premise_groups, run_stage1, run_stage2_group,
    run_stage3_group,
};

use super::WorldCommand;

// Defaults (the `utopia:` config block threads overrides in W-P9).
const GAP_THRESHOLD: usize = 1;
const STAGE3_MIN_WORDS: usize = 200;

pub fn run(project: &Path, cmd: WorldCommand) -> Result<()> {
    match cmd {
        WorldCommand::UtopiaCheck { book, stage, group, json } => {
            check(project, book.as_deref(), stage.as_deref(), group.as_deref(), json)
        }
        WorldCommand::UtopiaModel { book, group, json } => {
            model(project, book.as_deref(), group.as_deref(), json)
        }
        WorldCommand::UtopiaSuppress { finding, reason, book } => {
            suppress(project, book.as_deref(), &finding, &reason)
        }
        WorldCommand::UtopiaRefresh { book, stage } => {
            refresh(project, book.as_deref(), stage.unwrap_or(1))
        }
    }
}

fn open(project: &Path) -> Result<(ProjectLayout, Config, Store, Hierarchy)> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let h = Hierarchy::load(&store)?;
    Ok((layout, cfg, store, h))
}

fn ustore(store: &Store) -> Result<UtopiaStore> {
    UtopiaStore::open(store.project_root()).map_err(|e| Error::Store(e.to_string()))
}

fn check(
    project: &Path,
    book_name: Option<&str>,
    stage: Option<&str>,
    group_filter: Option<&str>,
    json: bool,
) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "utopia").map_err(Error::Store)?;
    let us = ustore(&store)?;

    // Stage 1 always (lazy).
    run_stage1(&us, &cfg, &layout, &h, book, GAP_THRESHOLD).map_err(|e| Error::Store(e.to_string()))?;
    let groups: Vec<_> = detect_premise_groups(&h, &layout, GAP_THRESHOLD)
        .into_iter()
        .filter(|g| group_filter.is_none_or(|f| g.name == f))
        .collect();

    let run2 = matches!(stage, Some("2") | Some("all"));
    let run3 = matches!(stage, Some("3") | Some("all"));
    if run2 {
        for g in &groups {
            run_stage2_group(&us, &cfg, &book.slug, &g.name)
                .map_err(|e| Error::Store(e.to_string()))?;
        }
    }
    if run3 {
        for g in &groups {
            run_stage3_group(&us, &cfg, &layout, &h, book, &g.name, STAGE3_MIN_WORDS, usize::MAX)
                .map_err(|e| Error::Store(e.to_string()))?;
        }
    }

    let findings = us.findings(&book.slug, true).map_err(|e| Error::Store(e.to_string()))?;

    if json {
        let arr: Vec<serde_json::Value> = findings
            .iter()
            .map(|f| {
                serde_json::json!({
                    "type": f.finding_type.as_code(),
                    "domain": f.finding_domain.as_code(),
                    "group": f.premise_group,
                    "description": f.description,
                    "chapter": f.chapter_ord,
                    "para_id": f.para_id,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
    } else {
        println!("Utopia coherence check — `{}` [{}]", book.title, cfg.language);
        println!("{}", "─".repeat(72));
        let chain: Vec<_> = findings
            .iter()
            .filter(|f| f.finding_type != FindingType::EntailmentViolation)
            .collect();
        let prose: Vec<_> = findings
            .iter()
            .filter(|f| f.finding_type == FindingType::EntailmentViolation)
            .collect();
        println!("Chain-logic findings ({}):", chain.len());
        for f in &chain {
            println!("  [{}] {}", f.premise_group, f.description);
        }
        println!("Entailment findings ({}):", prose.len());
        for f in &prose {
            let loc = f.chapter_ord.map(|c| format!("ch.{c} ")).unwrap_or_default();
            println!("  {loc}{}", f.description);
        }
        println!("{}", "─".repeat(72));
        if !run2 {
            println!("Stage 2 (full pairing) not run. Re-run with `--stage 2` or `--stage all`.");
        }
        if !run3 {
            println!("Stage 3 (entailment scan) not run. Re-run with `--stage 3` or `--stage all`.");
        }
    }

    // Exit code gate: 2 if any entailment violation, else 1 if any chain finding.
    let has_entail = findings.iter().any(|f| f.finding_type == FindingType::EntailmentViolation);
    let has_chain = findings.iter().any(|f| f.finding_type != FindingType::EntailmentViolation);
    if has_entail {
        std::process::exit(2);
    }
    if has_chain {
        std::process::exit(1);
    }
    Ok(())
}

fn model(project: &Path, book_name: Option<&str>, group_filter: Option<&str>, json: bool) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "utopia").map_err(Error::Store)?;
    let us = ustore(&store)?;
    run_stage1(&us, &cfg, &layout, &h, book, GAP_THRESHOLD).map_err(|e| Error::Store(e.to_string()))?;
    let claims = us.all_claims(&book.slug).map_err(|e| Error::Store(e.to_string()))?;
    let claims: Vec<_> = claims
        .into_iter()
        .filter(|c| group_filter.is_none_or(|f| c.premise_group == f))
        .collect();

    if json {
        let arr: Vec<serde_json::Value> = claims
            .iter()
            .map(|c| {
                serde_json::json!({
                    "group": c.premise_group,
                    "type": c.claim_type.as_code(),
                    "text": c.claim_text,
                    "source_para_id": c.source_para_id,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&arr).unwrap_or_default());
        return Ok(());
    }

    println!("Utopia model — `{}`", book.title);
    println!("{}", "─".repeat(60));
    if claims.is_empty() {
        println!("  (no claims — add para:utopia-* paragraphs to the World book, then re-run)");
    }
    let mut last_group = String::new();
    for c in &claims {
        if c.premise_group != last_group {
            println!("\n[{}]", c.premise_group);
            last_group = c.premise_group.clone();
        }
        println!("  {} {:<12} {}", c.claim_type.glyph(), c.claim_type.label(), c.claim_text);
    }
    println!("{}", "─".repeat(60));
    Ok(())
}

fn suppress(project: &Path, book_name: Option<&str>, finding: &str, reason: &str) -> Result<()> {
    let (_layout, _cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "utopia").map_err(Error::Store)?;
    let us = ustore(&store)?;
    if us
        .suppress_finding(&book.slug, finding, reason)
        .map_err(|e| Error::Store(e.to_string()))?
    {
        eprintln!("suppressed finding {finding}: {reason}");
        Ok(())
    } else {
        Err(Error::Store(format!("no finding with id `{finding}`")))
    }
}

fn refresh(project: &Path, book_name: Option<&str>, stage: u8) -> Result<()> {
    let (layout, cfg, store, h) = open(project)?;
    let book = super::resolve_user_book(&h, book_name, "utopia").map_err(Error::Store)?;
    let us = ustore(&store)?;
    match stage {
        3 => {
            us.clear_chapter_scans(&book.slug).map_err(|e| Error::Store(e.to_string()))?;
            let mut found = 0;
            for g in detect_premise_groups(&h, &layout, GAP_THRESHOLD) {
                let (f, _, _) = run_stage3_group(
                    &us, &cfg, &layout, &h, book, &g.name, STAGE3_MIN_WORDS, usize::MAX,
                )
                .map_err(|e| Error::Store(e.to_string()))?;
                found += f;
            }
            eprintln!("utopia refresh: Stage 3 re-scanned — {found} entailment finding(s)");
        }
        _ => {
            us.invalidate_all_stage1(&book.slug).map_err(|e| Error::Store(e.to_string()))?;
            let n = run_stage1(&us, &cfg, &layout, &h, book, GAP_THRESHOLD)
                .map_err(|e| Error::Store(e.to_string()))?;
            eprintln!("utopia refresh: Stage 1 re-extracted — {n} claim(s)");
        }
    }
    Ok(())
}
