//! SOURCES-1 — the bibliography & citation engine.
//!
//! Citation entries are authored as **HJSON paragraphs** in the `Sources`
//! system book (no new content type — `content_type: "hjson"` already exists).
//! At assembly each entry is parsed into [`BibEntry`] and serialised to a single
//! `sources.bib`, which Typst's `#bibliography(...)` renders. This module holds
//! the pure pieces: the schema, the parse, the BibTeX serializer, and the
//! authoring template. No I/O — testable without a project.
//!
//! S-P0 is the foundation; later phases (S-P1…S-P5) consume it. The
//! module-level `dead_code` allow covers items not yet wired; it tightens as
//! phases land.
#![allow(dead_code)]

use serde::Deserialize;

/// One citation entry. Every field is `serde(default)` so a partial or
/// in-progress HJSON paragraph still parses; an entry with an empty `key` is
/// skipped at collection time (it can't be cited). Numeric-looking values
/// (`year: 2024`, `volume: 12`) are coerced to strings — HJSON parses bare
/// numbers as numbers, but the author shouldn't have to quote them.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(default)]
pub struct BibEntry {
    /// Citation key — cited in prose as `@smith2024`. Required (empty → skipped).
    #[serde(deserialize_with = "de_string")]
    pub key: String,
    /// BibTeX entry type — `article` | `book` | `misc` | `online` | … Defaults
    /// to `misc` at serialize time when empty.
    #[serde(deserialize_with = "de_string")]
    pub entry_type: String,
    #[serde(deserialize_with = "de_string")]
    pub author: String,
    #[serde(deserialize_with = "de_string")]
    pub title: String,
    #[serde(deserialize_with = "de_string")]
    pub year: String,
    // ── optional fields — absent → omitted from the .bib output ──
    #[serde(deserialize_with = "de_opt_string")]
    pub journal: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub volume: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub number: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub pages: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub publisher: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub booktitle: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub editor: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub edition: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub url: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub doi: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub isbn: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub note: Option<String>,
    /// HJSON field name `abstract` (a Rust keyword).
    #[serde(rename = "abstract", deserialize_with = "de_opt_string")]
    pub abstract_: Option<String>,
    #[serde(deserialize_with = "de_opt_string")]
    pub keywords: Option<String>,
}

/// A serde visitor that coerces any scalar (string / number / bool) to a
/// `String` — so unquoted `year: 2024` works.
struct ScalarString;
impl serde::de::Visitor<'_> for ScalarString {
    type Value = String;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a string, number, or boolean")
    }
    fn visit_str<E>(self, v: &str) -> Result<String, E> {
        Ok(v.to_string())
    }
    fn visit_string<E>(self, v: String) -> Result<String, E> {
        Ok(v)
    }
    fn visit_i64<E>(self, v: i64) -> Result<String, E> {
        Ok(v.to_string())
    }
    fn visit_u64<E>(self, v: u64) -> Result<String, E> {
        Ok(v.to_string())
    }
    fn visit_f64<E>(self, v: f64) -> Result<String, E> {
        Ok(v.to_string())
    }
    fn visit_bool<E>(self, v: bool) -> Result<String, E> {
        Ok(v.to_string())
    }
}

fn de_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    d.deserialize_any(ScalarString)
}

fn de_opt_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    // Present field → coerce its scalar; an explicit `null` deserializes here as
    // a unit, which `deserialize_any` routes to a None via the option wrapper.
    Ok(Some(d.deserialize_any(ScalarString)?))
}

impl BibEntry {
    /// Parse one HJSON paragraph body into an entry. Tolerant — unknown fields
    /// are ignored (forward-compatible), and a malformed body yields `None`.
    pub fn from_hjson(body: &str) -> Option<BibEntry> {
        serde_hjson::from_str::<BibEntry>(body).ok()
    }

    /// Whether this entry is usable (has a non-empty key).
    pub fn is_valid(&self) -> bool {
        !self.key.trim().is_empty()
    }

    /// Serialise to a BibTeX entry. Fields with empty/absent values are omitted;
    /// the entry type defaults to `misc` when blank.
    pub fn to_bibtex(&self) -> String {
        let entry_type = {
            let t = self.entry_type.trim();
            if t.is_empty() { "misc" } else { t }
        };
        let mut lines: Vec<String> = Vec::new();
        let mut push = |name: &str, value: &str| {
            let v = value.trim();
            if !v.is_empty() {
                lines.push(format!("  {name} = {{{v}}}"));
            }
        };
        push("author", &self.author);
        push("title", &self.title);
        push("year", &self.year);
        push("journal", self.journal.as_deref().unwrap_or(""));
        push("volume", self.volume.as_deref().unwrap_or(""));
        push("number", self.number.as_deref().unwrap_or(""));
        push("pages", self.pages.as_deref().unwrap_or(""));
        push("publisher", self.publisher.as_deref().unwrap_or(""));
        push("booktitle", self.booktitle.as_deref().unwrap_or(""));
        push("editor", self.editor.as_deref().unwrap_or(""));
        push("edition", self.edition.as_deref().unwrap_or(""));
        push("url", self.url.as_deref().unwrap_or(""));
        push("doi", self.doi.as_deref().unwrap_or(""));
        push("isbn", self.isbn.as_deref().unwrap_or(""));
        push("note", self.note.as_deref().unwrap_or(""));
        push("abstract", self.abstract_.as_deref().unwrap_or(""));
        push("keywords", self.keywords.as_deref().unwrap_or(""));
        format!("@{entry_type}{{{key},\n{body}\n}}\n", key = self.key.trim(), body = lines.join(",\n"))
    }
}

/// Compile a list of entries into one `sources.bib` string, skipping invalid
/// (keyless) entries. Returns the count of entries emitted alongside the text.
pub fn compile_bibtex(entries: &[BibEntry]) -> (String, usize) {
    let valid: Vec<&BibEntry> = entries.iter().filter(|e| e.is_valid()).collect();
    let body = valid.iter().map(|e| e.to_bibtex()).collect::<Vec<_>>().join("\n");
    (body, valid.len())
}

/// The HJSON template seeded into a freshly-created Sources paragraph.
// NOTE: HJSON unquoted strings run to end-of-line, so an inline `// …` after a
// value becomes PART of the value. Keep comments on their own line.
pub const ENTRY_TEMPLATE: &str = "{
  // Citation key — insert in prose as @smith2024
  key: change-me
  // entry_type: article | book | misc | online | inproceedings | …
  entry_type: article
  author: Last, First
  title: Title of the work
  year: 2024
  // Optional — delete unused fields:
  // journal: Journal Name
  // volume: 1
  // number: 2
  // pages: 10-20
  // publisher: Publisher Name
  // url: https://example.com
  // doi: 10.xxxx/xxxxx
  // note: Additional note
}
";

/// Derive a citation key from a free-text paragraph title. Keeps ASCII
/// alphanumerics plus `_ : -`, lowercases, and must begin with a letter (the
/// `@([a-zA-Z][a-zA-Z0-9_:-]*)` cite-token grammar). Falls back to the template
/// placeholder when nothing usable survives (e.g. a CJK-only title).
fn slugify_key(title: &str) -> String {
    let mut key = String::new();
    for ch in title.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | ':' | '-') {
            key.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() {
            // collapse runs of separators — drop them, keys are unspaced
            continue;
        }
    }
    // Strip any leading non-letters so the key matches the cite grammar.
    while key.chars().next().is_some_and(|c| !c.is_ascii_alphabetic()) {
        key.remove(0);
    }
    if key.is_empty() { "change-me".to_string() } else { key }
}

/// Seed body for a citation paragraph created in the TUI under the Sources
/// book. The typed paragraph title becomes the citation `key`; the rest of the
/// authoring template is preserved verbatim. Mirrors
/// `cli::thread::seed_thread_body_for_tui`.
pub fn seed_sources_body_for_tui(title: &str) -> String {
    let key = slugify_key(title);
    ENTRY_TEMPLATE.replacen("key: change-me", &format!("key: {key}"), 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    // HJSON entries are multi-line (one field per line) — unquoted strings run
    // to end-of-line, which is exactly what makes them comfortable to author.

    #[test]
    fn parses_and_serialises_a_full_entry() {
        let body = "{\n  key: smith2024\n  entry_type: article\n  author: Smith, Jane\n  \
                     title: On Things\n  year: 2024\n  journal: J. Things\n  volume: 12\n  \
                     pages: 1-9\n}";
        let e = BibEntry::from_hjson(body).expect("parses");
        assert!(e.is_valid());
        let bib = e.to_bibtex();
        assert!(bib.starts_with("@article{smith2024,"), "{bib}");
        assert!(bib.contains("author = {Smith, Jane}"));
        assert!(bib.contains("journal = {J. Things}"));
        assert!(bib.contains("volume = {12}"));
        // Absent optionals are omitted.
        assert!(!bib.contains("doi"));
        assert!(bib.trim_end().ends_with('}'));
    }

    #[test]
    fn partial_entry_omits_missing_fields_and_defaults_type() {
        // No entry_type, only key + title.
        let e = BibEntry::from_hjson("{\n  key: k1\n  title: Untyped\n}").unwrap();
        let bib = e.to_bibtex();
        assert!(bib.starts_with("@misc{k1,"), "blank type → misc: {bib}");
        assert!(bib.contains("title = {Untyped}"));
        assert!(!bib.contains("author"));
        assert!(!bib.contains("year"));
    }

    #[test]
    fn empty_key_is_invalid_and_skipped_by_compile() {
        let keyless = BibEntry::from_hjson("{\n  title: No Key\n}").unwrap();
        assert!(!keyless.is_valid());
        let ok = BibEntry::from_hjson("{\n  key: real\n  title: T\n}").unwrap();
        let (text, n) = compile_bibtex(&[keyless, ok]);
        assert_eq!(n, 1);
        assert!(text.contains("@misc{real,"));
        assert!(!text.contains("No Key"));
    }

    #[test]
    fn unknown_fields_are_tolerated() {
        // A field SOURCES-2 might add; must not break parsing.
        let e = BibEntry::from_hjson("{\n  key: k\n  title: T\n  some_future_field: x\n}").unwrap();
        assert_eq!(e.key, "k");
    }

    #[test]
    fn unicode_author_survives() {
        let e = BibEntry::from_hjson("{\n  key: u\n  author: Ulánov, Владимир\n  title: Т\n}").unwrap();
        assert!(e.to_bibtex().contains("Ulánov, Владимир"));
    }

    #[test]
    fn abstract_field_is_renamed() {
        let e = BibEntry::from_hjson("{\n  key: a\n  title: T\n  abstract: a summary\n}").unwrap();
        assert_eq!(e.abstract_.as_deref(), Some("a summary"));
        assert!(e.to_bibtex().contains("abstract = {a summary}"));
    }

    #[test]
    fn the_seed_template_parses() {
        // The bundled authoring template must itself be valid HJSON.
        let e = BibEntry::from_hjson(ENTRY_TEMPLATE).expect("template parses");
        assert_eq!(e.key, "change-me");
        assert_eq!(e.entry_type, "article");
    }

    #[test]
    fn tui_seed_uses_title_as_key_and_stays_valid() {
        let body = seed_sources_body_for_tui("Smith 2024");
        let e = BibEntry::from_hjson(&body).expect("seeded body parses");
        assert_eq!(e.key, "smith2024");
        assert_eq!(e.entry_type, "article");
    }

    #[test]
    fn slugify_key_handles_edge_cases() {
        assert_eq!(slugify_key("Smith, Jane 2024"), "smithjane2024");
        assert_eq!(slugify_key("doe:2023"), "doe:2023");
        // Leading digits are stripped (must start with a letter)…
        assert_eq!(slugify_key("2024 review"), "review");
        // …and a non-Latin-only title falls back to the placeholder.
        assert_eq!(slugify_key("Влади"), "change-me");
    }
}
