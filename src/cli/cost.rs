//! `inkhaven cost` — the unified AI cost dashboard (road to 1.4.0).
//!
//! The LLM-using subsystems each track their own daily call budget in their own
//! store. This reads them into one view: per-budget calls-today vs the daily cap,
//! plus a note on the per-run-only timeline elaboration. Read-only aggregation; it
//! changes no caps and enforces nothing. The `render_lines` output is shared by the
//! CLI here and the TUI panel (P1).

use std::path::Path;

use anyhow::Result;

use crate::inner_socrates::storage::InnerSocratesStore;
use crate::world::storage::WorldStore;

/// One persisted daily-capped LLM budget. The name is owned so the dashboard can
/// enumerate dynamically-keyed analytical-thread sub-budgets, not just a fixed set.
pub struct CostEntry {
    pub name: String,
    pub calls_today: i64,
    pub daily_cap: i64,
}

pub struct CostReport {
    /// `YYYY-MM-DD` the tallies are for.
    pub day: String,
    pub entries: Vec<CostEntry>,
}

impl CostReport {
    pub fn total_calls(&self) -> i64 {
        self.entries.iter().map(|e| e.calls_today).sum()
    }
}

/// Read each subsystem's call tally for `day` (gracefully zero when a store is
/// absent). The caps come from the stores' shared consts, so they match the
/// preflights exactly.
///
/// Extensible by design: the world slow track is a single budget, but the Inner
/// Socrates store keys usage by **sub-budget**, so every analytical thread that
/// records a cost-capped slow pass under its own key appears here automatically.
/// To surface a new analytical thread's AI cost, record its calls via
/// `InnerSocratesStore::record_llm_call(day, "<thread-key>")` — no dashboard change
/// needed.
pub fn gather(project: &Path, day: &str) -> CostReport {
    let mut entries = vec![CostEntry {
        name: "world fact-check (slow)".to_string(),
        calls_today: WorldStore::open_for_project(project)
            .ok()
            .and_then(|s| s.llm_calls_today(day).ok())
            .unwrap_or(0),
        daily_cap: WorldStore::DAILY_CALL_CAP,
    }];

    // Inner Socrates: the canonical slow track always shown, plus any other
    // analytical-thread sub-budgets recorded today.
    let usage = InnerSocratesStore::open_for_project(project)
        .ok()
        .and_then(|s| s.llm_usage_today(day).ok())
        .unwrap_or_default();
    let mut have_slow = false;
    let mut extra: Vec<(String, i64)> = Vec::new();
    for (sub, calls) in usage {
        if sub == InnerSocratesStore::SLOW_SUB_BUDGET {
            have_slow = true;
            entries.push(CostEntry {
                name: "inner socrates (slow)".to_string(),
                calls_today: calls,
                daily_cap: InnerSocratesStore::DAILY_CALL_CAP,
            });
        } else {
            extra.push((sub, calls));
        }
    }
    if !have_slow {
        entries.push(CostEntry {
            name: "inner socrates (slow)".to_string(),
            calls_today: 0,
            daily_cap: InnerSocratesStore::DAILY_CALL_CAP,
        });
    }
    for (sub, calls) in extra {
        entries.push(CostEntry {
            name: format!("inner socrates · {sub}"),
            calls_today: calls,
            daily_cap: InnerSocratesStore::DAILY_CALL_CAP,
        });
    }

    CostReport { day: day.to_string(), entries }
}

/// A 20-cell usage bar + percentage for a `used / cap` budget.
fn bar(used: i64, cap: i64) -> String {
    if cap <= 0 {
        return String::new();
    }
    const WIDTH: i64 = 20;
    let used = used.max(0);
    let filled = ((used * WIDTH) / cap).clamp(0, WIDTH);
    let pct = (used * 100 / cap).clamp(0, 999);
    format!(
        "[{}{}] {pct}%",
        "█".repeat(filled as usize),
        "░".repeat((WIDTH - filled) as usize)
    )
}

/// Render the report as lines (shared by the CLI + the TUI panel).
pub fn render_lines(report: &CostReport) -> Vec<String> {
    let mut out = vec![format!("AI cost — LLM calls today ({})", report.day), String::new()];
    for e in &report.entries {
        out.push(format!(
            "  {:<26} {:>3} / {:<3}  {}",
            e.name,
            e.calls_today,
            e.daily_cap,
            bar(e.calls_today, e.daily_cap)
        ));
    }
    out.push(String::new());
    out.push(format!("  {:<26} {:>3}", "total slow-track calls", report.total_calls()));
    out.push(String::new());
    out.push("  timeline elaboration: per-run cap only (not a daily budget)".into());
    out
}

pub fn run(project: &Path) -> Result<()> {
    crate::project::ProjectLayout::new(project).require_initialized()?;
    let day = chrono::Utc::now().format("%Y-%m-%d").to_string();
    for line in render_lines(&gather(project, &day)) {
        println!("{line}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_clamps_and_percents() {
        assert!(bar(0, 200).contains("0%"));
        assert!(bar(100, 200).contains("50%"));
        // Over-cap: the bar fills completely but the percentage reports the truth.
        let over = bar(300, 200);
        assert!(over.contains("150%"), "got {over}");
        assert!(!over.contains('░'), "over-cap bar is full");
        assert_eq!(bar(5, 0), "", "a zero cap renders no bar");
    }

    #[test]
    fn report_totals_and_renders() {
        let r = CostReport {
            day: "2026-06-24".into(),
            entries: vec![
                CostEntry { name: "a".into(), calls_today: 3, daily_cap: 200 },
                // An arbitrary future analytical-thread sub-budget renders the same.
                CostEntry { name: "inner socrates · drift".into(), calls_today: 7, daily_cap: 150 },
            ],
        };
        assert_eq!(r.total_calls(), 10);
        let lines = render_lines(&r);
        assert!(lines.iter().any(|l| l.contains("2026-06-24")));
        assert!(lines.iter().any(|l| l.contains("total slow-track calls")));
        assert!(lines.iter().any(|l| l.contains("10")));
        assert!(lines.iter().any(|l| l.contains("elaboration")));
    }
}
