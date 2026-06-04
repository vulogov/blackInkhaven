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
    /// `.typ` file on disk is 0 bytes AND bdslib has
    /// no content for the node either.  Real data
    /// loss — prose for that paragraph is gone.
    ZeroByteFile,
    /// DB has a paragraph row with no on-disk file
    /// AND no bdslib content.  Same shape: real
    /// data loss.  Delete the row to clean up.
    OrphanParagraphRow,
    /// Variant of OrphanParagraphRow with a
    /// suspicious rel-path (empty / `..` segments).
    /// Kept as a separate class so the user can
    /// see the path-malformation pattern.
    MissingReferencedFile,
    /// `<paragraph>.comments.json` doesn't parse as
    /// JSON.  User comments unreadable.
    CorruptCommentsSidecar,
    /// 1.2.15+ — paragraph row exists, disk file
    /// is missing OR zero-byte, but bdslib has
    /// non-empty content.  This is RECOVERABLE:
    /// re-save the paragraph from the TUI (or use
    /// the autofix rematerialize path) and the
    /// disk file comes back from bdslib content.
    /// Common for system books like Prompts / Help /
    /// Typst whose paragraphs are auto-seeded from
    /// embedded defaults at first open — and the
    /// disk file was later deleted (manually,
    /// import script, partial restore).  The
    /// editor's `load_paragraph` reads bdslib as
    /// a fallback so the paragraph is still
    /// openable; this finding is informational.
    BdslibOnly,
    /// 1.2.16+ Phase A.6 — character mentioned in
    /// the first 30% of the manuscript but absent
    /// from the last 30%.  Flags potentially-
    /// dropped characters whose arcs the author
    /// forgot to wrap up.  Info severity — false
    /// positives expected (a deliberately-dropped
    /// minor character is a legitimate authorial
    /// choice).  No autofix.
    DroppedCharacter,
    /// 1.2.16+ Phase A.6 — chapter word count
    /// > 3× or < 0.3× the trailing 5-chapter
    /// mean.  Flags pacing collapses (a 12K-word
    /// chapter sandwiched between 4K-word ones,
    /// or vice versa) the author may have
    /// shipped without noticing.  Info severity
    /// — could be intentional (epilogue is
    /// supposed to be short).  No autofix.
    PacingCollapse,
    /// 1.2.16+ Phase A.6 — thread whose newest
    /// waypoint is older than 30 days.  Mirrors
    /// the 1.2.14 `inkhaven thread doctor`
    /// dormant detector; surfaced here so the
    /// dashboard + the doctor TUI panel both
    /// report on stalled arcs.  Info severity
    /// — a thread can be paused on purpose
    /// (saved for a later book).  No autofix.
    StalledThread,
    /// 1.2.16+ Phase A.5 — near-miss spelling of a
    /// canonical multi-word name from the
    /// Characters / Places / Artefacts system
    /// books.  Catches typos like
    /// "Aerin Stormbreaker" when the canonical
    /// entry is "Aerin Stormbringer" — shared
    /// first word + the rest differs by a small
    /// edit distance.  Info severity (could be
    /// an intentional variant) — no autofix.
    NamingInconsistency,
    /// 1.2.19+ C.1 — a distinctive word reused close
    /// together (≥ `editor.echo_min_repeats` times within
    /// `editor.echo_window` paragraphs of a chapter).
    /// Catches the revision-stage echo tic.  Multilingual
    /// via the project's Snowball stemmer + stop-words
    /// (exact-form fallback for non-Snowball languages).
    /// Info severity — an echo can be deliberate
    /// (anaphora, refrain).  No autofix.
    EchoRepetition,
    /// 1.2.19+ C.2 — a numeric / temporal / spatial
    /// contradiction: a directed distance reversed at the
    /// same magnitude close together ("200 leagues north"
    /// … "200 leagues south"), or two different durations
    /// in immediate proximity ("the three-day journey …
    /// after a week").  Multilingual via the per-language
    /// continuity lexicon (en/fr/es bundled; others skip
    /// with a clear message).  Info severity — framed as
    /// "review whether these refer to the same thing."
    /// No autofix.
    NumericContradiction,
    /// 1.2.19+ C.3 — a character attribute that changes
    /// across chapters in the continuity bible ("ch.3:
    /// eyes green; ch.17: eyes brown").  Reads the
    /// `inkhaven continuity extract` sidecar; multilingual
    /// drift comparison via the project's Snowball stemmer
    /// so inflected restatements don't false-flag.  Info
    /// severity — an attribute can legitimately change
    /// (an injury, dyed hair).  No autofix.
    ContinuityDrift,
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
            ScanClass::BdslibOnly => "bdslib-only",
            ScanClass::DroppedCharacter => "dropped-character",
            ScanClass::PacingCollapse => "pacing-collapse",
            ScanClass::StalledThread => "stalled-thread",
            ScanClass::NamingInconsistency => "naming-inconsistency",
            ScanClass::EchoRepetition => "echo-repetition",
            ScanClass::NumericContradiction => "numeric-contradiction",
            ScanClass::ContinuityDrift => "continuity-drift",
        }
    }

    /// Parse from the CLI `--class <name>` argument.
    pub fn from_slug(s: &str) -> Option<Self> {
        Some(match s {
            "zero-byte-file" => ScanClass::ZeroByteFile,
            "orphan-paragraph-row" => ScanClass::OrphanParagraphRow,
            "missing-referenced-file" => ScanClass::MissingReferencedFile,
            "corrupt-comments-sidecar" => ScanClass::CorruptCommentsSidecar,
            "bdslib-only" => ScanClass::BdslibOnly,
            "dropped-character" => ScanClass::DroppedCharacter,
            "pacing-collapse" => ScanClass::PacingCollapse,
            "stalled-thread" => ScanClass::StalledThread,
            "naming-inconsistency" => ScanClass::NamingInconsistency,
            "echo-repetition" => ScanClass::EchoRepetition,
            "numeric-contradiction" => ScanClass::NumericContradiction,
            "continuity-drift" => ScanClass::ContinuityDrift,
            _ => return None,
        })
    }

    pub const ALL: [ScanClass; 12] = [
        ScanClass::ZeroByteFile,
        ScanClass::OrphanParagraphRow,
        ScanClass::MissingReferencedFile,
        ScanClass::CorruptCommentsSidecar,
        ScanClass::BdslibOnly,
        ScanClass::DroppedCharacter,
        ScanClass::PacingCollapse,
        ScanClass::StalledThread,
        ScanClass::NamingInconsistency,
        ScanClass::EchoRepetition,
        ScanClass::NumericContradiction,
        ScanClass::ContinuityDrift,
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

    // 1.2.15+ — the zero-byte + orphan checks both
    // need to consult bdslib for fallback content,
    // so they emit `BdslibOnly` findings too.  When
    // the caller selects only one of those classes,
    // we keep the cross-class findings filtered
    // down via the `run(...)` guard below.
    if run(ScanClass::ZeroByteFile) || run(ScanClass::BdslibOnly) {
        for finding in scan_zero_byte_files(&layout, &hierarchy, &store) {
            if run(finding.class) {
                report.findings.push(finding);
            }
        }
    }
    if run(ScanClass::OrphanParagraphRow)
        || run(ScanClass::MissingReferencedFile)
        || run(ScanClass::BdslibOnly)
    {
        for finding in scan_orphans_and_missing(&layout, &hierarchy, &store) {
            if run(finding.class) {
                report.findings.push(finding);
            }
        }
    }
    if run(ScanClass::CorruptCommentsSidecar) {
        report.findings.extend(scan_corrupt_comments(&layout, &hierarchy));
    }
    // 1.2.16+ Phase A.6 — plot-mining detectors.
    // Each adds its own findings independently;
    // the doctor TUI panel + the CLI consumer
    // group them naturally via the `class` slug.
    if run(ScanClass::DroppedCharacter) {
        report.findings.extend(scan_dropped_characters(&layout, &hierarchy));
    }
    if run(ScanClass::PacingCollapse) {
        report.findings.extend(scan_pacing_collapse(&layout, &hierarchy));
    }
    if run(ScanClass::StalledThread) {
        report.findings.extend(scan_stalled_threads(&layout, &hierarchy));
    }
    if run(ScanClass::NamingInconsistency) {
        report.findings.extend(scan_naming_inconsistencies(&layout, &hierarchy));
    }
    // 1.2.19+ C.1 — echo / repetition-at-distance.
    if run(ScanClass::EchoRepetition) {
        report.findings.extend(scan_echoes(&layout, &hierarchy, &cfg));
    }
    // 1.2.19+ C.2 — numeric / temporal / spatial
    // contradictions.
    if run(ScanClass::NumericContradiction) {
        report.findings.extend(scan_numeric_contradictions(&layout, &hierarchy, &cfg));
    }
    // 1.2.19+ C.3 — continuity-bible drift.
    if run(ScanClass::ContinuityDrift) {
        report.findings.extend(scan_continuity_drift(&layout, &cfg));
    }

    Ok(report)
}

/// 1.2.19+ C.3 — flag character attributes that change
/// across chapters in the continuity bible.  Reads the
/// `inkhaven continuity extract` sidecar; empty / absent
/// sidecar → no findings.  Drift comparison uses the
/// bible's recorded language (falls back to the project
/// `language`).
fn scan_continuity_drift(
    layout: &ProjectLayout,
    cfg: &Config,
) -> Vec<ScanFinding> {
    let Ok(bible) = crate::continuity_bible::ContinuityBible::load(&layout.root)
    else {
        return Vec::new();
    };
    if bible.facts.is_empty() {
        return Vec::new();
    }
    let language = if !bible.language.trim().is_empty() {
        bible.language.clone()
    } else if !cfg.language.trim().is_empty() {
        cfg.language.clone()
    } else {
        "english".to_string()
    };
    crate::continuity_bible::detect_drift(&bible, &language)
        .into_iter()
        .map(|d| {
            let where_ = d
                .conflicts
                .iter()
                .map(|(ch, v)| format!("{ch}: {v}"))
                .collect::<Vec<_>>()
                .join("; ");
            ScanFinding {
                class: ScanClass::ContinuityDrift,
                severity: ScanSeverity::Info,
                path: None,
                detail: format!(
                    "continuity drift: `{}`'s `{}` changes across chapters — {where_}",
                    d.character, d.attribute,
                ),
            }
        })
        .collect()
}

/// 1.2.19+ C.2 — flag numeric / temporal / spatial
/// contradictions per user-book chapter.  Multilingual
/// via the project `language`'s continuity lexicon; skips
/// (with no findings) when no lexicon is bundled for the
/// language — the CLI surfaces that separately so the
/// user knows to bootstrap one.
fn scan_numeric_contradictions(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    cfg: &Config,
) -> Vec<ScanFinding> {
    let language = if cfg.language.trim().is_empty() {
        "english".to_string()
    } else {
        cfg.language.clone()
    };
    let Some(lexicon) = crate::continuity::built_in_lexicon(&language) else {
        // No lexicon for this language — graceful skip.
        return Vec::new();
    };
    let contra_cfg = crate::continuity::ContradictionConfig::default();

    let mut out: Vec<ScanFinding> = Vec::new();
    for chapter_id in collect_user_book_chapter_ordinals(hierarchy) {
        let prose = read_chapter_prose(layout, hierarchy, chapter_id);
        let plain = crate::audiobook::typst_to_plain(&prose);
        let sentences = crate::continuity::split_sentences(&plain);
        if sentences.is_empty() {
            continue;
        }
        let quantities =
            crate::continuity::extract_quantities(&sentences, &lexicon);
        let chapter_label = hierarchy
            .get(chapter_id)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let chapter_path = hierarchy
            .get(chapter_id)
            .and_then(|n| n.file.clone());
        for c in crate::continuity::detect_contradictions(&quantities, &contra_cfg)
        {
            let what = match c.kind {
                crate::continuity::ContradictionKind::DirectionReversal => {
                    "direction reversal"
                }
                crate::continuity::ContradictionKind::TemporalMismatch => {
                    "duration mismatch"
                }
            };
            out.push(ScanFinding {
                class: ScanClass::NumericContradiction,
                severity: ScanSeverity::Info,
                path: chapter_path.clone(),
                detail: format!(
                    "{what}: `{}` vs `{}` — review whether these refer to the same thing (chapter `{}`)",
                    c.a_raw, c.b_raw, chapter_label,
                ),
            });
        }
    }
    out
}

/// 1.2.19+ C.1 — flag distinctive words reused close
/// together within each user-book chapter.  Multilingual
/// via the project's `language` (Snowball stemmer +
/// stop-words; exact-form fallback otherwise).
fn scan_echoes(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    cfg: &Config,
) -> Vec<ScanFinding> {
    let echo_cfg = crate::echo::EchoConfig {
        window: cfg.editor.echo_window.max(1),
        min_repeats: cfg.editor.echo_min_repeats.max(2),
        max_global: cfg.editor.echo_max_global.max(1),
        ..crate::echo::EchoConfig::default()
    };
    let language = if cfg.language.trim().is_empty() {
        "english".to_string()
    } else {
        cfg.language.clone()
    };

    let mut out: Vec<ScanFinding> = Vec::new();
    for chapter_id in collect_user_book_chapter_ordinals(hierarchy) {
        let paragraphs = collect_chapter_paragraph_prose(layout, hierarchy, chapter_id);
        if paragraphs.len() < 2 {
            continue;
        }
        let chapter_label = hierarchy
            .get(chapter_id)
            .map(|n| n.title.clone())
            .unwrap_or_default();
        let chapter_path = hierarchy
            .get(chapter_id)
            .and_then(|n| n.file.clone());
        let findings =
            crate::echo::detect_echoes(&paragraphs, &language, None, &echo_cfg);
        for f in findings {
            let where_ = if f.para_start == f.para_end {
                format!("¶{}", f.para_start)
            } else {
                format!("¶{}–{}", f.para_start, f.para_end)
            };
            // Lead with the headword (the first surface
            // form — readable) rather than the raw stem
            // (`alway`), listing the other inflections when
            // they differ.
            let headword = f
                .surface_forms
                .first()
                .cloned()
                .unwrap_or_else(|| f.stem.clone());
            let forms = if f.surface_forms.len() > 1 {
                format!(" (forms: {})", f.surface_forms.join(", "))
            } else {
                String::new()
            };
            out.push(ScanFinding {
                class: ScanClass::EchoRepetition,
                severity: ScanSeverity::Info,
                path: chapter_path.clone(),
                detail: format!(
                    "echo: `{}`{} appears {}× within {} (chapter `{}`)",
                    headword, forms, f.count, where_, chapter_label,
                ),
            });
        }
    }
    out
}

/// Per-paragraph plain prose for a chapter (markup
/// stripped via the audiobook plain-text pass), in
/// reading order.  Empty paragraphs are skipped.
fn collect_chapter_paragraph_prose(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    chapter_id: uuid::Uuid,
) -> Vec<String> {
    use crate::store::NodeKind;
    let mut out = Vec::new();
    for id in hierarchy.collect_subtree(chapter_id) {
        let Some(p) = hierarchy.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = p.file.as_ref() else { continue };
        let abs = layout.root.join(rel);
        let Ok(text) = std::fs::read_to_string(&abs) else { continue };
        let plain = crate::audiobook::typst_to_plain(&text);
        if !plain.trim().is_empty() {
            out.push(plain);
        }
    }
    out
}

/// 1.2.15+ — does bdslib have non-empty content
/// for this node?  Returns `Some(byte_len)` when
/// content is present, `None` otherwise.  Errors
/// from the store call are treated as "no content"
/// — the scan would rather under-report than crash.
fn bdslib_content_len(store: &Store, id: uuid::Uuid) -> Option<usize> {
    match store.get_content(id) {
        Ok(Some(bytes)) if !bytes.is_empty() => Some(bytes.len()),
        _ => None,
    }
}

fn scan_zero_byte_files(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    store: &Store,
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
            // 1.2.15+ — disk is 0 bytes, but bdslib
            // may still hold the prose.  If it does,
            // this is recoverable — surface as
            // BdslibOnly / Info instead of
            // Critical data loss.
            match bdslib_content_len(store, node.id) {
                Some(n) => out.push(ScanFinding {
                    class: ScanClass::BdslibOnly,
                    severity: ScanSeverity::Info,
                    path: Some(abs.display().to_string()),
                    detail: format!(
                        "paragraph `{}` has 0-byte disk file but bdslib holds {} bytes — re-save in the editor or autofix to rematerialize",
                        node.slug, n,
                    ),
                }),
                None => out.push(ScanFinding {
                    class: ScanClass::ZeroByteFile,
                    severity: ScanSeverity::Critical,
                    path: Some(abs.display().to_string()),
                    detail: format!(
                        "paragraph `{}` resolves to a 0-byte file AND bdslib has no content — prose lost",
                        node.slug,
                    ),
                }),
            }
        }
    }
    out
}

fn scan_orphans_and_missing(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    store: &Store,
) -> Vec<ScanFinding> {
    let mut out: Vec<ScanFinding> = Vec::new();
    for node in hierarchy.iter() {
        let Some(rel) = node.file.as_ref() else { continue };
        let abs = layout.root.join(rel);
        match std::fs::metadata(&abs) {
            Ok(_) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // 1.2.15+ — disk is gone; check
                // bdslib before declaring this a
                // real orphan.  System-book seeds
                // (Prompts / Help / Typst) and any
                // paragraph created by a flow that
                // writes only to bdslib live here
                // legitimately.  The editor's
                // `load_paragraph` reads bdslib as
                // a fallback, so the paragraph is
                // still openable.  Repair path:
                // re-save (or `--autofix
                // rematerialize`) writes the bdslib
                // content back to disk.
                if let Some(n) = bdslib_content_len(store, node.id) {
                    out.push(ScanFinding {
                        class: ScanClass::BdslibOnly,
                        severity: ScanSeverity::Info,
                        path: Some(abs.display().to_string()),
                        detail: format!(
                            "paragraph `{}` has no disk file but bdslib holds {} bytes — recoverable",
                            node.slug, n,
                        ),
                    });
                    continue;
                }
                // No disk, no bdslib content —
                // genuine orphan.  "Malformed path"
                // sub-classifier preserved from
                // D.1.
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
                        "paragraph row `{}` points at missing file {} and bdslib has no content either",
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
        ScanClass::BdslibOnly => {
            // 1.2.15+ — rematerialize the disk file
            // from bdslib content.  Non-destructive:
            // never overwrites an existing on-disk
            // file (the scan said it was missing or
            // 0 bytes; we double-check at write
            // time so a concurrent save isn't
            // clobbered).  Atomic via io_atomic.
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
            let mut found_id: Option<uuid::Uuid> = None;
            for node in hierarchy.iter() {
                if node.file.as_deref() == Some(rel.as_str()) {
                    found_id = Some(node.id);
                    break;
                }
            }
            let id = found_id.ok_or_else(|| {
                Error::Store(format!(
                    "no DB row matches {rel} — was the project mutated between scan and fix?"
                ))
            })?;
            let bytes = store
                .get_content(id)
                .map_err(|e| Error::Store(format!("bdslib read for {rel}: {e}")))?
                .ok_or_else(|| {
                    Error::Store(format!("bdslib has no content for {rel} — refusing to write empty file"))
                })?;
            if bytes.is_empty() {
                return Err(Error::Store(format!(
                    "bdslib has 0-byte content for {rel} — refusing to write empty file"
                )));
            }
            if let Some(parent) = abs_path.parent() {
                std::fs::create_dir_all(parent).map_err(Error::Io)?;
            }
            // Don't clobber a real on-disk file.
            // Re-check at write time.
            if let Ok(md) = std::fs::metadata(&abs_path) {
                if md.len() > 0 {
                    return Err(Error::Store(format!(
                        "disk file {abs} grew non-empty between scan and fix — refusing to overwrite"
                    )));
                }
            }
            crate::io_atomic::write(&abs_path, &bytes).map_err(Error::Io)?;
            Ok(format!(
                "rematerialized {} ({} bytes) from bdslib",
                rel,
                bytes.len()
            ))
        }
        // 1.2.16+ Phase A.6 — author-judgment
        // findings.  No auto-repair: only the
        // author can decide whether a dropped
        // character was intentional / a chapter's
        // pacing collapse was meant to land that
        // way / a thread was paused on purpose.
        ScanClass::DroppedCharacter
        | ScanClass::PacingCollapse
        | ScanClass::StalledThread
        | ScanClass::NamingInconsistency
        | ScanClass::EchoRepetition
        | ScanClass::NumericContradiction
        | ScanClass::ContinuityDrift => Err(Error::Store(format!(
            "no autofix for class `{}` — this is an author-judgment finding (review the prose / outline / threads)",
            finding.class.slug(),
        ))),
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

// ── 1.2.16+ Phase A.6 — plot-mining detectors ─────────────────

/// Threshold for the "dormant thread" / "dropped
/// character" heuristics — mirror the 1.2.14
/// thread doctor's 30-day window so all
/// stalled-arc reports agree.
const DORMANT_DAYS: u64 = 30;

/// Fraction of the manuscript counted as
/// "introduction" vs. "wrap-up" for the dropped-
/// character heuristic.  A character mentioned
/// in the first 30% of chapters but absent from
/// the last 30% is flagged.
const DROPPED_CHARACTER_INTRO_FRACTION: f64 = 0.30;
const DROPPED_CHARACTER_OUTRO_FRACTION: f64 = 0.30;

/// Pacing collapse thresholds: chapter word
/// counts more than 3× the trailing 5-chapter
/// mean (suspicious long) or less than 30% of
/// it (suspicious short) flag.
const PACING_HIGH_RATIO: f64 = 3.0;
const PACING_LOW_RATIO: f64 = 0.30;
const PACING_TRAILING_WINDOW: usize = 5;

fn scan_dropped_characters(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    use crate::store::{NodeKind, SYSTEM_TAG_CHARACTERS};

    // Step 1 — collect character names from the
    // Characters system book.
    let Some(chars_root) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_CHARACTERS)
    }) else {
        return Vec::new();
    };
    let character_names: Vec<String> = hierarchy
        .collect_subtree(chars_root.id)
        .into_iter()
        .filter_map(|id| hierarchy.get(id))
        .filter(|n| n.kind == NodeKind::Paragraph)
        .map(|n| n.title.clone())
        .filter(|t| !t.trim().is_empty())
        .collect();
    if character_names.is_empty() {
        return Vec::new();
    }

    // Step 2 — collect user-book chapter ordinals.
    let chapter_ordinals = collect_user_book_chapter_ordinals(hierarchy);
    let total_chapters = chapter_ordinals.len();
    if total_chapters < 5 {
        // Too few chapters to apply the heuristic
        // — a 3-chapter manuscript with a
        // character only in chapter 1 doesn't
        // necessarily mean "dropped".
        return Vec::new();
    }
    let intro_cap = (total_chapters as f64 * DROPPED_CHARACTER_INTRO_FRACTION) as usize;
    let outro_start = total_chapters
        .saturating_sub((total_chapters as f64 * DROPPED_CHARACTER_OUTRO_FRACTION) as usize);

    // Step 3 — for each character, find the
    // first + last chapter ordinal that mentions
    // them.  Case-insensitive substring match
    // — the same heuristic the existing lexicon
    // overlay uses for cheap detection.
    let mut findings: Vec<ScanFinding> = Vec::new();
    let mut chapter_bodies_cache: Vec<(usize, String)> = Vec::with_capacity(total_chapters);
    for (ordinal, chapter_node) in chapter_ordinals.iter().enumerate() {
        let body = read_chapter_prose(layout, hierarchy, *chapter_node);
        chapter_bodies_cache.push((ordinal, body.to_lowercase()));
    }
    for name in &character_names {
        let needle = name.to_lowercase();
        let mut first_seen: Option<usize> = None;
        let mut last_seen: Option<usize> = None;
        for (ordinal, body) in &chapter_bodies_cache {
            if body.contains(&needle) {
                if first_seen.is_none() {
                    first_seen = Some(*ordinal);
                }
                last_seen = Some(*ordinal);
            }
        }
        let (Some(first), Some(last)) = (first_seen, last_seen) else { continue };
        // Character appeared at all.  Dropped iff:
        //   first in intro (< intro_cap) AND
        //   last NOT in outro (< outro_start).
        if first < intro_cap && last < outro_start {
            findings.push(ScanFinding {
                class: ScanClass::DroppedCharacter,
                severity: ScanSeverity::Info,
                path: None,
                detail: format!(
                    "character `{name}` first appears in chapter {} (of {}) but is absent from the last {:.0}% (last seen chapter {})",
                    first + 1,
                    total_chapters,
                    DROPPED_CHARACTER_OUTRO_FRACTION * 100.0,
                    last + 1,
                ),
            });
        }
    }
    findings
}

fn scan_pacing_collapse(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    let chapter_ordinals = collect_user_book_chapter_ordinals(hierarchy);
    if chapter_ordinals.len() < PACING_TRAILING_WINDOW + 1 {
        return Vec::new();
    }
    let counts: Vec<i64> = chapter_ordinals
        .iter()
        .map(|&id| {
            let body = read_chapter_prose(layout, hierarchy, id);
            crate::progress::count_words(&body)
        })
        .collect();
    classify_pacing(&counts, hierarchy, &chapter_ordinals)
}

/// 1.2.16+ Phase A.6 — pure classifier for pacing
/// collapse.  Exposed for unit testing without
/// fs setup.  Takes parallel slices of chapter
/// word counts + chapter UUIDs; returns one
/// Info finding per outlier chapter.
pub(crate) fn classify_pacing(
    counts: &[i64],
    hierarchy: &crate::store::hierarchy::Hierarchy,
    chapter_ids: &[uuid::Uuid],
) -> Vec<ScanFinding> {
    let mut findings: Vec<ScanFinding> = Vec::new();
    for (i, &count) in counts.iter().enumerate().skip(PACING_TRAILING_WINDOW) {
        let window = &counts[i - PACING_TRAILING_WINDOW..i];
        let mean: f64 = window.iter().sum::<i64>() as f64 / window.len() as f64;
        if mean <= 0.0 {
            continue;
        }
        let ratio = count as f64 / mean;
        let (descriptor, severe) = if ratio > PACING_HIGH_RATIO {
            ("notably longer", true)
        } else if ratio < PACING_LOW_RATIO {
            ("notably shorter", true)
        } else {
            ("", false)
        };
        if !severe {
            continue;
        }
        let title = chapter_ids
            .get(i)
            .and_then(|id| hierarchy.get(*id))
            .map(|n| n.title.clone())
            .unwrap_or_else(|| format!("chapter {}", i + 1));
        findings.push(ScanFinding {
            class: ScanClass::PacingCollapse,
            severity: ScanSeverity::Info,
            path: None,
            detail: format!(
                "chapter `{title}` ({count} words) is {descriptor} than the trailing {} chapters (mean {:.0}, ratio {:.2}×)",
                PACING_TRAILING_WINDOW, mean, ratio,
            ),
        });
    }
    findings
}

fn scan_stalled_threads(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    use crate::store::{NodeKind, SYSTEM_TAG_THREADS};
    let Some(threads_root) = hierarchy.iter().find(|n| {
        n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(SYSTEM_TAG_THREADS)
    }) else {
        return Vec::new();
    };
    let threshold = std::time::SystemTime::now()
        - std::time::Duration::from_secs(DORMANT_DAYS * 86400);
    let mut findings: Vec<ScanFinding> = Vec::new();
    for thread in hierarchy.children_of(Some(threads_root.id)) {
        if thread.kind != NodeKind::Chapter {
            continue;
        }
        let mut newest: Option<std::time::SystemTime> = None;
        let mut waypoint_count = 0usize;
        for waypoint in hierarchy.children_of(Some(thread.id)) {
            if waypoint.kind != NodeKind::Paragraph {
                continue;
            }
            waypoint_count += 1;
            let Some(rel) = waypoint.file.as_ref() else { continue };
            let abs = layout.root.join(rel);
            let Ok(md) = std::fs::metadata(&abs) else { continue };
            let Ok(mtime) = md.modified() else { continue };
            newest = Some(match newest {
                Some(prev) if prev >= mtime => prev,
                _ => mtime,
            });
        }
        let stalled = match newest {
            Some(t) => t < threshold,
            None => waypoint_count > 0, // has waypoints but no readable mtime
        };
        if waypoint_count == 0 {
            // Empty thread is its own thing — flag it
            // as stalled too, for now (no waypoints =
            // no progress).
            findings.push(ScanFinding {
                class: ScanClass::StalledThread,
                severity: ScanSeverity::Info,
                path: None,
                detail: format!(
                    "thread `{}` has no waypoints yet",
                    thread.title,
                ),
            });
            continue;
        }
        if stalled {
            findings.push(ScanFinding {
                class: ScanClass::StalledThread,
                severity: ScanSeverity::Info,
                path: None,
                detail: format!(
                    "thread `{}` newest waypoint is > {} days old ({} waypoints total)",
                    thread.title, DORMANT_DAYS, waypoint_count,
                ),
            });
        }
    }
    findings
}

/// 1.2.16+ Phase A.5 — naming-inconsistency
/// detector.  Walks every entry in the
/// Characters / Places / Artefacts system books;
/// for each canonical multi-word name, looks for
/// near-miss occurrences in manuscript prose.
///
/// Single-word canonical names are skipped — too
/// many natural variants ("Aerin", "Aragorn")
/// to detect typos without burying the user in
/// false positives.  Multi-word names anchor on
/// the first word; the rest is matched against
/// the prose's next word via Levenshtein
/// distance.
fn scan_naming_inconsistencies(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<ScanFinding> {
    use crate::store::{
        SYSTEM_TAG_ARTEFACTS, SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_PLACES,
    };
    let canonical_names = collect_multi_word_canonical_names(
        hierarchy,
        &[SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_PLACES, SYSTEM_TAG_ARTEFACTS],
    );
    if canonical_names.is_empty() {
        return Vec::new();
    }
    // Concatenate all user-book chapter prose
    // once.  Detection runs over the joined
    // string per canonical name.
    let chapter_ordinals = collect_user_book_chapter_ordinals(hierarchy);
    let mut prose = String::new();
    for id in &chapter_ordinals {
        prose.push_str(&read_chapter_prose(layout, hierarchy, *id));
        prose.push('\n');
    }
    classify_naming_inconsistencies(&canonical_names, &prose)
}

/// 1.2.16+ Phase A.5 — pure classifier.  Exposed
/// for unit testing without fs setup.
///
/// Returns one finding per (canonical name,
/// near-miss variant) pair.  Same variant
/// repeated multiple times only fires once.
pub(crate) fn classify_naming_inconsistencies(
    canonical_names: &[String],
    prose: &str,
) -> Vec<ScanFinding> {
    let mut findings: Vec<ScanFinding> = Vec::new();
    for canonical in canonical_names {
        let parts: Vec<&str> = canonical.split_whitespace().collect();
        if parts.len() < 2 {
            continue;
        }
        let head = parts[0];
        let canonical_tail = parts[1..].join(" ");
        let canonical_lc = canonical.to_lowercase();
        // Walk prose for occurrences of `head` (case-insensitive),
        // capture the next whitespace-delimited word(s) matching
        // `canonical_tail.len()`-word window.
        let mut seen_variants: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let prose_lc = prose.to_lowercase();
        let head_lc = head.to_lowercase();
        let mut search_start = 0usize;
        while let Some(pos) = prose_lc[search_start..].find(&head_lc) {
            let abs_pos = search_start + pos;
            // Ensure `head` is at a word boundary —
            // previous char must be non-word.
            let prev_char = if abs_pos == 0 {
                ' '
            } else {
                prose_lc[..abs_pos].chars().last().unwrap_or(' ')
            };
            search_start = abs_pos + head_lc.len();
            if prev_char.is_alphanumeric() || prev_char == '_' {
                continue;
            }
            // After head: skip whitespace, capture
            // the next `canonical_tail.len()`-word
            // chunk.
            let rest = &prose[search_start..];
            let after = rest.trim_start();
            let need_words = canonical_tail.split_whitespace().count();
            let candidate: String = after
                .split_whitespace()
                .take(need_words)
                .collect::<Vec<&str>>()
                .join(" ");
            if candidate.is_empty() {
                continue;
            }
            // Strip trailing punctuation from the
            // candidate so "Stormbreaker," matches
            // "Stormbreaker" cleanly.
            let candidate_clean: String = candidate
                .trim_end_matches(|c: char| !c.is_alphanumeric())
                .to_string();
            if candidate_clean.is_empty() {
                continue;
            }
            let full = format!("{head} {candidate_clean}");
            // Skip exact-match occurrences (this
            // IS the canonical).
            if full.eq_ignore_ascii_case(canonical) {
                continue;
            }
            // Also skip if the full lowercased
            // string equals the canonical lower —
            // catches case differences.
            if full.to_lowercase() == canonical_lc {
                continue;
            }
            // Edit distance check on the variable
            // part.
            let dist = levenshtein(&candidate_clean.to_lowercase(), &canonical_tail.to_lowercase());
            if dist == 0 {
                continue;
            }
            // Heuristic: a typo is plausible when
            // the distance is small relative to
            // the length of the longer string.
            let max_len = candidate_clean
                .chars()
                .count()
                .max(canonical_tail.chars().count());
            if max_len == 0 {
                continue;
            }
            let ratio = dist as f64 / max_len as f64;
            // 0.0 < ratio <= 0.5 catches small
            // typos but excludes wholly different
            // words like "Aerin and Borin"
            // (distance very high).
            if ratio > 0.5 {
                continue;
            }
            if !seen_variants.insert(full.to_lowercase()) {
                continue;
            }
            findings.push(ScanFinding {
                class: ScanClass::NamingInconsistency,
                severity: ScanSeverity::Info,
                path: None,
                detail: format!(
                    "near-miss `{full}` in prose vs. canonical `{canonical}` (edit distance {dist})",
                ),
            });
        }
    }
    findings
}

/// Walk the named system books and collect every
/// entry's title.  Returns only multi-word
/// names (single-word names skip the naming
/// heuristic — too many false positives).
fn collect_multi_word_canonical_names(
    hierarchy: &crate::store::hierarchy::Hierarchy,
    system_tags: &[&str],
) -> Vec<String> {
    use crate::store::NodeKind;
    let mut out: Vec<String> = Vec::new();
    for tag in system_tags {
        let Some(book) = hierarchy.iter().find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(*tag)
        }) else {
            continue;
        };
        for id in hierarchy.collect_subtree(book.id) {
            let Some(n) = hierarchy.get(id) else { continue };
            if n.kind != NodeKind::Paragraph {
                continue;
            }
            let title = n.title.trim();
            if title.split_whitespace().count() < 2 {
                continue;
            }
            out.push(title.to_string());
        }
    }
    out
}

/// 1.2.16+ Phase A.5 — Levenshtein edit distance.
/// Standard DP; O(n*m).  Exposed for unit tests.
pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let a_chars: Vec<char> = a.chars().collect();
    let b_chars: Vec<char> = b.chars().collect();
    let n = a_chars.len();
    let m = b_chars.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut curr: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        curr[0] = i;
        for j in 1..=m {
            let cost = if a_chars[i - 1] == b_chars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[m]
}

/// Collect chapter UUIDs in user-book order
/// (skips chapters under system books like
/// Characters / Places / etc.).  Used by the
/// dropped-character + pacing-collapse
/// detectors.
fn collect_user_book_chapter_ordinals(
    hierarchy: &crate::store::hierarchy::Hierarchy,
) -> Vec<uuid::Uuid> {
    use crate::store::NodeKind;
    let mut out = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Chapter {
            continue;
        }
        let ancestors = hierarchy.ancestors(node);
        let under_system = ancestors
            .iter()
            .any(|a| a.kind == NodeKind::Book && a.system_tag.is_some());
        if !under_system {
            out.push(node.id);
        }
    }
    out
}

/// Concatenate every paragraph body under
/// `chapter_id` into one big string.  Used by
/// the prose-scanning detectors.
fn read_chapter_prose(
    layout: &ProjectLayout,
    hierarchy: &crate::store::hierarchy::Hierarchy,
    chapter_id: uuid::Uuid,
) -> String {
    use crate::store::NodeKind;
    let mut body = String::new();
    for id in hierarchy.collect_subtree(chapter_id) {
        let Some(p) = hierarchy.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(rel) = p.file.as_ref() else { continue };
        let abs = layout.root.join(rel);
        let Ok(text) = std::fs::read_to_string(&abs) else { continue };
        body.push_str(&text);
        body.push('\n');
    }
    body
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

    // 1.2.16+ Phase A.6 — new class slugs roundtrip.

    #[test]
    fn new_class_slugs_match_kebab_case_pattern() {
        for class in [
            ScanClass::DroppedCharacter,
            ScanClass::PacingCollapse,
            ScanClass::StalledThread,
        ] {
            let slug = class.slug();
            assert_eq!(
                ScanClass::from_slug(slug),
                Some(class),
                "slug `{slug}` should roundtrip"
            );
            assert!(slug.contains('-'), "slug `{slug}` should be kebab-case");
        }
    }

    #[test]
    fn new_classes_are_in_all_const() {
        for class in [
            ScanClass::DroppedCharacter,
            ScanClass::PacingCollapse,
            ScanClass::StalledThread,
        ] {
            assert!(
                ScanClass::ALL.contains(&class),
                "{class:?} should be in ScanClass::ALL"
            );
        }
    }

    // classify_pacing tests use a minimal
    // throwaway hierarchy + UUID list to exercise
    // the windowing logic without fs setup.

    #[test]
    fn pacing_below_window_size_emits_nothing() {
        // 5 chapters total, window is 5 — no
        // chapter has a trailing window of 5
        // earlier chapters, so nothing fires.
        let counts: Vec<i64> = vec![5000, 5000, 5000, 5000, 5000];
        let ids: Vec<uuid::Uuid> = (0..counts.len())
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        let hierarchy = empty_hierarchy_for_tests();
        let findings = classify_pacing(&counts, &hierarchy, &ids);
        assert!(findings.is_empty());
    }

    #[test]
    fn pacing_uniform_chapters_emit_nothing() {
        let counts: Vec<i64> = vec![5000; 12];
        let ids: Vec<uuid::Uuid> = (0..counts.len())
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        let hierarchy = empty_hierarchy_for_tests();
        let findings = classify_pacing(&counts, &hierarchy, &ids);
        assert!(findings.is_empty());
    }

    #[test]
    fn pacing_long_outlier_flagged() {
        // Steady 5000-word chapters, then one
        // 20000-word chapter.  Trailing 5 mean
        // is 5000, ratio is 4.0× → flag.
        let counts: Vec<i64> = vec![5000, 5000, 5000, 5000, 5000, 20000];
        let ids: Vec<uuid::Uuid> = (0..counts.len())
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        let hierarchy = empty_hierarchy_for_tests();
        let findings = classify_pacing(&counts, &hierarchy, &ids);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, ScanClass::PacingCollapse);
        assert_eq!(findings[0].severity, ScanSeverity::Info);
        assert!(findings[0].detail.contains("notably longer"));
        assert!(findings[0].detail.contains("4.00×"));
    }

    #[test]
    fn pacing_short_outlier_flagged() {
        // 5000-word baseline, then a 1000-word
        // chapter.  Ratio 0.20 → below 0.30
        // threshold → flag.
        let counts: Vec<i64> = vec![5000, 5000, 5000, 5000, 5000, 1000];
        let ids: Vec<uuid::Uuid> = (0..counts.len())
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        let hierarchy = empty_hierarchy_for_tests();
        let findings = classify_pacing(&counts, &hierarchy, &ids);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].detail.contains("notably shorter"));
        assert!(findings[0].detail.contains("0.20×"));
    }

    #[test]
    fn pacing_moderate_variation_passes() {
        // 5000-word baseline, then 8000 (ratio
        // 1.6×) — within the 3.0× / 0.3× bounds
        // → no flag.
        let counts: Vec<i64> = vec![5000, 5000, 5000, 5000, 5000, 8000];
        let ids: Vec<uuid::Uuid> = (0..counts.len())
            .map(|_| uuid::Uuid::new_v4())
            .collect();
        let hierarchy = empty_hierarchy_for_tests();
        let findings = classify_pacing(&counts, &hierarchy, &ids);
        assert!(findings.is_empty());
    }

    /// Helper — build an empty hierarchy via the
    /// existing Default impl.  `classify_pacing`
    /// only reads the chapter title for the
    /// finding's `detail` field; an empty
    /// hierarchy means the test detail falls
    /// back to "chapter N".
    fn empty_hierarchy_for_tests() -> crate::store::hierarchy::Hierarchy {
        crate::store::hierarchy::Hierarchy::default()
    }

    // ── 1.2.16+ Phase A.5 — naming / glossary tests ───────

    #[test]
    fn levenshtein_zero_for_identical() {
        assert_eq!(super::levenshtein("Aerin", "Aerin"), 0);
        assert_eq!(super::levenshtein("", ""), 0);
    }

    #[test]
    fn levenshtein_one_for_single_edit() {
        // single substitution
        assert_eq!(super::levenshtein("cat", "bat"), 1);
        // single insertion
        assert_eq!(super::levenshtein("cat", "cats"), 1);
        // single deletion
        assert_eq!(super::levenshtein("cats", "cat"), 1);
    }

    #[test]
    fn levenshtein_handles_multi_char_distance() {
        // "Stormbringer" vs "Stormbreaker":
        // first 7 chars identical, then `ing` →
        // `eak` (3 substitutions), then `er` ===
        // `er`.  Distance = 3.
        assert_eq!(super::levenshtein("Stormbringer", "Stormbreaker"), 3);
    }

    #[test]
    fn naming_flags_near_miss() {
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose = "In the morning, Aerin Stormbreaker rode west.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].class, ScanClass::NamingInconsistency);
        assert_eq!(findings[0].severity, ScanSeverity::Info);
        assert!(findings[0].detail.contains("Aerin Stormbreaker"));
        assert!(findings[0].detail.contains("Aerin Stormbringer"));
    }

    #[test]
    fn naming_no_finding_when_canonical_present() {
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose = "Aerin Stormbringer rode west.  Later, Aerin Stormbringer drew her sword.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert!(findings.is_empty());
    }

    #[test]
    fn naming_dedupes_repeated_variants() {
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose =
            "Aerin Stormbreaker rode west.  Then Aerin Stormbreaker turned back.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        // Same variant appears twice — only one
        // finding.
        assert_eq!(findings.len(), 1);
    }

    #[test]
    fn naming_skips_single_word_canonicals() {
        let canonical = vec!["Aerin".to_string()];
        // Any near-miss variants are unmanageable
        // for single-word names; we just don't try.
        let prose = "Aerinn rode west.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert!(findings.is_empty());
    }

    #[test]
    fn naming_skips_wholly_different_continuations() {
        let canonical = vec!["Aerin Stormbringer".to_string()];
        // "Aerin and Borin" is not a near-miss; the
        // second word's edit distance from
        // "Stormbringer" is way above the 50%
        // tolerance.
        let prose = "Aerin and Borin rode west.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert!(findings.is_empty());
    }

    #[test]
    fn naming_respects_word_boundary_on_head() {
        // "Aerinet" should NOT match the "Aerin"
        // prefix — word boundary check.
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose = "The aerinet was lowered into the sea Stormbringer waited.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert!(findings.is_empty());
    }

    #[test]
    fn naming_strips_trailing_punctuation() {
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose = "She called: Aerin Stormbreaker, where are you?";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert_eq!(findings.len(), 1);
        // Detail mentions the cleaned variant, not
        // the comma-suffixed one.
        assert!(
            findings[0].detail.contains("Aerin Stormbreaker"),
            "got: {}",
            findings[0].detail
        );
    }

    #[test]
    fn naming_case_insensitive_match_against_canonical() {
        // Lowercased canonical should be matched
        // against equally and skipped (not flagged
        // as a typo).
        let canonical = vec!["Aerin Stormbringer".to_string()];
        let prose = "aerin stormbringer rode west.";
        let findings = super::classify_naming_inconsistencies(&canonical, prose);
        assert!(findings.is_empty());
    }
}
