//! `inkhaven inner-editor …` — the terminal surface for the Inner Editor
//! literary/stylistic companion. `engage` runs one Editor pass over a paragraph
//! (the same engine the TUI chord/ambient trigger uses); `findings` inspects
//! what's persisted; `config show` and `usage` report state. Read-only except
//! the engagement, which records findings + usage like the TUI path.

use std::path::Path;

use uuid::Uuid;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::inner_editor::types::EditorSeverity;
use crate::inner_editor::{self, EngageInput, InnerEditorStore};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::Store;

use super::{EditorConfigCommand, EditorFindingsCommand, InnerEditorCommand};

pub fn run(project: &Path, cmd: InnerEditorCommand) -> Result<()> {
    match cmd {
        InnerEditorCommand::Engage { text, paragraph, force } => {
            engage(project, text, paragraph, force)
        }
        InnerEditorCommand::Findings(c) => match c {
            EditorFindingsCommand::List { severity } => findings_list(project, severity),
            EditorFindingsCommand::History { paragraph } => findings_history(project, paragraph),
        },
        InnerEditorCommand::Config(c) => match c {
            EditorConfigCommand::Show => config_show(project),
        },
        InnerEditorCommand::Usage => usage(project),
    }
}

/// Run one engagement over a literal `--text` or a `--paragraph <id>`.
fn engage(
    project: &Path,
    text: Option<String>,
    paragraph: Option<String>,
    force: bool,
) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;

    let (prose, preceding, paragraph_id, chapter_id) = match (text, paragraph) {
        (Some(t), _) => (t, Vec::new(), None, None),
        (None, Some(pid_s)) => {
            let pid = Uuid::parse_str(&pid_s)
                .map_err(|e| Error::Config(format!("bad paragraph id `{pid_s}`: {e}")))?;
            let store = Store::open(layout.clone(), &cfg)?;
            let h = Hierarchy::load(&store)?;
            let gc = inner_editor::gather_context(
                &store,
                &h,
                pid,
                cfg.inner_editor.context.preceding_paragraphs,
            )
            .ok_or_else(|| Error::Config(format!("paragraph `{pid}` not found")))?;
            (gc.prose, gc.preceding, Some(pid), gc.chapter_id)
        }
        (None, None) => {
            return Err(Error::Config("give --text \"…\" or --paragraph <id>".into()))
        }
    };

    let outcome = inner_editor::engage(EngageInput {
        project: project.to_path_buf(),
        paragraph_id,
        chapter_id,
        prose,
        preceding,
        language: String::new(),
        snapshot_id: None,
        system_override: None,
        force,
    })
    .map_err(|e| Error::Store(format!("{e}")))?;

    if let Some(note) = &outcome.note {
        eprintln!("{note}");
    }
    if outcome.findings.is_empty() {
        println!("\u{270e} no observations");
        return Ok(());
    }
    for f in &outcome.findings {
        println!("\u{270e} {} [{}] {}", f.severity.label(), f.category.label(), f.observation);
        if let Some(ev) = &f.evidence {
            println!("    evidence: {ev}");
        }
    }
    let supp = if outcome.suppressed > 0 {
        format!(" · {} suppressed by declared intent", outcome.suppressed)
    } else {
        String::new()
    };
    println!("\n{} observation(s){supp}", outcome.findings.len());
    Ok(())
}

fn findings_list(project: &Path, severity: Option<String>) -> Result<()> {
    let store = InnerEditorStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-editor store: {e}")))?;
    let filter = severity.map(|s| EditorSeverity::from_id(&s));
    let all = store.list_findings().map_err(|e| Error::Store(format!("{e}")))?;
    let shown: Vec<_> = all
        .iter()
        .filter(|sf| filter.is_none_or(|fl| sf.finding.severity == fl))
        .collect();
    if shown.is_empty() {
        println!("no findings");
        return Ok(());
    }
    for sf in &shown {
        println!(
            "\u{270e} {} [{}] {}",
            sf.finding.severity.label(),
            sf.finding.category.label(),
            sf.finding.observation
        );
    }
    println!("\n{} finding(s)", shown.len());
    Ok(())
}

fn findings_history(project: &Path, paragraph: String) -> Result<()> {
    let pid = Uuid::parse_str(&paragraph)
        .map_err(|e| Error::Config(format!("bad paragraph id `{paragraph}`: {e}")))?;
    let store = InnerEditorStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-editor store: {e}")))?;
    let hist = store.findings_history(pid).map_err(|e| Error::Store(format!("{e}")))?;
    if hist.is_empty() {
        println!("no history for paragraph {pid}");
        return Ok(());
    }
    for (at, f) in &hist {
        let when = chrono::DateTime::<chrono::Utc>::from_timestamp(*at, 0)
            .map(|d| d.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_default();
        println!(
            "{when}  \u{270e} {} [{}] {}",
            f.severity.label(),
            f.category.label(),
            f.observation
        );
    }
    println!("\n{} finding(s) over time.", hist.len());
    Ok(())
}

fn config_show(project: &Path) -> Result<()> {
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let ie = &cfg.inner_editor;
    println!("Inner Editor — {}", if ie.enabled { "enabled" } else { "disabled" });
    println!(
        "  tone: {} · verbosity: {} · praise: {}",
        ie.persona.tone, ie.persona.verbosity, ie.persona.praise_frequency
    );
    println!(
        "  genre-aware: {} (genre: {}) · belief stance: {}",
        ie.persona.genre_aware,
        cfg.genre.as_deref().unwrap_or("none declared"),
        ie.persona.belief_stance_enabled
    );
    println!(
        "  idle: {}s · cooldown: {}s · max findings/¶: {}",
        ie.engagement.idle_threshold_seconds,
        ie.engagement.cooldown_seconds,
        ie.engagement.max_findings_per_paragraph
    );
    println!("  context: {} preceding ¶", ie.context.preceding_paragraphs);
    let thresh_note = if ie.output.severity_threshold == "note" {
        " (Praise hidden by default)"
    } else {
        ""
    };
    println!("  output threshold: {}{thresh_note}", ie.output.severity_threshold);
    let tuning = inner_editor::resolve_tuning(&ie.persona);
    let cats: Vec<&str> = tuning.active_categories.iter().map(|c| c.id()).collect();
    println!("  active categories ({}): {}", cats.len(), cats.join(", "));
    Ok(())
}

fn usage(project: &Path) -> Result<()> {
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let store = InnerEditorStore::open_for_project(project)
        .map_err(|e| Error::Store(format!("inner-editor store: {e}")))?;
    let rows = store.llm_usage_today(&day).map_err(|e| Error::Store(format!("{e}")))?;
    println!("Inner Editor usage — {day}");
    if rows.is_empty() {
        println!("  no calls today");
    }
    for (sub, calls) in &rows {
        println!("  {sub}: {calls} call(s)");
    }
    let eng = store
        .llm_calls_today(&day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET)
        .unwrap_or(0);
    let cap = cfg.inner_editor.llm.editor_engagement.max_calls_per_day;
    println!("  daily cap (informative): {eng}/{cap}");
    Ok(())
}
