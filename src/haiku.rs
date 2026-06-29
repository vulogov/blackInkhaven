//! HAIKU-1 — zero-AI startup / on-new-paragraph / on-demand haiku.
//!
//! Five curated, original poems per language (EN / RU / DE / FR / ES), all
//! `&'static str` baked into the binary — present even on an airgapped machine
//! in the first millisecond of startup. A process-global `AtomicUsize` rotates
//! the choice so the three triggers (startup / new paragraph / `Ctrl+Z p`) each
//! advance it. Language lookup reuses `ai::prompts::iso_from_long`.
//!
//! No AI, no network, no runtime generation: the poem is chosen from the table,
//! never composed. Each language carries three Tier-A poems (writing / the
//! manuscript) in slots 0–2 and two Tier-B poems (the writer's surroundings) in
//! slots 3–4, so the first two encounters are always manuscript-focused.

use std::sync::atomic::{AtomicUsize, Ordering};

static ROTATION: AtomicUsize = AtomicUsize::new(0);

pub struct Haiku {
    pub lang: &'static str,
    /// Five `[line1, line2, line3]` poems, 0-indexed.
    pub poems: [[&'static str; 3]; 5],
}

/// Advance the rotation counter and return the next haiku for the given
/// ISO-639-1 code. Falls back to English for any unsupported code.
pub fn next_for_lang(iso: &str) -> [&'static str; 3] {
    let idx = ROTATION.fetch_add(1, Ordering::Relaxed) % 5;
    let table = HAIKU_TABLE
        .iter()
        .find(|h| h.lang.eq_ignore_ascii_case(iso))
        .unwrap_or(&HAIKU_TABLE[0]); // index 0 is always English
    table.poems[idx]
}

static HAIKU_TABLE: &[Haiku] = &[
    Haiku {
        lang: "en",
        poems: [
            ["Blank page catches light —", "the sentence that wants writing", "waits inside the pen."],
            ["Draft saved at midnight —", "the cursor blinks on, patient,", "for tomorrow's word."],
            ["Old coffee, cold now.", "A paragraph breaks in two,", "then finds its own end."],
            ["Frost on the window.", "The lamp makes its small circle.", "Outside: more dark."],
            ["Between two words: air —", "the space that carries the weight", "before ink arrives."],
        ],
    },
    Haiku {
        lang: "ru",
        poems: [
            ["Белый лист молчит.", "Перо застыло над ним —", "слово ещё спит."],
            ["Ночная глава.", "Чернила помнят всё сами —", "я только пишу."],
            ["Первая строка.", "Всё остальное — туман,", "но эта — живёт."],
            ["Свет фонаря в дожде.", "Окно запотело к утру.", "Кофе ещё горячий."],
            ["Тишина в доме.", "Часы отбивают час.", "Слова ещё ждут."],
        ],
    },
    Haiku {
        lang: "de",
        poems: [
            ["Leere Seite, still.", "Ein Satz sucht seinen Anfang", "irgendwo im Licht."],
            ["Mitternacht. Kaffee.", "Das Kapitel hat kein Ende —", "noch nicht, noch nicht ganz."],
            ["Erste Zeile, fertig.", "Der Rest schläft noch im Nichts —", "der Bleistift wartet."],
            ["Regen an der Scheibe.", "Die Lampe macht ihren Kreis.", "Draußen: mehr Dunkel."],
            ["Zwischen zwei Wörtern", "liegt die Stille, die entscheidet,", "was als nächstes kommt."],
        ],
    },
    Haiku {
        lang: "fr",
        poems: [
            ["Page vide, ce soir.", "La phrase cherche sa lumière", "dans le silence."],
            ["Minuit. La plume", "s'arrête au milieu du mot —", "demain, elle finit."],
            ["Premier paragraphe.", "Le reste n'existe pas encore.", "Assez pour ce soir."],
            ["Pluie sur la vitre.", "La lampe fait son petit cercle.", "Dehors : encore du noir."],
            ["Entre deux mots : l'air —", "l'espace qui porte le sens", "avant l'encre."],
        ],
    },
    Haiku {
        lang: "es",
        poems: [
            ["Hoja en blanco, luz.", "La primera palabra espera", "dentro del silencio."],
            ["Medianoche ya.", "El cursor parpadea solo,", "esperando la voz."],
            ["Primera línea.", "Todo lo demás: niebla —", "pero esta existe."],
            ["Lluvia en el cristal.", "La lámpara hace su círculo.", "Fuera: más oscuridad."],
            ["Entre dos palabras", "vive el silencio que elige", "lo que vendrá después."],
        ],
    },
];

/// Emit one haiku to the Output pane for the given (long-form) book language.
/// No-op if the Output store is not yet installed. `Lifetime::Session(1)` keeps
/// only the most recent haiku, so the pane never accumulates them.
pub fn emit_for_lang(lang_long: &str) {
    use crate::pane::output::{Lifetime, Message, Severity, kinds};

    let iso = crate::ai::prompts::iso_from_long(lang_long);
    let lines = next_for_lang(iso);
    // `text` is the single-line form (for anything that reads metadata["text"],
    // e.g. ink.io / search); the pane renders the `haiku_lines` array as three
    // indented lines.
    let inline = format!("{} / {} / {}", lines[0], lines[1], lines[2]);
    let msg = Message::new(
        kinds::HAIKU,
        Severity::Info,
        Lifetime::Session(1),
        serde_json::json!({
            "text": inline,
            "haiku_lines": [lines[0], lines[1], lines[2]],
            "category": "haiku",
        }),
    );
    crate::pane::output::emit(&msg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_covers_five_languages_five_nonempty_poems() {
        for code in ["en", "ru", "de", "fr", "es"] {
            let h = HAIKU_TABLE
                .iter()
                .find(|h| h.lang == code)
                .unwrap_or_else(|| panic!("missing haiku table for {code}"));
            assert_eq!(h.poems.len(), 5, "wrong poem count for {code}");
            for (i, poem) in h.poems.iter().enumerate() {
                for (j, line) in poem.iter().enumerate() {
                    assert!(!line.trim().is_empty(), "empty line {j} in poem {i} for {code}");
                }
            }
        }
    }

    #[test]
    fn english_is_index_zero_for_fallback() {
        assert_eq!(HAIKU_TABLE[0].lang, "en");
        assert!(HAIKU_TABLE.iter().all(|h| h.lang != "zh")); // unsupported → fallback
    }

    #[test]
    fn rotation_advances_and_wraps_and_never_panics() {
        // Pull a full cycle + 1; every result is three non-empty lines and the
        // index wraps modulo 5 (the unknown language resolves to English).
        let first = next_for_lang("zh");
        assert!(first.iter().all(|l| !l.is_empty()));
        for _ in 0..6 {
            let p = next_for_lang("en");
            assert_eq!(p.len(), 3);
        }
    }
}
