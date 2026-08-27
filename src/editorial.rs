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
    pub fn response(&self) -> ResponseKind {
        response_kind(&self.category)
    }
}

/// REDLINE-1 (RD-P0) — how a finding can be turned into an author-confirmed change.
/// Only [`Rewrite`](ResponseKind::Rewrite) ever touches prose, and only through the
/// existing confirmed-diff + snapshot contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn label(self) -> &'static str {
        match self {
            ResponseKind::Rewrite => "rewrite",
            ResponseKind::Decision => "decision",
            ResponseKind::Brief => "brief",
        }
    }

    /// REDLINE-1 (RD-P6) — the one-glyph marker the Editorial Pass shows per row:
    /// ✎ a diff-reviewed prose rewrite, ⇄ a guided authorial decision, ✉ a
    /// revision brief (advice, never a rewrite).
    pub fn glyph(self) -> char {
        match self {
            ResponseKind::Rewrite => '✎',
            ResponseKind::Decision => '⇄',
            ResponseKind::Brief => '✉',
        }
    }
}

/// Classify a finding category into its [`ResponseKind`]. A *Rewrite* is a category
/// with an honest single-paragraph prose fix; a *Decision* needs the author to
/// choose which way is right (a continuity break, a described-two-ways drift, a
/// prose-vs-fact conflict) before a targeted rewrite; everything structural or
/// book-level is a *Brief*. Unknown categories default to Brief — the safest, since
/// a Brief never edits prose. Pure.
pub fn response_kind(category: &str) -> ResponseKind {
    match category {
        // Honest single-locus prose fixes. `decision-resolve` is the synthetic
        // reconcile category the Decision flow rewrites through — it never appears
        // as a surfaced finding, but classifying it Rewrite keeps the RD-P7
        // invariant clean: every category with a [`fix_spec`] is a Rewrite.
        "echo" | "pacing" | "show-tell" | "filter" | "editor"
        | "anachronism" | "decision-resolve" => ResponseKind::Rewrite,
        // The author must choose which way is right; then we reconcile.
        "co_location" | "char_facts" | "drift" | "introduce" | "confusion"
        | "unpaid_setup" | "numeric" | "continuity" | "fact" | "world"
        | "premature_knowledge" | "leaked_secret" | "dropped_reveal"
        // BONDS (3.1): a relationship whose state changed with no scene to turn
        // it — the author chooses (add the scene, or soften the declaration).
        // unwritten_bond / dropped_bond stay Brief (advice, no single-locus fix).
        | "unearned_shift" => {
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
        // REDLINE-1 (RD-P3) — the synthetic slug the decision flow rewrites through:
        // the author has stated how to resolve the issue (passed as the rewrite's
        // note), and the AI applies it locally to the anchored paragraph.
        "decision-resolve" => FixSpec {
            slug: "editorial-fix-decision",
            builtin: "You reconcile a consistency issue in the paragraph below according to the \
author's decision, which follows this instruction. Change ONLY what is needed to make the paragraph \
consistent with that decision — leave everything else untouched. Preserve the meaning elsewhere, \
the author's voice, the language, and any Typst markup verbatim. Output ONLY the rewritten \
paragraph, no preamble.",
            label: "reconcile",
            scope: FixScope::Paragraph,
        },
        // REDLINE-1 (RD-P6) — the Inner Editor's own craft observation. The
        // finding's text is passed as the rewrite's note (like the decision flow),
        // so the model addresses THIS note rather than a generic category recipe.
        "editor" => FixSpec {
            slug: "editorial-fix-editor",
            builtin: "You revise the paragraph below to address the editor's craft note, which \
follows this instruction. Make the smallest change that honestly resolves the note — preserve the \
meaning, the author's voice, the paragraph's language, and any Typst markup verbatim. Output ONLY \
the rewritten paragraph, no preamble.",
            label: "editor-note",
            scope: FixScope::Paragraph,
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
///
/// REDLINE-1 (RD-P7) reversibility invariant: because `rewritable()` requires a
/// [`fix_spec`] and every fixable category is [`ResponseKind::Rewrite`], this
/// queue is Rewrite-only — a Decision or Brief finding can never enter the batch.
/// Finding-aware `editor` rewrites are excluded too (each carries its own note,
/// applied one at a time). Every fix the batch (or single `f`) applies streams
/// through `start_editorial_rewrite` → the AI diff review, which snapshots the
/// pre-rewrite prose (F6-restorable) before replacing — the sole prose-write path.
pub fn batch_fix_queue(findings: &[EditorialFinding], filter: Option<&str>) -> Vec<BatchFix> {
    findings
        .iter()
        .filter(|f| filter.is_none_or(|c| f.category == c))
        .filter(|f| f.rewritable())
        // REDLINE-1 (RD-P6) — an `editor` rewrite is finding-aware (each carries
        // its own observation as the rewrite's note), so it's applied one at a
        // time via `f`, never through the noteless batch sweep.
        .filter(|f| f.category != "editor")
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

/// 3.3.0 E2 — does a finding pass the Editorial Pass's active filters? The two
/// axes AND together: a `category` (`None` = all) and a `kind` response-kind
/// (`None` = all). The single predicate the pass's three filter sites share
/// (key handler, cursor recount, render). Pure.
pub fn matches_view(f: &EditorialFinding, category: Option<&str>, kind: Option<ResponseKind>) -> bool {
    category.is_none_or(|c| f.category == c) && kind.is_none_or(|k| f.response() == k)
}

/// 3.3.0 E1 — drop the findings whose fingerprint was **skipped** this session,
/// returning the kept findings and the number dropped. Unlike [`Dismissed`]
/// (deferrals, persisted to disk), session skips live only in the editor's
/// memory, so this filter is applied every time the Editorial Pass re-collects.
/// Pure.
pub fn drop_session_skips(
    findings: Vec<EditorialFinding>,
    skips: &BTreeSet<String>,
) -> (Vec<EditorialFinding>, usize) {
    if skips.is_empty() {
        return (findings, 0);
    }
    let before = findings.len();
    let kept: Vec<EditorialFinding> =
        findings.into_iter().filter(|f| !skips.contains(&f.fingerprint())).collect();
    let dropped = before - kept.len();
    (kept, dropped)
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
/// register) routes it — all Brief (a book-level voice observation, no single-locus
/// fix). B4 — the `drift` kind is surfaced as category `voice-drift`, distinct from
/// WORLD-2 semantic `drift` (which is a Decision with a paragraph anchor); sharing
/// the bare `drift` category routed this anchorless finding to a dead-end Decision
/// and lumped voice- and semantic-drift into one filter bucket. B2 — returns `None`
/// for `Praise` (celebratory — nothing to fix), mirroring [`from_editor_finding`].
pub(crate) fn from_stylist_finding(
    f: &crate::inner_stylist::Finding,
) -> Option<EditorialFinding> {
    use crate::inner_stylist::Severity as SS;
    let severity = match f.severity {
        SS::Praise => return None,
        SS::Concern => Severity::Warn,
        SS::Note => Severity::Info,
    };
    let category = match f.kind {
        "drift" => "voice-drift".to_string(),
        other => other.to_string(),
    };
    Some(EditorialFinding {
        category,
        severity,
        location: Location::default(),
        message: f.message.clone(),
        hint: None,
        source: "stylist",
        autofixable: false,
    })
}

/// REDLINE-1 (RD-P6) — an Inner Editor craft observation → the worklist as a
/// finding-aware `editor` Rewrite: the observation itself becomes the rewrite's
/// note (see [`fix_spec`]'s `editor` recipe), so the AI addresses THIS note.
/// Returns `None` for Praise (celebratory — nothing to fix), for a suppressed
/// finding (a declared intent already covers it), or when the finding has no
/// paragraph anchor (nothing to rewrite). The observation stays in the
/// paragraph's language; its evidence rides along as the hint.
pub(crate) fn from_editor_finding(
    f: &crate::inner_editor::StoredEditorFinding,
) -> Option<EditorialFinding> {
    use crate::inner_editor::types::EditorSeverity as ES;
    let fin = &f.finding;
    if fin.suppressed_by.is_some() || matches!(fin.severity, ES::Praise) {
        return None;
    }
    let paragraph = f.paragraph_id?;
    Some(EditorialFinding {
        category: "editor".into(),
        severity: match fin.severity {
            ES::Concern => Severity::Warn,
            _ => Severity::Info,
        },
        location: Location { paragraph: Some(paragraph), ..Default::default() },
        message: fin.observation.clone(),
        hint: fin.evidence.clone(),
        source: "editor",
        autofixable: false,
    })
}

/// KEN-1 (KEN-P4) — an epistemic-continuity finding (who knows what, when) → the
/// worklist. `kind` (premature_knowledge / leaked_secret / dropped_reveal) routes
/// it (all Decision — the author chooses: fix the leak, move the reveal, or add a
/// grant). `anchor` is the offending paragraph.
pub(crate) fn from_knowledge_finding(f: &crate::ken::KnowledgeFinding) -> EditorialFinding {
    use crate::ken::Severity as KS;
    EditorialFinding {
        category: f.kind.to_string(),
        severity: match f.severity {
            KS::Break => Severity::Error,
            KS::Notice => Severity::Warn,
            KS::Info => Severity::Info,
        },
        location: Location { chapter: chapter_label(f.chapter), paragraph: f.anchor, ..Default::default() },
        message: f.message.clone(),
        hint: None,
        source: "knowledge",
        autofixable: false,
    }
}

/// BD-P3 — map a BONDS relationship finding into the shared worklist. The
/// character pair is already spelled out in the message, so this mirrors
/// [`from_knowledge_finding`] exactly; the `category` is the finding kind so
/// [`response_kind`] can route `unearned_shift` to a Decision (the others stay
/// Brief). BONDS shares KEN's `Severity`, so the mapping is identical.
pub(crate) fn from_bonds_finding(f: &crate::bonds::BondFinding) -> EditorialFinding {
    use crate::ken::Severity as KS;
    EditorialFinding {
        category: f.kind.to_string(),
        severity: match f.severity {
            KS::Break => Severity::Error,
            KS::Notice => Severity::Warn,
            KS::Info => Severity::Info,
        },
        location: Location { chapter: chapter_label(f.chapter), paragraph: f.anchor, ..Default::default() },
        message: f.message.clone(),
        hint: None,
        source: "bonds",
        autofixable: false,
    }
}

/// `"ch. N"` for a 1-based chapter ordinal, or `None` for book-level (0).
fn chapter_label(chapter: u32) -> Option<String> {
    (chapter > 0).then(|| format!("ch. {chapter}"))
}

/// Rank + dedup a flat list of findings into the report: sort by severity
/// (error first), then category, then message, then location; drop findings
/// identical in category + message + full location. B1 — the location tie-break
/// includes paragraph + char_range so two occurrences of the same word in one
/// chapter stay adjacent-but-distinct (only exact duplicates collapse), rather
/// than one silently swallowing the other.
pub fn aggregate(mut findings: Vec<EditorialFinding>) -> EditorialReport {
    findings.sort_by(|a, b| {
        a.severity
            .rank()
            .cmp(&b.severity.rank())
            .then_with(|| a.category.cmp(&b.category))
            .then_with(|| a.message.cmp(&b.message))
            .then_with(|| a.location.chapter.cmp(&b.location.chapter))
            .then_with(|| a.location.path.cmp(&b.location.path))
            .then_with(|| a.location.paragraph.cmp(&b.location.paragraph))
            .then_with(|| a.location.char_range.cmp(&b.location.char_range))
    });
    findings.dedup_by(|a, b| {
        a.category == b.category
            && a.message == b.message
            && a.location.chapter == b.location.chapter
            && a.location.path == b.location.path
            && a.location.paragraph == b.location.paragraph
            && a.location.char_range == b.location.char_range
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
    fn aggregate_keeps_distinct_occurrences_but_drops_exact_duplicates() {
        // B1 — two "very"s in the same chapter differ only by char_range; both must
        // survive. An exact duplicate (same everything) still collapses to one.
        let mk = |para: Option<Uuid>, range: Option<(usize, usize)>| EditorialFinding {
            category: "filter".into(),
            severity: Severity::Info,
            location: Location {
                chapter: Some("ch. 1".into()),
                paragraph: para,
                char_range: range,
                path: None,
            },
            message: "filter word: `very` — consider cutting".into(),
            hint: None,
            source: "stylist",
            autofixable: false,
        };
        let p = Uuid::new_v4();
        let a = mk(Some(p), Some((10, 14)));
        let b = mk(Some(p), Some((40, 44))); // same word, different spot
        let dup = a.clone(); // exact duplicate of a
        let report = aggregate(vec![a, b, dup]);
        let n = report
            .findings
            .iter()
            .filter(|f| f.category == "filter")
            .count();
        assert_eq!(n, 2, "both occurrences kept, exact dup dropped: {:#?}", report.findings);
    }

    #[test]
    fn stylist_praise_is_dropped_not_surfaced() {
        // B2 — a Praise "all voices distinct" finding must NOT enter the worklist
        // (it would sit there forever on a healthy book); Concern/Note still map.
        use crate::inner_stylist::{Finding as SF, Severity as SS};
        let praise = SF {
            severity: SS::Praise,
            kind: "distinctiveness",
            key: "k".into(),
            message: "all distinct — nobody reads like anybody else.".into(),
        };
        assert!(from_stylist_finding(&praise).is_none(), "praise dropped");
        let concern = SF { severity: SS::Concern, ..praise.clone() };
        let note = SF { severity: SS::Note, ..praise.clone() };
        assert_eq!(from_stylist_finding(&concern).unwrap().severity, Severity::Warn);
        assert_eq!(from_stylist_finding(&note).unwrap().severity, Severity::Info);
    }

    #[test]
    fn stylist_drift_is_voice_drift_and_routes_to_brief() {
        // B4 — the stylist's voice `drift` is disambiguated from WORLD-2 semantic
        // `drift`: distinct category, and Brief (not the anchorless dead-end Decision).
        use crate::inner_stylist::{Finding as SF, Severity as SS};
        let f = SF {
            severity: SS::Note,
            kind: "drift",
            key: "k".into(),
            message: "voice drifts toward the narrator's register.".into(),
        };
        let e = from_stylist_finding(&f).unwrap();
        assert_eq!(e.category, "voice-drift");
        assert_eq!(response_kind(&e.category), ResponseKind::Brief);
        // Semantic drift keeps its Decision routing (it has a paragraph anchor).
        assert_eq!(response_kind("drift"), ResponseKind::Decision);
    }

    #[test]
    fn matches_view_ands_category_and_kind() {
        // "echo" → Rewrite, "co_location" → Decision, "structure" → Brief.
        let echo = EditorialFinding {
            category: "echo".into(),
            severity: Severity::Info,
            location: Location::default(),
            message: "m".into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        let decision = EditorialFinding { category: "co_location".into(), ..echo.clone() };
        // No filters → everything passes.
        assert!(matches_view(&echo, None, None));
        assert!(matches_view(&decision, None, None));
        // Kind filter alone.
        assert!(matches_view(&echo, None, Some(ResponseKind::Rewrite)));
        assert!(!matches_view(&echo, None, Some(ResponseKind::Decision)));
        assert!(matches_view(&decision, None, Some(ResponseKind::Decision)));
        // The two axes AND: echo passes category=echo but not kind=Decision.
        assert!(matches_view(&echo, Some("echo"), Some(ResponseKind::Rewrite)));
        assert!(!matches_view(&echo, Some("echo"), Some(ResponseKind::Decision)));
        assert!(!matches_view(&echo, Some("co_location"), Some(ResponseKind::Rewrite)));
    }

    #[test]
    fn session_skips_drop_matching_findings_only() {
        let mk = |msg: &str| EditorialFinding {
            category: "echo".into(),
            severity: Severity::Info,
            location: Location::default(),
            message: msg.into(),
            hint: None,
            source: "doctor",
            autofixable: false,
        };
        let (a, b, c) = (mk("echo: `about` ×5"), mk("echo: `wind` ×4"), mk("echo: `grey` ×3"));
        let skips: BTreeSet<String> = [a.fingerprint(), c.fingerprint()].into_iter().collect();
        let (kept, dropped) = drop_session_skips(vec![a, b.clone(), c], &skips);
        assert_eq!(dropped, 2);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].fingerprint(), b.fingerprint(), "only the un-skipped finding survives");
        // Empty skip set is a no-op (and doesn't clone-walk).
        let (kept2, dropped2) = drop_session_skips(vec![b.clone()], &BTreeSet::new());
        assert_eq!(dropped2, 0);
        assert_eq!(kept2.len(), 1);
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
        // Honest single-locus prose fixes. B5 — every surfaced Rewrite category
        // MUST have a fix_spec, else the cockpit shows the ✎ glyph with a no-op
        // `f` (the RD-P7 broken-affordance the invariant forbids). `voice` was
        // Rewrite with no fix_spec and no producer — dropped.
        for c in ["echo", "pacing", "show-tell", "filter", "editor", "anachronism"] {
            assert_eq!(response_kind(c), Rewrite, "{c} is a Rewrite");
            assert!(fix_spec(c).is_some(), "{c} Rewrite must have a fix_spec");
        }
        // `voice` is no longer a Rewrite (dead category → Brief default).
        assert_eq!(response_kind("voice"), Brief, "dead `voice` category → Brief");
        // The author must choose which way is right, then reconcile.
        for c in ["co_location", "char_facts", "drift", "introduce", "confusion", "unpaid_setup", "unearned_shift"] {
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
    fn response_kind_glyphs_are_distinct_and_stable() {
        assert_eq!(ResponseKind::Rewrite.glyph(), '✎');
        assert_eq!(ResponseKind::Decision.glyph(), '⇄');
        assert_eq!(ResponseKind::Brief.glyph(), '✉');
    }

    #[test]
    fn inner_editor_findings_become_finding_aware_editor_rewrites() {
        use crate::inner_editor::types::{
            EditorCategory, EditorFinding, EditorSeverity,
        };
        use crate::inner_editor::StoredEditorFinding;
        let para = uuid::Uuid::now_v7();
        let mk = |sev: EditorSeverity, paragraph: Option<uuid::Uuid>, suppressed: Option<String>| {
            StoredEditorFinding {
                id: uuid::Uuid::now_v7(),
                paragraph_id: paragraph,
                finding: EditorFinding {
                    category: EditorCategory::StyleObservation,
                    severity: sev,
                    observation: "the verb tense wobbles mid-paragraph".into(),
                    observation_en: "the verb tense wobbles mid-paragraph".into(),
                    evidence: Some("«walked» then «walks»".into()),
                    conditional: false,
                    suppressed_by: suppressed,
                },
            }
        };
        // A Concern with an anchor → a rewritable `editor` finding whose message is
        // the observation and whose hint is the evidence.
        let f = from_editor_finding(&mk(EditorSeverity::Concern, Some(para), None))
            .expect("Concern + anchor → a finding");
        assert_eq!(f.category, "editor");
        assert_eq!(f.severity, Severity::Warn);
        assert_eq!(f.location.paragraph, Some(para));
        assert_eq!(f.message, "the verb tense wobbles mid-paragraph");
        assert_eq!(f.hint.as_deref(), Some("«walked» then «walks»"));
        assert_eq!(f.response(), ResponseKind::Rewrite);
        assert!(f.rewritable(), "editor + a paragraph ⇒ AI-rewritable");
        assert_eq!(f.source, "editor");
        // A Note maps to Info but is still a rewritable editor finding.
        assert_eq!(
            from_editor_finding(&mk(EditorSeverity::Note, Some(para), None)).unwrap().severity,
            Severity::Info
        );
        // Praise, a suppressed finding, and an anchorless finding all drop out.
        assert!(from_editor_finding(&mk(EditorSeverity::Praise, Some(para), None)).is_none());
        assert!(
            from_editor_finding(&mk(EditorSeverity::Concern, Some(para), Some("declared".into())))
                .is_none()
        );
        assert!(from_editor_finding(&mk(EditorSeverity::Concern, None, None)).is_none());
    }

    #[test]
    fn editor_rewrites_are_excluded_from_the_batch_sweep() {
        let mk = |cat: &str| EditorialFinding {
            category: cat.into(),
            severity: Severity::Warn,
            location: Location { paragraph: Some(uuid::Uuid::now_v7()), ..Default::default() },
            message: "m".into(),
            hint: None,
            source: "x",
            autofixable: false,
        };
        let findings = vec![mk("echo"), mk("editor"), mk("filter")];
        let q = batch_fix_queue(&findings, None);
        // echo + filter walk the batch; the finding-aware editor rewrite does not.
        let cats: Vec<&str> = q.iter().map(|(_, c, _)| c.as_str()).collect();
        assert_eq!(cats, vec!["echo", "filter"]);
    }

    #[test]
    fn batch_is_rewrite_only_the_reversibility_invariant() {
        // RD-P7 — the AI rewrite path (batch `F` and single `f`) is the ONLY thing
        // that mutates prose, and it snapshots before replacing. Guard that only
        // Rewrite findings can reach it: every category with a fix recipe is a
        // Rewrite, so a Decision/Brief can never acquire one and slip through.
        for cat in ["echo", "pacing", "show-tell", "filter", "anachronism", "editor", "decision-resolve"] {
            assert!(fix_spec(cat).is_some(), "{cat} should have a fix recipe");
            assert_eq!(response_kind(cat), ResponseKind::Rewrite, "{cat} must be a Rewrite");
        }
        // The converse over a mixed report: only Rewrite findings (minus the
        // finding-aware `editor`) walk the batch, in display order.
        let mk = |cat: &str| EditorialFinding {
            category: cat.into(),
            severity: Severity::Warn,
            location: Location { paragraph: Some(uuid::Uuid::now_v7()), ..Default::default() },
            message: "m".into(),
            hint: None,
            source: "x",
            autofixable: false,
        };
        let report = vec![
            mk("echo"),        // Rewrite → batched
            mk("co_location"), // Decision → never
            mk("shape_sag"),   // Brief → never
            mk("editor"),      // Rewrite but finding-aware → excluded
            mk("anachronism"), // Rewrite → batched
        ];
        let q = batch_fix_queue(&report, None);
        assert_eq!(
            q.iter().map(|(_, c, _)| c.as_str()).collect::<Vec<_>>(),
            vec!["echo", "anachronism"]
        );
        assert!(
            q.iter().all(|(_, c, _)| response_kind(c) == ResponseKind::Rewrite),
            "everything the batch yields is Rewrite-classified"
        );
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
        // RD-P3 — the decision flow reconciles through a Paragraph fix.
        assert_eq!(fix_spec("decision-resolve").unwrap().scope, FixScope::Paragraph);
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
