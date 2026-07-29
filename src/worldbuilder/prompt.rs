//! WBLD-1 (WB-P2) — the worldbuilder AI system prompt and its context sections.
//!
//! The prompt is built fresh each turn from up to five sections: the world state
//! (a declaration summary of `world.hjson` for now — the full compiled-layer
//! summary lands with the realworld bridge in WB-P5), pinned World nodes, the
//! semantically-retrieved `fact:world` paragraphs, and pinned Facts. WB-P3 adds a
//! leading plausibility-warnings section (contradiction awareness); the `warnings`
//! parameter is that hook, empty until then.

use std::path::Path;

use uuid::Uuid;

use crate::config::Config;
use crate::store::Store;
use crate::store::hierarchy::Hierarchy;
use crate::world::types::WorldDefinition;

use super::app::FACT_WORLD_TAG;

/// Assemble the worldbuilder system prompt. Empty sections are omitted so the
/// context stays tight.
pub(super) fn build_system_prompt(
    world_name: &str,
    language: &str,
    warnings: &str,
    world_state: &str,
    pinned_world: &str,
    world_facts: &str,
    pinned_facts: &str,
) -> String {
    let mut s = format!(
        "You are a worldbuilding assistant helping an author develop a coherent, \
         internally-consistent fictional world named {world_name}. You help them build, \
         research, and refine their world. You NEVER generate prose, and you NEVER decide \
         what the world contains — the author decides; you measure, validate, and record. \
         When the author's declaration contradicts the world's physics you propose options, \
         you do not refuse or rewrite. Reason and reply in {language}.\n"
    );
    let section = |s: &mut String, label: &str, body: &str| {
        if !body.trim().is_empty() {
            s.push_str(&format!("\n=== {label} ===\n{}\n", body.trim()));
        }
    };
    // WB-P3 prepends this — an errored/plausibility gate can then be addressed.
    section(&mut s, "CURRENT PLAUSIBILITY WARNINGS (address if relevant)", warnings);
    section(&mut s, "WORLD STATE", world_state);
    section(&mut s, "PINNED WORLD NODES", pinned_world);
    section(&mut s, "WORLD FACTS (retrieved)", world_facts);
    section(&mut s, "PINNED FACTS", pinned_facts);
    s
}

/// A compact declaration summary of `world.hjson` — what the author has declared,
/// not the full compiled consequences (that arrives in WB-P5). `None` when there
/// is no world yet.
pub(super) fn world_declaration_summary(project_root: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(project_root.join("world.hjson")).ok()?;
    let def = WorldDefinition::from_hjson(&raw).ok()?;
    let mut lines = vec![format!("World: {}", def.name.trim())];
    if !def.primary_language.trim().is_empty() {
        lines.push(format!("Primary language: {}", def.primary_language.trim()));
    }
    if !def.nations.is_empty() {
        let names: Vec<&str> = def.nations.iter().map(|n| n.name.trim()).filter(|s| !s.is_empty()).collect();
        if !names.is_empty() {
            lines.push(format!("Nations: {}", names.join(", ")));
        }
    }
    if !def.cultures.is_empty() {
        let names: Vec<&str> = def.cultures.iter().map(|c| c.nation.trim()).filter(|s| !s.is_empty()).collect();
        if !names.is_empty() {
            lines.push(format!("Cultures: {}", names.join(", ")));
        }
    }
    if let Some(m) = &def.magic {
        if m.enabled && !m.rules.is_empty() {
            let kinds: Vec<&str> = m.rules.iter().map(|r| r.kind.trim()).filter(|s| !s.is_empty()).collect();
            lines.push(format!("Magic ({} rule(s)): {}", m.rules.len(), kinds.join(", ")));
        } else {
            lines.push("Magic: none".to_string());
        }
    }
    let mut declared = Vec::new();
    for (label, present) in [
        ("geology", def.geology.is_some()),
        ("geography", def.geography.is_some()),
        ("hydrology", def.hydrology.is_some()),
        ("economy", def.economy.is_some()),
        ("history", def.history.is_some()),
        ("ecology", def.ecology.is_some()),
    ] {
        if present {
            declared.push(label);
        }
    }
    if !declared.is_empty() {
        lines.push(format!("Declared layers: astronomy, {}", declared.join(", ")));
    }
    Some(lines.join("\n"))
}

/// Semantically retrieve `fact:world`-tagged Facts paragraphs for `query`. Runs
/// the ordinary book RAG over the Facts book, then keeps only the tagged
/// paragraphs (RFC: "filter to fact:world by .tags after retrieval, no new
/// index"). Returns a composed context block, or empty when nothing matches.
pub(super) fn retrieve_world_facts(
    store: &Store,
    hierarchy: &Hierarchy,
    cfg: &Config,
    facts_book_id: Uuid,
    query: &str,
) -> String {
    let Ok(passages) = crate::book_rag::retrieval::retrieve(
        store,
        hierarchy,
        &cfg.book_rag,
        facts_book_id,
        query,
    ) else {
        return String::new();
    };
    let mut world: Vec<crate::book_rag::RetrievedPassage> = passages
        .into_iter()
        .filter(|p| {
            hierarchy
                .get(p.id)
                .map(|n| n.tags.iter().any(|t| t == FACT_WORLD_TAG))
                .unwrap_or(false)
        })
        .collect();
    world.truncate(cfg.book_rag.top_k.max(1));
    if world.is_empty() {
        String::new()
    } else {
        crate::book_rag::compose_context_prefix(&world)
    }
}

/// The pinned nodes' text (title + body) for a pin list — full node text goes
/// into the prompt so the author's chosen anchors are always in context.
pub(super) fn pinned_nodes_text(store: &Store, hierarchy: &Hierarchy, pins: &[Uuid]) -> String {
    let mut out = String::new();
    for id in pins {
        let Some(node) = hierarchy.get(*id) else { continue };
        let body = store
            .get_content(*id)
            .ok()
            .flatten()
            .map(|b| String::from_utf8_lossy(&b).trim().to_string())
            .unwrap_or_default();
        out.push_str(&format!("- {}", node.title.trim()));
        if !body.is_empty() {
            out.push_str(&format!(": {body}"));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_only_non_empty_sections_in_order() {
        let p = build_system_prompt(
            "Aldoria",
            "English",
            "",                       // no warnings (WB-P3)
            "World: Aldoria",         // state
            "",                       // no pinned world
            "- Velmari tidal harbour", // world facts
            "",                       // no pinned facts
        );
        assert!(p.contains("named Aldoria"));
        assert!(p.contains("=== WORLD STATE ==="));
        assert!(p.contains("=== WORLD FACTS (retrieved) ==="));
        // Empty sections are omitted.
        assert!(!p.contains("PINNED WORLD NODES"));
        assert!(!p.contains("PINNED FACTS"));
        assert!(!p.contains("CURRENT PLAUSIBILITY WARNINGS"));
        // WORLD STATE precedes WORLD FACTS.
        assert!(p.find("WORLD STATE").unwrap() < p.find("WORLD FACTS").unwrap());
    }

    #[test]
    fn warnings_section_leads_when_present() {
        let p = build_system_prompt("W", "English", "! demographics: landlocked", "state", "", "", "");
        let w = p.find("CURRENT PLAUSIBILITY WARNINGS").unwrap();
        let s = p.find("WORLD STATE").unwrap();
        assert!(w < s, "warnings must precede world state");
    }
}
