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

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rust_stemmers::Stemmer;
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
    /// 1.3.12 — manuscript fingerprint at scan time; consumers flag the
    /// findings stale when it no longer matches. 0 in older sidecars.
    #[serde(default)]
    pub manuscript_fingerprint: u64,
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

/// 1.3.8 WORLD-1 — one pair of facts that contradict each other *within*
/// the Facts book (internal consistency), distinct from `FactFinding`
/// (prose-vs-fact).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FactConflict {
    /// The first fact (briefly quoted).
    pub a: String,
    /// The second fact it contradicts.
    pub b: String,
    /// One-line explanation of the contradiction.
    pub detail: String,
}

/// The internal-consistency report — every self-contradicting fact pair.
/// Serialised to `<project>/.inkhaven/facts_check.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FactCheckReport {
    #[serde(default)]
    pub version: String,
    /// Hash of the fact texts the check ran over — lets the Editorial Pass
    /// note staleness when the Facts book has moved since.
    #[serde(default)]
    pub content_hash: u64,
    pub conflicts: Vec<FactConflict>,
    /// 1.3.12 — manuscript fingerprint at scan time; consumers flag the
    /// findings stale when it no longer matches. 0 in older sidecars.
    #[serde(default)]
    pub manuscript_fingerprint: u64,
}

impl FactCheckReport {
    pub fn sidecar_path(project_root: &Path) -> PathBuf {
        project_root.join(".inkhaven").join("facts_check.json")
    }
    pub fn load(project_root: &Path) -> std::io::Result<Self> {
        let path = Self::sidecar_path(project_root);
        match std::fs::read_to_string(&path) {
            Ok(s) => serde_json::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }
    pub fn save(&self, project_root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(project_root);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }
    /// Hash the fact texts (order-independent) so a moved Facts book
    /// invalidates the cache.
    pub fn compute_hash(facts: &[String]) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut sorted: Vec<&String> = facts.iter().collect();
        sorted.sort();
        let mut h = std::collections::hash_map::DefaultHasher::new();
        for f in sorted {
            f.hash(&mut h);
        }
        h.finish()
    }
}

/// 1.3.8 WORLD-1 — load series-shared canon from a directory: one fact per
/// file (the file stem, de-slugified, is the title; the contents are the
/// body). Plain `.txt` / `.md` / `.typ` files only; flat (non-recursive).
/// Returns sorted `(title, body)` pairs.
pub fn shared_facts(dir: &Path) -> Vec<(String, String)> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let ext_ok = path
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| matches!(x.to_lowercase().as_str(), "txt" | "md" | "typ" | "text"))
            .unwrap_or(false);
        if !ext_ok {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let Ok(body) = std::fs::read_to_string(&path) else {
            continue;
        };
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        out.push((stem.replace(['-', '_'], " "), body.to_string()));
    }
    out.sort();
    out
}

/// Parse the internal-consistency reply — one conflict per line,
/// `fact A | fact B | why`. Tolerant of list markers / a "none" sentinel /
/// blank + malformed lines. Pure.
pub fn parse_conflicts(raw: &str) -> Vec<FactConflict> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•', ' ']).trim();
        if line.is_empty() || !line.contains('|') {
            continue;
        }
        let parts: Vec<&str> = line.splitn(3, '|').map(str::trim).collect();
        if parts.len() == 3 && !parts[0].is_empty() && !parts[1].is_empty() {
            // skip a header row
            if parts[0].eq_ignore_ascii_case("fact a") || parts[0].eq_ignore_ascii_case("fact") {
                continue;
            }
            out.push(FactConflict {
                a: parts[0].to_string(),
                b: parts[1].to_string(),
                detail: parts[2].to_string(),
            });
        }
    }
    out
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

// ── FF.3 — fact extraction from prose ─────────────────────

/// One world-fact the AI proposes from the manuscript prose, awaiting
/// the author's accept/reject before it joins the Facts book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactCandidate {
    /// Coarse category (`climate`, `geography`, `chronology`, …).
    pub category: String,
    /// The proposed fact, a short self-contained sentence.
    pub statement: String,
    /// Chapter the fact was inferred from (provenance).
    pub chapter: String,
}

/// Parse the AI extraction response — one candidate per line in the
/// form `category | statement`.  Tolerant: blank + malformed lines,
/// list markers, a header row, and a "none" sentinel are skipped.
/// Pure.
pub fn parse_candidates(raw: &str, chapter: &str) -> Vec<FactCandidate> {
    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches(['-', '*', '•']).trim();
        if line.is_empty() {
            continue;
        }
        let parts: Vec<&str> = line.splitn(2, '|').map(str::trim).collect();
        if parts.len() != 2 {
            continue;
        }
        let (category, statement) = (parts[0], parts[1]);
        if category.is_empty() || statement.is_empty() {
            continue;
        }
        if category.eq_ignore_ascii_case("category")
            && statement.eq_ignore_ascii_case("statement")
        {
            continue;
        }
        if category.eq_ignore_ascii_case("none") {
            continue;
        }
        out.push(FactCandidate {
            category: category.to_string(),
            statement: statement.to_string(),
            chapter: chapter.to_string(),
        });
    }
    out
}

/// Normalised token set for dedup: lowercase + `ё`-fold + Snowball-stem
/// each word, drop punctuation/empties.  Inflected restatements of the
/// same fact collapse to the same set.
pub fn normalise_tokens(text: &str, stemmer: &Option<Stemmer>) -> BTreeSet<String> {
    text.split_whitespace()
        .map(|w| {
            let trimmed = w.trim_matches(|c: char| !c.is_alphanumeric());
            crate::text::normalize_stem(trimmed, stemmer)
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Jaccard similarity of two normalised token sets ≥ `threshold`?  Used
/// to skip a candidate that restates an existing (or already-kept) fact.
/// Empty sets are never near-duplicates.
pub fn near_duplicate(a: &BTreeSet<String>, b: &BTreeSet<String>, threshold: f64) -> bool {
    if a.is_empty() || b.is_empty() {
        return false;
    }
    let inter = a.intersection(b).count() as f64;
    let union = a.union(b).count() as f64;
    union > 0.0 && (inter / union) >= threshold
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_conflicts_reads_pairs_and_skips_noise() {
        let raw = "Here are the contradictions:\n\
                   fact A | fact B | why   (this header is skipped)\n\
                   - winters are mild | the harbor freezes each January | a mild winter can't freeze a harbor\n\
                   the capital is inland | the capital is a port | can't be both\n\
                   none of the rest conflict";
        let c = parse_conflicts(raw);
        assert_eq!(c.len(), 2, "two real pairs; header + prose lines skipped");
        assert_eq!(c[0].a, "winters are mild");
        assert_eq!(c[0].b, "the harbor freezes each January");
        assert!(c[0].detail.contains("freeze"));
        assert_eq!(c[1].a, "the capital is inland");
    }

    #[test]
    fn shared_facts_reads_a_directory_of_fact_files() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("the-harbor.txt"), "freezes each January").unwrap();
        std::fs::write(dir.path().join("the_capital.md"), "  200 miles inland  ").unwrap();
        std::fs::write(dir.path().join("empty.txt"), "   ").unwrap(); // skipped (blank)
        std::fs::write(dir.path().join("notes.json"), "{}").unwrap(); // skipped (ext)
        let facts = shared_facts(dir.path());
        assert_eq!(facts.len(), 2, "only the two non-empty text files");
        // sorted; stems de-slugified to titles; bodies trimmed
        assert_eq!(facts[0], ("the capital".into(), "200 miles inland".into()));
        assert_eq!(facts[1], ("the harbor".into(), "freezes each January".into()));
    }

    #[test]
    fn check_hash_is_order_independent() {
        let a = FactCheckReport::compute_hash(&["x".into(), "y".into()]);
        let b = FactCheckReport::compute_hash(&["y".into(), "x".into()]);
        assert_eq!(a, b, "reordering facts doesn't change the hash");
        assert_ne!(a, FactCheckReport::compute_hash(&["x".into(), "z".into()]));
    }

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
            manuscript_fingerprint: 0,
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

    // ── FF.3 extraction ───────────────────────────────

    #[test]
    fn parses_category_statement_candidates() {
        let raw = "Here are the world facts:\n\
                   - climate | The Sael basin is equatorial; no winter.\n\
                   geography | The capital is three days' ride inland.\n\
                   category | statement\n\
                   none\n\
                   half a line";
        let c = parse_candidates(raw, "Arrivals");
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].category, "climate");
        assert_eq!(c[0].statement, "The Sael basin is equatorial; no winter.");
        assert_eq!(c[0].chapter, "Arrivals");
        assert_eq!(c[1].category, "geography");
    }

    #[test]
    fn near_duplicate_detects_inflected_restatement() {
        let stemmer: Option<Stemmer> = crate::config::parse_stemmer_language("english")
            .map(Stemmer::create);
        let a = normalise_tokens("The capital is three days' ride inland", &stemmer);
        let b = normalise_tokens("the capitals are three days riding inland", &stemmer);
        // Inflected restatement → high overlap, flagged as duplicate.
        assert!(near_duplicate(&a, &b, 0.6));
        // A genuinely different fact is not.
        let c = normalise_tokens("Winter lasts six months in the north", &stemmer);
        assert!(!near_duplicate(&a, &c, 0.6));
        // Empty never matches.
        assert!(!near_duplicate(&a, &BTreeSet::new(), 0.6));
    }
}
