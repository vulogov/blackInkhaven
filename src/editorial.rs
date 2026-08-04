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

    /// REDLINE-1 (RD-P0) — the *kind of help* this finding can be acted on with: a
    /// confirmed-diff [`ResponseKind::Rewrite`], a guided [`ResponseKind::Decision`],
    /// or a [`ResponseKind::Brief`]. Derived from the category (RD-P1's converters
    /// pick the category that carries the right default).
    #[allow(dead_code)] // consumed by RD-P1 (the queue) / RD-P6 (the surface).
    pub fn response(&self) -> ResponseKind {
        response_kind(&self.category)
    }
}

/// REDLINE-1 (RD-P0) — how a finding can be turned into an author-confirmed change.
/// Only [`Rewrite`](ResponseKind::Rewrite) ever touches prose, and only through the
/// existing confirmed-diff + snapshot contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // consumed by RD-P1 / RD-P3 (decision) / RD-P4 (brief) / RD-P6.
pub enum ResponseKind {
    /// A diff-reviewed local prose fix (there's an honest single-locus rewrite).
    Rewrite,
    /// A guided authorial choice — you decide which way is right, then REDLINE
    /// reconciles the other as a confirmed rewrite.
    Decision,
    /// A concrete revision suggestion with no rewrite — structure is yours to move.
    Brief,
}

impl ResponseKind {
    #[allow(dead_code)]
    pub fn label(self) -> &'static str {
        match self {
            ResponseKind::Rewrite => "rewrite",
            ResponseKind::Decision => "decision",
            ResponseKind::Brief => "brief",
        }
    }
}

/// Classify a finding category into its [`ResponseKind`]. A *Rewrite* is a category
/// with an honest single-paragraph prose fix; a *Decision* needs the author to
/// choose which way is right (a continuity break, a described-two-ways drift, a
/// prose-vs-fact conflict) before a targeted rewrite; everything structural or
/// book-level is a *Brief*. Unknown categories default to Brief — the safest, since
/// a Brief never edits prose. Pure.
#[allow(dead_code)] // consumed by RD-P1's converters + the surface.
pub fn response_kind(category: &str) -> ResponseKind {
    match category {
        // Honest single-locus prose fixes.
        "echo" | "pacing" | "show-tell" | "filter" | "editor" | "voice"
        | "anachronism" => ResponseKind::Rewrite,
        // The author must choose which way is right; then we reconcile.
        "co_location" | "char_facts" | "drift" | "introduce" | "confusion"
        | "unpaid_setup" | "numeric" | "continuity" | "fact" | "world" => {
            ResponseKind::Decision
        }
        // Structural / book-level — a suggestion, never a rewrite.
        _ => ResponseKind::Brief,
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
        "filter" => FixSpec {
            slug: "editorial-fix-filter",
            builtin: "You remove filter words — intensifier crutches and hedges that weaken prose \
(\"just\", \"really\", \"very\", \"seemed\", \"felt\"). If cutting the marked word leaves the \
sentence intact, return the phrase without it; otherwise replace it with sharper wording — while \
preserving the meaning, the author's voice, the language, and any Typst markup verbatim.",
            label: "de-filter",
            scope: FixScope::Span,
        },
        // REDLINE-1 (RD-P2) — the deterministic anachronism detector already marks
        // the offending phrase (a char_range), so this is a Span fix like the others.
        "anachronism" => FixSpec {
            slug: "editorial-fix-anachronism",
            builtin: "You replace an anachronistic word or phrase — one that postdates the story's \
setting — with an era- and world-appropriate equivalent, while preserving the meaning, the author's \
voice, the language, and any Typst markup verbatim. If no single word fits, lightly rephrase. Output \
ONLY the replacement text for the marked phrase — no « » markers, none of the surrounding \
paragraph, no preamble.",
            label: "period-fit",
            scope: FixScope::Span,
        },
        _ => return None,
    })
}

/// One AI-rewritable fix the batch walk applies: `(paragraph, category,
/// char_range)`. The span is `None` for whole-paragraph categories.
pub type BatchFix = (Uuid, String, Option<(usize, usize)>);

/// The ordered list of AI-rewritable fixes the cockpit's `F` (batch fix-all)
/// walks: every finding matching `filter` (`None` = all) that is
/// [`EditorialFinding::rewritable`], in the findings' display order. Pure.
pub fn batch_fix_queue(findings: &[EditorialFinding], filter: Option<&str>) -> Vec<BatchFix> {
    findings
        .iter()
        .filter(|f| filter.is_none_or(|c| f.category == c))
        .filter(|f| f.rewritable())
        .map(|f| {
            (
                f.location.paragraph.expect("rewritable ⇒ has a paragraph"),
                f.category.clone(),
                f.location.char_range,
            )
        })
        .collect()
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
    /// 1.3.12 — at least one AI sidecar (facts / drift) predates the latest
    /// edits, so some findings may be stale.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub stale: bool,
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

/// Map a semantic-drift contradiction (1.3.10 WORLD-2) — two descriptions of
/// the same entity that diverge across the manuscript. Jumps to the later,
/// divergent passage (`paragraph_b`); jump-only (no honest single-paragraph
/// auto-rewrite for "the tavern's atmosphere changed across 18 chapters").
pub fn from_drift_conflict(c: &crate::drift::DriftConflict) -> EditorialFinding {
    EditorialFinding {
        category: "drift".into(),
        severity: Severity::Warn,
        location: Location {
            chapter: Some(c.chapter_b.clone()),
            paragraph: c.paragraph_b,
            char_range: None,
            path: None,
        },
        message: format!(
            "drift: {} — “{}” ({}) ⟷ “{}” ({})",
            c.entity, c.a, c.chapter_a, c.b, c.chapter_b
        ),
        hint: (!c.detail.trim().is_empty()).then(|| c.detail.clone()),
        source: "drift",
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

/// REDLINE-1 (RD-P1) — a SENTINEL continuity break → the worklist. `paragraph` is
/// the caller-resolved anchor (the finding's own, or its chapter's first). The
/// `kind` becomes the category, so [`response_kind`] routes it (co_location /
/// char_facts / introduce → Decision, single-paragraph numeric → …).
pub(crate) fn from_continuity_finding(
    f: &crate::continuity_intel::ContinuityFinding,
    paragraph: Option<Uuid>,
) -> EditorialFinding {
    use crate::continuity_intel::Severity as CS;
    EditorialFinding {
        category: f.kind.to_string(),
        severity: match f.severity {
            CS::Contradiction => Severity::Error,
            CS::Warning => Severity::Warn,
            CS::Info => Severity::Info,
        },
        location: Location { chapter: chapter_label(f.chapter), paragraph, ..Default::default() },
        message: f.message.clone(),
        hint: None,
        source: "continuity",
        autofixable: false,
    }
}

/// REDLINE-1 (RD-P1) — a LECTOR read-through finding → the worklist.
pub(crate) fn from_lector_finding(
    f: &crate::lector::ReaderFinding,
    paragraph: Option<Uuid>,
) -> EditorialFinding {
    use crate::lector::Severity as LS;
    EditorialFinding {
        category: f.kind.to_string(),
        severity: match f.severity {
            LS::Concern => Severity::Warn,
            _ => Severity::Info,
        },
        location: Location { chapter: chapter_label(f.chapter), paragraph, ..Default::default() },
        message: f.message.clone(),
        hint: None,
        source: "read-through",
        autofixable: false,
    }
}

/// REDLINE-1 (RD-P1) — an Inner Stylist (CHORUS) voice finding → the worklist.
/// Book-level (no paragraph); its `kind` (distinctiveness / drift / pov / tense /
/// register) routes it (all Brief today).
pub(crate) fn from_stylist_finding(f: &crate::inner_stylist::Finding) -> EditorialFinding {
    use crate::inner_stylist::Severity as SS;
    EditorialFinding {
        category: f.kind.to_string(),
        severity: match f.severity {
            SS::Concern => Severity::Warn,
            _ => Severity::Info,
        },
        location: Location::default(),
        message: f.message.clone(),
        hint: None,
        source: "stylist",
        autofixable: false,
    }
}

/// `"ch. N"` for a 1-based chapter ordinal, or `None` for book-level (0).
fn chapter_label(chapter: u32) -> Option<String> {
    (chapter > 0).then(|| format!("ch. {chapter}"))
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
        stale: false,
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
    fn drift_conflict_maps_to_jump_only_drift_finding() {
        let pid = uuid::Uuid::now_v7();
        let c = crate::drift::DriftConflict {
            entity: "The Drunken Goose".into(),
            kind: crate::drift::EntityKind::Place,
            a: "cramped and smoky".into(),
            b: "airy and bright".into(),
            chapter_a: "ch-2".into(),
            chapter_b: "ch-20".into(),
            paragraph_b: Some(pid),
            detail: "a tavern can't be both".into(),
        };
        let f = from_drift_conflict(&c);
        assert_eq!(f.category, "drift");
        assert_eq!(f.severity, Severity::Warn);
        assert_eq!(f.location.paragraph, Some(pid), "jumps to the later passage");
        assert_eq!(f.location.chapter.as_deref(), Some("ch-20"));
        assert!(f.message.contains("The Drunken Goose") && f.message.contains("airy"));
        assert_eq!(f.hint.as_deref(), Some("a tavern can't be both"));
        assert!(!f.rewritable(), "drift is jump-only");
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
    fn response_kind_classifies_by_category() {
        use ResponseKind::*;
        // Honest single-locus prose fixes.
        for c in ["echo", "pacing", "show-tell", "filter", "editor", "voice", "anachronism"] {
            assert_eq!(response_kind(c), Rewrite, "{c} is a Rewrite");
        }
        // The author must choose which way is right, then reconcile.
        for c in ["co_location", "char_facts", "drift", "introduce", "confusion", "unpaid_setup"] {
            assert_eq!(response_kind(c), Decision, "{c} is a Decision");
        }
        // Structural / book-level, and anything unknown → Brief (never edits prose).
        for c in ["structure", "shape_sag", "put_down_risk", "distinctiveness", "tension", "mystery-kind"] {
            assert_eq!(response_kind(c), Brief, "{c} is a Brief");
        }
    }

    #[test]
    fn finding_reports_its_response_kind() {
        let f = EditorialFinding {
            category: "co_location".into(),
            severity: Severity::Info,
            location: Location::default(),
            message: "m".into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        assert_eq!(f.response(), ResponseKind::Decision);
        assert_eq!(ResponseKind::Decision.label(), "decision");
    }

    #[test]
    fn judgment_readers_convert_into_the_worklist() {
        let para = uuid::Uuid::now_v7();
        // SENTINEL continuity → a Decision item, anchored, Contradiction → Error.
        let cf = crate::continuity_intel::ContinuityFinding {
            kind: "co_location",
            severity: crate::continuity_intel::Severity::Contradiction,
            chapter: 3,
            anchor: Some(para),
            entities: vec![],
            message: "Mara is in two places".into(),
            source: "co_location",
            dedup_key: "k".into(),
        };
        let f = from_continuity_finding(&cf, cf.anchor);
        assert_eq!(f.category, "co_location");
        assert_eq!(f.severity, Severity::Error);
        assert_eq!(f.location.paragraph, Some(para));
        assert_eq!(f.location.chapter.as_deref(), Some("ch. 3"));
        assert_eq!(f.response(), ResponseKind::Decision);
        assert_eq!(f.source, "continuity");

        // LECTOR read-through → routed by its kind; chapter-only anchor honoured.
        let lf = crate::lector::ReaderFinding {
            kind: "put_down_risk",
            severity: crate::lector::Severity::Concern,
            chapter: 5,
            anchor: None,
            entities: vec![],
            message: "flat run".into(),
            source: "walk",
            dedup_key: "k2".into(),
        };
        let f = from_lector_finding(&lf, None);
        assert_eq!(f.category, "put_down_risk");
        assert_eq!(f.severity, Severity::Warn);
        assert_eq!(f.response(), ResponseKind::Brief);
        assert_eq!(f.source, "read-through");
    }

    #[test]
    fn fix_scope_paragraph_vs_span() {
        assert_eq!(fix_spec("echo").unwrap().scope, FixScope::Paragraph);
        assert_eq!(fix_spec("pacing").unwrap().scope, FixScope::Paragraph);
        assert_eq!(fix_spec("show-tell").unwrap().scope, FixScope::Span);
        assert_eq!(fix_spec("filter").unwrap().scope, FixScope::Span);
        // RD-P2 — anachronism is now a Span fix (Rewrite-classified + fixable).
        assert_eq!(fix_spec("anachronism").unwrap().scope, FixScope::Span);
        assert_eq!(response_kind("anachronism"), ResponseKind::Rewrite);
        let anach = EditorialFinding {
            category: "anachronism".into(),
            severity: Severity::Warn,
            location: Location { paragraph: Some(uuid::Uuid::now_v7()), ..Default::default() },
            message: "m".into(),
            hint: None,
            source: "world",
            autofixable: false,
        };
        assert!(anach.rewritable(), "anachronism + a paragraph → rewritable");
    }

    #[test]
    fn batch_fix_queue_keeps_only_filtered_rewritable_in_order() {
        let mk = |cat: &str, para: bool, span: Option<(usize, usize)>| EditorialFinding {
            category: cat.into(),
            severity: Severity::Info,
            location: Location {
                paragraph: para.then(uuid::Uuid::now_v7),
                char_range: span,
                ..Default::default()
            },
            message: "m".into(),
            hint: None,
            source: "style",
            autofixable: false,
        };
        let findings = vec![
            mk("show-tell", true, Some((0, 3))), // rewritable (span)
            mk("structure", true, None),         // judgment → not rewritable
            mk("echo", false, None),             // no paragraph → not rewritable
            mk("filter", true, Some((4, 8))),    // rewritable (span)
        ];
        // no filter → both rewritable ones, in order
        let all = batch_fix_queue(&findings, None);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].1, "show-tell");
        assert_eq!(all[0].2, Some((0, 3)));
        assert_eq!(all[1].1, "filter");
        // category filter narrows
        let only = batch_fix_queue(&findings, Some("filter"));
        assert_eq!(only.len(), 1);
        assert_eq!(only[0].1, "filter");
        assert_eq!(only[0].2, Some((4, 8)));
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
