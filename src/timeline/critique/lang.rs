//! TIMELINE-2-INTEGRATION P5 — per-language reason text for the critique findings.
//!
//! The detectors carry only *structured facts* (significance, staleness, suspicion
//! factors); this module is the single source that turns those facts into the
//! human reason lines, in the five baseline languages (EN/RU/ES/FR/DE). The
//! detectors render English here (so `body_en` and the pattern text never drift),
//! and the emission layer re-renders in the project's language. Reuses the
//! fact-checker's [`Lang`] enum.

use super::types::{FuzzyOverlapFinding, OrphanFinding, Significance, Staleness};
pub use crate::world::fact_check_lang::Lang;

/// Map the project's `config.language` name to a baseline [`Lang`] (English on an
/// unrecognised value).
pub fn lang_from_name(name: &str) -> Lang {
    match name.trim().to_ascii_lowercase().as_str() {
        "russian" | "ru" | "rus" => Lang::Ru,
        "spanish" | "es" | "spa" | "español" | "espanol" => Lang::Es,
        "french" | "fr" | "fra" | "français" | "francais" => Lang::Fr,
        "german" | "de" | "deu" | "ger" | "deutsch" => Lang::De,
        _ => Lang::En,
    }
}

// ── orphan reasons ───────────────────────────────────────────────────────────

/// The localized reason lines for an orphan finding.
pub fn orphan_reasons(
    significance: Significance,
    staleness: Staleness,
    age_days: Option<i64>,
    lang: Lang,
) -> Vec<String> {
    let mut out = vec![orphan_unlinked(lang).to_string()];
    out.push(orphan_significance(significance, lang).to_string());
    match (staleness, age_days) {
        (Staleness::Old, Some(age)) => out.push(orphan_old(age, lang)),
        (Staleness::Recent, Some(age)) if age >= 0 => out.push(orphan_recent(age, lang)),
        _ => {}
    }
    out
}

fn orphan_unlinked(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "No paragraphs, characters, or places are linked to this event.",
        Lang::Ru => "К этому событию не привязаны абзацы, персонажи или места.",
        Lang::Es => "No hay párrafos, personajes ni lugares vinculados a este evento.",
        Lang::Fr => "Aucun paragraphe, personnage ni lieu n'est lié à cet événement.",
        Lang::De => "Diesem Ereignis sind keine Absätze, Figuren oder Orte zugeordnet.",
    }
}

fn orphan_significance(sig: Significance, lang: Lang) -> &'static str {
    match (sig, lang) {
        (Significance::High, Lang::En) => "A concrete date and detailed title suggest a significant event.",
        (Significance::High, Lang::Ru) => "Точная дата и подробное название указывают на важное событие.",
        (Significance::High, Lang::Es) => "Una fecha concreta y un título detallado sugieren un evento significativo.",
        (Significance::High, Lang::Fr) => "Une date précise et un titre détaillé suggèrent un événement important.",
        (Significance::High, Lang::De) => "Ein konkretes Datum und ein ausführlicher Titel deuten auf ein bedeutendes Ereignis hin.",
        (Significance::Moderate, Lang::En) => "Has some detail but sits unconnected.",
        (Significance::Moderate, Lang::Ru) => "Содержит некоторые детали, но остаётся несвязанным.",
        (Significance::Moderate, Lang::Es) => "Tiene algún detalle pero está sin conectar.",
        (Significance::Moderate, Lang::Fr) => "Comporte quelques détails mais reste non relié.",
        (Significance::Moderate, Lang::De) => "Hat einige Details, bleibt aber unverbunden.",
        (Significance::Low, Lang::En) => "Stub event with minimal metadata.",
        (Significance::Low, Lang::Ru) => "Заготовка события с минимумом метаданных.",
        (Significance::Low, Lang::Es) => "Evento incipiente con metadatos mínimos.",
        (Significance::Low, Lang::Fr) => "Événement ébauché avec des métadonnées minimales.",
        (Significance::Low, Lang::De) => "Rumpf-Ereignis mit minimalen Metadaten.",
    }
}

fn orphan_old(age: i64, lang: Lang) -> String {
    match lang {
        Lang::En => format!("Orphaned for {age} days."),
        Lang::Ru => format!("Осиротело уже {age} дн."),
        Lang::Es => format!("Huérfano desde hace {age} días."),
        Lang::Fr => format!("Orphelin depuis {age} jours."),
        Lang::De => format!("Seit {age} Tagen verwaist."),
    }
}

fn orphan_recent(age: i64, lang: Lang) -> String {
    match lang {
        Lang::En => format!("Recently added ({age} days ago)."),
        Lang::Ru => format!("Добавлено недавно ({age} дн. назад)."),
        Lang::Es => format!("Añadido recientemente (hace {age} días)."),
        Lang::Fr => format!("Ajouté récemment (il y a {age} jours)."),
        Lang::De => format!("Kürzlich hinzugefügt (vor {age} Tagen)."),
    }
}

/// Re-render an orphan finding's reasons in `lang` (from its structured fields).
pub fn localize_orphan(f: &OrphanFinding, lang: Lang) -> Vec<String> {
    orphan_reasons(f.significance, f.staleness, f.age_days, lang)
}

// ── overlap reasons ──────────────────────────────────────────────────────────

/// The localized reason lines for a fuzzy-overlap finding.
pub fn overlap_reasons(f: &FuzzyOverlapFinding, lang: Lang) -> Vec<String> {
    let mut out = Vec::new();
    if f.is_cluster {
        out.push(cluster_head(f.total_events, lang));
        let cap = super::fuzzy_overlap::CLUSTER_LIST_CAP;
        if f.total_events > cap {
            out.push(and_more(f.total_events - cap, lang));
        }
        if !f.shared_places.is_empty() {
            out.push(cluster_shared_place(lang).to_string());
        }
        if !f.shared_characters.is_empty() {
            out.push(cluster_shared_char(lang).to_string());
        }
    } else {
        out.push(pair_head(f.precision.as_str(), lang));
        if f.same_track {
            out.push(pair_same_track(&f.track, lang));
        }
        if !f.shared_characters.is_empty() {
            out.push(pair_shared_char(lang).to_string());
        }
        if !f.shared_places.is_empty() {
            out.push(pair_shared_place(lang).to_string());
        }
    }
    out
}

fn pair_head(prec: &str, lang: Lang) -> String {
    match lang {
        Lang::En => format!("Two {prec}-precision events have overlapping windows."),
        Lang::Ru => format!("Два события с точностью «{prec}» имеют пересекающиеся окна."),
        Lang::Es => format!("Dos eventos con precisión «{prec}» tienen ventanas que se solapan."),
        Lang::Fr => format!("Deux événements de précision « {prec} » ont des fenêtres qui se chevauchent."),
        Lang::De => format!("Zwei Ereignisse mit „{prec}“-Genauigkeit haben überlappende Fenster."),
    }
}

fn cluster_head(n: usize, lang: Lang) -> String {
    match lang {
        Lang::En => format!("{n} fuzzy-precision events share an overlapping window."),
        Lang::Ru => format!("{n} событий с нечёткой точностью имеют общее пересекающееся окно."),
        Lang::Es => format!("{n} eventos de precisión difusa comparten una ventana superpuesta."),
        Lang::Fr => format!("{n} événements à précision floue partagent une fenêtre qui se chevauche."),
        Lang::De => format!("{n} Ereignisse mit unscharfer Genauigkeit teilen ein überlappendes Fenster."),
    }
}

fn and_more(n: usize, lang: Lang) -> String {
    match lang {
        Lang::En => format!("and {n} more events"),
        Lang::Ru => format!("и ещё {n} событий"),
        Lang::Es => format!("y {n} eventos más"),
        Lang::Fr => format!("et {n} autres événements"),
        Lang::De => format!("und {n} weitere Ereignisse"),
    }
}

fn pair_same_track(track: &str, lang: Lang) -> String {
    match lang {
        Lang::En => format!("Both on the \"{track}\" track."),
        Lang::Ru => format!("Оба на дорожке «{track}»."),
        Lang::Es => format!("Ambos en la pista «{track}»."),
        Lang::Fr => format!("Tous deux sur la piste « {track} »."),
        Lang::De => format!("Beide auf der Spur „{track}“."),
    }
}

fn pair_shared_char(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "They share a character.",
        Lang::Ru => "У них общий персонаж.",
        Lang::Es => "Comparten un personaje.",
        Lang::Fr => "Ils partagent un personnage.",
        Lang::De => "Sie teilen sich eine Figur.",
    }
}

fn pair_shared_place(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "They share a place.",
        Lang::Ru => "У них общее место.",
        Lang::Es => "Comparten un lugar.",
        Lang::Fr => "Ils partagent un lieu.",
        Lang::De => "Sie teilen sich einen Ort.",
    }
}

fn cluster_shared_place(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "All events share a common place.",
        Lang::Ru => "У всех событий общее место.",
        Lang::Es => "Todos los eventos comparten un lugar común.",
        Lang::Fr => "Tous les événements partagent un lieu commun.",
        Lang::De => "Alle Ereignisse teilen sich einen gemeinsamen Ort.",
    }
}

fn cluster_shared_char(lang: Lang) -> &'static str {
    match lang {
        Lang::En => "All events share a common character.",
        Lang::Ru => "У всех событий общий персонаж.",
        Lang::Es => "Todos los eventos comparten un personaje común.",
        Lang::Fr => "Tous les événements partagent un personnage commun.",
        Lang::De => "Alle Ereignisse teilen sich eine gemeinsame Figur.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timeline::Precision;
    use uuid::Uuid;

    #[test]
    fn lang_name_mapping() {
        assert_eq!(lang_from_name("russian"), Lang::Ru);
        assert_eq!(lang_from_name("Français"), Lang::Fr);
        assert_eq!(lang_from_name("klingon"), Lang::En);
    }

    #[test]
    fn orphan_reasons_localize_and_count() {
        let en = orphan_reasons(Significance::High, Staleness::Old, Some(92), Lang::En);
        assert_eq!(en.len(), 3);
        assert!(en[2].contains("92 days"));
        let ru = orphan_reasons(Significance::High, Staleness::Old, Some(92), Lang::Ru);
        assert!(ru[0].contains("событию"));
        assert!(ru[2].contains("92"));
        // No age → no third line.
        assert_eq!(orphan_reasons(Significance::Low, Staleness::Recent, None, Lang::De).len(), 2);
    }

    fn overlap(is_cluster: bool, total: usize, same_track: bool, places: Vec<Uuid>) -> FuzzyOverlapFinding {
        FuzzyOverlapFinding {
            event_ids: vec![Uuid::nil(), Uuid::nil()],
            titles: vec!["A".into(), "B".into()],
            track: "main".into(),
            overlap_window: (0, 10),
            suspicion: super::super::types::Suspicion::Moderate,
            is_cluster,
            same_track,
            precision: Precision::Season,
            total_events: total,
            shared_characters: vec![],
            shared_places: places,
            severity: super::super::types::CritSeverity::Info,
            reasons: vec![],
        }
    }

    #[test]
    fn overlap_pair_reasons_localize() {
        let f = overlap(false, 2, true, vec![]);
        let es = overlap_reasons(&f, Lang::Es);
        assert!(es[0].contains("precisión"));
        assert!(es.iter().any(|r| r.contains("pista")));
    }

    #[test]
    fn overlap_cluster_shared_place_line() {
        let f = overlap(true, 3, true, vec![Uuid::nil()]);
        let de = overlap_reasons(&f, Lang::De);
        assert!(de[0].contains("Ereignisse"));
        assert!(de.iter().any(|r| r.contains("gemeinsamen Ort")));
    }
}
