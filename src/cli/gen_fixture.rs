//! 1.2.18+ I.1.1 — synthetic project fixture generator.
//!
//! Produces a deterministic 10K-paragraph inkhaven
//! project under a given path.  Drives every I.1 + R.*
//! benchmark in the cycle: load time, tree scroll,
//! search round-trip, AI envelope assembly, paragraph
//! save throughput.
//!
//! Hidden from `--help` (internal use; not for
//! end-user projects).  Invoke via
//! `inkhaven gen-fixture <path>`.
//!
//! ## Shape (default `FixtureSpec`)
//!
//! * 5 user books × 20 chapters × 100 paragraphs =
//!   10 000 paragraphs.
//! * 3 system books: `Characters` (80 entries),
//!   `Places` (30 entries), `Threads` (12 threads, 3–8
//!   waypoints each).
//! * 200 inline comments distributed across
//!   paragraphs (~2 % comment-density, matches real
//!   projects).
//! * Tags at 15 % chapter-level coverage.
//!
//! ## Determinism
//!
//! Everything goes through a 64-bit xorshift PRNG
//! seeded from `FixtureSpec::seed` (default
//! `0xC0FFEE_DEADBEEF`).  Same seed + same shape
//! parameters = byte-identical project across runs.
//! CI runs lock the seed so bench results are
//! reproducible.
//!
//! ## Prose
//!
//! A hand-curated pool of ~90 sentences with
//! intentional vocabulary diversity + recurring
//! named entities (Helena, Marcus, Brennan,
//! Selene, the Harbor, the Garden, …).  Each
//! paragraph samples sentences to hit a target word
//! count drawn from a triangle distribution centred
//! at 450 words (stdev ~150 — matches the real-
//! project distribution).  Not semantically
//! meaningful but vocabulary-realistic enough to
//! exercise tokenisation + Snowball stemming +
//! embeddings + concordance.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::{InsertPosition, NodeKind, Store};

/// All knobs the generator understands.  Defaults
/// produce the proposal-spec 10K-paragraph shape; tests
/// shrink these to keep runtime sane.
#[derive(Debug, Clone)]
#[allow(dead_code)] // I.1.1.b fields land in the follow-up
pub struct FixtureSpec {
    pub books: usize,
    pub chapters_per_book: usize,
    pub paragraphs_per_chapter: usize,
    pub character_count: usize,
    pub place_count: usize,
    pub thread_count: usize,
    pub comment_count: usize,
    pub tag_coverage: f32,
    /// Target mean word count per paragraph.  Real
    /// inkhaven projects tend to sit around 400–500
    /// words per paragraph; the generator samples from
    /// a triangle distribution centred here.
    pub target_words_per_paragraph: u32,
    pub seed: u64,
    /// Language codes to spread across the books (round-robin), e.g.
    /// `["en","ru","fr","de","es"]`. Each book's prose is drawn from that
    /// language's pool, so a multilingual fixture exercises the Unicode path
    /// (tokenisation, embedding, search, export round-trips) — the 2.0
    /// verification harness's multilingual surface. Unknown codes fall back to
    /// English.
    pub languages: Vec<String>,
    /// `--force` semantics from `inkhaven init`: wipe
    /// the target directory without prompting.
    pub force: bool,
}

impl Default for FixtureSpec {
    fn default() -> Self {
        Self {
            books: 5,
            chapters_per_book: 20,
            paragraphs_per_chapter: 100,
            character_count: 80,
            place_count: 30,
            thread_count: 12,
            comment_count: 200,
            tag_coverage: 0.15,
            target_words_per_paragraph: 450,
            seed: 0xC0FFEE_DEAD_BEEF,
            languages: vec!["en".to_string()],
            force: false,
        }
    }
}

/// Summary returned to the caller for stdout
/// reporting.
///
/// `characters_created` / `places_created` /
/// `threads_created` are placeholders for I.1.1.b
/// (system-book population).  I.1.1 ships the user-
/// book backbone — the 10K-paragraph load that drives
/// every bench scenario.  System-book content
/// (Characters lexicon overlay, threads tree density)
/// improves bench realism but doesn't block the
/// harness, so it lands in a follow-up.
#[derive(Debug, Default)]
pub struct FixtureStats {
    pub books_created: usize,
    pub chapters_created: usize,
    pub paragraphs_created: usize,
    #[allow(dead_code)]
    pub characters_created: usize,
    #[allow(dead_code)]
    pub places_created: usize,
    #[allow(dead_code)]
    pub threads_created: usize,
}

pub fn run(path: &Path, spec: FixtureSpec) -> Result<FixtureStats> {
    // Re-use `inkhaven init` to lay down the project
    // skeleton + databases.  `empty` template keeps the
    // tree clean for us to fill.
    super::init::run(path, spec.force, "empty")?;

    let layout = ProjectLayout::new(path);
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;

    let mut rng = Xorshift64::new(spec.seed);

    let stats = build_hierarchy(&store, &cfg, &spec, &mut rng)?;
    store.checkpoint()?;
    Ok(stats)
}

fn build_hierarchy(
    store: &Store,
    cfg: &Config,
    spec: &FixtureSpec,
    rng: &mut Xorshift64,
) -> Result<FixtureStats> {
    let mut stats = FixtureStats::default();

    // ── User books ────────────────────────────────────
    let langs: Vec<&str> = if spec.languages.is_empty() {
        vec!["en"]
    } else {
        spec.languages.iter().map(String::as_str).collect()
    };
    for book_idx in 0..spec.books {
        // Each book takes one language (round-robin); its prose comes from that
        // language's pool so a multilingual fixture spreads Unicode across the
        // whole corpus.
        let lang = langs[book_idx % langs.len()];
        let pool = sentence_pool(lang);
        let lang_tag = if langs.len() > 1 { format!(" [{lang}]") } else { String::new() };
        let book_title = format!("Book {}: {}{lang_tag}", book_idx + 1, pick(BOOK_TITLES, rng));
        let hierarchy = Hierarchy::load(store)?;
        let book = store.create_node(
            cfg,
            &hierarchy,
            NodeKind::Book,
            &book_title,
            None,
            None,
            InsertPosition::End,
        )?;
        stats.books_created += 1;
        store.provision_user_book(cfg, &book)?;

        for chapter_idx in 0..spec.chapters_per_book {
            let chapter_title = format!(
                "Chapter {}: {}",
                chapter_idx + 1,
                pick(CHAPTER_TITLES, rng),
            );
            let hierarchy = Hierarchy::load(store)?;
            let book_ref = hierarchy.get(book.id).ok_or_else(|| {
                Error::Store("book vanished from hierarchy".into())
            })?;
            let chapter = store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Chapter,
                &chapter_title,
                Some(book_ref),
                None,
                InsertPosition::End,
            )?;
            stats.chapters_created += 1;

            for paragraph_idx in 0..spec.paragraphs_per_chapter {
                let paragraph_title = format!(
                    "{:03}. {}",
                    paragraph_idx + 1,
                    pick(PARAGRAPH_TITLES, rng),
                );
                let hierarchy = Hierarchy::load(store)?;
                let chapter_ref = hierarchy.get(chapter.id).ok_or_else(|| {
                    Error::Store("chapter vanished from hierarchy".into())
                })?;
                let mut paragraph = store.create_node(
                    cfg,
                    &hierarchy,
                    NodeKind::Paragraph,
                    &paragraph_title,
                    Some(chapter_ref),
                    None,
                    InsertPosition::End,
                )?;
                let target =
                    triangle_word_count(rng, spec.target_words_per_paragraph);
                // Compose a typst-flavoured body that
                // starts with the auto-generated heading
                // (`create_node` already wrote one but
                // the inkhaven save path always
                // overwrites with what the editor holds,
                // so we mirror that contract).
                let heading = format!("= {}\n\n", paragraph_title);
                let body = heading + &generate_prose(rng, target, pool);
                // Mirror the TUI save path: io_atomic
                // write to disk FIRST, then update the
                // store's bdslib copy + re-embed.  The
                // 1.2.15+ atomic semantics carry through.
                let rel = paragraph
                    .file
                    .as_ref()
                    .ok_or_else(|| {
                        Error::Store("paragraph node has no `file` path".into())
                    })?
                    .clone();
                let abs = store.project_root().join(&rel);
                crate::io_atomic::write(&abs, body.as_bytes())
                    .map_err(Error::Io)?;
                store.update_paragraph_content(
                    &mut paragraph,
                    body.as_bytes(),
                )?;
                stats.paragraphs_created += 1;
            }
        }
    }

    Ok(stats)
}

/// Sample a paragraph word count from a triangle
/// distribution centred at `target` (mean of three
/// uniforms ≈ symmetric, narrower tails than a single
/// uniform).  Clamped to `[100, 2 * target]` so we
/// never produce empty or absurdly-long paragraphs.
fn triangle_word_count(rng: &mut Xorshift64, target: u32) -> usize {
    // Three uniforms in [0, 2*target] → mean = target.
    let span = (target * 2) as u64;
    let a = rng.next_u64() % (span + 1);
    let b = rng.next_u64() % (span + 1);
    let c = rng.next_u64() % (span + 1);
    let avg = (a + b + c) / 3;
    avg.clamp(100, span) as usize
}

/// Build a paragraph body that hits ≈ `target_words`
/// by concatenating sentences from the pool until the
/// running word count crosses the target.  Returns
/// typst-flavoured prose (no leading heading — that's
/// `Store::create_node`'s responsibility).
fn generate_prose(rng: &mut Xorshift64, target_words: usize, pool: &[&str]) -> String {
    let mut out = String::new();
    let mut words = 0usize;
    while words < target_words {
        let sentence = pick(pool, rng);
        out.push_str(sentence);
        out.push(' ');
        words += sentence.split_whitespace().count();
    }
    out.push('\n');
    out
}

fn pick<'a>(pool: &[&'a str], rng: &mut Xorshift64) -> &'a str {
    let idx = (rng.next_u64() as usize) % pool.len();
    pool[idx]
}

/// xorshift64* — 19-line PRNG good enough for fixture
/// generation.  Deterministic; same seed → same
/// output.  Not cryptographic.
#[derive(Debug)]
struct Xorshift64 {
    state: u64,
}

impl Xorshift64 {
    fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0xDEAD_BEEF_DEAD_BEEF } else { seed },
        }
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    #[allow(dead_code)]
    fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }
}

// ── Static pools ─────────────────────────────────────

const BOOK_TITLES: &[&str] = &[
    "The Harbor Code",
    "When the Garden Sleeps",
    "Lessons from the Lantern Room",
    "An Inheritance of Salt",
    "The Quiet Geometry",
    "Beneath the Slate Roofs",
    "What the Tides Remember",
    "The Almanac of Small Hours",
];

const CHAPTER_TITLES: &[&str] = &[
    "Arrivals", "The Wharf", "First Light", "The Letter",
    "What Helena Knew", "Below Stairs", "The Garden Path",
    "Brennan's Errand", "A Sealed Door", "The Ledger",
    "Iris at the Window", "Marcus in Repose",
    "The Open Workshop", "After the Bell",
    "Selene Returns", "The Slow Spring", "The Outer Lane",
    "Theo's Map", "Vesper's Account", "The Granite Stair",
    "A Tally of Names", "Crossing the Square",
    "The Lacquered Box", "The Storm Watch",
    "Quiet Counsel", "The Untouched Hours",
];

const PARAGRAPH_TITLES: &[&str] = &[
    "Approach", "Threshold", "Pause", "Returning",
    "Counsel", "Inquiry", "Reverie", "Ledger",
    "Hand-off", "Quiet hour", "Margin", "Aside",
    "Outside the door", "After-fact", "Cross-check",
    "Outer lane", "Slow turn", "Carry-over",
];

// I.1.1.b — wired in by the system-book population
// follow-up.  The names appear in SENTENCE_POOL today
// (Helena, Marcus, etc) so the Characters lexicon
// overlay still has matches when run against the
// fixture.
#[allow(dead_code)]
const CHARACTER_NAMES: &[&str] = &[
    "Helena", "Marcus", "Brennan", "Selene", "Iris",
    "Theo", "Vesper", "Jonas", "Anneka", "Caradoc",
    "Maren", "Soren", "Lior", "Tamsin", "Renn",
    "Ovid", "Petra", "Edrick", "Mira", "Halden",
    "Tess", "Garrick", "Oriel", "Sabin", "Linnea",
    "Yves", "Cora", "Belden", "Astra", "Niven",
];

#[allow(dead_code)]
const PLACE_NAMES: &[&str] = &[
    "The Harbor", "The Garden", "The Lantern Room",
    "The Old Market", "The Outer Lane", "The Square",
    "The Slate Roofs", "The Granite Stair",
    "The Workshop", "The Wharf", "The Lower Library",
    "The Storm Watch", "The Brackenfield",
    "The Quiet Walk", "The Lacquer House",
];

#[allow(dead_code)]
const THREAD_NAMES: &[&str] = &[
    "Inheritance subplot",
    "The secret correspondence",
    "Marcus' redemption arc",
    "The harbor master mystery",
    "Helena's reckoning",
    "Iris and the workshop",
    "Brennan's quiet betrayal",
    "The lantern-room investigation",
    "What the tides reveal",
    "The almanac and the heir",
    "Selene's homecoming",
    "The slow-burn rivalry",
];

/// The prose pool for a language code (defaults to English). Used to spread a
/// multilingual fixture across the store→embed→search→export pipeline, so the
/// Unicode / non-ASCII path is exercised end to end (the 2.0 harness).
fn sentence_pool(lang: &str) -> &'static [&'static str] {
    match lang {
        "ru" => SENTENCE_POOL_RU,
        "fr" => SENTENCE_POOL_FR,
        "de" => SENTENCE_POOL_DE,
        "es" => SENTENCE_POOL_ES,
        _ => SENTENCE_POOL,
    }
}

/// Russian (Cyrillic) — recurring entities Елена / Маркус / Гавань.
const SENTENCE_POOL_RU: &[&str] = &[
    "Елена остановилась на пороге, прислушиваясь к шагам внизу.",
    "Маркус долго молчал, потом кивнул один раз и ушёл прочь.",
    "Утренний свет упал на окно и окрасил комнату янтарём.",
    "Дождь снаружи превратился в ровный ритм по шиферной крыше.",
    "Письмо на столе осталось нераспечатанным, печать блестела в свете лампы.",
    "Бреннан медленно перевернул страницу, будто слова могли ускользнуть.",
    "Сад одичал за годы, что за ним никто толком не следил.",
    "Селена поставила чашку и ждала ответ, который уже знала.",
    "Гавань была тиха после последнего шторма, лодки жались в своих слипах.",
    "Ирис держала руки на коленях, но глаз не сводила с двери.",
];

/// French (accents/ligatures) — Hélène / Marcus / le Port.
const SENTENCE_POOL_FR: &[&str] = &[
    "Hélène s'arrêta sur le seuil, guettant le bruit des pas en contrebas.",
    "Marcus ne dit rien un long moment, puis hocha la tête et s'éloigna.",
    "La lumière du matin tomba sur la fenêtre et peignit la pièce d'ambre.",
    "Dehors, la pluie était devenue un rythme régulier sur le toit d'ardoise.",
    "La lettre sur le bureau restait cachetée, son sceau brillant sous la lampe.",
    "Brennan tourna lentement la page, comme si les mots pouvaient lui échapper.",
    "Le jardin s'était ensauvagé depuis que plus personne ne l'entretenait.",
    "Séléné posa la tasse et attendit la réponse qu'elle connaissait déjà.",
    "Le port était calme depuis la dernière tempête, les barques serrées à quai.",
    "Iris gardait les mains jointes sur ses genoux, les yeux rivés à la porte.",
];

/// German (umlauts/ß) — Helena / Markus / der Hafen.
const SENTENCE_POOL_DE: &[&str] = &[
    "Helena hielt an der Schwelle inne und lauschte den Schritten darunter.",
    "Markus schwieg lange, dann nickte er einmal und ging fort.",
    "Das Morgenlicht fiel aufs Fenster und färbte den Raum bernsteinfarben.",
    "Draußen war der Regen zu einem stetigen Takt auf dem Schieferdach geworden.",
    "Der Brief auf dem Tisch blieb ungeöffnet, sein Siegel glänzte im Lampenlicht.",
    "Brennan wandte langsam die Seite um, als könnten ihm die Wörter entgleiten.",
    "Der Garten war verwildert, seit sich niemand mehr richtig um ihn kümmerte.",
    "Selene stellte die Tasse ab und wartete auf die Antwort, die sie schon kannte.",
    "Der Hafen lag still seit dem letzten Sturm, die Boote fest in ihren Buchten.",
    "Iris hielt die Hände im Schoß gefaltet, doch ihr Blick blieb an der Tür.",
];

/// Spanish (ñ/accents/¿¡) — Helena / Marcos / el Puerto.
const SENTENCE_POOL_ES: &[&str] = &[
    "Helena se detuvo en el umbral, atenta al ruido de pasos abajo.",
    "Marcos no dijo nada un largo rato; luego asintió una vez y se marchó.",
    "La luz de la mañana cayó sobre la ventana y pintó la habitación de ámbar.",
    "Afuera, la lluvia se había vuelto un ritmo constante sobre el tejado de pizarra.",
    "La carta sobre el escritorio seguía cerrada, su sello brillante a la lámpara.",
    "Brennan pasó la página despacio, como si las palabras pudieran escapársele.",
    "El jardín se había vuelto salvaje desde que nadie lo cuidaba como es debido.",
    "Selene dejó la taza y esperó la respuesta que ya conocía.",
    "El puerto estaba tranquilo tras la última tormenta, las barcas bien amarradas.",
    "Iris mantenía las manos en el regazo, pero no apartaba los ojos de la puerta.",
];

const SENTENCE_POOL: &[&str] = &[
    "Helena paused at the threshold, listening for the sound of footsteps below.",
    "Marcus said nothing for a long moment, then nodded once and walked away.",
    "The morning light fell across the window and painted the room amber.",
    "Outside, the rain had become a steady rhythm against the slate roof.",
    "The letter on the desk was unopened, its seal still bright in the lamplight.",
    "Brennan turned the page slowly, as if the words might escape him.",
    "The garden had grown wild in the years since anyone tended it properly.",
    "Selene set down the cup and waited for the answer she already knew.",
    "There was a particular silence in the hall, the kind that comes after good news.",
    "Iris kept her hands folded in her lap, but her eyes did not leave the door.",
    "The harbor had been quiet since the last storm, the boats tucked tight in their slips.",
    "Theo walked the outer lane with the careful gait of a man counting steps.",
    "Vesper's account had been precise, but precision was not the same as truth.",
    "A bell rang twice in the lower square, and the workshop fell to attention.",
    "Helena considered the ledger and the small column of figures that would not balance.",
    "The lantern in the upstairs room had not been lit for three full nights.",
    "Marcus' coat was damp at the shoulders, though he had walked only from the gate.",
    "Brennan kept his promises, but never quite in the way one expected.",
    "The granite stair was slick under last night's frost, and Selene went down with care.",
    "There was a name in the margin of the page, and Iris recognised the hand.",
    "The almanac's spine had cracked along the bind, exposing a slim envelope between the boards.",
    "Outside the workshop, the children had stopped their game to watch the strangers pass.",
    "Theo set his map flat on the table and weighted the corners with two grey stones.",
    "Selene returned the borrowed key to its hook by the door, careful not to make a sound.",
    "Marcus had built the box himself, lacquered black, and only Helena knew the latch.",
    "There was something honest about the way Brennan refused to look up from his book.",
    "The wind had a salt edge to it, and Iris pulled her shawl tighter against the cold.",
    "Vesper's quiet counsel had carried them through harder winters than this one.",
    "A small ledger lay on the corner table, its first page filled with names crossed out.",
    "Helena took the back stair, which the household considered private but not actually secret.",
    "The garden gate complained on its hinges, an old sound that everyone knew but no one fixed.",
    "Soren came in shaking the rain from his hat and looked at no one in particular.",
    "Mira said the news plainly, because plain was what the moment required.",
    "The lacquered box held three keys, two letters, and a coin that no longer rang true.",
    "Iris had loved the workshop before she loved the man who taught her to use it.",
    "The harbor master kept his ledgers in code, and only he could read the latest column.",
    "There was a chair by the window that had not been moved in eleven years.",
    "Helena rinsed the cup and set it on the rack, and did not look at the door.",
    "Theo had drawn the harbor as a clean line, but the harbor itself had been less clear.",
    "Brennan considered the question with the patience of a man who answered only after thought.",
    "Selene paused at the foot of the granite stair and listened to the empty hall above.",
    "The almanac said the spring would be slow, and so far the almanac had been right.",
    "There was a small fire in the lower library, and Iris let herself be drawn to it.",
    "Marcus said he had not been to the workshop in months, but the dust said otherwise.",
    "Vesper kept a list of names that nobody else was permitted to read.",
    "The slate roofs caught the late light and threw it back in dull copper.",
    "Helena returned from the garden with three small apples and a question she had not asked.",
    "Brennan offered to walk her to the outer lane, but Helena declined without explaining why.",
    "The harbor in the late afternoon belonged to the gulls and the tide.",
    "Marcus took down the ledger, opened it to a page near the end, and waited.",
    "Selene crossed the square at a measured pace, neither hurried nor slow.",
    "Iris had a habit of finishing other people's sentences, but only when she agreed with them.",
    "Theo's quiet had a different weight depending on who was in the room.",
    "The lantern room was empty, but the chair had been moved closer to the window.",
    "Helena did not believe in coincidences any more than she believed in the almanac's promises.",
    "Brennan's letter was three pages long, and the third page mattered most.",
    "The granite stair was steep where it turned, and Mira always took it with one hand on the rail.",
    "Outside, the wind had dropped, and the harbor had taken on its evening glass.",
    "Marcus folded the map carefully along its old creases and slid it back into the drawer.",
    "Selene had not seen the workshop since the spring before last, and it looked smaller than she remembered.",
    "Iris kept the almanac on the kitchen shelf, where everyone could reach it but no one ever did.",
    "Theo asked the question he had been carrying since the harbor master last refused him.",
    "Helena set the kettle on, and the kettle began its slow work of waking the room.",
    "Vesper had a way of saying nothing that conveyed considerable disapproval.",
    "Brennan turned the lacquered box once in his hands, set it down, and did not open it.",
    "There was a chair in the workshop that had once been Marcus' father's, and Marcus refused to use it.",
    "The lower library smelled of damp paper and woodsmoke, the way Iris liked it.",
    "Helena did not raise her voice; she did not need to.",
    "Selene's eyes went to the granite stair, then to the lantern, then to the door.",
    "There was a knock on the outer gate, three measured taps, and then silence.",
    "Marcus considered the offer, but his answer had been the same answer for many years.",
    "The harbor in the morning was cold and crisp and very nearly without colour.",
    "Brennan said he would not be back before the bell, and the household knew the bell well.",
    "Iris had the kind of memory that recovered facts only when they were no longer useful.",
    "Theo's map had no scale, but it had the shape of the truth of the place.",
    "Helena rinsed the lamp chimney with care, because the lamp was older than the lamp room itself.",
    "Selene had decided, somewhere between the square and the gate, that she would not ask the question after all.",
    "The almanac said the autumn would be mild, and the almanac had been wrong before.",
    "Vesper did not approve of the new ledger, but Vesper rarely approved of anything new.",
    "Brennan walked Helena home through the outer lane, and they spoke of nothing important until the gate.",
    "There was a moment, just before the bell rang, when the workshop felt almost empty.",
    "Marcus had been the kind of boy who counted footsteps; he was now a man who counted hours.",
    "The lacquered box had not been opened in eleven years, but the latch still worked smoothly.",
    "Iris remembered the harbor master's name only because she had heard it twice in the same week.",
    "Theo took the outer lane back to the harbor, partly to think and partly to avoid the square.",
    "Helena had finished the ledger by the time the kettle began to sing.",
    "Selene paused at the doorway of the workshop and let her eyes adjust to the lamplight.",
    "There was a particular silence in the lantern room that night, the kind that comes after a long argument.",
    "Brennan was the sort of man who could read a letter in one sitting and quote it three weeks later.",
    "The almanac had been Brennan's mother's, and Brennan kept it because she had wanted him to.",
    "Iris closed the book with the slow finality of a person who has decided something important.",
    "The harbor's bells rang the half hour, and three of the workshop lamps came on at once.",
    "Helena set the cup down on the table without a sound, and Marcus understood the message.",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn small_spec() -> FixtureSpec {
        FixtureSpec {
            books: 1,
            chapters_per_book: 2,
            paragraphs_per_chapter: 3,
            character_count: 4,
            place_count: 3,
            thread_count: 2,
            comment_count: 5,
            tag_coverage: 0.1,
            target_words_per_paragraph: 50,
            seed: 0x1234_5678_9ABC_DEF0,
            languages: vec!["en".to_string()],
            force: true,
        }
    }

    #[test]
    fn sentence_pools_carry_non_ascii_prose_and_fall_back_to_english() {
        // Every language pool is non-empty.
        for lang in ["en", "ru", "fr", "de", "es"] {
            assert!(!sentence_pool(lang).is_empty(), "{lang} pool is empty");
        }
        // The non-English pools actually contain non-ASCII characters (the whole
        // point — the multilingual fixture must exercise the Unicode path).
        let has_non_ascii = |lang: &str| sentence_pool(lang).iter().any(|s| !s.is_ascii());
        assert!(has_non_ascii("ru"), "ru pool should be Cyrillic");
        assert!(has_non_ascii("fr"), "fr pool should carry accents");
        assert!(has_non_ascii("de"), "de pool should carry umlauts/ß");
        assert!(has_non_ascii("es"), "es pool should carry accents/ñ");
        // Cyrillic specifically for ru.
        assert!(
            sentence_pool("ru").iter().any(|s| s.chars().any(|c| ('\u{0400}'..='\u{04FF}').contains(&c))),
            "ru pool should contain Cyrillic code points"
        );
        // English is ASCII; an unknown code falls back to it (content-equal).
        assert!(sentence_pool("en").iter().all(|s| s.is_ascii()));
        assert_eq!(sentence_pool("zz"), sentence_pool("en"));
    }

    #[test]
    fn xorshift_is_deterministic() {
        let mut a = Xorshift64::new(42);
        let mut b = Xorshift64::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn xorshift_zero_seed_substitutes() {
        // A zero seed would collapse xorshift to zero
        // output forever; constructor swaps in a
        // non-zero default.
        let mut rng = Xorshift64::new(0);
        let first = rng.next_u64();
        let second = rng.next_u64();
        assert_ne!(first, 0);
        assert_ne!(first, second);
    }

    #[test]
    fn triangle_word_count_lands_in_range() {
        let mut rng = Xorshift64::new(0xDEAD);
        let target = 450;
        let mut samples = Vec::new();
        for _ in 0..500 {
            samples.push(triangle_word_count(&mut rng, target));
        }
        // Every sample within the documented bounds.
        for s in &samples {
            assert!(*s >= 100);
            assert!(*s <= (target * 2) as usize);
        }
        // Mean lands within 15% of target — triangle
        // distribution + 500 samples is enough to make
        // this tight.
        let mean: f64 =
            samples.iter().map(|s| *s as f64).sum::<f64>() / 500.0;
        assert!(
            (mean - target as f64).abs() < target as f64 * 0.15,
            "expected mean ≈ {target}, got {mean}",
        );
    }

    #[test]
    fn generate_prose_hits_target_word_count() {
        let mut rng = Xorshift64::new(0xC0FFEE);
        let body = generate_prose(&mut rng, 100, SENTENCE_POOL);
        let words = body.split_whitespace().count();
        // The generator stops at first overshoot, so
        // we land in [target, target + longest_sentence].
        assert!(words >= 100);
        assert!(words < 200, "expected ≤ target + a sentence, got {words}");
    }

    #[test]
    fn generate_prose_is_deterministic_per_seed() {
        let mut a = Xorshift64::new(0x1);
        let mut b = Xorshift64::new(0x1);
        assert_eq!(
            generate_prose(&mut a, 100, SENTENCE_POOL),
            generate_prose(&mut b, 100, SENTENCE_POOL),
        );
    }

    #[test]
    #[ignore = "runs the full fixture pipeline, which embeds paragraphs and needs the ONNX model (network-fetched); flaky in CI, run locally"]
    fn small_fixture_creates_expected_shape() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("project");
        let stats = run(&path, small_spec()).unwrap();
        assert_eq!(stats.books_created, 1);
        assert_eq!(stats.chapters_created, 2);
        assert_eq!(stats.paragraphs_created, 6);
        // metadata.db, blobs.db, vectors/, books/...
        assert!(path.join("metadata.db").exists());
        assert!(path.join("books").exists());
    }

    #[test]
    #[ignore = "runs the full fixture pipeline, which embeds paragraphs and needs the ONNX model (network-fetched); flaky in CI, run locally"]
    fn small_fixture_is_byte_deterministic() {
        let tmp_a = tempfile::tempdir().unwrap();
        let tmp_b = tempfile::tempdir().unwrap();
        let path_a = tmp_a.path().join("p");
        let path_b = tmp_b.path().join("p");
        let spec_a = small_spec();
        let spec_b = small_spec();
        let stats_a = run(&path_a, spec_a).unwrap();
        let stats_b = run(&path_b, spec_b).unwrap();
        assert_eq!(stats_a.paragraphs_created, stats_b.paragraphs_created);

        // Compare prose for the first paragraph of the
        // first chapter of the first book.  Same seed →
        // same bytes.
        let books_a = path_a.join("books");
        let books_b = path_b.join("books");
        let walk_a = collect_paragraph_files(&books_a);
        let walk_b = collect_paragraph_files(&books_b);
        assert_eq!(walk_a.len(), walk_b.len());
        for (a, b) in walk_a.iter().zip(walk_b.iter()) {
            let body_a = std::fs::read(a).unwrap();
            let body_b = std::fs::read(b).unwrap();
            assert_eq!(
                body_a, body_b,
                "expected byte-identical bodies for the same seed",
            );
        }
    }

    fn collect_paragraph_files(root: &Path) -> Vec<std::path::PathBuf> {
        let mut out = Vec::new();
        walk(root, &mut out);
        out.sort();
        out
    }

    fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
        let Ok(read) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in read.flatten() {
            let p = entry.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().map(|e| e == "typ").unwrap_or(false) {
                out.push(p);
            }
        }
    }
}
