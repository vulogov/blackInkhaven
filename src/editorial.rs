//! 1.3.6 EDITORIAL-1 — The Editorial Pass: one ranked, walkable revision
//! worklist unifying every detector's findings.
//!
//! Pure. The aggregator maps each detector's native finding into a common
//! [`EditorialFinding`] and ranks them; the CLI (`inkhaven edit`) runs the
//! scans / reads the sidecars and feeds them here, and the TUI cockpit
//! walks the result. No detection lives here — only normalization.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::cli::doctor_scan::{ScanFinding, ScanSeverity};

/// Worklist severity — the editorial tri, mapped from each source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Error,
    Warn,
    Info,
}

impl Severity {
    fn rank(self) -> u8 {
        match self {
            Severity::Error => 0,
            Severity::Warn => 1,
            Severity::Info => 2,
        }
    }
    pub fn icon(self) -> char {
        match self {
            Severity::Error => '✗',
            Severity::Warn => '⚠',
            Severity::Info => '·',
        }
    }
}

impl From<ScanSeverity> for Severity {
    fn from(s: ScanSeverity) -> Self {
        match s {
            ScanSeverity::Critical => Severity::Error,
            ScanSeverity::Warning => Severity::Warn,
            ScanSeverity::Info => Severity::Info,
        }
    }
}

/// Where a finding points — resolved as far as the source allows. A
/// `paragraph` node id (+ optional `char_range`) enables jump-to-location;
/// otherwise `chapter` / `path` is the best the cockpit can do.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Location {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub char_range: Option<(usize, usize)>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Location {
    pub fn chapter(slug_or_title: impl Into<String>) -> Self {
        Self {
            chapter: Some(slug_or_title.into()),
            ..Default::default()
        }
    }
    pub fn path(p: impl Into<String>) -> Self {
        Self {
            path: Some(p.into()),
            ..Default::default()
        }
    }
    /// A short label for the worklist row (chapter, else the file name).
    pub fn label(&self) -> String {
        if let Some(c) = &self.chapter {
            return c.clone();
        }
        if let Some(p) = &self.path {
            return std::path::Path::new(p)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_else(|| p.clone());
        }
        "—".to_string()
    }
}

/// One unified finding in the editorial worklist.
#[derive(Debug, Clone, Serialize)]
pub struct EditorialFinding {
    pub category: String,
    pub severity: Severity,
    pub location: Location,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hint: Option<String>,
    /// The detector this came from ("doctor" | "facts" | "plan").
    pub source: &'static str,
    pub autofixable: bool,
}

impl EditorialFinding {
    /// A stable fingerprint for defer / skip — category + message. It moves
    /// when the prose moves (the message changes), so a deferred finding
    /// resurfaces only once it's genuinely renewed.
    pub fn fingerprint(&self) -> String {
        format!("{}\u{1}{}", self.category, self.message)
    }

    /// Whether the cockpit's `f` can stream an AI rewrite for this finding:
    /// its category is paragraph-rewritable AND it resolved to a paragraph.
    pub fn rewritable(&self) -> bool {
        self.location.paragraph.is_some() && fix_spec(&self.category).is_some()
    }
}

/// Whether an AI fix rewrites the whole paragraph or only the flagged phrase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixScope {
    /// Rewrite the entire paragraph (echo / pacing — there's no single phrase
    /// to isolate).
    Paragraph,
    /// Rewrite only the finding's `char_range` and splice it back into the
    /// paragraph (show-tell / anachronism / filter — a localized phrase).
    Span,
}

/// The AI-rewrite recipe for a rewritable editorial category — the
/// prompt-override slug (Prompts book / `prompts.hjson`), the built-in
/// instruction, a short label for the snapshot annotation + status, and the
/// rewrite scope. The judgment categories (structure / continuity / fact /
/// scene) return None: there's no honest single-paragraph rewrite for "the
/// midpoint sags".
#[derive(Debug, Clone, Copy)]
pub struct FixSpec {
    pub slug: &'static str,
    pub builtin: &'static str,
    pub label: &'static str,
    pub scope: FixScope,
}

pub fn fix_spec(category: &str) -> Option<FixSpec> {
    Some(match category {
        "echo" => FixSpec {
            slug: "editorial-fix-echo",
            builtin: "Rewrite the paragraph below to remove the distracting word repetition — vary \
the over-used word with synonyms or restructuring — while preserving the meaning, the author's \
voice, the paragraph's language, and any Typst markup verbatim. Output ONLY the rewritten \
paragraph, no preamble.",
            label: "de-echo",
            scope: FixScope::Paragraph,
        },
        "pacing" => FixSpec {
            slug: "editorial-fix-pacing",
            builtin: "Tighten the overlong paragraph below: cut padding, break or trim run-on \
sentences, sharpen the prose — while preserving the meaning, the author's voice, the paragraph's \
language, and any Typst markup verbatim. Output ONLY the rewritten paragraph, no preamble.",
            label: "tighten",
            scope: FixScope::Paragraph,
        },
        "show-tell" => FixSpec {
            slug: "editorial-fix-show-tell",
            builtin: "You rewrite telling prose to SHOW it — replacing a named emotion or abstract \
summary with concrete action, sensation, and detail — while preserving the meaning, the author's \
voice, the language, and any Typst markup verbatim.",
            label: "show-not-tell",
            scope: FixScope::Span,
        },
        _ => return None,
    })
}

/// Replace the half-open char range `[start, end)` of `original` with
/// `replacement`, preserving everything outside it (including a trailing
/// newline). Char-indexed (not byte); an out-of-range or inverted span is
/// clamped so this never panics. Pure.
pub fn splice_span(original: &str, range: (usize, usize), replacement: &str) -> String {
    let chars: Vec<char> = original.chars().collect();
    let n = chars.len();
    let start = range.0.min(n);
    let end = range.1.clamp(start, n);
    let mut out: String = chars[..start].iter().collect();
    out.push_str(replacement);
    out.extend(chars[end..].iter());
    out
}

/// Clean a model's span-rewrite reply down to the bare replacement phrase:
/// trims whitespace and strips any wrapping quotes / guillemets / backticks
/// the model added despite instructions. Pure.
pub fn extract_phrase(raw: &str) -> String {
    raw.trim()
        .trim_matches(|c: char| {
            c.is_whitespace()
                || matches!(c, '"' | '\'' | '`' | '«' | '»' | '\u{201c}' | '\u{201d}' | '\u{2018}' | '\u{2019}')
        })
        .to_string()
}

/// The ranked worklist + per-severity counts.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EditorialReport {
    pub findings: Vec<EditorialFinding>,
    pub errors: usize,
    pub warnings: usize,
    pub infos: usize,
    /// How many findings were hidden by the defer sidecar (0 when shown).
    #[serde(skip_serializing_if = "is_zero")]
    pub deferred: usize,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// The set of deferred (dismissed) finding fingerprints — sidecar
/// `.inkhaven/editorial-dismissed.json`. A finding the author has judged
/// (not-now or accepted) stays hidden until its prose changes.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Dismissed {
    #[serde(default)]
    pub fingerprints: BTreeSet<String>,
}

impl Dismissed {
    pub fn sidecar_path(root: &Path) -> PathBuf {
        root.join(".inkhaven").join("editorial-dismissed.json")
    }
    pub fn load(root: &Path) -> Self {
        std::fs::read_to_string(Self::sidecar_path(root))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, root: &Path) -> std::io::Result<()> {
        let path = Self::sidecar_path(root);
        if let Some(p) = path.parent() {
            std::fs::create_dir_all(p)?;
        }
        let body = serde_json::to_vec_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        crate::io_atomic::write(&path, &body)
    }
    /// Add a fingerprint to the deferred set (load → insert → save).
    pub fn defer(root: &Path, fingerprint: &str) -> std::io::Result<()> {
        let mut d = Self::load(root);
        d.fingerprints.insert(fingerprint.to_string());
        d.save(root)
    }
    /// Forget every deferral.
    pub fn clear(root: &Path) -> std::io::Result<()> {
        Self::default().save(root)
    }
}

// ── per-source mappers (pure) ───────────────────────────────────────

/// Map a doctor [`ScanFinding`] to an editorial finding — IFF it's an
/// editorial (manuscript-readiness) class. Project-integrity classes
/// (zero-byte files, orphan rows, bdslib drift) return `None`: they belong
/// to `doctor`, not the editorial pass.
pub fn from_scan_finding(f: &ScanFinding) -> Option<EditorialFinding> {
    let category = f.class.editorial_category()?;
    // Prefer the file path; else the chapter the detail embeds (the
    // chapter-scale detectors say "… (chapter `Title`)") so the cockpit can
    // still jump to the chapter.
    let location = match &f.path {
        Some(p) => Location::path(p.clone()),
        None => chapter_from_detail(&f.detail).map(Location::chapter).unwrap_or_default(),
    };
    Some(EditorialFinding {
        category: category.to_string(),
        severity: f.severity.into(),
        location,
        message: f.detail.clone(),
        hint: None,
        source: "doctor",
        autofixable: false,
    })
}

/// Pull the chapter title out of a doctor detail that embeds it as
/// `chapter \`Title\``. Returns None when there's no such token.
fn chapter_from_detail(detail: &str) -> Option<String> {
    let i = detail.find("chapter `")? + "chapter `".len();
    let rest = &detail[i..];
    let j = rest.find('`')?;
    Some(rest[..j].to_string())
}

/// Map a Facts-scan contradiction.
pub fn from_fact_finding(f: &crate::facts_scan::FactFinding) -> EditorialFinding {
    EditorialFinding {
        category: "fact".into(),
        severity: Severity::Warn,
        location: Location::chapter(f.chapter.clone()),
        message: format!("“{}” contradicts: {}", f.claim, f.fact),
        hint: (!f.detail.trim().is_empty()).then(|| f.detail.clone()),
        source: "facts",
        autofixable: false,
    }
}

/// Map a Facts internal-consistency conflict (1.3.8) — two facts that
/// contradict each other within the Facts book.
pub fn from_fact_conflict(c: &crate::facts_scan::FactConflict) -> EditorialFinding {
    EditorialFinding {
        category: "world".into(),
        severity: Severity::Warn,
        location: Location::default(), // a pair of facts — book-level
        message: format!("facts conflict: {} ⟷ {}", c.a, c.b),
        hint: (!c.detail.trim().is_empty()).then(|| c.detail.clone()),
        source: "facts",
        autofixable: false,
    }
}

/// Map a `plan check` warning string into a structure finding. The category
/// is the warning's prefix before `:` (gap / drift / pacing / tension /
/// scene / sequel / rhythm / thread); the whole string is the message.
pub fn from_plan_warning(w: &str) -> EditorialFinding {
    let prefix = w.split_once(':').map(|(c, _)| c.trim()).unwrap_or("structure");
    let category = match prefix {
        "tension" => "tension",
        "scene" | "sequel" | "rhythm" => "scene",
        "thread" | "threads" => "thread",
        _ => "structure",
    };
    EditorialFinding {
        category: category.to_string(),
        severity: Severity::Warn,
        location: Location::default(),
        message: w.to_string(),
        hint: None,
        source: "plan",
        autofixable: false,
    }
}

/// Rank + dedup a flat list of findings into the report: sort by severity
/// (error first), then category, then message; drop findings identical in
/// category + message + location.
pub fn aggregate(mut findings: Vec<EditorialFinding>) -> EditorialReport {
    findings.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.message.cmp(&b.message))
    });
    findings.dedup_by(|a, b| {
        a.category == b.category
            && a.message == b.message
            && a.location.chapter == b.location.chapter
            && a.location.path == b.location.path
    });
    let errors = findings.iter().filter(|f| f.severity == Severity::Error).count();
    let warnings = findings.iter().filter(|f| f.severity == Severity::Warn).count();
    let infos = findings.iter().filter(|f| f.severity == Severity::Info).count();
    EditorialReport {
        findings,
        errors,
        warnings,
        infos,
        deferred: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::doctor_scan::{ScanClass, ScanFinding, ScanSeverity};

    fn scan(class: ScanClass, sev: ScanSeverity, detail: &str) -> ScanFinding {
        ScanFinding {
            class,
            severity: sev,
            path: Some(format!("/p/{}.typ", detail.len())),
            detail: detail.into(),
        }
    }

    #[test]
    fn integrity_classes_are_not_editorial() {
        // a data-integrity class never enters the editorial worklist
        assert!(from_scan_finding(&scan(ScanClass::ZeroByteFile, ScanSeverity::Critical, "x")).is_none());
        assert!(from_scan_finding(&scan(ScanClass::BdslibOnly, ScanSeverity::Warning, "x")).is_none());
        // an author-judgment class does
        let e = from_scan_finding(&scan(ScanClass::EchoRepetition, ScanSeverity::Info, "echo")).unwrap();
        assert_eq!(e.category, "echo");
        assert_eq!(e.source, "doctor");
    }

    #[test]
    fn doctor_finding_picks_up_the_embedded_chapter() {
        let f = scan(
            ScanClass::EchoRepetition,
            ScanSeverity::Info,
            "echo: `about` appears 5× within ¶1–3 (chapter `Chapter 6: The Letter`)",
        );
        // the scan() helper sets a path, so path wins; clear it to test the
        // chapter-extraction fallback
        let mut f = f;
        f.path = None;
        let e = from_scan_finding(&f).unwrap();
        assert_eq!(e.location.chapter.as_deref(), Some("Chapter 6: The Letter"));
    }

    #[test]
    fn plan_warning_category_from_prefix() {
        assert_eq!(from_plan_warning("drift: `Midpoint` lands at 64%").category, "structure");
        assert_eq!(from_plan_warning("tension: `Midpoint` is flat").category, "tension");
        assert_eq!(from_plan_warning("sequel: `X` never decides").category, "scene");
        assert_eq!(from_plan_warning("thread: `Y` references unknown").category, "thread");
    }

    #[test]
    fn defer_sidecar_round_trips_and_fingerprint_is_stable() {
        let f = EditorialFinding {
            category: "echo".into(),
            severity: Severity::Info,
            location: Location::default(),
            message: "echo: `about` ×5".into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        let fp = f.fingerprint();
        assert_eq!(fp, f.clone().fingerprint(), "fingerprint is stable");
        let dir = tempfile::tempdir().unwrap();
        Dismissed::defer(dir.path(), &fp).unwrap();
        let d = Dismissed::load(dir.path());
        assert!(d.fingerprints.contains(&fp));
        Dismissed::clear(dir.path()).unwrap();
        assert!(Dismissed::load(dir.path()).fingerprints.is_empty());
    }

    #[test]
    fn rewritable_needs_a_paragraph_and_a_fixable_category() {
        let mk = |cat: &str, para: bool| EditorialFinding {
            category: cat.into(),
            severity: Severity::Info,
            location: Location {
                paragraph: para.then(uuid::Uuid::now_v7),
                ..Default::default()
            },
            message: "m".into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        assert!(mk("echo", true).rewritable(), "echo + a paragraph → rewritable");
        assert!(!mk("echo", false).rewritable(), "no paragraph → not rewritable");
        assert!(!mk("structure", true).rewritable(), "judgment category → not rewritable");
        assert!(fix_spec("echo").is_some() && fix_spec("structure").is_none());
    }

    #[test]
    fn fix_scope_paragraph_vs_span() {
        assert_eq!(fix_spec("echo").unwrap().scope, FixScope::Paragraph);
        assert_eq!(fix_spec("pacing").unwrap().scope, FixScope::Paragraph);
        assert_eq!(fix_spec("show-tell").unwrap().scope, FixScope::Span);
    }

    #[test]
    fn splice_span_replaces_only_the_range() {
        // "She was angry." — replace "was angry" (chars 4..13) with the showing
        assert_eq!(
            splice_span("She was angry.", (4, 13), "clenched her fists"),
            "She clenched her fists."
        );
        // a trailing newline outside the range survives
        assert_eq!(splice_span("foo bar\n", (0, 3), "baz"), "baz bar\n");
        // an out-of-range span clamps instead of panicking
        assert_eq!(splice_span("hi", (5, 9), "X"), "hiX");
        // an inverted span clamps end up to start (insertion)
        assert_eq!(splice_span("hello", (3, 1), "_"), "hel_lo");
    }

    #[test]
    fn extract_phrase_strips_wrapping_quotes_and_space() {
        assert_eq!(extract_phrase("  \"clenched her fists\" \n"), "clenched her fists");
        assert_eq!(extract_phrase("«spyglass»"), "spyglass");
        assert_eq!(extract_phrase("plain words"), "plain words");
    }

    #[test]
    fn aggregate_ranks_errors_first_and_dedups() {
        let mk = |sev, cat: &str, msg: &str| EditorialFinding {
            category: cat.into(),
            severity: sev,
            location: Location::default(),
            message: msg.into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        let r = aggregate(vec![
            mk(Severity::Info, "echo", "z"),
            mk(Severity::Error, "continuity", "a"),
            mk(Severity::Warn, "pacing", "m"),
            mk(Severity::Error, "continuity", "a"), // exact dup → dropped
        ]);
        assert_eq!(r.findings.len(), 3, "the duplicate is dropped");
        assert_eq!(r.findings[0].severity, Severity::Error, "errors first");
        assert_eq!((r.errors, r.warnings, r.infos), (1, 1, 1));
    }
}
