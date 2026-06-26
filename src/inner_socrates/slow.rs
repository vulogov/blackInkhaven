//! The Slow track — the LLM pass that surfaces the semantically-deep Socratic
//! findings the deterministic patterns can't: hidden assumptions, internal
//! tensions, framings, significance, structural echoes. Five prose categories
//! (the three timeline categories arrive in a later phase). Like WORLD-4's slow
//! track it is opt-in and cost-capped; the pure pieces here — the system prompt,
//! the prompt builder, the response parser — are testable without a provider, and
//! it reuses WORLD-4's `estimate_tokens` / `slow_preflight` / `backoff_delay` /
//! `is_transient` for the cost + retry machinery.
//!
//! The non-prescriptive spine holds: the prompt demands **questions**, never
//! corrections, and the parser keeps only the five prose categories.

use super::intent::{ConsultationResult, FindingContext, IntentLedger};
use super::types::{Category, Persona, Severity, SocraticFinding};

/// The Socratic system prompt — a careful reader who asks, never prescribes.
/// **Genre-aware** (1.4.6 AUDIENCE-1): the fiction framing is replaced by a
/// neutral "prose" framing plus a genre-specific context line, so nonfiction /
/// technical / documentation authors get a calibrated interrogator. Call
/// `slow_system(cfg.genre.as_deref())` at the call site.
pub fn slow_system(genre: Option<&str>) -> String {
    // Default (no genre, or an unknown one) preserves the pre-AUDIENCE-1 framing:
    // a fiction reader. AUDIENCE-1 is purely additive — only an explicitly
    // declared genre changes the framing.
    let context_line = match slow_genre_context(genre) {
        Some(ctx) => format!("You are reading a {ctx}."),
        None => "You are reading a fiction manuscript.".to_string(),
    };
    format!(
        "You are a Socratic reader — a careful interlocutor in the classical sense. {context_line} \
         Your task is to surface QUESTIONS about a paragraph of prose: \
         the assumptions it treats as given, the tensions inside it, the stance its \
         framing presupposes, what the passage does for the work, and the echoes it \
         carries of earlier passages. You never correct, never suggest changes, never \
         rewrite, never praise. Every finding is a question that helps the author see \
         what they have chosen. Be conservative: only raise a question when there is \
         something genuinely worth examining; if the paragraph is plain and self-aware, \
         return nothing. Respect the author's declared intentions (listed below) and do \
         not re-raise what the fast pass already found. Respond ONLY with a JSON array; \
         each item is {{\"category\": one of \
         assumption_surfacing|tension_detection|framing_interrogation|significance_probing|\
         implicit_comparison, \
         \"severity\": notice|inquiry|probe, \"question\": a one-sentence question in your \
         voice in the paragraph's language, \"question_en\": the same question in English}}. \
         Return [] if nothing rises."
    )
}

/// A short genre context string for the Slow-track system prompt — the clause
/// that follows "You are reading a …". Returns `None` for unknown / unset genre
/// (the prompt degrades to neutral "prose manuscript" framing). The key set
/// mirrors Inner Editor's `genre_fragment()`; the text differs (this primes the
/// *interrogator*, that primes the *editor*).
pub fn slow_genre_context(genre: Option<&str>) -> Option<&'static str> {
    let g = genre?.trim().to_ascii_lowercase().replace([' ', '-'], "_");
    Some(match g.as_str() {
        // ── Fiction genres — "fiction manuscript" framing, now with specificity ──
        "literary" | "literary_realism" | "literary_fiction" | "realism" =>
            "literary fiction manuscript — attend to psychological depth and the texture of the ordinary",
        "fantasy" | "high_fantasy" | "epic_fantasy" =>
            "fantasy manuscript — invented registers and world-rules are conventional",
        "scifi" | "sci_fi" | "science_fiction" =>
            "science fiction manuscript — technical and speculative registers are expected",
        "mystery" | "thriller" | "crime" =>
            "mystery or thriller manuscript — pace and concealment are structural concerns",
        "memoir" | "creative_nonfiction" | "essay" =>
            "memoir or essay — the first-person voice and reflective stance are the craft",
        "historical" | "historical_fiction" =>
            "historical fiction manuscript — period register and anachronism are live questions",
        "romance" => "romance manuscript — emotional interiority and dialogue carry the genre",
        "horror" => "horror manuscript — dread lives in rhythm and restraint",
        "ya" | "young_adult" => "young adult manuscript — immediacy of voice is central",
        "comedy" | "humor" | "humour" | "satire" =>
            "comedy or satire — timing and sentence rhythm are craft",
        // ── Nonfiction / technical genres — the fiction framing is replaced entirely ──
        "nonfiction" | "general_nonfiction" =>
            "nonfiction book — arguments must be supported, assumptions surfaced, scope stated",
        "technical" | "technical_writing" | "it" | "software" | "engineering" =>
            "technical document — procedures must be complete, claims testable, prerequisites explicit",
        "documentation" | "docs" | "api_docs" | "reference" =>
            "documentation — each instruction must be followable; success criteria must be clear",
        "academic" | "scholarly" | "research" =>
            "academic or scholarly text — claims require support, scope must be stated, logic must hold",
        "science" | "popular_science" | "science_writing" =>
            "science writing — evidence must support each claim; analogies must not overstep",
        "business" | "management" =>
            "business or management book — practical claims must be testable; assumptions must be named",
        _ => return None,
    })
}

/// A compact description of the active persona for the prompt — its character and
/// the categories it leans into.
pub fn persona_summary(persona: &Persona) -> String {
    let mut s = format!("READER PERSONA: {}\n", persona.name);
    if !persona.voice_summary.is_empty() {
        s.push_str(&format!("- {}\n", persona.voice_summary));
    }
    if !persona.voice_notes.is_empty() {
        s.push_str(&format!("{}\n", persona.voice_notes.trim()));
    }
    // The categories this persona weights above default.
    let leaned: Vec<&str> = Category::SLOW
        .iter()
        .filter(|c| persona.emphasis_for(**c) > 1.0)
        .map(|c| c.id())
        .collect();
    if !leaned.is_empty() {
        s.push_str(&format!("- Pays particular attention to: {}.\n", leaned.join(", ")));
    }
    s
}

/// A compact summary of the intent ledger entries (the declared choices the
/// reader should respect), for the prompt.
pub fn intent_summary(ledger: &IntentLedger) -> String {
    if ledger.entries.is_empty() {
        return "None.".to_string();
    }
    ledger
        .entries
        .iter()
        .map(|e| format!("- {} ({}): {}", e.kind.id(), e.description, scope_brief(&e.scope)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn scope_brief(scope: &super::intent::IntentScope) -> String {
    use super::intent::IntentScope as S;
    match scope {
        S::Project => "project-wide".into(),
        S::Chapter(c) => format!("chapter {c}"),
        S::ParagraphRange { from, to } => format!("¶ {from}–{to}"),
        S::Character(c) => format!("character {c}"),
        S::Scene(s) => format!("scene {s}"),
        S::TimelineRange { from, to } => format!("time {from}–{to}"),
    }
}

/// Build the Slow-track user prompt for one paragraph: persona + declared intents
/// + the fast findings to skip (the seam) + the paragraph itself.
pub fn build_slow_prompt(
    persona: &Persona,
    paragraph: &str,
    intent_summary: &str,
    fast_findings: &[SocraticFinding],
    lang: super::lang::Lang,
) -> String {
    let already = if fast_findings.is_empty() {
        "(none)".to_string()
    } else {
        fast_findings.iter().map(|f| format!("- {}", f.question)).collect::<Vec<_>>().join("\n")
    };
    let language = super::lang::language_name(lang);
    format!(
        "{persona}\n\nDECLARED INTENTIONS (respect these — do not question what they cover):\n\
         {intent_summary}\n\n\
         ALREADY ASKED by the fast pass (do NOT repeat):\n{already}\n\n\
         The paragraph is in {language}; write each `question` in {language} and its `question_en` \
         in English.\n\n\
         PARAGRAPH:\n{paragraph}\n\n\
         Return the JSON array of Socratic questions.",
        persona = persona_summary(persona),
    )
}

/// Parse the LLM's JSON response into prose findings (the five prose categories).
pub fn parse_slow_findings(raw: &str, persona_id: &str) -> Vec<SocraticFinding> {
    parse_findings(raw, persona_id, &PROSE_CATEGORIES)
}

/// Parse the LLM's JSON response into timeline findings (the three timeline
/// categories).
pub fn parse_timeline_findings(raw: &str, persona_id: &str) -> Vec<SocraticFinding> {
    parse_findings(raw, persona_id, &TIMELINE_CATEGORIES)
}

/// Parse the LLM's JSON response into findings, attributed to `persona_id`,
/// keeping only the `allowed` categories. Tolerant of fences / surrounding prose.
pub fn parse_findings(raw: &str, persona_id: &str, allowed: &[Category]) -> Vec<SocraticFinding> {
    let Some(json) = extract_json_array(raw) else {
        return Vec::new();
    };
    let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|v| {
            let category = Category::from_id(v.get("category").and_then(|c| c.as_str())?)?;
            if !allowed.contains(&category) {
                return None;
            }
            let question = v.get("question").and_then(|q| q.as_str())?.trim().to_string();
            if question.is_empty() {
                return None;
            }
            // English fallback: the model's `question_en` if present, else the
            // question itself (the AI-bridge tolerates either).
            let question_en = v
                .get("question_en")
                .and_then(|q| q.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| question.clone());
            let severity = match v.get("severity").and_then(|s| s.as_str()) {
                Some("probe") => Severity::Probe,
                Some("notice") => Severity::Notice,
                _ => Severity::Inquiry,
            };
            Some(SocraticFinding {
                category,
                severity,
                persona_id: persona_id.to_string(),
                question_en,
                question,
                suppressed_by: None,
            })
        })
        .collect()
}

/// The five Slow-track prose categories valid as LLM output (timeline categories
/// are handled by a different pass).
const PROSE_CATEGORIES: [Category; 5] = [
    Category::AssumptionSurfacing,
    Category::TensionDetection,
    Category::FramingInterrogation,
    Category::SignificanceProbing,
    Category::ImplicitComparison,
];

/// The three timeline-aware Slow categories.
const TIMELINE_CATEGORIES: [Category; 3] = [
    Category::DramatizationGap,
    Category::ImplicationTracing,
    Category::TemporalDensity,
];

/// The system prompt for the timeline pass — the prose against the timeline.
pub const TIMELINE_SYSTEM: &str = "You are a Socratic reader examining a fiction manuscript against \
its timeline of events. Your task is to surface QUESTIONS — never corrections — about the relationship \
between what the timeline declares and what the prose actually dramatizes: events the timeline names but \
no paragraph depicts (a dramatization gap), events whose consequences should ripple forward in the prose \
but don't visibly (an implication left untraced), and stretches where many events cluster in world-time \
but the prose passes over them lightly (a temporal density the rhythm may not honour). Respect the \
author's declared intentions (some gaps and ambiguities are deliberate). Be conservative — backstory \
need not be dramatized, and absence is often a choice. Respond ONLY with a JSON array; each item is \
{\"category\": one of dramatization_gap|implication_tracing|temporal_density, \"severity\": \
notice|inquiry|probe, \"question\": a one-sentence question in your voice, \"question_en\": the same in \
English}. Return [] if the prose and timeline sit well together.";

/// Build the timeline-pass prompt: persona + declared intents + a summary of the
/// timeline (events, depicted-or-not) + the densest cluster.
pub fn build_timeline_prompt(
    persona: &Persona,
    timeline_summary: &str,
    densest_cluster: usize,
    intent_summary: &str,
) -> String {
    format!(
        "{persona}\n\nDECLARED INTENTIONS (respect these — some temporal gaps are deliberate):\n\
         {intent_summary}\n\n\
         TIMELINE (each event, its world-time, and whether the prose depicts it):\n{timeline_summary}\n\
         The densest stretch holds {densest_cluster} events close together in world-time.\n\n\
         Return the JSON array of Socratic questions about the prose's relationship to this timeline.",
        persona = persona_summary(persona),
    )
}

/// Post-process raw LLM findings: drop categories the persona mutes, and suppress
/// those a declared intent covers (the lazy consultation, same as the Fast track).
pub fn apply_persona_and_ledger(
    findings: Vec<SocraticFinding>,
    persona: &Persona,
    ledger: &IntentLedger,
    ctx: &FindingContext,
) -> Vec<SocraticFinding> {
    findings
        .into_iter()
        .filter(|f| !persona.mutes(f.category))
        .filter(|f| matches!(ledger.consult(f.category, ctx), ConsultationResult::Emit))
        .collect()
}

/// Pull the first JSON array out of a possibly-fenced, possibly-chatty reply.
fn extract_json_array(raw: &str) -> Option<String> {
    let start = raw.find('[')?;
    let end = raw.rfind(']')?;
    if end <= start {
        return None;
    }
    Some(raw[start..=end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn socrates() -> Persona {
        Persona::default_inner_socrates()
    }

    #[test]
    fn slow_system_is_genre_aware() {
        // No genre → the pre-AUDIENCE-1 fiction framing (purely additive: an
        // author who never sets a genre sees exactly the old behaviour).
        let default = slow_system(None);
        assert!(default.contains("fiction manuscript"), "{default}");
        // Nonfiction genres swap the framing entirely (no "fiction").
        let tech = slow_system(Some("technical"));
        assert!(tech.contains("technical document"), "{tech}");
        assert!(!tech.to_lowercase().contains("fiction"));
        let docs = slow_system(Some("documentation"));
        assert!(docs.contains("instruction must be followable"), "{docs}");
        // Fiction genres get a (more specific) fiction framing.
        let fant = slow_system(Some("fantasy"));
        assert!(fant.contains("fantasy manuscript"), "{fant}");
        // An unknown genre degrades to the fiction default, not a nonfiction one.
        assert!(slow_system(Some("interpretive-dance")).contains("fiction manuscript"));
        // The JSON contract (5 prose categories) survives in every framing.
        for s in [&default, &tech, &fant] {
            assert!(s.contains("assumption_surfacing|tension_detection"), "category list missing");
            assert!(s.contains("question_en"));
        }
    }

    #[test]
    fn slow_genre_context_aliases_and_unknown() {
        // Aliases and case/separator normalisation resolve.
        assert!(slow_genre_context(Some("IT")).unwrap().contains("technical document"));
        assert!(slow_genre_context(Some("popular science")).unwrap().contains("science writing"));
        assert!(slow_genre_context(Some("Literary-Fiction")).unwrap().contains("literary fiction"));
        // Every known genre yields a distinct, non-empty clause.
        let keys = [
            "literary", "fantasy", "scifi", "mystery", "memoir", "historical", "romance",
            "horror", "ya", "comedy", "nonfiction", "technical", "documentation", "academic",
            "science", "business",
        ];
        let mut seen = std::collections::BTreeSet::new();
        for k in keys {
            let c = slow_genre_context(Some(k)).unwrap_or_else(|| panic!("no context for {k}"));
            assert!(!c.is_empty());
            assert!(seen.insert(c), "duplicate context for {k}");
        }
        assert_eq!(slow_genre_context(None), None);
        assert_eq!(slow_genre_context(Some("nonsense")), None);
    }

    #[test]
    fn prompt_carries_persona_intent_and_seam() {
        let fast = vec![SocraticFinding {
            category: Category::ModalClaims,
            severity: Severity::Inquiry,
            persona_id: "inner-socrates".into(),
            question: "What alternatives did you leave out?".into(),
            question_en: "What alternatives did you leave out?".into(),
            suppressed_by: None,
        }];
        let p = build_slow_prompt(
            &socrates(),
            "The regent declared war.",
            "None.",
            &fast,
            super::super::lang::Lang::En,
        );
        assert!(p.contains("Inner Socrates"));
        assert!(p.contains("The regent declared war."));
        assert!(p.contains("do NOT repeat"));
        assert!(p.contains("What alternatives did you leave out?"));
        assert!(p.contains("DECLARED INTENTIONS"));
        assert!(p.contains("English"));
    }

    #[test]
    fn parses_fenced_prose_findings_only() {
        let raw = "Sure:\n```json\n[\
            {\"category\":\"assumption_surfacing\",\"severity\":\"inquiry\",\"question\":\"What does this assume?\"},\
            {\"category\":\"framing_interrogation\",\"severity\":\"probe\",\"question\":\"Whose stance is this?\"},\
            {\"category\":\"dramatization_gap\",\"severity\":\"inquiry\",\"question\":\"timeline cat — should be dropped\"}\
            ]\n```";
        let f = parse_slow_findings(raw, "inner-socrates");
        assert_eq!(f.len(), 2, "the timeline category is dropped: {f:?}");
        assert_eq!(f[0].category, Category::AssumptionSurfacing);
        assert_eq!(f[0].severity, Severity::Inquiry);
        assert_eq!(f[1].severity, Severity::Probe);
        assert!(f.iter().all(|x| x.persona_id == "inner-socrates"));
    }

    #[test]
    fn timeline_parser_keeps_only_timeline_categories() {
        let raw = "[\
            {\"category\":\"dramatization_gap\",\"severity\":\"inquiry\",\"question\":\"Why is the pact never shown?\"},\
            {\"category\":\"assumption_surfacing\",\"severity\":\"inquiry\",\"question\":\"prose cat — dropped here\"}\
            ]";
        let f = parse_timeline_findings(raw, "inner-socrates");
        assert_eq!(f.len(), 1, "only the timeline category survives: {f:?}");
        assert_eq!(f[0].category, Category::DramatizationGap);
    }

    #[test]
    fn timeline_prompt_carries_summary_and_density() {
        let p = build_timeline_prompt(&socrates(), "- t=10 Coronation: depicted\n", 3, "None.");
        assert!(p.contains("Coronation"));
        assert!(p.contains("3 events"));
        assert!(p.contains("DECLARED INTENTIONS"));
    }

    #[test]
    fn parses_empty_and_garbage() {
        assert!(parse_slow_findings("[]", "inner-socrates").is_empty());
        assert!(parse_slow_findings("no json", "inner-socrates").is_empty());
        assert!(parse_slow_findings("", "inner-socrates").is_empty());
    }

    #[test]
    fn intent_summary_none_when_empty() {
        assert_eq!(intent_summary(&IntentLedger::default()), "None.");
    }

    #[test]
    fn ledger_suppresses_slow_findings() {
        use super::super::intent::{IntentEntry, IntentKind, IntentScope, ScopeLevel};
        let findings = vec![SocraticFinding {
            category: Category::AssumptionSurfacing,
            severity: Severity::Inquiry,
            persona_id: "inner-socrates".into(),
            question: "What does this assume?".into(),
            question_en: "What does this assume?".into(),
            suppressed_by: None,
        }];
        let ledger = IntentLedger {
            entries: vec![IntentEntry {
                id: "e1".into(),
                kind: IntentKind::DeliberateAmbiguity,
                description: "intended".into(),
                scope: IntentScope::Chapter("ch07".into()),
                coverage: vec![Category::AssumptionSurfacing],
                scope_level: ScopeLevel::Project,
            }],
        };
        let ctx = FindingContext { chapter_id: Some("ch07".into()), ..Default::default() };
        let kept = apply_persona_and_ledger(findings, &socrates(), &ledger, &ctx);
        assert!(kept.is_empty(), "declared intent suppresses the slow finding");
    }
}
