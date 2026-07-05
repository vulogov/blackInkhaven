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

use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

pub mod semantic;

static ROTATION: AtomicUsize = AtomicUsize::new(0);
/// Per-process starting offset, seeded once from the wall clock. Without it the
/// in-memory `ROTATION` resets to 0 every launch, so the startup haiku would be
/// the same poem each time; the seed makes each session begin somewhere new.
static SEED: OnceLock<usize> = OnceLock::new();

pub struct Haiku {
    pub lang: &'static str,
    /// Five `[line1, line2, line3]` poems, 0-indexed.
    pub poems: [[&'static str; 3]; 5],
}

/// The next rotation slot (0..5): a per-process clock seed plus a monotonic
/// counter, so successive haikus differ and the first one varies by launch.
fn rotation_slot() -> usize {
    let seed = *SEED.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos() as usize)
            .unwrap_or(0)
    });
    let n = ROTATION.fetch_add(1, Ordering::Relaxed);
    seed.wrapping_add(n) % 5
}

/// The poem for `iso` at slot `idx` (mod 5). Falls back to English for any
/// unsupported code.
fn poem_at(iso: &str, idx: usize) -> [&'static str; 3] {
    let table = HAIKU_TABLE
        .iter()
        .find(|h| h.lang.eq_ignore_ascii_case(iso))
        .unwrap_or(&HAIKU_TABLE[0]); // index 0 is always English
    table.poems[idx % 5]
}

/// Advance the rotation and return the next haiku for the given ISO-639-1 code.
pub fn next_for_lang(iso: &str) -> [&'static str; 3] {
    poem_at(iso, rotation_slot())
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

    // Keep exactly one haiku in the pane: dismiss any prior ones before emitting.
    // The Output store is persistent on disk, and `Lifetime::Session(1)` is only
    // trimmed by the lazy `cleanup()` pass — so without this, haikus from this
    // session *and previous launches* pile up (one per startup + per paragraph).
    if let Some(store) = crate::pane::output::active() {
        if let Ok(prior) = store.by_kind(kinds::HAIKU) {
            for m in &prior {
                let _ = store.dismiss(m.id);
            }
        }
    }

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

/// HAIKU-2 — emit one haiku, choosing *semantically* when possible.
///
/// `context` is the paragraph body / title text used as the similarity query;
/// `embed_fn` wraps `store.embed_batch`. The caller passes `embed_fn: Some(..)`
/// **only when the embedding engine is already warm** (see
/// `Store::embedding_is_loaded`), so this never triggers the ~470 ms cold model
/// load on the UI thread. On `None` context / cold cache / trivial context, it
/// falls back to the HAIKU-1 rotation — the writer sees the same UX either way.
pub fn emit_with_context(
    lang_long: &str,
    context: Option<&str>,
    embed_fn: Option<&dyn Fn(&[&str]) -> anyhow::Result<Vec<Vec<f32>>>>,
) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity};

    let iso = crate::ai::prompts::iso_from_long(lang_long);
    // Advance the rotation exactly once — used for the semantic tiebreak and as
    // the fallback slot, so a semantic emit costs the same one step as HAIKU-1.
    let slot = rotation_slot();

    let mut used_semantic = false;
    let lines: [&'static str; 3] = (|| -> Option<[&'static str; 3]> {
        let ctx = context?;
        let embed = embed_fn?;
        // Need real signal: at least 8 non-whitespace characters.
        if ctx.chars().filter(|c| !c.is_whitespace()).count() < 8 {
            return None;
        }
        // MiniLM's window is ~256 tokens; 512 bytes is a safe proxy.
        let snippet: String = ctx.chars().take(512).collect();
        if let Err(e) = semantic::warm_cache(|texts| embed(texts)) {
            tracing::debug!(target: "inkhaven::haiku", "semantic warm failed: {e:#}");
            return None;
        }
        let query_vec = embed(&[snippet.as_str()]).ok()?.into_iter().next()?;
        let picked = semantic::select(iso, &query_vec, slot)?;
        used_semantic = true;
        Some(picked)
    })()
    .unwrap_or_else(|| poem_at(iso, slot));

    // ── emit (mirrors emit_for_lang) ─────────────────────────────────────
    if let Some(store) = crate::pane::output::active() {
        if let Ok(prior) = store.by_kind(kinds::HAIKU) {
            for m in &prior {
                let _ = store.dismiss(m.id);
            }
        }
    }
    let inline = format!("{} / {} / {}", lines[0], lines[1], lines[2]);
    let msg = Message::new(
        kinds::HAIKU,
        Severity::Info,
        Lifetime::Session(1),
        serde_json::json!({
            "text": inline,
            "haiku_lines": [lines[0], lines[1], lines[2]],
            "category": if used_semantic { "haiku_semantic" } else { "haiku" },
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
