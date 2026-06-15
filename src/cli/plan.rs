//! 1.3.2 PLANNING-1 P0 — `inkhaven plan` subcommand.
//!
//! `plan init` scaffolds a story-structure framework's beats into the
//! `Planning` system book as HJSON-fronted paragraphs (the Threads
//! pattern).  The deterministic coverage/pacing report (`plan check`) and
//! the AI analyze pass arrive in later phases.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::planning::Framework;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::{InsertPosition, Store, SYSTEM_TAG_PLANNING};

use super::PlanCommand;

pub fn run(project: &Path, cmd: PlanCommand) -> Result<()> {
    match cmd {
        PlanCommand::Init { framework } => init(project, framework.as_deref()),
    }
}

fn init(project: &Path, framework: Option<&str>) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;

    let fw = match framework {
        Some(s) => Framework::parse(s).ok_or_else(|| {
            Error::Store(format!(
                "plan init: unknown framework `{s}` \
                 (three_act|save_the_cat|story_circle|hero_journey|seven_point)"
            ))
        })?,
        None => Framework::ThreeAct,
    };

    let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    let planning = planning_book(&h)?;
    // Refuse to clobber an existing structure.
    if !h.children_of(Some(planning.id)).is_empty() {
        return Err(Error::Store(
            "plan init: the Planning book already has beats — remove them to re-init".into(),
        ));
    }

    let beats = fw.seed_beats();
    for beat in &beats {
        // Reload before each create so later beats see the earlier ones
        // (slug + order) — the same pattern facts/threads use.
        let h = Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
        let planning = planning_book(&h)?;
        let mut node = store.create_node(
            &cfg,
            &h,
            NodeKind::Paragraph,
            &beat.beat,
            Some(&planning),
            None,
            InsertPosition::End,
        )?;
        let body = crate::planning::beat_body(beat);
        node.content_type = Some("hjson".to_string());
        // Disk write first (the editor reads the .typ off disk), then the
        // bdslib-only metadata/content update — the Threads pattern.
        if let Some(rel) = &node.file {
            let abs = store.project_root().join(rel);
            std::fs::write(&abs, body.as_bytes()).map_err(Error::Io)?;
        }
        store
            .update_paragraph_content(&mut node, body.as_bytes())
            .map_err(|e| Error::Store(format!("plan init: seed beat: {e}")))?;
    }

    println!(
        "plan init: seeded {} {} beats into the Planning book",
        beats.len(),
        fw.label(),
    );
    eprintln!("  next: map each beat to a chapter, then `inkhaven plan check` (P1)");
    Ok(())
}

fn planning_book(h: &Hierarchy) -> Result<crate::store::node::Node> {
    h.iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_PLANNING)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store("plan init: Planning book missing — reopen the project to seed it".into())
        })
}
