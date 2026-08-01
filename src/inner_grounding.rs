//! INNER-GROUND-1 — shared reader grounding for the Inner family.
//!
//! A neutral prose prefix telling an Inner-family reader (Inner Socrates, Inner
//! Editor) what the author has *declared* about the work — the cast and their
//! arcs, the symbol library, the world's open coherence tensions — so its
//! observations stay consistent with the author's world instead of reading blind.
//!
//! This is the reader-agnostic sibling of `inner_theologian::grounding`: same
//! declared sources, neutral framing (no theology voice, no scope-findings
//! argument). Each source degrades cleanly — a missing feature or empty store
//! contributes nothing — and `None` is returned when nothing grounds, so a reader
//! with no declared context behaves exactly as before.

use std::path::Path;

/// Compose the shared grounding prefix, or `None` when the author has declared
/// nothing this reader could ground on. Opens the project store read-only and
/// resolves the primary manuscript book itself, so a caller need only pass the
/// project root.
pub(crate) fn build_grounding(
    project_root: &Path,
    shared_store: Option<crate::store::Store>,
) -> Option<String> {
    let layout = crate::project::ProjectLayout::new(project_root);
    let cfg = crate::config::Config::load_layered(&layout.config_path()).ok()?;
    // 1.8.34 (hardening) — reuse the App-held main store when present (the engage
    // worker path), rather than a fresh `Store::open` on the worker thread that
    // opens a second instance of the primary DB (lock-race with the UI) and re-runs
    // `configure`/`load_store_scripts` side effects per engagement. Open fresh only
    // on the CLI path (no App). The store is only read here (hierarchy load).
    let store = match shared_store {
        Some(s) => s,
        None => crate::store::Store::open(layout, &cfg).ok()?,
    };
    let h = crate::store::hierarchy::Hierarchy::load(&store).ok()?;
    let book = crate::cli::resolve_user_book(&h, None, "inner grounding").ok()?;
    let book_slug = book.slug.as_str();

    // Multilingual: the grounding preamble is keyed off the project language, not
    // English-only. The declared data (names, symbols, event titles) is already in
    // the author's language; only the labels/header are localized.
    let lx = Labels::for_language(&cfg.language);
    let mut parts: Vec<String> = Vec::new();

    // Characters — the declared cast and where their arcs are going.
    if let Ok(cs) = crate::character::CharStore::open(project_root) {
        if let Ok(decls) = cs.all_declarations(book_slug) {
            let cast: Vec<String> = decls
                .iter()
                .map(|d| format!("{} ({})", d.character_name.trim(), d.arc_type.as_code()))
                .filter(|s| !s.starts_with('('))
                .collect();
            if !cast.is_empty() {
                let shown: Vec<String> = cast.iter().take(6).cloned().collect();
                parts.push(format!("• {}: {}", lx.characters, shown.join("; ")));
            }
        }
    }

    // Myth — the declared symbol/motif library the prose is meant to answer to.
    if let Ok(ms) = crate::myth::MythStore::open(project_root) {
        let mut named: Vec<String> = Vec::new();
        if let Ok(symbols) = ms.symbols(book_slug) {
            named.extend(
                symbols
                    .iter()
                    .flat_map(|s| s.vocabulary.iter())
                    .map(|v| v.trim().to_string())
                    .filter(|v| !v.is_empty()),
            );
        }
        if let Ok(motifs) = ms.motifs(book_slug) {
            named.extend(motifs.iter().map(|m| m.name.trim().to_string()).filter(|n| !n.is_empty()));
        }
        named.sort();
        named.dedup();
        if !named.is_empty() {
            let shown: Vec<String> = named.iter().take(8).cloned().collect();
            parts.push(format!("• {}: {}", lx.symbols, shown.join(", ")));
        }
    }

    // World — open coherence tensions the reader should not contradict.
    if let Ok(us) = crate::world::utopia::UtopiaStore::open(project_root) {
        if let Ok(findings) = us.findings(book_slug, true) {
            let n = findings.len();
            if n > 0 {
                parts.push(format!("• {}: {n}", lx.tensions));
            }
        }
    }

    // History — the declared story timeline the prose must stay chronologically
    // true to. (This is the anchor the Inner Historian reads against.)
    let mut events: Vec<(i64, String)> = h
        .flatten()
        .into_iter()
        .filter_map(|(n, _)| n.event.as_ref().map(|e| (n, e)))
        .filter(|(n, _)| descends_from(&h, n, book.id))
        .map(|(n, e)| (e.start_ticks, n.title.trim().to_string()))
        .filter(|(_, t)| !t.is_empty())
        .collect();
    events.sort_by_key(|(t, _)| *t);
    if !events.is_empty() {
        let shown: Vec<String> = events.iter().take(4).map(|(_, t)| t.clone()).collect();
        parts.push(format!("• {}: {}", lx.timeline, shown.join(" → ")));
    }

    // SEMNET-P7 — relational signals the graph knows that no single declared-data
    // store computes: recurring character pairings (co-appearance across the
    // book's events) and unresolved factual contradictions. The reader now also
    // reads what the *graph* has established. Degrades cleanly when the graph is
    // bare (nothing rebuilt/confronted yet).
    let book_event_ids: Vec<uuid::Uuid> = h
        .flatten()
        .into_iter()
        .filter(|(n, _)| n.event.is_some() && descends_from(&h, n, book.id))
        .map(|(n, _)| n.id)
        .collect();
    if let Ok(sig) = store.grounding_signals(&book_event_ids) {
        if !sig.recurring_pairs.is_empty() {
            let names: Vec<String> = sig
                .recurring_pairs
                .iter()
                .take(4)
                .filter_map(|(a, b)| {
                    let na = h.get(*a).map(|n| n.title.trim().to_string()).filter(|t| !t.is_empty())?;
                    let nb = h.get(*b).map(|n| n.title.trim().to_string()).filter(|t| !t.is_empty())?;
                    Some(format!("{na} ↔ {nb}"))
                })
                .collect();
            if !names.is_empty() {
                parts.push(format!("• {}: {}", lx.pairings, names.join("; ")));
            }
        }
        if sig.open_contradictions > 0 {
            parts.push(format!("• {}: {}", lx.contradictions, sig.open_contradictions));
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(format!("{}\n{}", lx.header, parts.join("\n")))
    }
}

/// Localized labels for the grounding preamble, in the five first-class
/// languages (English fallback). Only the framing is translated; the declared
/// values are the author's own words.
struct Labels {
    header: &'static str,
    characters: &'static str,
    symbols: &'static str,
    tensions: &'static str,
    timeline: &'static str,
    pairings: &'static str,
    contradictions: &'static str,
}

impl Labels {
    fn for_language(cfg_language: &str) -> Labels {
        match cfg_language.trim().to_lowercase().as_str() {
            "ru" | "russian" | "русский" => Labels {
                header: "ОСНОВА — автор уже установил следующее об этом произведении; держите текст в согласии с этим:",
                characters: "Персонажи и их арки",
                symbols: "Символы и мотивы",
                tensions: "Открытых противоречий мира",
                timeline: "Хронология истории (от ранних событий)",
                pairings: "Повторяющиеся пары персонажей",
                contradictions: "Неразрешённых противоречий фактов",
            },
            "fr" | "french" | "français" | "francais" => Labels {
                header: "ANCRAGE — l'auteur a déjà établi ceci au sujet de cette œuvre ; gardez le texte cohérent avec cela :",
                characters: "Personnages et leurs arcs",
                symbols: "Symboles et motifs",
                tensions: "Tensions de cohérence du monde en cours",
                timeline: "Chronologie du récit (du plus ancien)",
                pairings: "Paires de personnages récurrentes",
                contradictions: "Contradictions de faits non résolues",
            },
            "de" | "german" | "deutsch" => Labels {
                header: "GRUNDLAGE — der Autor hat über dieses Werk bereits Folgendes festgelegt; halte den Text damit konsistent:",
                characters: "Figuren und ihre Bögen",
                symbols: "Symbole und Motive",
                tensions: "Offene Welt-Kohärenz-Spannungen",
                timeline: "Zeitleiste der Geschichte (früheste zuerst)",
                pairings: "Wiederkehrende Figurenpaare",
                contradictions: "Ungelöste Faktenwidersprüche",
            },
            "es" | "spanish" | "español" | "espanol" => Labels {
                header: "BASE — el autor ya ha establecido lo siguiente sobre esta obra; mantén el texto coherente con ello:",
                characters: "Personajes y sus arcos",
                symbols: "Símbolos y motivos",
                tensions: "Tensiones de coherencia del mundo abiertas",
                timeline: "Cronología del relato (de los más antiguos)",
                pairings: "Parejas de personajes recurrentes",
                contradictions: "Contradicciones de hechos sin resolver",
            },
            _ => Labels {
                header: "GROUNDING — the author has already declared the following about this work; keep the prose consistent with it:",
                characters: "Characters and their arcs",
                symbols: "Symbols and motifs",
                tensions: "Open world-coherence tensions",
                timeline: "Story timeline (earliest first)",
                pairings: "Recurring character pairings",
                contradictions: "Unresolved fact contradictions",
            },
        }
    }
}

/// Whether `node` sits anywhere under the book `book_id` (walks parent links).
fn descends_from(
    h: &crate::store::hierarchy::Hierarchy,
    node: &crate::store::node::Node,
    book_id: uuid::Uuid,
) -> bool {
    let mut cur = node;
    loop {
        if cur.id == book_id {
            return true;
        }
        let Some(pid) = cur.parent_id else { return false };
        match h.get(pid) {
            Some(p) => cur = p,
            None => return false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grounding_labels_are_localized_off_the_project_language() {
        // The preamble keys off the project language, not English-only.
        assert!(Labels::for_language("russian").header.starts_with("ОСНОВА"));
        assert!(Labels::for_language("ru").characters.contains("Персонажи"));
        assert_eq!(Labels::for_language("french").symbols, "Symboles et motifs");
        assert!(Labels::for_language("deutsch").timeline.contains("Zeitleiste"));
        assert!(Labels::for_language("español").tensions.contains("Tensiones"));
        // Unknown / English → English fallback.
        assert!(Labels::for_language("english").header.starts_with("GROUNDING"));
        assert!(Labels::for_language("klingon").header.starts_with("GROUNDING"));
    }

    #[test]
    fn a_project_with_nothing_declared_grounds_to_none() {
        // No inkhaven project at this path → every source degrades to nothing, so
        // a reader behaves exactly as before (reads blind, no prefix).
        let dir = std::env::temp_dir().join(format!("inner-ground-none-{}", std::process::id()));
        std::fs::create_dir_all(&dir).ok();
        assert!(build_grounding(&dir, None).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
