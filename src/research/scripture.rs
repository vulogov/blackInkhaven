//! RESRCH-SCRIPTURE (1.6.18+) — the sacred-source adapters: `/bible`, `/quran`,
//! `/bookofmormon`. Each fetches a **verse-structured** passage from a keyless,
//! public-domain, multilingual source and ingests it through the existing
//! `/import` path (chunk → embed as a `research_source`), auto-citing a *stable*
//! SOURCES-1 key so the author can later cite a specific locus — `@bible[John
//! 3:16]`, `@quran[2:255]`, `@bookofmormon[1 Nephi 3:7]` — which the Index
//! Locorum harvests.
//!
//! Sources (all keyless, all public-domain text):
//! * **Bible** — bolls.life `get-chapter/{translation}/{book}/{chapter}/`. The book
//!   numbering is canonical Protestant 1–66 (John = 43) and stable across
//!   translations, so one table serves every language. Defaults: en=WEB, ru=SYNOD
//!   (Synodal), fr=FRLSG (Louis Segond), de=LUT (Luther), es=RV1960 — all
//!   overridable via `research.scripture.bible_translation`.
//! * **Quran** — api.alquran.cloud `surah/{n}/{edition}`. Defaults: en=en.sahih,
//!   ru=ru.kuliev, fr=fr.hamidullah, de=de.bubenheim, es=es.cortes; the Arabic
//!   original is `quran-uthmani`.
//! * **Book of Mormon** — the bcbooks `scriptures-json` corpus (1830 public-domain
//!   English). English only: the modern translations are under copyright, and we
//!   source public-domain only.
//!
//! Mirrors `/archive` and `/wikisource`: `reqwest` + `serde_json`, no HTML parser
//! (a tiny tag stripper handles bolls' inline Strong's markup).

use anyhow::{Result, anyhow};
use serde_json::Value as Json;

use crate::config::ScriptureConfig;

/// Which sacred text an invocation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Work {
    Bible,
    Quran,
    BookOfMormon,
}

impl Work {
    /// The stable id used in metadata + the command name suffix.
    pub(super) fn id(self) -> &'static str {
        match self {
            Work::Bible => "bible",
            Work::Quran => "quran",
            Work::BookOfMormon => "bookofmormon",
        }
    }

    /// The command the user typed (for usage strings).
    pub(super) fn command(self) -> &'static str {
        match self {
            Work::Bible => "/bible",
            Work::Quran => "/quran",
            Work::BookOfMormon => "/bookofmormon",
        }
    }

    /// The **stable** SOURCES-1 cite key — so every ingested passage of a work
    /// shares one `@key`, and `@bible[John 3:16]` groups under it in the Index
    /// Locorum regardless of which chapter seeded it.
    pub(super) fn cite_key(self) -> &'static str {
        match self {
            Work::Bible => "bible",
            Work::Quran => "quran",
            Work::BookOfMormon => "book-of-mormon",
        }
    }

    /// A human title for the work (used in the auto-cite `BibEntry`).
    fn work_title(self) -> &'static str {
        match self {
            Work::Bible => "The Holy Bible",
            Work::Quran => "The Qur'an",
            Work::BookOfMormon => "The Book of Mormon",
        }
    }

    /// Parse a work id (`bible` / `quran` / `bookofmormon`) — for the CLI + command.
    pub(super) fn parse(s: &str) -> Option<Work> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bible" => Some(Work::Bible),
            "quran" | "qur'an" | "koran" => Some(Work::Quran),
            "bookofmormon" | "book-of-mormon" | "bom" => Some(Work::BookOfMormon),
            _ => None,
        }
    }
}

/// One verse — its canonical locus (`John 3:16`) and text.
#[derive(Debug, Clone)]
pub(super) struct Verse {
    pub reference: String,
    pub text: String,
}

/// A fetched, verse-structured scripture passage.
#[derive(Debug, Clone)]
pub(super) struct ScripturePassage {
    pub work: Work,
    /// The translation / edition code the text came from (e.g. `WEB`, `ru.kuliev`).
    pub translation: String,
    /// The human passage label (e.g. `John 3`, `Surah 2 (Al-Baqara)`, `1 Nephi 3`).
    pub reference: String,
    /// The citation-clean locus prefix that goes inside `@key[…]` (e.g. `John 3`,
    /// `2`, `1 Nephi 3`) — verse loci are `{locus_prefix}:{verse}`.
    pub locus_prefix: String,
    /// Where the text was fetched from (a real, followable URL).
    pub source_url: String,
    pub verses: Vec<Verse>,
}

impl ScripturePassage {
    /// The ingest body: one `LOCUS\tTEXT` line per verse, so each embedded chunk
    /// is self-describing (the RAG snippet shows which verses it covers, and the
    /// locus is quotable straight into `@key[...]`).
    pub(super) fn body_text(&self) -> String {
        let mut s = String::new();
        for v in &self.verses {
            s.push_str(&v.reference);
            s.push('\t');
            s.push_str(&v.text);
            s.push('\n');
        }
        s
    }

    /// A human name for the ingested source (`Bible · John 3 (WEB)`).
    pub(super) fn source_name(&self) -> String {
        format!(
            "{} · {} ({})",
            self.work.work_title(),
            self.reference,
            self.translation
        )
    }

    /// The `@key[…:<verse>]` hint an author would type to cite a locus here.
    pub(super) fn locus_hint(&self) -> String {
        format!("@{}[{}:<verse>]", self.work.cite_key(), self.locus_prefix)
    }

    /// The stable SOURCES-1 `BibEntry` (auto-cite). Every passage of a work maps to
    /// the same key; the note records the translation actually ingested.
    pub(super) fn to_bibentry(&self) -> crate::sources::BibEntry {
        crate::sources::BibEntry {
            key: self.work.cite_key().to_string(),
            entry_type: "book".to_string(),
            author: String::new(),
            title: self.work.work_title().to_string(),
            year: String::new(),
            url: Some(self.source_url.clone()),
            note: Some(format!(
                "Public-domain scripture · translation {} · cite loci as `@{}[{}]`",
                self.translation,
                self.work.cite_key(),
                sample_locus(self.work),
            )),
            // The stable cite key doubles as the built-in reference-scheme name, so
            // loci like `@bible[John 3:16]` validate with no configuration.
            scheme: Some(self.work.cite_key().to_string()),
            ..Default::default()
        }
    }
}

/// A representative locus for the cite-key hint in the `BibEntry` note.
fn sample_locus(work: Work) -> &'static str {
    match work {
        Work::Bible => "John 3:16",
        Work::Quran => "2:255",
        Work::BookOfMormon => "1 Nephi 3:7",
    }
}

pub(super) fn available(cfg: &ScriptureConfig) -> bool {
    cfg.enabled
}

fn client() -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent("inkhaven-research/1.0 (https://crates.io/crates/inkhaven)")
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|e| anyhow!("http client: {e}"))
}

/// Fetch a passage of `work` for `query` (a reference like `John 3` / `2` / `1
/// Nephi 3`), picking the translation for `lang` unless the config forces one.
/// Owned args → spawnable.
pub(super) async fn fetch(
    cfg: ScriptureConfig,
    work: Work,
    query: String,
    lang: String,
) -> Result<ScripturePassage> {
    match work {
        Work::Bible => fetch_bible(cfg, query, lang).await,
        Work::Quran => fetch_quran(cfg, query, lang).await,
        Work::BookOfMormon => fetch_bom(cfg, query).await,
    }
}

// ── Bible (bolls.life) ───────────────────────────────────────────────────────

/// Default bolls translation code per project language (all public-domain except
/// the widely-used es RV1960, which the user can override). Unknown → WEB.
fn default_bible_translation(lang: &str) -> &'static str {
    match lang {
        "ru" => "SYNOD",
        "fr" => "FRLSG",
        "de" => "LUT",
        "es" => "RV1960",
        _ => "WEB",
    }
}

async fn fetch_bible(cfg: ScriptureConfig, query: String, lang: String) -> Result<ScripturePassage> {
    let (num, en_name, chapter) = parse_bible_ref(&query)
        .ok_or_else(|| anyhow!("usage: /bible <book> <chapter> (e.g. `/bible John 3`)"))?;
    let translation = cfg
        .bible_translation
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| default_bible_translation(&lang).to_string());
    let base = cfg.bible_endpoint.trim_end_matches('/');
    let url = format!("{base}/get-chapter/{translation}/{num}/{chapter}/");
    let client = client()?;
    let json: Json = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("bible fetch: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("bible decode: {e}"))?;
    let arr = json
        .as_array()
        .ok_or_else(|| anyhow!("unexpected bible response for {en_name} {chapter}"))?;
    let mut verses = Vec::new();
    for v in arr {
        let n = v.get("verse").and_then(Json::as_u64).unwrap_or(0);
        let text = strip_html(v.get("text").and_then(Json::as_str).unwrap_or(""));
        if text.is_empty() {
            continue;
        }
        verses.push(Verse { reference: format!("{en_name} {chapter}:{n}"), text });
    }
    if verses.is_empty() {
        return Err(anyhow!(
            "no verses for {en_name} {chapter} in `{translation}` — check the reference \
             or set research.scripture.bible_translation"
        ));
    }
    let locus_prefix = format!("{en_name} {chapter}");
    Ok(ScripturePassage {
        work: Work::Bible,
        translation,
        reference: locus_prefix.clone(),
        locus_prefix,
        source_url: url,
        verses,
    })
}

/// Parse `<book> <chapter>[:verse]` → `(book_number, canonical_english_name,
/// chapter)`. The trailing integer (optionally `chapter:verse`) is the chapter;
/// everything before it is the book. A missing chapter defaults to 1.
fn parse_bible_ref(query: &str) -> Option<(u32, &'static str, u32)> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    // Split off a trailing chapter token (the last whitespace-separated word that
    // begins with a digit). Everything before it is the book name.
    let (book_part, chapter) = match q.rsplit_once(char::is_whitespace) {
        Some((book, tail)) if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) => {
            let ch = tail.split([':', '.']).next().unwrap_or(tail);
            (book.trim(), ch.parse::<u32>().unwrap_or(1).max(1))
        }
        // No trailing chapter (e.g. bare "Genesis") → chapter 1.
        _ => (q, 1),
    };
    let (num, name) = resolve_book(book_part)?;
    Some((num, name, chapter))
}

/// Resolve a book name / number / prefix to its canonical Protestant number and
/// English name. Accepts English names, Russian (Synodal) names, a bare number
/// (1–66), or an unambiguous ≥3-char prefix of either name. Unicode-aware.
fn resolve_book(input: &str) -> Option<(u32, &'static str)> {
    let norm = normalize(input);
    if norm.is_empty() {
        return None;
    }
    if let Ok(n) = norm.parse::<u32>() {
        return BIBLE_BOOKS.iter().find(|(num, ..)| *num == n).map(|&(n, en, _)| (n, en));
    }
    // Exact match on either name.
    if let Some(&(n, en, _)) =
        BIBLE_BOOKS.iter().find(|(_, en, ru)| normalize(en) == norm || normalize(ru) == norm)
    {
        return Some((n, en));
    }
    // Unambiguous prefix match (≥3 chars) on either name.
    if norm.chars().count() >= 3 {
        let hits: Vec<&(u32, &str, &str)> = BIBLE_BOOKS
            .iter()
            .filter(|(_, en, ru)| normalize(en).starts_with(&norm) || normalize(ru).starts_with(&norm))
            .collect();
        if hits.len() == 1 {
            return Some((hits[0].0, hits[0].1));
        }
    }
    None
}

/// LOCI — canonicalize a Bible book name / abbreviation / Russian (Synodal) name
/// to its canonical English form, for the Index Locorum (so `Иоанна`, `Joh`, and
/// `John` collapse to one). `None` when unrecognized. Reuses [`resolve_book`], so
/// it accepts exact names, a bare number (1–66), and unambiguous ≥3-char prefixes.
pub(crate) fn canonical_bible_book(input: &str) -> Option<&'static str> {
    resolve_book(input).map(|(_, en)| en)
}

/// Lowercase + collapse internal whitespace (Unicode-aware) for name matching.
fn normalize(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Strip inline HTML tags (bolls embeds Strong's `<S>1234</S>` and the odd `<br>`)
/// and collapse whitespace. Entities are already decoded by `serde_json`.
fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The 66 canonical Protestant books: `(number, English, Russian Synodal)`. bolls
/// uses this numbering for every translation, so it is language-independent.
#[rustfmt::skip]
const BIBLE_BOOKS: &[(u32, &str, &str)] = &[
    (1, "Genesis", "Бытие"), (2, "Exodus", "Исход"), (3, "Leviticus", "Левит"),
    (4, "Numbers", "Числа"), (5, "Deuteronomy", "Второзаконие"), (6, "Joshua", "Иисус Навин"),
    (7, "Judges", "Судей"), (8, "Ruth", "Руфь"), (9, "1 Samuel", "1 Царств"),
    (10, "2 Samuel", "2 Царств"), (11, "1 Kings", "3 Царств"), (12, "2 Kings", "4 Царств"),
    (13, "1 Chronicles", "1 Паралипоменон"), (14, "2 Chronicles", "2 Паралипоменон"),
    (15, "Ezra", "Ездра"), (16, "Nehemiah", "Неемия"), (17, "Esther", "Есфирь"),
    (18, "Job", "Иов"), (19, "Psalms", "Псалтирь"), (20, "Proverbs", "Притчи"),
    (21, "Ecclesiastes", "Екклесиаст"), (22, "Song of Solomon", "Песнь Песней"),
    (23, "Isaiah", "Исаия"), (24, "Jeremiah", "Иеремия"), (25, "Lamentations", "Плач Иеремии"),
    (26, "Ezekiel", "Иезекииль"), (27, "Daniel", "Даниил"), (28, "Hosea", "Осия"),
    (29, "Joel", "Иоиль"), (30, "Amos", "Амос"), (31, "Obadiah", "Авдий"),
    (32, "Jonah", "Иона"), (33, "Micah", "Михей"), (34, "Nahum", "Наум"),
    (35, "Habakkuk", "Аввакум"), (36, "Zephaniah", "Софония"), (37, "Haggai", "Аггей"),
    (38, "Zechariah", "Захария"), (39, "Malachi", "Малахия"), (40, "Matthew", "Матфея"),
    (41, "Mark", "Марка"), (42, "Luke", "Луки"), (43, "John", "Иоанна"),
    (44, "Acts", "Деяния"), (45, "Romans", "Римлянам"), (46, "1 Corinthians", "1 Коринфянам"),
    (47, "2 Corinthians", "2 Коринфянам"), (48, "Galatians", "Галатам"),
    (49, "Ephesians", "Ефесянам"), (50, "Philippians", "Филиппийцам"),
    (51, "Colossians", "Колоссянам"), (52, "1 Thessalonians", "1 Фессалоникийцам"),
    (53, "2 Thessalonians", "2 Фессалоникийцам"), (54, "1 Timothy", "1 Тимофею"),
    (55, "2 Timothy", "2 Тимофею"), (56, "Titus", "Титу"), (57, "Philemon", "Филимону"),
    (58, "Hebrews", "Евреям"), (59, "James", "Иакова"), (60, "1 Peter", "1 Петра"),
    (61, "2 Peter", "2 Петра"), (62, "1 John", "1 Иоанна"), (63, "2 John", "2 Иоанна"),
    (64, "3 John", "3 Иоанна"), (65, "Jude", "Иуды"), (66, "Revelation", "Откровение"),
];

// ── Quran (api.alquran.cloud) ────────────────────────────────────────────────

/// Default alquran.cloud edition per project language. Unknown → en.sahih.
fn default_quran_edition(lang: &str) -> &'static str {
    match lang {
        "ru" => "ru.kuliev",
        "fr" => "fr.hamidullah",
        "de" => "de.bubenheim",
        "es" => "es.cortes",
        "ar" => "quran-uthmani",
        _ => "en.sahih",
    }
}

async fn fetch_quran(cfg: ScriptureConfig, query: String, lang: String) -> Result<ScripturePassage> {
    let edition = cfg
        .quran_translation
        .clone()
        .filter(|t| !t.trim().is_empty())
        .unwrap_or_else(|| default_quran_edition(&lang).to_string());
    let base = cfg.quran_endpoint.trim_end_matches('/');
    let client = client()?;
    let surah = resolve_surah(&client, base, &query).await?;
    let url = format!("{base}/surah/{surah}/{edition}");
    let json: Json = client
        .get(&url)
        .send()
        .await
        .map_err(|e| anyhow!("quran fetch: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("quran decode: {e}"))?;
    let data = json
        .get("data")
        .ok_or_else(|| anyhow!("unexpected quran response for surah {surah}"))?;
    let english = data.get("englishName").and_then(Json::as_str).unwrap_or("");
    let ayahs = data
        .get("ayahs")
        .and_then(Json::as_array)
        .ok_or_else(|| anyhow!("surah {surah} has no ayahs in `{edition}`"))?;
    let mut verses = Vec::new();
    for a in ayahs {
        let n = a.get("numberInSurah").and_then(Json::as_u64).unwrap_or(0);
        let text = a.get("text").and_then(Json::as_str).unwrap_or("").trim().to_string();
        if text.is_empty() {
            continue;
        }
        verses.push(Verse { reference: format!("{surah}:{n}"), text });
    }
    if verses.is_empty() {
        return Err(anyhow!("surah {surah} returned no text in `{edition}`"));
    }
    let reference = if english.is_empty() {
        format!("Surah {surah}")
    } else {
        format!("Surah {surah} ({english})")
    };
    Ok(ScripturePassage {
        work: Work::Quran,
        translation: edition,
        reference,
        locus_prefix: surah.to_string(),
        source_url: url,
        verses,
    })
}

/// A surah number (1–114) from a bare number, or by matching a name against the
/// surah list (English name / translation, case-insensitive substring).
async fn resolve_surah(client: &reqwest::Client, base: &str, query: &str) -> Result<u32> {
    let q = query.trim();
    if q.is_empty() {
        return Err(anyhow!("usage: /quran <surah number or name> (e.g. `/quran 2`)"));
    }
    if let Ok(n) = q.parse::<u32>() {
        if (1..=114).contains(&n) {
            return Ok(n);
        }
        return Err(anyhow!("surah must be 1–114 (got {n})"));
    }
    let needle = q.to_lowercase();
    let json: Json = client
        .get(format!("{base}/surah"))
        .send()
        .await
        .map_err(|e| anyhow!("quran surah list: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("quran surah list decode: {e}"))?;
    let list = json.get("data").and_then(Json::as_array).cloned().unwrap_or_default();
    for s in &list {
        let en = s.get("englishName").and_then(Json::as_str).unwrap_or("").to_lowercase();
        let tr = s
            .get("englishNameTranslation")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_lowercase();
        if en.contains(&needle) || tr.contains(&needle) {
            if let Some(n) = s.get("number").and_then(Json::as_u64) {
                return Ok(n as u32);
            }
        }
    }
    Err(anyhow!("no surah matching `{q}` — try its number (1–114)"))
}

// ── Book of Mormon (bcbooks scriptures-json) ─────────────────────────────────

async fn fetch_bom(cfg: ScriptureConfig, query: String) -> Result<ScripturePassage> {
    let (book_query, chapter) = parse_bom_ref(&query)
        .ok_or_else(|| anyhow!("usage: /bookofmormon <book> <chapter> (e.g. `/bookofmormon 1 Nephi 3`)"))?;
    let client = client()?;
    let json: Json = client
        .get(&cfg.bom_url)
        .send()
        .await
        .map_err(|e| anyhow!("book of mormon fetch: {e}"))?
        .json()
        .await
        .map_err(|e| anyhow!("book of mormon decode: {e}"))?;
    let books = json
        .get("books")
        .and_then(Json::as_array)
        .ok_or_else(|| anyhow!("unexpected book-of-mormon corpus shape"))?;
    let needle = normalize(&book_query);
    let book = books
        .iter()
        .find(|b| {
            let name = b.get("book").and_then(Json::as_str).unwrap_or("");
            let nn = normalize(name);
            nn == needle || nn.starts_with(&needle)
        })
        .ok_or_else(|| anyhow!("no Book of Mormon book matching `{book_query}`"))?;
    let book_name = book.get("book").and_then(Json::as_str).unwrap_or("").to_string();
    let chapters = book.get("chapters").and_then(Json::as_array).cloned().unwrap_or_default();
    let ch = chapters
        .iter()
        .find(|c| c.get("chapter").and_then(Json::as_u64) == Some(chapter as u64))
        .ok_or_else(|| anyhow!("{book_name} has no chapter {chapter}"))?;
    let mut verses = Vec::new();
    for v in ch.get("verses").and_then(Json::as_array).cloned().unwrap_or_default() {
        let reference = v
            .get("reference")
            .and_then(Json::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| format!("{book_name} {chapter}"));
        let text = v.get("text").and_then(Json::as_str).unwrap_or("").trim().to_string();
        if !text.is_empty() {
            verses.push(Verse { reference, text });
        }
    }
    if verses.is_empty() {
        return Err(anyhow!("{book_name} {chapter} has no verses"));
    }
    let locus_prefix = format!("{book_name} {chapter}");
    Ok(ScripturePassage {
        work: Work::BookOfMormon,
        translation: "1830".to_string(),
        reference: locus_prefix.clone(),
        locus_prefix,
        source_url: cfg.bom_url.clone(),
        verses,
    })
}

/// Parse `<book> <chapter>` → `(book_name, chapter)`; the trailing integer is the
/// chapter (default 1), the rest is the book (may contain a leading numeral, e.g.
/// `1 Nephi`).
fn parse_bom_ref(query: &str) -> Option<(String, u32)> {
    let q = query.trim();
    if q.is_empty() {
        return None;
    }
    match q.rsplit_once(char::is_whitespace) {
        Some((book, tail)) if tail.chars().all(|c| c.is_ascii_digit()) && !tail.is_empty() => {
            Some((book.trim().to_string(), tail.parse::<u32>().unwrap_or(1).max(1)))
        }
        _ => Some((q.to_string(), 1)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_parse_and_cite_keys() {
        assert_eq!(Work::parse("bible"), Some(Work::Bible));
        assert_eq!(Work::parse("Qur'an"), Some(Work::Quran));
        assert_eq!(Work::parse("book-of-mormon"), Some(Work::BookOfMormon));
        assert_eq!(Work::parse("torah"), None);
        assert_eq!(Work::Bible.cite_key(), "bible");
        assert_eq!(Work::Quran.cite_key(), "quran");
        assert_eq!(Work::BookOfMormon.cite_key(), "book-of-mormon");
    }

    #[test]
    fn resolves_books_by_name_number_prefix_and_russian() {
        assert_eq!(resolve_book("John"), Some((43, "John")));
        assert_eq!(resolve_book("  john "), Some((43, "John")));
        assert_eq!(resolve_book("43"), Some((43, "John")));
        // Unambiguous prefix.
        assert_eq!(resolve_book("Matt"), Some((40, "Matthew")));
        assert_eq!(resolve_book("Gen"), Some((1, "Genesis")));
        // Russian Synodal name (multilingual).
        assert_eq!(resolve_book("Иоанна"), Some((43, "John")));
        assert_eq!(resolve_book("Бытие"), Some((1, "Genesis")));
        // Numbered book.
        assert_eq!(resolve_book("1 Corinthians"), Some((46, "1 Corinthians")));
        // Out of range / unknown.
        assert_eq!(resolve_book("99"), None);
        assert_eq!(resolve_book("Nephi"), None);
    }

    #[test]
    fn book_table_is_complete_and_ordered() {
        assert_eq!(BIBLE_BOOKS.len(), 66);
        for (i, (num, en, ru)) in BIBLE_BOOKS.iter().enumerate() {
            assert_eq!(*num as usize, i + 1, "book {en} out of order");
            assert!(!ru.is_empty(), "missing Russian name for {en}");
        }
    }

    #[test]
    fn parses_bible_references() {
        assert_eq!(parse_bible_ref("John 3"), Some((43, "John", 3)));
        assert_eq!(parse_bible_ref("John 3:16"), Some((43, "John", 3)));
        assert_eq!(parse_bible_ref("1 Corinthians 13"), Some((46, "1 Corinthians", 13)));
        assert_eq!(parse_bible_ref("Genesis"), Some((1, "Genesis", 1)));
        assert_eq!(parse_bible_ref("Иоанна 3"), Some((43, "John", 3)));
        assert_eq!(parse_bible_ref(""), None);
        assert_eq!(parse_bible_ref("Nowhere 5"), None);
    }

    #[test]
    fn parses_bom_references() {
        assert_eq!(parse_bom_ref("1 Nephi 3"), Some(("1 Nephi".to_string(), 3)));
        assert_eq!(parse_bom_ref("Alma 32"), Some(("Alma".to_string(), 32)));
        assert_eq!(parse_bom_ref("Moroni"), Some(("Moroni".to_string(), 1)));
        assert_eq!(parse_bom_ref(""), None);
    }

    #[test]
    fn strips_html_and_collapses_whitespace() {
        // Tags are removed; tag *content* (e.g. a Strong's number) survives — the
        // default translations (WEB/SYNOD/…) carry no such markup, so this only
        // matters for opt-in Strong's-tagged editions.
        assert_eq!(strip_html("God <br/> so  loved  the world"), "God so loved the world");
        assert_eq!(strip_html("plain text"), "plain text");
        assert_eq!(strip_html("line<br/>break"), "linebreak");
    }

    #[test]
    fn default_translations_cover_project_languages() {
        for lang in ["en", "ru", "fr", "de", "es"] {
            assert!(!default_bible_translation(lang).is_empty());
            assert!(!default_quran_edition(lang).is_empty());
        }
        assert_eq!(default_bible_translation("ru"), "SYNOD");
        assert_eq!(default_quran_edition("ru"), "ru.kuliev");
    }

    #[test]
    fn passage_body_and_bibentry() {
        let p = ScripturePassage {
            work: Work::Bible,
            translation: "WEB".into(),
            reference: "John 3".into(),
            locus_prefix: "John 3".into(),
            source_url: "https://bolls.life/x".into(),
            verses: vec![
                Verse { reference: "John 3:16".into(), text: "For God so loved…".into() },
                Verse { reference: "John 3:17".into(), text: "For God sent not…".into() },
            ],
        };
        let body = p.body_text();
        assert!(body.contains("John 3:16\tFor God so loved…"));
        assert!(body.lines().count() == 2);
        assert_eq!(p.source_name(), "The Holy Bible · John 3 (WEB)");
        assert_eq!(p.locus_hint(), "@bible[John 3:<verse>]");
        let e = p.to_bibentry();
        assert_eq!(e.key, "bible");
        assert_eq!(e.title, "The Holy Bible");
        assert!(e.note.as_deref().unwrap().contains("@bible[John 3:16]"));
        assert!(e.is_valid());
    }
}
