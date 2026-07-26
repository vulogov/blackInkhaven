//! IE-P2 — the engagement engine: the synchronous orchestration one Editor pass
//! runs, shared by the CLI (`inkhaven inner-editor engage`) and a TUI background
//! thread (the manual chord / paragraph-pause, IE-P3/P4). Self-contained (takes
//! a project path + owned prose) so it can run off-thread without borrowing the
//! app.
//!
//! Flow: load config → resolve tuning → build the localized prompt → one LLM
//! call (informative cap, transient-retry, mirrors `socratic_llm_call`) → parse
//! → consult the intent ledger → severity-sort + cap → persist. The pure pieces
//! (prompt build, parse, consult) are unit-tested separately; this wires them.

use std::path::PathBuf;

use anyhow::{anyhow, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::intent::FindingContext;
use crate::inner_socrates::storage::InnerSocratesStore;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::NodeKind;
use crate::store::Store;
use crate::world::proposals::now_secs;

use super::intent_consult::{consult, intent_summary};
use super::parse::parse_findings;
use super::prompt::{build_user_prompt, language_name, resolve_tuning, system_prompt, tuning_block};
use super::storage::InnerEditorStore;
use super::types::EditorFinding;

/// Everything one engagement needs. Owned, so it crosses a thread boundary.
pub struct EngageInput {
    pub project: PathBuf,
    pub paragraph_id: Option<Uuid>,
    pub chapter_id: Option<String>,
    /// The paragraph being observed.
    pub prose: String,
    /// Preceding paragraphs (oldest → newest), for interpretation context.
    pub preceding: Vec<String>,
    /// ISO-639-1 code; empty → resolve from the project language.
    pub language: String,
    /// Snapshot active at emission (for findings-history; `None` if unknown).
    pub snapshot_id: Option<Uuid>,
    /// A system-prompt override resolved through the `Prompts` chain by the
    /// caller; `None` → the bundled localized const.
    pub system_override: Option<String>,
    /// Skip the informative cap warning (manual `--force`).
    pub force: bool,
}

/// The result of an engagement.
pub struct EngageOutcome {
    /// Findings to surface (already consulted, severity-sorted, capped).
    pub findings: Vec<EditorFinding>,
    /// How many findings the intent ledger suppressed.
    pub suppressed: usize,
    pub calls_used: i64,
    pub daily_cap: i64,
    /// A non-fatal note (disabled / no categories / over the informative cap).
    pub note: Option<String>,
}

impl EngageOutcome {
    fn skipped(note: &str) -> Self {
        Self { findings: Vec::new(), suppressed: 0, calls_used: 0, daily_cap: 0, note: Some(note.into()) }
    }
}

/// Run one Editor engagement. Errors only on hard failures (config / no provider
/// / LLM error after retries); a disabled feature or empty category set returns
/// a skipped outcome with a note.
pub fn engage(input: EngageInput) -> Result<EngageOutcome> {
    let cfg = Config::load_layered(&ProjectLayout::new(&input.project).config_path())
        .map_err(|e| anyhow!("config: {e}"))?;
    if !cfg.inner_editor.enabled {
        return Ok(EngageOutcome::skipped(
            "Inner Editor is disabled (inner_editor.enabled: false)",
        ));
    }
    let tuning = resolve_tuning(&cfg.inner_editor.persona);
    if tuning.active_categories.is_empty() {
        return Ok(EngageOutcome::skipped("no Inner Editor categories are enabled"));
    }
    if input.prose.trim().is_empty() {
        return Ok(EngageOutcome::skipped("paragraph is empty"));
    }

    crate::dayclock::set_boundary(cfg.goals.day_boundary);
    let day = crate::dayclock::today_key();
    let lang_code = if input.language.trim().is_empty() {
        crate::ai::prompts::iso_from_long(&cfg.language).to_string()
    } else {
        input.language.trim().to_string()
    };

    let ie_store = InnerEditorStore::open_for_project(&input.project)
        .map_err(|e| anyhow!("inner-editor store: {e}"))?;
    // The shared intent ledger lives in inner_socrates.db; best-effort read.
    let rows = InnerSocratesStore::open_for_project(&input.project)
        .ok()
        .and_then(|s| s.list_intent_rows_raw().ok())
        .unwrap_or_default();

    // Build the prompt.
    let tblock = tuning_block(&tuning, cfg.genre.as_deref());
    let user = build_user_prompt(
        &tblock,
        &intent_summary(&rows),
        &input.preceding,
        &input.prose,
        language_name(&lang_code),
    );
    // INNER-GROUND-1 — open with the shared reader grounding (declared cast,
    // symbol library, open world tensions) as a labelled preamble, so a craft
    // note respects the author's declared world rather than reading blind.
    // Self-disables when nothing is declared.
    let user = match crate::inner_grounding::build_grounding(&input.project) {
        Some(g) => format!("{g}\n\n{user}"),
        None => user,
    };
    let system = input
        .system_override
        .clone()
        .unwrap_or_else(|| system_prompt(&lang_code).to_string());

    // The LLM call — informative cap, transient-retry (mirrors socratic_llm_call).
    let cap = cfg.inner_editor.llm.editor_engagement.max_calls_per_day;
    let used = ie_store
        .llm_calls_today(&day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET)
        .unwrap_or(0);
    let mut note = None;
    if !input.force && cap > 0 && used >= cap {
        note = Some(format!(
            "past today's Inner Editor budget ({used}/{cap} calls) — continuing \
             (the cap is informative; see `inkhaven cost`)."
        ));
    }
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("no LLM provider for Inner Editor: {e}"))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| anyhow!("resolving provider: {e}"))?;

    let max_attempts = cfg.inner_editor.llm.backoff_max_retries.max(1) as u32;
    let mut last_err = String::new();
    let mut raw = None;
    for attempt in 0..max_attempts {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.clone()),
            user.clone(),
        ) {
            Ok(r) => {
                let _ = ie_store.record_llm_call(&day, InnerEditorStore::ENGAGEMENT_SUB_BUDGET);
                raw = Some(r);
                break;
            }
            Err(e) => {
                last_err = e.to_string();
                if attempt + 1 < max_attempts
                    && crate::world::fact_check_slow::is_transient(&last_err)
                {
                    std::thread::sleep(crate::world::fact_check_slow::backoff_delay(attempt));
                    continue;
                }
                break;
            }
        }
    }
    let Some(raw) = raw else {
        return Err(anyhow!("LLM error: {last_err}"));
    };

    // Parse → consult → severity-sort → cap → persist.
    let parsed = parse_findings(&raw, &tuning.active_categories);
    let ctx = FindingContext {
        paragraph_id: input.paragraph_id.map(|p| p.to_string()),
        chapter_id: input.chapter_id.clone(),
        ..Default::default()
    };
    let (mut kept, suppressed) = consult(parsed, &rows, &ctx);
    // Concern first, then Note, then Praise — so the cap keeps the most salient.
    kept.sort_by(|a, b| b.severity.rank().cmp(&a.severity.rank()));
    let max_findings = cfg.inner_editor.engagement.max_findings_per_paragraph;
    if max_findings > 0 && kept.len() > max_findings {
        kept.truncate(max_findings);
    }

    if let Some(pid) = input.paragraph_id {
        // Atomic clear + inserts + engagement stamp — no partial state on failure,
        // and the error is logged rather than swallowed (1.8.23 hardening) so a
        // persist failure isn't silent (findings shown but never saved).
        if let Err(e) = ie_store.replace_paragraph_findings(
            pid,
            &kept,
            input.chapter_id.as_deref(),
            Some(&lang_code),
            input.snapshot_id,
            now_secs(),
        ) {
            tracing::warn!(
                target: "inkhaven::inner_editor",
                "persisting engage findings for {pid} failed: {e}"
            );
        }
    }

    Ok(EngageOutcome { findings: kept, suppressed: suppressed.len(), calls_used: used + 1, daily_cap: cap, note })
}

/// The current paragraph's prose + its `n` preceding paragraphs (document
/// order) + its enclosing chapter id — the store-backed context the CLI path
/// gathers. The TUI path builds its own (the open buffer may be unsaved).
pub struct GatheredContext {
    pub prose: String,
    pub preceding: Vec<String>,
    pub chapter_id: Option<String>,
}

pub fn gather_context(
    store: &Store,
    hierarchy: &Hierarchy,
    paragraph_id: Uuid,
    n_preceding: usize,
) -> Option<GatheredContext> {
    let paras: Vec<Uuid> = hierarchy
        .flatten()
        .into_iter()
        .filter(|(n, _)| n.kind == NodeKind::Paragraph)
        .map(|(n, _)| n.id)
        .collect();
    let idx = paras.iter().position(|&id| id == paragraph_id)?;
    let lo = idx.saturating_sub(n_preceding);
    let preceding: Vec<String> = paras[lo..idx]
        .iter()
        .map(|&id| body(store, id))
        .filter(|s| !s.trim().is_empty())
        .collect();
    let chapter_id = hierarchy.get(paragraph_id).and_then(|node| {
        hierarchy
            .ancestors(node)
            .into_iter()
            .find(|a| a.kind == NodeKind::Chapter)
            .map(|c| c.id.to_string())
    });
    Some(GatheredContext { prose: body(store, paragraph_id), preceding, chapter_id })
}

fn body(store: &Store, id: Uuid) -> String {
    match store.get_content(id) {
        Ok(Some(b)) => String::from_utf8_lossy(&b).into_owned(),
        _ => String::new(),
    }
}
