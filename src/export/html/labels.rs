//! TDOC-4 — UI chrome localised to the book's language. Content is authored in the
//! book's language already; this covers the site's own strings (nav, pager, footer).
//! First-class: English (default), Russian, French, German, Spanish.

use serde::Serialize;

/// The site's chrome strings, handed to templates as `labels`.
#[derive(Debug, Clone, Serialize)]
pub struct Labels {
    pub contents: &'static str,
    pub home: &'static str,
    pub search: &'static str,
    pub skip: &'static str,
    pub built_with: &'static str,
}

impl Labels {
    pub fn for_language(cfg_language: &str) -> Labels {
        match cfg_language.trim().to_lowercase().as_str() {
            "ru" | "russian" | "русский" => Labels {
                contents: "Содержание",
                home: "Начало",
                search: "Поиск",
                skip: "Перейти к содержимому",
                built_with: "Сделано с помощью",
            },
            "fr" | "french" | "français" | "francais" => Labels {
                contents: "Sommaire",
                home: "Accueil",
                search: "Rechercher",
                skip: "Aller au contenu",
                built_with: "Réalisé avec",
            },
            "de" | "german" | "deutsch" => Labels {
                contents: "Inhalt",
                home: "Start",
                search: "Suche",
                skip: "Zum Inhalt springen",
                built_with: "Erstellt mit",
            },
            "es" | "spanish" | "español" | "espanol" => Labels {
                contents: "Contenido",
                home: "Inicio",
                search: "Buscar",
                skip: "Ir al contenido",
                built_with: "Hecho con",
            },
            _ => Labels {
                contents: "Contents",
                home: "Home",
                search: "Search",
                skip: "Skip to content",
                built_with: "Built with",
            },
        }
    }

    /// The two-letter `lang` attribute for `<html>`.
    pub fn html_lang(cfg_language: &str) -> &'static str {
        match cfg_language.trim().to_lowercase().as_str() {
            "ru" | "russian" | "русский" => "ru",
            "fr" | "french" | "français" | "francais" => "fr",
            "de" | "german" | "deutsch" => "de",
            "es" | "spanish" | "español" | "espanol" => "es",
            "it" | "italian" | "italiano" => "it",
            "pt" | "portuguese" | "português" | "portugues" => "pt",
            _ => "en",
        }
    }
}
