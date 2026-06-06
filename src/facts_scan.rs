//! 1.2.21+ FF.2 — the whole-manuscript fact-scan report.
//!
//! `inkhaven facts scan` walks the manuscript, semantically retrieves
//! the relevant entries from the Facts book per chapter, and asks the
//! LLM to flag prose that contradicts them.  The findings are recorded
//! as `(chapter, claim, fact, detail)` rows in a project sidecar
//! (`<project>/.inkhaven/facts_scan.json`), printable as human text or
//! `--json` for CI.
//!
//! This module is pure: the finding model, the sidecar round-trip, and
//! the response parser.  The (non-deterministic) AI call lives in the
//! CLI (`crate::cli::facts_scan`).  Unlike the deterministic `doctor
//! --scan` classes, a fact-check is inherently semantic, so it ships as
//! a standalone AI command (the `continuity` / `tension` pattern), not a
//! ScanClass.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One flagged contradiction between the prose and an established fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactFinding {
    /// Chapter title the contradiction was found in.
    pub chapter: String,
    /// 0-based chapter index in display order (a stable sort key).
    #[serde(default)]
    pub chapter_index: usize,
    /// The contradicting phrase / claim, quoted from the prose.
    pub claim: String,
    /// The established fact it violates.
    pub fact: String,
    /// One-line explanation of the contradiction.
    pub detail: String,
}

/// The whole scan — every flagged contradiction.  Serialised to
/// `<project>/.inkhaven/facts_scan.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactScanReport {
    /// Inkhaven version that produced the scan.
    #[serde(default)]
    pub version: String,
    /// Manuscript language the scan ran in.
    #[serde(default)]
    pub language: String,
    pub findings: Vec<FactFinding>,
}

impl FactScanReport {
    /// Sidecar path for a project.
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("facts_scan.json")
    }

    /// Load the report from the project sidecar.  Returns an empty
    /// report (not an error) when the sidecar doesn't exist yet.
    pub fn load(project_root: &Path) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    /// Persist the report atomically to the sidecar.
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }
}

/// Parse the AI response — one contradiction per line in the
/// pipe-delimited form `claim | fact | detail`.  Tolerant: blank +
/// malformed lines, list markers, a header row, and a "none" sentinel
/// are skipped; a chapter the model finds consistent simply yields
/// nothing.  Pure, so the (non-deterministic) live call is the only
/// untested boundary.
pub fn parse_findings(raw: &str, chapter: &str, chapter_index: usize) -> Vec<FactFinding> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
        if parts.len() != 3 {
            continue;
        }
        let (claim, fact, detail) = (parts[0], parts[1], parts[2]);
        if claim.is_empty() || fact.is_empty() || detail.is_empty() {
            continue;
        }
        // Skip a header row the model might echo.
        if claim.eq_ignore_ascii_case("claim") && fact.eq_ignore_ascii_case("fact") {
            continue;
        }
        // Skip an explicit "no contradictions" sentinel.
        if claim.eq_ignore_ascii_case("none")
            || claim.eq_ignore_ascii_case("no contradictions")
        {
            continue;
        }
        out.push(FactFinding {
            chapter: chapter.to_string(),
            chapter_index,
            claim: claim.to_string(),
            fact: fact.to_string(),
            detail: detail.to_string(),
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pipe_delimited_findings() {
        let raw = "the capital's first snow | Climate: equatorial, no winter | the basin has no winter\n\
                   two days after leaving the coast | Geography: capital is 3 days' ride inland | distance understated";
        let f = parse_findings(raw, "The Wharf", 2);
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].claim, "the capital's first snow");
        assert_eq!(f[0].fact, "Climate: equatorial, no winter");
        assert_eq!(f[0].chapter, "The Wharf");
        assert_eq!(f[0].chapter_index, 2);
        assert_eq!(f[1].claim, "two days after leaving the coast");
    }

    #[test]
    fn skips_malformed_preamble_header_and_markers() {
        let raw = "Here is what I found:\n\
                   \n\
                   - snow in the capital | Climate: tropical | no winter here\n\
                   this line has no pipes\n\
                   claim | fact | detail\n\
                   half a claim | only one bar\n\
                   * overnight ride | Geography: 3 days | far too fast";
        let f = parse_findings(raw, "Ch1", 0);
        // Two well-formed rows (header + no-pipe + half row skipped).
        assert_eq!(f.len(), 2);
        assert_eq!(f[0].claim, "snow in the capital");
        assert_eq!(f[1].claim, "overnight ride");
    }

    #[test]
    fn none_sentinel_yields_nothing() {
        assert!(parse_findings("none | — | —", "Ch1", 0).is_empty());
        assert!(parse_findings("", "Ch1", 0).is_empty());
        assert!(parse_findings("No contradictions found.", "Ch1", 0).is_empty());
    }

    #[test]
    fn sidecar_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let report = FactScanReport {
            version: "1.2.21".into(),
            language: "english".into(),
            findings: vec![FactFinding {
                chapter: "The Wharf".into(),
                chapter_index: 2,
                claim: "first snow".into(),
                fact: "Climate: equatorial".into(),
                detail: "no winter".into(),
            }],
        };
        report.save(tmp.path()).unwrap();
        let loaded = FactScanReport::load(tmp.path()).unwrap();
        assert_eq!(loaded.findings, report.findings);
        assert_eq!(loaded.language, "english");
    }

    #[test]
    fn load_missing_sidecar_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let r = FactScanReport::load(tmp.path()).unwrap();
        assert!(r.findings.is_empty());
    }
}
