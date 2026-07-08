//! HAIKU-1 — zero-AI startup / on-new-paragraph / on-demand haiku.
//!
//! Five curated, original poems per language (EN / RU / DE / FR / ES, and the
//! haiku-only PT / IT / JA), all `&'static str` baked into the binary — present
//! even on an airgapped machine
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

/// A batch text-embedding function — a wrapper over `Store::embed_batch`, so this
/// module carries no `Store` dependency. `Some` only when the engine is warm.
type EmbedFn<'a> = &'a dyn Fn(&[&str]) -> anyhow::Result<Vec<Vec<f32>>>;

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

/// Resolve a book language to a haiku ISO code. Extends the shared
/// `ai::prompts::iso_from_long` (en/ru/fr/de/es) with the haiku-only languages
/// Portuguese / Italian / Japanese — a purely additive delight, kept local so no
/// other feature inherits a language it has no resources for.
fn haiku_iso(lang_long: &str) -> &'static str {
    match lang_long.to_lowercase().as_str() {
        "portuguese" | "português" | "portugues" => "pt",
        "italian" | "italiano" => "it",
        "japanese" | "日本語" | "nihongo" => "ja",
        other => crate::ai::prompts::iso_from_long(other),
    }
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
    Haiku {
        lang: "pt",
        poems: [
            ["Página em branco —", "a frase que quer nascer", "dorme na caneta."],
            ["Rascunho à meia-noite.", "O cursor pisca paciente,", "à espera do amanhã."],
            ["Primeiro parágrafo.", "O resto ainda é névoa —", "mas este já vive."],
            ["Chuva na vidraça.", "A lâmpada faz seu círculo.", "Lá fora: mais escuro."],
            ["Entre duas palavras", "vive o silêncio que escolhe", "o que virá depois."],
        ],
    },
    Haiku {
        lang: "it",
        poems: [
            ["Pagina bianca —", "la frase che vuole nascere", "dorme nella penna."],
            ["Bozza a mezzanotte.", "Il cursore lampeggia paziente,", "aspetta il domani."],
            ["Primo paragrafo.", "Il resto è ancora nebbia —", "ma questo già vive."],
            ["Pioggia sul vetro.", "La lampada fa il suo cerchio.", "Fuori: più buio."],
            ["Tra due parole", "vive il silenzio che sceglie", "ciò che verrà poi."],
        ],
    },
    Haiku {
        lang: "ja",
        poems: [
            ["白紙が光る —", "生まれたい一文が", "ペンの中で待つ"],
            ["真夜中の草稿。", "カーソルは静かに待つ", "明日の言葉を"],
            ["最初の一行。", "あとはまだ霧の中 —", "でもこれは生きる"],
            ["窓を打つ雨。", "ランプが小さな輪を描く。", "外はもっと暗い"],
            ["二つの言葉の間 —", "次に来るものを選ぶ", "静けさが住む"],
        ],
    },
];

/// Emit one haiku to the Output pane for the given (long-form) book language.
/// No-op if the Output store is not yet installed. `Lifetime::Session(1)` keeps
/// only the most recent haiku, so the pane never accumulates them.
pub fn emit_for_lang(lang_long: &str) {
    use crate::pane::output::{Lifetime, Message, Severity, kinds};

    let iso = haiku_iso(lang_long);
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
    embed_fn: Option<EmbedFn>,
) {
    let iso = haiku_iso(lang_long);
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

    emit_lines(lines, used_semantic);
}

/// HAIKU-3 — emit a haiku chosen to reflect the WHOLE book rather than the current
/// paragraph, from a precomputed *centroid* (the caller averages the manuscript's
/// paragraph embeddings via [`centroid`] and caches it). `embed_fn` is `Some` only
/// when the engine is warm — used here just to warm the poem cache; on a `None`
/// centroid / cold cache it falls back to the HAIKU-1 rotation. No AI, no network.
pub fn emit_book(lang_long: &str, book_centroid: Option<&[f32]>, embed_fn: Option<EmbedFn>) {
    let iso = haiku_iso(lang_long);
    let slot = rotation_slot();

    let mut used_semantic = false;
    let lines: [&'static str; 3] = (|| -> Option<[&'static str; 3]> {
        let cvec = book_centroid?;
        let embed = embed_fn?;
        if let Err(e) = semantic::warm_cache(|t| embed(t)) {
            tracing::debug!(target: "inkhaven::haiku", "semantic warm failed: {e:#}");
            return None;
        }
        let picked = semantic::select(iso, cvec, slot)?;
        used_semantic = true;
        Some(picked)
    })()
    .unwrap_or_else(|| poem_at(iso, slot));

    emit_lines(lines, used_semantic);
}

/// The centroid of a set of embedding vectors — see [`mean_unit`]. Exposed so a
/// caller can compute and cache the whole-book centroid for [`emit_book`].
pub(crate) fn centroid(vecs: &[Vec<f32>]) -> Option<Vec<f32>> {
    mean_unit(vecs)
}

/// The mean of a set of (L2-normalised) embedding vectors, re-normalised to a unit
/// vector — the centroid that stands in for the whole set. `None` on an empty set,
/// a zero-length vector, a dimension mismatch, or a zero-magnitude mean.
fn mean_unit(vecs: &[Vec<f32>]) -> Option<Vec<f32>> {
    let dim = vecs.first()?.len();
    if dim == 0 || vecs.iter().any(|v| v.len() != dim) {
        return None;
    }
    let mut acc = vec![0f32; dim];
    for v in vecs {
        for (a, x) in acc.iter_mut().zip(v) {
            *a += x;
        }
    }
    let norm = acc.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm < 1e-9 {
        return None;
    }
    for a in acc.iter_mut() {
        *a /= norm;
    }
    Some(acc)
}

/// Emit three chosen lines to the Output pane, keeping exactly one haiku there
/// (dismiss any prior). Shared by the HAIKU-2 and HAIKU-3 emit paths.
fn emit_lines(lines: [&'static str; 3], used_semantic: bool) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity};

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
        for code in ["en", "ru", "de", "fr", "es", "pt", "it", "ja"] {
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
    fn mean_unit_averages_and_normalises() {
        // HAIKU-3: the book centroid is the re-normalised mean of the samples.
        let m = mean_unit(&[vec![1.0, 0.0], vec![0.0, 1.0]]).unwrap();
        assert!((m[0] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        assert!((m[1] - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-5);
        let mag = (m[0] * m[0] + m[1] * m[1]).sqrt();
        assert!((mag - 1.0).abs() < 1e-5, "centroid is a unit vector");
        // Degenerate inputs return None (caller falls back to rotation).
        assert!(mean_unit(&[]).is_none());
        assert!(mean_unit(&[vec![0.0, 0.0]]).is_none(), "zero magnitude");
        assert!(mean_unit(&[vec![1.0, 0.0], vec![1.0, 0.0, 0.0]]).is_none(), "dim mismatch");
    }

    #[test]
    fn haiku_iso_resolves_extra_languages_and_falls_back() {
        assert_eq!(haiku_iso("Portuguese"), "pt");
        assert_eq!(haiku_iso("italiano"), "it");
        assert_eq!(haiku_iso("Japanese"), "ja");
        assert_eq!(haiku_iso("Russian"), "ru"); // shared set still resolves
        assert_eq!(haiku_iso("Klingon"), "en"); // unknown → English
        // Every resolved code has a poem table.
        for lang in ["Portuguese", "italiano", "Japanese"] {
            let iso = haiku_iso(lang);
            assert!(HAIKU_TABLE.iter().any(|h| h.lang == iso), "no table for {iso}");
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
