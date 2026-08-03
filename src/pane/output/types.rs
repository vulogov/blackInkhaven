//! PANE-1 — the Output message envelope (RFC §7.2).
//!
//! One universal outer shape every emitter writes through. `kind` is a string
//! (not a closed enum) so future subsystems and Bund-registered kinds extend the
//! channel without touching this type; the well-known kinds at launch are the
//! consts in [`kinds`]. `metadata` is kind-specific JSON.
//!
//! Timestamps are unix seconds (`i64`), matching the in-tree progress/blob
//! stores rather than the RFC's `TIMESTAMP` columns — simpler to round-trip
//! through DuckDB, same ordering semantics.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// The well-known message kinds at PANE-1 launch (RFC §8.2). New kinds are just
/// new strings; these consts keep the in-tree emitters from drifting.
pub mod kinds {
    pub const BUND_PRINT: &str = "bund_print";
    pub const BUND_LOG: &str = "bund_log";
    pub const TRANSLATION_RESULT: &str = "translation_result";
    pub const TRANSLATION_MEMORY_LISTING: &str = "translation_memory_listing";
    pub const TRANSLATION_CORPUS_PROGRESS: &str = "translation_corpus_progress";
    pub const TRANSLATION_EVAL_RESULT: &str = "translation_eval_result";
    pub const TRANSLATION_EXPORT_RESULT: &str = "translation_export_result";
    pub const TRANSLATION_UNCOVERED_WORD_REPORT: &str = "translation_uncovered_word_report";
    pub const LEXICON_PROPOSAL: &str = "lexicon_proposal";
    pub const VARIETY_RENDERING: &str = "variety_rendering";
    pub const AI_TASK_COMPLETE: &str = "ai_task_complete";
    /// WORLD-4 Branch B — a fact-checker finding (prose vs the simulated world).
    pub const FACT_CHECK_WARNING: &str = "fact_check_warning";
    /// WORLD-4 — a compiler-proposed record awaiting acceptance.
    pub const WORLD_COMPILER_PROPOSAL: &str = "world_compiler_proposal";
    /// INNER_SOCRATES-1 — a Socratic question about the prose (Fast or Slow track).
    pub const SOCRATIC_INQUIRY: &str = "socratic_inquiry";
    /// INNER_EDITOR-1 — an Inner Editor literary/stylistic observation (Praise/Note/Concern).
    pub const INNER_EDITOR_OBSERVATION: &str = "inner_editor_observation";
    /// TIMELINE-2-INTEGRATION — an orphaned timeline event (links to nothing).
    pub const TIMELINE_ORPHAN_WARNING: &str = "timeline_orphan_warning";
    /// TIMELINE-2-INTEGRATION — fuzzy-precision events whose windows collide.
    pub const TIMELINE_FUZZY_OVERLAP_WARNING: &str = "timeline_fuzzy_overlap_warning";
    /// NARR-1 — a narrative-voice drift finding (a chapter metric crossed its
    /// threshold vs the baseline). Always informational.
    pub const PROSE_DRIFT: &str = "prose_drift";
    /// DIALOG-1 — a dialogue finding (zero-attribution / said-bookism density /
    /// talking-head sequence). Always informational.
    pub const DIALOGUE: &str = "dialogue";
    /// WORLD-6 — a utopian/dystopian coherence finding (chain logic or
    /// entailment violation). Always informational.
    pub const UTOPIA: &str = "utopia";
    /// CHAR-1 — a character-arc finding (stall, Planning-Board coverage gap, or
    /// a cached arc-completeness problem). Always informational.
    pub const CHAR: &str = "char";
    /// HAIKU-1 — a single haiku in the book's language. Informational;
    /// `Lifetime::Session(1)` so only the most recent one lives at a time.
    pub const HAIKU: &str = "haiku";
    /// INNER-THEOLOGIAN-1 — a fast-track ethical signal (moral invisibility /
    /// consequence gap / sacred levity). Always informational, never a verdict.
    pub const THEOLOGIAN: &str = "theologian";
    /// MYTH-1 — a deterministic myth finding (archetype vacant/absent, motif
    /// absent from the final act). Always informational.
    pub const MYTH: &str = "myth";
    /// TDOC-1 — a `verify`-marked code block that failed its runner (stale
    /// example). Deterministic; the detail carries the runner's output.
    pub const DOC_VERIFY: &str = "doc_verify";
    /// NF-CITE — an uncited factual claim (a statistic / date / quotation /
    /// attributed finding with no `@key`). Deterministic (fast track); the AI track
    /// adds Facts-book support.
    pub const SOURCING: &str = "sourcing";
    /// ARG-1 — an argument gap: a central claim with no support, or a citation that
    /// supports no claim. Semantic (AI); anchored to the chapter.
    pub const ARGUMENT: &str = "argument";
    /// XREF (1.6.15+) — an unresolved Typst cross-reference (`@ref` to a label
    /// that doesn't exist). Deterministic; promoted from the compile diagnostics
    /// so a dangling `@fig:`/`@eq:`/`@tbl:` is actionable, not buried in stderr.
    pub const XREF: &str = "xref";
    /// SCHOLAR P4 (1.6.18+) — a `Ctrl+V ?` confront finding: the open paragraph
    /// judged against the research corpus (Facts + Sources). Anchored to the
    /// paragraph; against-relations are Warnings, supporting ones Info.
    pub const CONFRONT: &str = "confront";
    /// LOCI (1.6.19+) — a `Ctrl+V c` locus-lint finding: an `@key[locus]` in the
    /// open paragraph whose reference does not match its source's scheme.
    /// Deterministic; anchored to the paragraph; always a Warning.
    pub const LOCUS: &str = "locus";
    /// RIGOR (1.6.20+) — a reasoning-rigor finding (false dichotomy /
    /// question-begging / straw man / overgeneralization / non-sequitur).
    /// Deterministic, zero-AI; anchored to the paragraph; always advisory (Info).
    pub const RIGOR: &str = "rigor";
    /// ORACLE (1.7.9+) — a conlang well-formedness finding: a word in the prose
    /// that segments into a language's inventory but breaks its phonotactics.
    /// Deterministic, zero-AI; anchored to the paragraph; always advisory (Info).
    pub const ORACLE: &str = "oracle";
    /// POEM (1.8.11+) — an Inner Poet fast-track finding: a verse line's metre or
    /// a stanza's rhyme measured against its declared `poem:` form. Deterministic,
    /// zero-AI; anchored to the verse paragraph.
    pub const POEM: &str = "poem";

    /// INNER-STYLIST-1 (CHORUS CH-P7) — a voice-at-scale observation synthesised
    /// from the CHORUS pillars (distinctiveness, drift, POV, tense, register).
    /// Deterministic, book-wide; Praise/Note → Info, Concern → Warning.
    pub const STYLIST: &str = "stylist";

    /// SENTINEL-1 (CT-P4) — a unified continuity break from the always-on
    /// deterministic sweep (co-location, numeric, char-facts, and the
    /// referenced-before-introduced invariant). Contradiction → Warning, else Info.
    pub const CONTINUITY: &str = "continuity";
}

/// Visual / priority class (RFC §7.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Info,
    Warning,
    Contradiction,
    Progress,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Warning => "warning",
            Severity::Contradiction => "contradiction",
            Severity::Progress => "progress",
        }
    }

    pub fn parse(s: &str) -> Self {
        match s {
            "warning" => Severity::Warning,
            "contradiction" => Severity::Contradiction,
            "progress" => Severity::Progress,
            _ => Severity::Info,
        }
    }

    /// The severity icon used by the pane (RFC §8.6).
    pub fn icon(self) -> char {
        match self {
            Severity::Info => '●',
            Severity::Warning => '⚠',
            Severity::Contradiction => '⊗',
            Severity::Progress => '↻',
        }
    }
}

/// How long a message lives (RFC §7.4).
#[derive(Debug, Clone, PartialEq)]
pub enum Lifetime {
    /// Keep only the most recent N of this kind (count-bounded; enforced at
    /// cleanup, not by wall-clock).
    Session(usize),
    /// Expire this many hours after emission.
    Hours(f32),
    /// Keep until accepted / rejected / dismissed.
    UntilActedOn,
    /// Expire when the referenced paragraph changes.
    UntilParagraphEdited(Uuid),
    /// Keep until explicitly dismissed.
    Never,
}

impl Lifetime {
    /// `(lifetime_kind, lifetime_value)` columns for storage.
    pub fn to_columns(&self) -> (&'static str, Option<String>) {
        match self {
            Lifetime::Session(n) => ("session", Some(n.to_string())),
            Lifetime::Hours(h) => ("hours", Some(h.to_string())),
            Lifetime::UntilActedOn => ("until_acted", None),
            Lifetime::UntilParagraphEdited(p) => ("until_paragraph_edit", Some(p.to_string())),
            Lifetime::Never => ("never", None),
        }
    }

    pub fn from_columns(kind: &str, value: Option<&str>) -> Self {
        match kind {
            "session" => Lifetime::Session(value.and_then(|v| v.parse().ok()).unwrap_or(100)),
            "hours" => Lifetime::Hours(value.and_then(|v| v.parse().ok()).unwrap_or(1.0)),
            "until_paragraph_edit" => value
                .and_then(|v| Uuid::parse_str(v).ok())
                .map(Lifetime::UntilParagraphEdited)
                .unwrap_or(Lifetime::UntilActedOn),
            "never" => Lifetime::Never,
            _ => Lifetime::UntilActedOn,
        }
    }

    /// The wall-clock expiry (unix secs) implied by this lifetime, or `None` when
    /// it doesn't expire by time (`Session` is count-bounded; the others persist
    /// until acted on / edited / dismissed).
    pub fn expires_at(&self, created_at: i64) -> Option<i64> {
        match self {
            Lifetime::Hours(h) => Some(created_at + (h * 3600.0) as i64),
            _ => None,
        }
    }
}

/// One of the seven interaction primitives (RFC §7.3). Kind-specific extras
/// (e.g. `Remember`) live outside this enum and are handled by the UI layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionId {
    Primary,
    Dismiss,
    Pin,
    AskAi,
    Promote,
    Snooze,
    Expand,
}

impl ActionId {
    pub fn as_str(self) -> &'static str {
        match self {
            ActionId::Primary => "primary",
            ActionId::Dismiss => "dismiss",
            ActionId::Pin => "pin",
            ActionId::AskAi => "ask_ai",
            ActionId::Promote => "promote",
            ActionId::Snooze => "snooze",
            ActionId::Expand => "expand",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "primary" => ActionId::Primary,
            "dismiss" => ActionId::Dismiss,
            "pin" => ActionId::Pin,
            "ask_ai" => ActionId::AskAi,
            "promote" => ActionId::Promote,
            "snooze" => ActionId::Snooze,
            "expand" => ActionId::Expand,
            _ => return None,
        })
    }
}

/// Current unix-seconds timestamp.
pub fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A structured Output notification (RFC §7.2).
#[derive(Debug, Clone)]
pub struct Message {
    pub id: Uuid,
    pub kind: String,
    pub timestamp: i64,
    pub metadata: serde_json::Value,
    pub actions: Vec<ActionId>,
    pub severity: Severity,
    pub lifetime: Lifetime,
    pub group_key: Option<String>,
    pub pinned: bool,
    pub snoozed_until: Option<i64>,
    pub dismissed: bool,
    pub dismissed_at: Option<i64>,
    pub expires_at: Option<i64>,
    pub source_paragraph_id: Option<Uuid>,
    pub source_language_id: Option<String>,
    pub trace_id: Option<Uuid>,
}

impl Message {
    /// Start a new message — a UUIDv7 id, the current timestamp, the
    /// lifetime-implied `expires_at`, and the default action set for its severity
    /// (every kind gets Dismiss / Expand; everything but Progress also gets
    /// AskAi). Refine with the `with_*` builders.
    pub fn new(
        kind: impl Into<String>,
        severity: Severity,
        lifetime: Lifetime,
        metadata: serde_json::Value,
    ) -> Self {
        let timestamp = now_secs();
        let expires_at = lifetime.expires_at(timestamp);
        let mut actions = vec![ActionId::Dismiss, ActionId::Expand, ActionId::Pin];
        if severity != Severity::Progress {
            actions.push(ActionId::AskAi);
        }
        Message {
            id: Uuid::now_v7(),
            kind: kind.into(),
            timestamp,
            metadata,
            actions,
            severity,
            lifetime,
            group_key: None,
            pinned: false,
            snoozed_until: None,
            dismissed: false,
            dismissed_at: None,
            expires_at,
            source_paragraph_id: None,
            source_language_id: None,
            trace_id: None,
        }
    }

    pub fn with_actions(mut self, actions: Vec<ActionId>) -> Self {
        self.actions = actions;
        self
    }

    pub fn with_group_key(mut self, key: impl Into<String>) -> Self {
        self.group_key = Some(key.into());
        self
    }

    pub fn with_source_paragraph(mut self, id: Uuid) -> Self {
        self.source_paragraph_id = Some(id);
        self
    }

    pub fn with_source_language(mut self, lang: impl Into<String>) -> Self {
        self.source_language_id = Some(lang.into());
        self
    }

    pub fn with_trace(mut self, id: Uuid) -> Self {
        self.trace_id = Some(id);
        self
    }

    /// Whether this message is currently visible: not dismissed, not snoozed
    /// past `now`, and not time-expired.
    pub fn is_active(&self, now: i64) -> bool {
        !self.dismissed
            && self.snoozed_until.map(|t| t <= now).unwrap_or(true)
            && self.expires_at.map(|t| t > now).unwrap_or(true)
    }
}
