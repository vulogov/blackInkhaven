//! 1.2.15+ Phase D.1 — project-wide problem scan.
//!
//! Extends the existing `inkhaven doctor` informational
//! dump (TTS voices, typst engine, dep versions, etc.)
//! with a structured scan over the project tree + DB.
//! Each finding has a `class`, `severity`, optional
//! `path`, and a human-readable `detail` string.
//!
//! Classes implemented in D.1 — all disk-side, no DB
//! mutation:
//!
//!   * `ZeroByteFile` — `.typ` file on disk is 0
//!     bytes.  Probably a save failure or a power
//!     loss truncation; the user's prose for that
//!     paragraph is gone.
//!   * `OrphanParagraphRow` — DB has a paragraph
//!     row whose `file` rel-path doesn't resolve
//!     to anything on disk.
//!   * `MissingReferencedFile` — DB row's `file`
//!     field is set, the path resolves under the
//!     project root, but `fs::metadata` returns
//!     NotFound.  Same shape as OrphanParagraphRow
//!     but kept separate so a future
//!     PendingPaperOrphan check can distinguish
//!     "row points to nothing" from "row's path is
//!     malformed".
//!   * `CorruptCommentsSidecar` — `<para>.comments.
//!     json` parses to invalid JSON.  User
//!     comments for that paragraph are unreadable
//!     until fixed.
//!
//! DB-side classes (FTS index mismatch, vector
//! index mismatch, content-hash drift) land in
//! D.2 / a follow-up — they need the Store handle
//! beyond `Hierarchy::load`.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::Store;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanClass {
    ZeroByteFile,
    OrphanParagraphRow,
    MissingReferencedFile,
    CorruptCommentsSidecar,
}

impl ScanClass {
    /// Lower-case kebab name for CLI `--class` and
    /// JSON output.
    pub fn slug(&self) -> &'static str {
        match self {
            ScanClass::ZeroByteFile => "zero-byte-file",
            ScanClass::OrphanParagraphRow => "orphan-paragraph-row",
            ScanClass::MissingReferencedFile => "missing-referenced-file",
            ScanClass::CorruptCommentsSidecar => "corrupt-comments-sidecar",
        }
    }

    /// Parse from the CLI `--class <name>` argument.
    pub fn from_slug(s: &str) -> Option<Self> {
        Some(match s {
            "zero-byte-file" => ScanClass::ZeroByteFile,
            "orphan-paragraph-row" => ScanClass::OrphanParagraphRow,
            "missing-referenced-file" => ScanClass::MissingReferencedFile,
            "corrupt-comments-sidecar" => ScanClass::CorruptCommentsSidecar,
            _ => return None,
        })
    }

    pub const ALL: [ScanClass; 4] = [
        ScanClass::ZeroByteFile,
        ScanClass::OrphanParagraphRow,
        ScanClass::MissingReferencedFile,
        ScanClass::CorruptCommentsSidecar,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScanSeverity {
    /// User data lost OR irrecoverable from this
    /// state — block CI on this.
    Critical,
    /// User data at risk OR data-integrity drift —
    /// surface to the user, recommend a fix.
    Warning,
    /// FYI — nothing to fix urgently but worth
    /// knowing about.
    Info,
}

impl ScanSeverity {
    pub fn slug(&self) -> &'static str {
        match self {
            ScanSeverity::Critical => "critical",
            ScanSeverity::Warning => "warning",
            ScanSeverity::Info => "info",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanFinding {
    pub class: ScanClass,
    pub severity: ScanSeverity,
    /// Project-relative or absolute path the
    /// finding points at.  Absent for findings
    /// that don't map to a single file (currently
    /// none, but reserved for future DB-only
    /// findings).
    pub path: Option<String>,
    /// Free-form one-line summary.  Stable across
    /// invocations so users can grep / dedupe.
    pub detail: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanReport {
    /// Inkhaven version that produced the report.
    pub version: String,
    /// UTC ISO 8601 with seconds resolution.
    pub generated_at: String,
    pub project_root: String,
    pub findings: Vec<ScanFinding>,
}

impl ScanReport {
    pub fn new(project_root: &Path) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            generated_at: chrono::Utc::now()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
            project_root: project_root.display().to_string(),
            findings: Vec::new(),
        }
    }

    /// Count findings at or above the given severity.
    pub fn count_at_or_above(&self, severity: ScanSeverity) -> usize {
        self.findings
            .iter()
            .filter(|f| severity_at_or_above(f.severity, severity))
            .count()
    }
}

fn severity_at_or_above(have: ScanSeverity, want: ScanSeverity) -> bool {
    let rank = |s| match s {
        ScanSeverity::Info => 1,
        ScanSeverity::Warning => 2,
        ScanSeverity::Critical => 3,
    };
    rank(have) >= rank(want)
}

/// Run the scan across every selected class.
/// `selected = None` runs all classes.
pub fn scan_project(
    project: &Path,
    selected: Option<ScanClass>,
) -> Result<ScanReport> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy =
        crate::store::hierarchy::Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;

    let mut report = ScanReport::new(&layout.root);

    let run = |c: ScanClass| selected.map_or(true, |s| s == c);

    if run(ScanClass::ZeroByteFile) {
        report.findings.extend(scan_zero_byte_files(&layout, &hierarchy));
    }
    if run(ScanClass::OrphanParagraphRow) || run(ScanClass::MissingReferencedFile) {
        // The two classes share most of the walk.
        for finding in scan_orphans_and_missing(&layout, &hierarchy) {
            if run(finding.class) {
                report.findings.push(finding);
            }
        }
    }
    if run(ScanClass::CorruptCommentsSidecar) {
        report.findings.extend(scan_corrupt_comments(&layout, &hierarchy));
    }

    Ok(report)
}

fn scan_zero_byte_files(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    let mut out: Vec<ScanFinding> = Vec::new();
    for node in hierarchy.iter() {
        let Some(rel) = node.file.as_ref() else { continue };
        if !rel.ends_with(".typ") {
            continue;
        }
        let abs = layout.root.join(rel);
        let Ok(md) = std::fs::metadata(&abs) else { continue };
        if md.len() == 0 {
            out.push(ScanFinding {
                class: ScanClass::ZeroByteFile,
                severity: ScanSeverity::Critical,
                path: Some(abs.display().to_string()),
                detail: format!(
                    "paragraph `{}` resolves to a 0-byte file — prose lost",
                    node.slug,
                ),
            });
        }
    }
    out
}

fn scan_orphans_and_missing(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    let mut out: Vec<ScanFinding> = Vec::new();
    for node in hierarchy.iter() {
        let Some(rel) = node.file.as_ref() else { continue };
        let abs = layout.root.join(rel);
        match std::fs::metadata(&abs) {
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // "Orphan paragraph row" vs.
                // "missing referenced file" is a
                // taxonomic distinction the
                // proposal kept: if the rel-path
                // is plausibly malformed (empty,
                // dot-segments, etc.) we tag it
                // MissingReferencedFile; otherwise
                // OrphanParagraphRow.  In practice
                // both produce the same fix.
                let class = if rel.contains("..") || rel.is_empty() {
                    ScanClass::MissingReferencedFile
                } else {
                    ScanClass::OrphanParagraphRow
                };
                out.push(ScanFinding {
                    class,
                    severity: ScanSeverity::Warning,
                    path: Some(abs.display().to_string()),
                    detail: format!(
                        "paragraph row `{}` points at missing file {}",
                        node.slug,
                        abs.display(),
                    ),
                });
            }
            Err(e) => {
                out.push(ScanFinding {
                    class: ScanClass::MissingReferencedFile,
                    severity: ScanSeverity::Warning,
                    path: Some(abs.display().to_string()),
                    detail: format!(
                        "paragraph row `{}` -> {}: {e}",
                        node.slug,
                        abs.display(),
                    ),
                });
            }
        }
    }
    out
}

fn scan_corrupt_comments(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    let mut out: Vec<ScanFinding> = Vec::new();
    for node in hierarchy.iter() {
        let Some(rel) = node.file.as_ref() else { continue };
        if !rel.ends_with(".typ") {
            continue;
        }
        let abs = layout.root.join(rel);
        let sidecar = sidecar_path_for(&abs);
        if !sidecar.exists() {
            continue;
        }
        let Ok(raw) = std::fs::read_to_string(&sidecar) else {
            continue;
        };
        if raw.trim().is_empty() {
            continue;
        }
        if serde_json::from_str::<serde_json::Value>(&raw).is_err() {
            out.push(ScanFinding {
                class: ScanClass::CorruptCommentsSidecar,
                severity: ScanSeverity::Warning,
                path: Some(sidecar.display().to_string()),
                detail: format!(
                    "comments sidecar for `{}` doesn't parse as JSON",
                    node.slug,
                ),
            });
        }
    }
    out
}

/// `<file>.typ` → `<file>.typ.comments.json`.
/// Mirrors the editor's `crate::tui::comments::
/// sidecar_path` shape (the tui module is closed
/// to non-tui callers, so we re-derive the same
/// extension here).
fn sidecar_path_for(typ_path: &Path) -> PathBuf {
    let mut s = typ_path.as_os_str().to_os_string();
    s.push(".comments.json");
    PathBuf::from(s)
}

/// 1.2.15+ Phase D.2 — apply one finding's repair
/// in-place.  Returns a one-line summary of what
/// was done (which the caller logs + prints).
///
/// Each fix is irreversible for the file-touching
/// cases (delete row + file).  The caller is
/// responsible for confirming with the user
/// before calling — `doctor::run_autofix` does the
/// prompting; this fn just applies.
pub fn apply_fix(
    project: &Path,
    finding: &ScanFinding,
) -> Result<String> {
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg).map_err(|e| Error::Store(e.to_string()))?;
    let hierarchy =
        crate::store::hierarchy::Hierarchy::load(&store).map_err(|e| Error::Store(e.to_string()))?;
    match finding.class {
        ScanClass::ZeroByteFile
        | ScanClass::OrphanParagraphRow
        | ScanClass::MissingReferencedFile => {
            // Resolve the finding back to a node
            // via the rel-path embedded in path.
            // The finding's path is absolute; strip
            // the project root prefix to get rel.
            let abs = finding
                .path
                .as_deref()
                .ok_or_else(|| Error::Store("finding has no path".into()))?;
            let abs_path = std::path::PathBuf::from(abs);
            let rel = abs_path
                .strip_prefix(&layout.root)
                .map_err(|e| Error::Store(format!("path {} not under project root: {e}", abs)))?
                .to_string_lossy()
                .into_owned();
            let mut to_delete: Vec<uuid::Uuid> = Vec::new();
            for node in hierarchy.iter() {
                if node.file.as_deref() == Some(rel.as_str()) {
                    to_delete.push(node.id);
                }
            }
            if to_delete.is_empty() {
                return Err(Error::Store(format!(
                    "no DB row matches {rel} — was the project mutated between scan and fix?"
                )));
            }
            store
                .delete_subtree(std::path::Path::new(&rel), &to_delete)
                .map_err(|e| Error::Store(format!("delete row {rel}: {e}")))?;
            Ok(format!(
                "deleted {} DB row(s) + file {} ({})",
                to_delete.len(),
                rel,
                finding.class.slug()
            ))
        }
        ScanClass::CorruptCommentsSidecar => {
            let abs = finding
                .path
                .as_deref()
                .ok_or_else(|| Error::Store("finding has no path".into()))?;
            let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%S").to_string();
            let dest = format!("{abs}.corrupt-{stamp}.bak");
            std::fs::rename(abs, &dest).map_err(Error::Io)?;
            Ok(format!(
                "moved corrupt sidecar {} → {}",
                abs, dest
            ))
        }
    }
}

/// Append one line to `<project>/.inkhaven/doctor.log`
/// recording the fix that was applied.  Format
/// mirrors the health log: UTC | OUTCOME | CLASS |
/// detail.  Silent on I/O errors (log is
/// diagnostic, not load-bearing).
pub fn log_fix(project: &Path, finding: &ScanFinding, outcome: &Result<String>) {
    let path = project.join(".inkhaven").join("doctor.log");
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let (kind, detail) = match outcome {
        Ok(s) => ("OK", s.clone()),
        Err(e) => ("ERR", e.to_string()),
    };
    let line = format!(
        "{now}|{kind}|{}|{}\n",
        finding.class.slug(),
        detail.replace('\n', " "),
    );
    use std::io::Write;
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| f.write_all(line.as_bytes()));
}

/// Pretty-print findings to stdout.  Used by the
/// human-readable doctor output path.
pub fn print_human(report: &ScanReport) {
    println!("Project scan");
    println!(
        "  generated_at  : {}\n  project_root  : {}",
        report.generated_at, report.project_root,
    );
    if report.findings.is_empty() {
        println!("  findings      : none — project is clean");
        return;
    }
    println!("  findings      : {}", report.findings.len());
    println!();
    for (i, f) in report.findings.iter().enumerate() {
        let path = f.path.as_deref().unwrap_or("-");
        println!(
            "  [{n}] {sev:>8} · {class:<26} · {path}",
            n = i + 1,
            sev = f.severity.slug(),
            class = f.class.slug(),
        );
        println!("        {}", f.detail);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class_slugs_distinct_and_roundtrip() {
        let mut seen = std::collections::HashSet::new();
        for c in ScanClass::ALL {
            assert!(seen.insert(c.slug()));
            assert_eq!(ScanClass::from_slug(c.slug()), Some(c));
        }
        assert_eq!(ScanClass::from_slug("nonsense"), None);
    }

    #[test]
    fn severity_ordering_critical_warning_info() {
        assert!(super::severity_at_or_above(
            ScanSeverity::Critical,
            ScanSeverity::Warning
        ));
        assert!(super::severity_at_or_above(
            ScanSeverity::Warning,
            ScanSeverity::Info
        ));
        assert!(!super::severity_at_or_above(
            ScanSeverity::Info,
            ScanSeverity::Warning
        ));
    }

    #[test]
    fn count_at_or_above_warning() {
        let mut r = ScanReport::new(std::path::Path::new("/tmp/x"));
        r.findings.push(ScanFinding {
            class: ScanClass::ZeroByteFile,
            severity: ScanSeverity::Critical,
            path: None,
            detail: String::new(),
        });
        r.findings.push(ScanFinding {
            class: ScanClass::CorruptCommentsSidecar,
            severity: ScanSeverity::Warning,
            path: None,
            detail: String::new(),
        });
        r.findings.push(ScanFinding {
            class: ScanClass::OrphanParagraphRow,
            severity: ScanSeverity::Info,
            path: None,
            detail: String::new(),
        });
        assert_eq!(r.count_at_or_above(ScanSeverity::Warning), 2);
        assert_eq!(r.count_at_or_above(ScanSeverity::Critical), 1);
        assert_eq!(r.count_at_or_above(ScanSeverity::Info), 3);
    }

    #[test]
    fn sidecar_path_appends_comments_json() {
        let p = std::path::Path::new("/tmp/x/foo.typ");
        let s = sidecar_path_for(p);
        assert_eq!(s.to_string_lossy(), "/tmp/x/foo.typ.comments.json");
    }

    #[test]
    fn report_serialises_roundtrip() {
        let mut r = ScanReport::new(std::path::Path::new("/tmp/x"));
        r.findings.push(ScanFinding {
            class: ScanClass::ZeroByteFile,
            severity: ScanSeverity::Critical,
            path: Some("/tmp/x/foo.typ".into()),
            detail: "prose lost".into(),
        });
        let json = serde_json::to_string(&r).unwrap();
        let parsed: ScanReport = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.findings.len(), 1);
        assert_eq!(parsed.findings[0].class, ScanClass::ZeroByteFile);
        assert_eq!(parsed.findings[0].path.as_deref(), Some("/tmp/x/foo.typ"));
    }
}
