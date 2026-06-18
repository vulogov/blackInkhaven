//! 1.3.6 EDITORIAL-1 P0 — `inkhaven edit`: run the editorial detectors,
//! read the already-computed sidecars, and fold in `plan check`'s
//! structural findings, then hand them to the pure aggregator
//! ([`crate::editorial`]) for one ranked worklist.
//!
//! Deterministic: no live AI here — the AI findings (Facts / tension) are
//! read from their sidecars where present (the `--deep` tier that *runs*
//! them arrives in P3).

use std::path::Path;

use crate::cli::doctor_scan::{self, ScanClass};
use crate::editorial::{self, EditorialFinding};
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;

/// Run every editorial detector, read the sidecars, fold in `plan check`,
/// resolve locations against the hierarchy, and aggregate — the shared
/// entry point for the CLI and the TUI cockpit. `only` filters by category.
pub fn collect(
    project: &Path,
    book_name: Option<&str>,
    only: Option<&[String]>,
    include_deferred: bool,
) -> Result<editorial::EditorialReport> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;

    let mut raw: Vec<EditorialFinding> = Vec::new();

    // 1) doctor's editorial classes — the default scan (deterministic) plus
    //    the opt-in unresolved-tension class (reads the tension ledger).
    let mut scan = doctor_scan::scan_project(project, None)?.findings;
    scan.extend(doctor_scan::scan_project(project, Some(ScanClass::UnresolvedTension))?.findings);
    raw.extend(scan.iter().filter_map(editorial::from_scan_finding));

    // 2) Facts-scan contradictions + internal-consistency conflicts
    //    (sidecars; empty if never run).
    if let Ok(facts) = crate::facts_scan::FactScanReport::load(&layout.root) {
        raw.extend(facts.findings.iter().map(editorial::from_fact_finding));
    }
    if let Ok(check) = crate::facts_scan::FactCheckReport::load(&layout.root) {
        raw.extend(check.conflicts.iter().map(editorial::from_fact_conflict));
    }
    // 2b) semantic-drift contradictions (1.3.10 sidecar; empty if never run).
    if let Ok(drift) = crate::drift::DriftReport::load(&layout.root) {
        raw.extend(drift.conflicts.iter().map(editorial::from_drift_conflict));
    }

    // 3) `plan check` structural findings (skipped when there's no plan).
    raw.extend(plan_warnings(project, book_name).into_iter().map(|w| editorial::from_plan_warning(&w)));

    // 4) deterministic prose-style detectors (show-don't-tell) over each
    //    paragraph + resolve every finding's location to a node id (so the
    //    cockpit can jump / rewrite). One store open.
    if let Ok(cfg) = crate::config::Config::load_layered(&layout.config_path()) {
        if let Ok(store) = crate::store::Store::open(layout.clone(), &cfg) {
            if let Ok(h) = Hierarchy::load(&store) {
                raw.extend(prose_style_findings(&cfg, &store, &h));
                resolve_locations(&mut raw, &h, &layout);
            }
        }
    }

    // --only category filter (after every source, incl. the prose scan).
    if let Some(cats) = only {
        raw.retain(|f| cats.iter().any(|c| c.trim().eq_ignore_ascii_case(&f.category)));
    }

    // Hide deferred findings (P2) unless explicitly included.
    let before = raw.len();
    if !include_deferred {
        let dismissed = editorial::Dismissed::load(&layout.root);
        if !dismissed.fingerprints.is_empty() {
            raw.retain(|f| !dismissed.fingerprints.contains(&f.fingerprint()));
        }
    }
    let deferred = before - raw.len();

    let mut report = editorial::aggregate(raw);
    report.deferred = deferred;
    Ok(report)
}

/// Surface the deterministic **show-don't-tell** regex (the editor overlay's
/// detector) into the worklist: walk every user-book paragraph, run the
/// detector per line, and emit a finding per telling phrase with a
/// paragraph-and-char-range location (jumpable, `f`-rewritable). The overlay's
/// *enabled* flag governs the live underline, not this deliberate pass; only
/// an empty word-list (nothing to match) skips it.
fn prose_style_findings(
    cfg: &crate::config::Config,
    store: &crate::store::Store,
    h: &Hierarchy,
) -> Vec<EditorialFinding> {
    let sdt = crate::tui::style_warnings::ShowDontTellDetector::new(
        &cfg.editor.style_warnings.show_dont_tell,
        &cfg.language,
    );
    let anach = crate::tui::style_warnings::AnachronismDetector::new(
        &cfg.editor.style_warnings.anachronism,
    );
    let filter = crate::tui::style_warnings::FilterWordsDetector::new(
        &cfg.editor.style_warnings.filter_words,
        &cfg.language,
    );
    if sdt.is_empty() && anach.is_empty() && filter.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for book in h.iter().filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none()) {
        for chapter in h.children_of(Some(book.id)) {
            if chapter.kind != NodeKind::Chapter {
                continue;
            }
            let chap = if chapter.title.trim().is_empty() {
                chapter.slug.clone()
            } else {
                chapter.title.clone()
            };
            for pid in h.collect_subtree(chapter.id) {
                if h.get(pid).map(|n| n.kind) != Some(NodeKind::Paragraph) {
                    continue;
                }
                let Some(body) = store.get_content(pid).ok().flatten() else {
                    continue;
                };
                let text = String::from_utf8_lossy(&body);
                if !sdt.is_empty() {
                    out.extend(paragraph_show_tell_findings(&text, pid, &chap, &sdt));
                }
                if !anach.is_empty() {
                    out.extend(paragraph_anachronism_findings(&text, pid, &chap, &anach));
                }
                if !filter.is_empty() {
                    out.extend(paragraph_filter_word_findings(&text, pid, &chap, &filter));
                }
            }
        }
    }
    out
}

/// Map a paragraph's anachronistic terms into `EditorialFinding`s (one per
/// flagged word, with its paragraph-relative char range). Pure.
fn paragraph_anachronism_findings(
    text: &str,
    pid: uuid::Uuid,
    chapter: &str,
    det: &crate::tui::style_warnings::AnachronismDetector,
) -> Vec<EditorialFinding> {
    let mut out = Vec::new();
    let mut row_off = 0usize;
    for line in text.lines() {
        for hit in det.detect(line) {
            let term: String = line
                .chars()
                .skip(hit.col_start)
                .take(hit.col_end.saturating_sub(hit.col_start))
                .collect();
            let earliest = det.earliest(&term);
            let yr = earliest.map(|y| format!(" (not before {y})")).unwrap_or_default();
            out.push(EditorialFinding {
                category: "anachronism".into(),
                severity: editorial::Severity::Warn,
                location: editorial::Location {
                    chapter: Some(chapter.to_string()),
                    paragraph: Some(pid),
                    char_range: Some((row_off + hit.col_start, row_off + hit.col_end)),
                    path: None,
                },
                message: format!("anachronism: “{}” postdates the setting{yr}", term.trim()),
                hint: None,
                source: "style",
                autofixable: false,
            });
        }
        row_off += line.chars().count() + 1;
    }
    out
}

/// Map a paragraph's text through the show-don't-tell detector into
/// `EditorialFinding`s — one per telling phrase, with its paragraph-relative
/// char range. Pure (the detector + text in, findings out).
fn paragraph_show_tell_findings(
    text: &str,
    pid: uuid::Uuid,
    chapter: &str,
    det: &crate::tui::style_warnings::ShowDontTellDetector,
) -> Vec<EditorialFinding> {
    let mut out = Vec::new();
    let mut row_off = 0usize; // char offset of the line's start in the paragraph
    for line in text.lines() {
        for hit in det.detect(line) {
            let phrase: String = line
                .chars()
                .skip(hit.col_start)
                .take(hit.col_end.saturating_sub(hit.col_start))
                .collect();
            out.push(EditorialFinding {
                category: "show-tell".into(),
                severity: editorial::Severity::Info,
                location: editorial::Location {
                    chapter: Some(chapter.to_string()),
                    paragraph: Some(pid),
                    char_range: Some((row_off + hit.col_start, row_off + hit.col_end)),
                    path: None,
                },
                message: format!("telling: “{}” — show it instead", phrase.trim()),
                hint: None,
                source: "style",
                autofixable: false,
            });
        }
        row_off += line.chars().count() + 1; // + the newline
    }
    out
}

/// Map a paragraph's filter words into `EditorialFinding`s — one per flagged
/// word, with its paragraph-relative char range. Pure. Findings land at
/// `Info` severity (they sort last, after errors/warnings) so surfacing the
/// noisiest detector doesn't bury sharper findings. Span-rewritable via `f`.
fn paragraph_filter_word_findings(
    text: &str,
    pid: uuid::Uuid,
    chapter: &str,
    det: &crate::tui::style_warnings::FilterWordsDetector,
) -> Vec<EditorialFinding> {
    let mut out = Vec::new();
    let mut row_off = 0usize; // char offset of the line's start in the paragraph
    for line in text.lines() {
        for hit in det.detect(line) {
            let word: String = line
                .chars()
                .skip(hit.col_start)
                .take(hit.col_end.saturating_sub(hit.col_start))
                .collect();
            out.push(EditorialFinding {
                category: "filter".into(),
                severity: editorial::Severity::Info,
                location: editorial::Location {
                    chapter: Some(chapter.to_string()),
                    paragraph: Some(pid),
                    char_range: Some((row_off + hit.col_start, row_off + hit.col_end)),
                    path: None,
                },
                message: format!("filter word: “{}” — consider cutting", word.trim()),
                hint: None,
                source: "style",
                autofixable: false,
            });
        }
        row_off += line.chars().count() + 1; // + the newline
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anachronism_mapping_names_the_term_and_earliest_year() {
        let det = crate::tui::style_warnings::AnachronismDetector::new(
            &crate::config::AnachronismConfig { year: Some(1840), terms: Vec::new() },
        );
        let id = uuid::Uuid::now_v7();
        let f = paragraph_anachronism_findings("She glanced at her wristwatch.", id, "ch-1", &det);
        assert_eq!(f.len(), 1);
        let e = &f[0];
        assert_eq!(e.category, "anachronism");
        assert_eq!(e.severity, crate::editorial::Severity::Warn);
        assert_eq!(e.location.paragraph, Some(id));
        assert!(e.location.char_range.is_some());
        assert!(e.message.contains("wristwatch") && e.message.contains("1900"));
        assert!(!e.rewritable(), "anachronism is jump-only, not AI-rewritable");
    }

    #[test]
    fn show_tell_mapping_locates_the_phrase_and_is_rewritable() {
        let det = crate::tui::style_warnings::ShowDontTellDetector::new(
            &crate::config::ShowDontTellConfig::default(),
            "english",
        );
        // "was angry" is a copula + emotion adjective → a telling hit.
        let id = uuid::Uuid::now_v7();
        let f = paragraph_show_tell_findings("She was angry at the news.", id, "ch-1", &det);
        assert_eq!(f.len(), 1, "one telling phrase flagged");
        let e = &f[0];
        assert_eq!(e.category, "show-tell");
        assert_eq!(e.location.paragraph, Some(id));
        assert!(e.location.char_range.is_some(), "carries a char range");
        assert!(e.rewritable(), "show-tell + a paragraph → f-rewritable");
        assert!(e.message.contains("show it instead"));
    }

    #[test]
    fn filter_word_mapping_is_info_and_span_rewritable() {
        // default config → built-in english filter words (just/really/very/…)
        let det = crate::tui::style_warnings::FilterWordsDetector::new(
            &crate::config::FilterWordsConfig::default(),
            "english",
        );
        assert!(!det.is_empty(), "built-in english list populates the detector");
        let id = uuid::Uuid::now_v7();
        let f = paragraph_filter_word_findings("It was very cold.", id, "ch-1", &det);
        assert_eq!(f.len(), 1, "one filter word flagged");
        let e = &f[0];
        assert_eq!(e.category, "filter");
        assert_eq!(e.severity, crate::editorial::Severity::Info, "filter words sort last");
        assert_eq!(e.location.paragraph, Some(id));
        assert!(e.location.char_range.is_some(), "carries a char range");
        assert!(e.rewritable(), "filter + a paragraph → f-rewritable (span)");
        assert!(e.message.contains("very") && e.message.contains("cutting"));
    }
}

/// Fill `location.paragraph` (the jump target) from a `path` (file match) or
/// a `chapter` title (→ chapter node → its first paragraph). Best-effort.
fn resolve_locations(findings: &mut [EditorialFinding], h: &Hierarchy, layout: &ProjectLayout) {
    for f in findings.iter_mut() {
        if f.location.paragraph.is_some() {
            continue;
        }
        // by file path → the owning paragraph
        if let Some(p) = &f.location.path {
            let rel = Path::new(p)
                .strip_prefix(&layout.root)
                .map(|r| r.to_string_lossy().into_owned())
                .unwrap_or_else(|_| p.clone());
            if let Some(node) = h.iter().find(|n| n.file.as_deref() == Some(rel.as_str())) {
                f.location.paragraph = Some(node.id);
                continue;
            }
        }
        // by chapter title/slug → the chapter's first paragraph
        if let Some(ch) = &f.location.chapter {
            if let Some(chapter) = h.iter().find(|n| {
                n.kind == NodeKind::Chapter
                    && (n.title.eq_ignore_ascii_case(ch) || n.slug.eq_ignore_ascii_case(ch))
            }) {
                if let Some(first) = h.collect_subtree(chapter.id).into_iter().find(|id| {
                    h.get(*id).map(|n| n.kind == NodeKind::Paragraph).unwrap_or(false)
                }) {
                    f.location.paragraph = Some(first);
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    project: &Path,
    json: bool,
    only: Option<Vec<String>>,
    book_name: Option<&str>,
    show_deferred: bool,
    deep: bool,
    provider: Option<&str>,
) -> Result<()> {
    if deep {
        if json {
            return Err(Error::Store(
                "edit: --deep can't combine with --json (the AI scans print progress) — run the scans separately, then `edit --json`".into(),
            ));
        }
        deep_refresh(project, provider);
    }
    let report = collect(project, book_name, only.as_deref(), show_deferred)?;

    if json {
        let out = serde_json::to_string_pretty(&report)
            .map_err(|e| Error::Store(format!("edit: {e}")))?;
        println!("{out}");
    } else {
        render(&report);
    }
    Ok(())
}

/// `--deep` — run the AI scans that populate the editorial sidecars (Facts
/// contradictions, the tension ledger, the continuity bible, semantic drift),
/// each printing its own progress, so the next `collect` sees the fresh
/// semantic findings. A scan that can't run (no provider) is skipped with a
/// note — the pass degrades to deterministic-only rather than aborting.
fn deep_refresh(project: &Path, provider: Option<&str>) {
    eprintln!("edit --deep: refreshing AI sidecars (facts · tension · continuity · drift)…");
    let p = || provider.map(String::from);
    if let Err(e) = super::facts_scan::run(project, super::FactsCommand::Scan { provider: p(), json: false }) {
        eprintln!("  facts scan skipped: {e}");
    }
    if let Err(e) = super::tension::run(project, super::TensionCommand::Scan { provider: p() }) {
        eprintln!("  tension scan skipped: {e}");
    }
    if let Err(e) = super::continuity::run(project, super::ContinuityCommand::Extract { provider: p() }) {
        eprintln!("  continuity extract skipped: {e}");
    }
    if let Err(e) = super::drift::run(project, super::DriftCommand::Scan { provider: p(), json: false }) {
        eprintln!("  drift scan skipped: {e}");
    }
    eprintln!();
}

/// The `plan check` warnings for the book, or empty when there's no plan
/// (no beats / no chapters) — the editorial pass never errors on a project
/// that hasn't adopted the Planning Board.
fn plan_warnings(project: &Path, book_name: Option<&str>) -> Vec<String> {
    let layout = ProjectLayout::new(project);
    let Ok(cfg) = crate::config::Config::load_layered(&layout.config_path()) else {
        return Vec::new();
    };
    let Ok(store) = crate::store::Store::open(layout.clone(), &cfg) else {
        return Vec::new();
    };
    let Ok(h) = crate::store::hierarchy::Hierarchy::load(&store) else {
        return Vec::new();
    };
    let Ok(book) = super::resolve_user_book(&h, book_name, "edit") else {
        return Vec::new();
    };
    let book = book.clone();
    match super::plan::build_report(&store, &layout, &h, &book, 0.10) {
        Ok((report, _, _)) => report.warnings,
        Err(_) => Vec::new(),
    }
}

fn render(report: &editorial::EditorialReport) {
    if report.findings.is_empty() {
        println!("editorial pass: ✓ no findings — the manuscript reads clean");
        return;
    }
    println!(
        "EDITORIAL PASS · {} finding(s)  ({} error · {} warn · {} info)\n",
        report.findings.len(),
        report.errors,
        report.warnings,
        report.infos,
    );
    for f in &report.findings {
        println!(
            "  {} {:<10} {:<14} {}",
            f.severity.icon(),
            f.category,
            truncate(&f.location.label(), 14),
            f.message,
        );
        if let Some(hint) = &f.hint {
            println!("                                    ↳ {hint}");
        }
    }
    if report.deferred > 0 {
        println!(
            "\n  ({} deferred, hidden — `--show-deferred` to include, or clear in the cockpit)",
            report.deferred
        );
    }
    println!("\n  walk + jump + defer in the cockpit: `Ctrl+V Shift+R` (1.3.6)");
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    }
}
