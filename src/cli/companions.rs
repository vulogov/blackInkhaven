//! `inkhaven companions` (COMPANIONS-1) — the examined-authorship cockpit: one
//! terminal view of the open findings across the Inner family (Socrates +
//! Editor), the shared intent ledger + pending promotions, and today's LLM cost
//! per companion. Read-only; reuses the per-feature stores and the cost gather.

use std::path::Path;

use crate::config::Config;
use crate::error::Result;
use crate::inner_editor::{EditorSeverity, InnerEditorStore};
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::inner_socrates::types::Severity as SocSeverity;
use crate::project::ProjectLayout;

pub fn run(project: &Path) -> Result<()> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();

    println!("Examined authorship — {}\n", project.display());

    // ── open findings (persisted; the latest engagement per paragraph) ──
    println!("Open findings:");
    if let Ok(s) = InnerSocratesStore::open_for_project(project) {
        let fs = s.list_findings().unwrap_or_default();
        let (mut notice, mut inquiry, mut probe) = (0, 0, 0);
        for f in &fs {
            match f.finding.severity {
                SocSeverity::Notice => notice += 1,
                SocSeverity::Inquiry => inquiry += 1,
                SocSeverity::Probe => probe += 1,
            }
        }
        println!(
            "  Inner Socrates  \u{25c7}  Notice {notice} \u{b7} Inquiry {inquiry} \u{b7} Probe {probe}   (total {})",
            fs.len()
        );
    }
    if let Ok(s) = InnerEditorStore::open_for_project(project) {
        let fs = s.list_findings().unwrap_or_default();
        let (mut praise, mut note, mut concern) = (0, 0, 0);
        for f in &fs {
            match f.finding.severity {
                EditorSeverity::Praise => praise += 1,
                EditorSeverity::Note => note += 1,
                EditorSeverity::Concern => concern += 1,
            }
        }
        println!(
            "  Inner Editor    \u{270e}  Praise {praise} \u{b7} Note {note} \u{b7} Concern {concern}     (total {})",
            fs.len()
        );
    }
    // WORLD fact-check findings are advisory — they run live into the Output pane
    // and aren't persisted; `inkhaven world` reports the world-consistency health.
    println!("  World           \u{25cf}  fact-check is advisory (live in Output; see `inkhaven world`)");
    println!();

    // ── shared intent ledger + pending promotions ──
    if let Ok(s) = InnerSocratesStore::open_for_project(project) {
        let intents = s.list_intents().map(|v| v.len()).unwrap_or(0);
        let soc = s.promotion_candidates(5).map(|v| v.len()).unwrap_or(0);
        let edi = InnerEditorStore::open_for_project(project)
            .ok()
            .and_then(|e| e.promotion_candidates(5).ok())
            .map(|v| v.len())
            .unwrap_or(0);
        println!(
            "Intent ledger:    {intents} entr(ies) \u{b7} {} promotion candidate(s) pending (socrates {soc}, editor {edi})",
            soc + edi
        );
        println!();
    }

    // ── today's LLM cost, per companion ──
    let report = crate::cli::cost::gather(
        project,
        &day,
        cfg.cost.world_daily_call_cap,
        cfg.cost.inner_socrates_daily_call_cap,
        cfg.inner_editor.llm.editor_engagement.max_calls_per_day,
    );
    for line in crate::cli::cost::render_lines(&report) {
        println!("{line}");
    }
    Ok(())
}
