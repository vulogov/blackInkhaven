//! MYTH-1 (M-P9) — the explicit LLM checks. Three passes over the **declared**
//! inventory: symbol consistency (is a symbol ever used against its declared
//! meaning / valence?), motif completeness (does a declared motif's arc land?),
//! and archetype role fulfilment (does the mapped character perform the declared
//! function?). Each pass gives the model only declared metadata plus concrete
//! prose excerpts — never the whole book — and parses a small JSON array.
//!
//! These are CLI / chord explicit (`inkhaven myth check`); the review pass stays
//! deterministic and zero-AI. Caps inform, never block. Findings are advisory.
//! Multilingual: the model is asked to answer in the project language.

use anyhow::{Result, anyhow};

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::prose::{ProseLanguage, resolve_prose_language};
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::pipeline::{excerpts_containing, motif_occurrence_excerpts};
use super::store::MythStore;
use super::{ArchetypeRole, FindingType, MythFinding};

/// Max prose excerpts handed to a single LLM check call (keeps the prompt bounded).
const MAX_EXCERPTS: usize = 8;

/// The blocking LLM call for a myth check. Mirrors `inner_theologian::llm` /
/// `world::utopia::llm`: resolve the provider, call `collect_blocking`, retry on
/// transient errors.
pub(crate) fn myth_llm_call(cfg: &Config, system: &str, user: &str) -> Result<String> {
    let ai = crate::ai::AiClient::from_config(&cfg.llm)
        .map_err(|e| anyhow!("no LLM provider for myth checks: {e}"))?;
    let (model, _env) = ai
        .resolve_provider(&cfg.llm, None)
        .map_err(|e| anyhow!("resolving provider: {e}"))?;
    let max_attempts = 3u32;
    let mut last_err = String::new();
    for attempt in 0..max_attempts {
        match crate::ai::stream::collect_blocking(
            ai.client.clone(),
            model.to_string(),
            Some(system.to_string()),
            user.to_string(),
        ) {
            Ok(r) => return Ok(r),
            Err(e) => {
                last_err = e.to_string();
                if attempt + 1 < max_attempts && crate::world::fact_check_slow::is_transient(&last_err) {
                    std::thread::sleep(crate::world::fact_check_slow::backoff_delay(attempt));
                    continue;
                }
                break;
            }
        }
    }
    Err(anyhow!("myth LLM error: {last_err}"))
}

/// First top-level JSON array in an LLM response (models wrap JSON in prose/fences).
fn extract_json_array(raw: &str) -> &str {
    match (raw.find('['), raw.rfind(']')) {
        (Some(a), Some(b)) if b > a => &raw[a..=b],
        _ => raw.trim(),
    }
}

/// The language name for the in-language directive (`Other` → English).
fn language_name(lang: &ProseLanguage) -> &'static str {
    match lang {
        ProseLanguage::En => "English",
        ProseLanguage::Ru => "Russian",
        ProseLanguage::De => "German",
        ProseLanguage::Fr => "French",
        ProseLanguage::Es => "Spanish",
        ProseLanguage::Other(_) => "English",
    }
}

const SYSTEM_PROMPT: &str =
    "You are a literary symbol- and pattern-analyst. You judge only whether the author's prose is \
     CONSISTENT with what the author themselves declared — you never invent symbols, never impose \
     interpretation, never moralise, and never rewrite. Report only concrete, evidence-backed \
     discrepancies. When the prose is consistent with the declaration, return an empty JSON array. \
     Reply with ONLY a JSON array, no prose around it.";

fn format_excerpts(ex: &[(u32, String)]) -> String {
    ex.iter()
        .map(|(ord, s)| format!("- (ch.{ord}) {s}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One `{evidence, explanation}` element from a check response.
fn parse_findings(
    raw: &str,
    finding_type: FindingType,
    entry_para_id: &str,
    prefix: &str,
) -> Vec<MythFinding> {
    let arr: serde_json::Value = match serde_json::from_str(extract_json_array(raw)) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let Some(items) = arr.as_array() else { return Vec::new() };
    items
        .iter()
        .filter_map(|it| {
            let explanation = it
                .get("explanation")
                .and_then(|v| v.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())?;
            let evidence = it
                .get("evidence")
                .and_then(|v| v.as_str())
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty());
            Some(MythFinding {
                finding_type,
                description: format!("{prefix}: {explanation}"),
                evidence,
                entry_para_id: Some(entry_para_id.to_string()),
                chapter_ord: None,
                suppressed: false,
            })
        })
        .collect()
}

/// Run all three LLM checks for a book, persisting + returning the findings. The
/// caller decides which subset to run via `which`. Each pass replaces its own
/// finding type's stored rows. Never touches the deterministic types.
pub(crate) fn run_llm_checks(
    store: &MythStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    consistency_min_chapters: u32,
    motif_min_occurrences: u32,
) -> Result<Vec<MythFinding>> {
    let (lang, _note) = resolve_prose_language(None, &cfg.language);
    let lang_dir = format!("Write every explanation in {}.", language_name(&lang));
    let now = chrono::Utc::now().to_rfc3339();
    let mut all = Vec::new();

    // ── symbol consistency ───────────────────────────────────────────────────
    store.clear_findings_of_type(&book.slug, FindingType::SymbolInconsistency)?;
    let mut sid = 0u32;
    for s in store.symbols(&book.slug)? {
        let chapters_present = store
            .density_for_symbol(&book.slug, &s.para_id)?
            .iter()
            .filter(|(_, c)| *c > 0)
            .count() as u32;
        if chapters_present < consistency_min_chapters {
            continue;
        }
        let excerpts = excerpts_containing(layout, h, book, &lang, &s.vocabulary, MAX_EXCERPTS);
        if excerpts.is_empty() {
            continue;
        }
        let user = format!(
            "A symbol is declared with these terms: {terms}.\nDeclared meaning: {meaning}\n\
             Declared valence: {valence}.\n\nPassages using the symbol:\n{ex}\n\n\
             Identify any passage where the symbol is used AGAINST its declared meaning or valence. \
             {lang_dir} Return a JSON array of objects {{\"evidence\": \"<the passage>\", \
             \"explanation\": \"<why it contradicts the declaration>\"}}. Empty array if all uses are consistent.",
            terms = s.vocabulary.join(", "),
            meaning = s.meaning,
            valence = s.valence.as_code(),
            ex = format_excerpts(&excerpts),
        );
        if let Ok(raw) = myth_llm_call(cfg, SYSTEM_PROMPT, &user) {
            let terms = s.vocabulary.first().cloned().unwrap_or_default();
            for f in parse_findings(&raw, FindingType::SymbolInconsistency, &s.para_id, &format!("symbol `{terms}`")) {
                sid += 1;
                store.upsert_finding(&book.slug, &format!("sym-{}-{sid}", s.para_id), &f, &now)?;
                all.push(f);
            }
        }
    }

    // ── motif completeness ───────────────────────────────────────────────────
    store.clear_findings_of_type(&book.slug, FindingType::MotifIncomplete)?;
    let total_chapters = super::pipeline::chapter_count(h, book);
    let mut mid = 0u32;
    for m in store.motifs(&book.slug)? {
        let occ_chapters = store.motif_chapters(&book.slug, &m.para_id)?;
        if (occ_chapters.len() as u32) < motif_min_occurrences {
            continue;
        }
        let excerpts = motif_occurrence_excerpts(store, layout, h, book, &m.para_id, MAX_EXCERPTS);
        if excerpts.is_empty() {
            continue;
        }
        let user = format!(
            "A recurring motif is declared.\nName: {name}\nDescription: {desc}\nValence: {valence}\n\
             Total chapters in the book: {total}. The motif appears in chapters: {chapters:?}.\n\n\
             Passages where it appears:\n{ex}\n\n\
             Judge whether the motif forms a COMPLETE narrative pattern (introduced, developed, and \
             paid off / resolved) or is structurally incomplete (e.g. introduced then abandoned, no \
             development, or no payoff). {lang_dir} Return a JSON array; if incomplete, ONE object \
             {{\"evidence\": \"<the gap>\", \"explanation\": \"<what phase is missing>\"}}; empty array if complete.",
            name = m.name,
            desc = m.description,
            valence = m.valence.as_code(),
            total = total_chapters,
            chapters = occ_chapters,
            ex = format_excerpts(&excerpts),
        );
        if let Ok(raw) = myth_llm_call(cfg, SYSTEM_PROMPT, &user) {
            for f in parse_findings(&raw, FindingType::MotifIncomplete, &m.para_id, &format!("motif `{}`", m.name)) {
                mid += 1;
                store.upsert_finding(&book.slug, &format!("mot-{}-{mid}", m.para_id), &f, &now)?;
                all.push(f);
            }
        }
    }

    // ── archetype role fulfilment ────────────────────────────────────────────
    store.clear_findings_of_type(&book.slug, FindingType::ArchetypeRoleUnfulfilled)?;
    let mut aid = 0u32;
    for a in store.archetypes(&book.slug)? {
        let name = a.character_name.trim();
        if name.is_empty() {
            continue; // vacancy is the deterministic check's job
        }
        let excerpts = excerpts_containing(layout, h, book, &lang, &[name.to_string()], MAX_EXCERPTS);
        if excerpts.is_empty() {
            continue;
        }
        let role_label = match &a.role {
            ArchetypeRole::Custom(s) => s.replace('_', " "),
            r => r.as_code().replace('_', " "),
        };
        let user = format!(
            "A character is mapped to a narrative archetype.\nCharacter: {name}\nArchetype role: {role}\n\
             Declared function: {func}\n\nPassages featuring the character:\n{ex}\n\n\
             Judge whether the character actually PERFORMS the declared role function in the prose. \
             {lang_dir} Return a JSON array; if the character does NOT fulfil the role, ONE object \
             {{\"evidence\": \"<passage>\", \"explanation\": \"<how the role goes unfulfilled>\"}}; \
             empty array if the role is fulfilled.",
            name = name,
            role = role_label,
            func = a.function_desc,
            ex = format_excerpts(&excerpts),
        );
        if let Ok(raw) = myth_llm_call(cfg, SYSTEM_PROMPT, &user) {
            for f in parse_findings(
                &raw,
                FindingType::ArchetypeRoleUnfulfilled,
                &a.para_id,
                &format!("{role_label} `{name}`"),
            ) {
                aid += 1;
                store.upsert_finding(&book.slug, &format!("arc-{}-{aid}", a.para_id), &f, &now)?;
                all.push(f);
            }
        }
    }

    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_and_parse_findings() {
        let raw = "Here:\n```json\n[{\"evidence\":\"the raven sang sweetly\",\"explanation\":\"used as hope, declared as death\"}]\n```";
        assert_eq!(
            extract_json_array(raw),
            "[{\"evidence\":\"the raven sang sweetly\",\"explanation\":\"used as hope, declared as death\"}]"
        );
        let fs = parse_findings(raw, FindingType::SymbolInconsistency, "s1", "symbol `raven`");
        assert_eq!(fs.len(), 1);
        assert_eq!(fs[0].finding_type, FindingType::SymbolInconsistency);
        assert_eq!(fs[0].entry_para_id.as_deref(), Some("s1"));
        assert!(fs[0].description.starts_with("symbol `raven`:"));
        assert_eq!(fs[0].evidence.as_deref(), Some("the raven sang sweetly"));
    }

    #[test]
    fn empty_array_yields_no_findings() {
        assert!(parse_findings("[]", FindingType::MotifIncomplete, "m1", "motif `x`").is_empty());
        // Missing explanation → skipped.
        assert!(parse_findings(
            "[{\"evidence\":\"e\"}]",
            FindingType::MotifIncomplete,
            "m1",
            "motif `x`"
        )
        .is_empty());
    }

    #[test]
    fn language_names_cover_all() {
        assert_eq!(language_name(&ProseLanguage::Ru), "Russian");
        assert_eq!(language_name(&ProseLanguage::Other("pl".into())), "English");
    }
}
