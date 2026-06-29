//! MYTH-1 (M-P7) — the deterministic (zero-AI) checks: archetype vacancy and
//! absence, and motif-absent-from-final-act. These ride the `Ctrl+B Shift+C`
//! review pass; the LLM consistency/completeness checks (M-P9) run only on
//! explicit `inkhaven myth check`.

use std::collections::HashSet;

use anyhow::Result;

use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::pipeline::{chapter_count, character_mention_chapters};
use super::store::MythStore;
use super::{ArchetypeRole, FindingType, MythFinding};

/// Roster of the Characters book (direct children titles, lowercased).
fn roster_lc(h: &Hierarchy) -> HashSet<String> {
    crate::character::character_names(h).into_iter().map(|n| n.trim().to_lowercase()).collect()
}

/// The structural zone (1-based chapter range) a role is expected to inhabit,
/// for the absence check. `None` → "throughout".
fn role_zone(role: &ArchetypeRole, total: u32) -> Option<(u32, u32)> {
    if total == 0 {
        return None;
    }
    match role {
        // A Herald announces the call — expected in the opening act (first ~20%).
        ArchetypeRole::Herald => Some((1, (total / 5).max(1))),
        // A Shadow looms over the middle (~20%–80%).
        ArchetypeRole::Shadow => Some(((total / 5).max(1), (total * 4 / 5).max(1))),
        _ => None,
    }
}

/// Run the deterministic archetype + motif-final-act checks; persist and return
/// the findings. Clears the prior deterministic findings first.
pub(crate) fn run_deterministic_checks(
    store: &MythStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    book: &Node,
    final_act_pct: u32,
) -> Result<Vec<MythFinding>> {
    let now = chrono::Utc::now().to_rfc3339();
    for ft in [
        FindingType::ArchetypeVacant,
        FindingType::ArchetypeAbsent,
        FindingType::MotifAbsentFinalAct,
    ] {
        store.clear_findings_of_type(&book.slug, ft)?;
    }

    let total = chapter_count(h, book);
    let roster = roster_lc(h);
    let mut out: Vec<MythFinding> = Vec::new();
    let mut emit = |f: MythFinding, idx: usize| -> Result<()> {
        store.upsert_finding(&book.slug, &format!("{}:{idx}", f.finding_type.as_code()), &f, &now)?;
        out.push(f);
        Ok(())
    };

    // ── archetypes ──────────────────────────────────────────────────────────────
    for (i, a) in store.archetypes(&book.slug)?.iter().enumerate() {
        let name = a.character_name.trim();
        if name.is_empty() || !roster.contains(&name.to_lowercase()) {
            emit(
                MythFinding {
                    finding_type: FindingType::ArchetypeVacant,
                    description: if name.is_empty() {
                        format!("{} role has no character assigned", a.role.as_code())
                    } else {
                        format!("{} role's character `{name}` is not in the Characters book", a.role.as_code())
                    },
                    evidence: None,
                    entry_para_id: Some(a.para_id.clone()),
                    chapter_ord: None,
                    suppressed: false,
                },
                i,
            )?;
            continue;
        }
        let chapters = character_mention_chapters(layout, h, book, name);
        // Overall absence: the mapped character barely appears.
        if chapters.len() < 3 {
            emit(
                MythFinding {
                    finding_type: FindingType::ArchetypeAbsent,
                    description: format!(
                        "{} ({name}) appears in only {} chapter(s) — thin for an archetype role",
                        a.role.as_code(),
                        chapters.len()
                    ),
                    evidence: None,
                    entry_para_id: Some(a.para_id.clone()),
                    chapter_ord: chapters.first().copied(),
                    suppressed: false,
                },
                i,
            )?;
        } else if let Some((lo, hi)) = role_zone(&a.role, total) {
            // Zone absence: present, but not in the structural zone the role wants.
            let in_zone = chapters.iter().any(|c| *c >= lo && *c <= hi);
            if !in_zone {
                emit(
                    MythFinding {
                        finding_type: FindingType::ArchetypeAbsent,
                        description: format!(
                            "{} ({name}) does not appear in its expected zone (ch.{lo}–{hi}); appears in ch.{}",
                            a.role.as_code(),
                            chapters.iter().map(|c| c.to_string()).collect::<Vec<_>>().join(",")
                        ),
                        evidence: None,
                        entry_para_id: Some(a.para_id.clone()),
                        chapter_ord: Some(lo),
                        suppressed: false,
                    },
                    i,
                )?;
            }
        }
    }

    // ── motifs absent from the final act ────────────────────────────────────────
    if total > 0 {
        let final_act_start = total.saturating_sub((total * final_act_pct / 100).max(1)) + 1;
        for (i, m) in store.motifs(&book.slug)?.iter().enumerate() {
            let chapters = store.motif_chapters(&book.slug, &m.para_id)?;
            if chapters.is_empty() {
                continue; // never tagged — not a final-act problem, just undetected
            }
            if !chapters.iter().any(|c| *c >= final_act_start) {
                let last = chapters.iter().max().copied().unwrap_or(0);
                emit(
                    MythFinding {
                        finding_type: FindingType::MotifAbsentFinalAct,
                        description: format!(
                            "motif \"{}\" has no occurrence in the final act (ch.{final_act_start}–{total}); last at ch.{last}",
                            m.name
                        ),
                        evidence: None,
                        entry_para_id: Some(m.para_id.clone()),
                        chapter_ord: Some(final_act_start),
                        suppressed: false,
                    },
                    1000 + i,
                )?;
            }
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_zone_bounds() {
        assert_eq!(role_zone(&ArchetypeRole::Herald, 20), Some((1, 4)));
        assert_eq!(role_zone(&ArchetypeRole::Shadow, 20), Some((4, 16)));
        assert_eq!(role_zone(&ArchetypeRole::Mentor, 20), None);
        assert_eq!(role_zone(&ArchetypeRole::Herald, 0), None);
    }
}
