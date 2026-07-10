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

pub mod coverage;

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

/// Like [`ScalarString`] but for `Option<String>` — a present scalar coerces to
/// `Some`, and an explicit `null` (unit / none) yields `None` instead of erroring
/// the whole entry.
struct OptScalarString;
impl<'de> serde::de::Visitor<'de> for OptScalarString {
    type Value = Option<String>;
    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a string, number, boolean, or null")
    }
    fn visit_unit<E>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_none<E>(self) -> Result<Option<String>, E> {
        Ok(None)
    }
    fn visit_some<D: serde::Deserializer<'de>>(self, d: D) -> Result<Option<String>, D::Error> {
        Ok(Some(d.deserialize_any(ScalarString)?))
    }
    fn visit_str<E>(self, v: &str) -> Result<Option<String>, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_string<E>(self, v: String) -> Result<Option<String>, E> {
        Ok(Some(v))
    }
    fn visit_i64<E>(self, v: i64) -> Result<Option<String>, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_u64<E>(self, v: u64) -> Result<Option<String>, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_f64<E>(self, v: f64) -> Result<Option<String>, E> {
        Ok(Some(v.to_string()))
    }
    fn visit_bool<E>(self, v: bool) -> Result<Option<String>, E> {
        Ok(Some(v.to_string()))
    }
}

fn de_opt_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    d.deserialize_any(OptScalarString)
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

    /// Serialise to an HJSON paragraph body (the on-disk citation format).
    /// Values are quoted + escaped so any content round-trips through
    /// [`BibEntry::from_hjson`] regardless of commas, colons, or `#`. Empty
    /// fields are omitted. Used by `sources import` to materialise paragraphs.
    pub fn to_hjson(&self) -> String {
        let mut out = String::from("{\n");
        let mut push = |name: &str, value: &str| {
            let v = value.trim();
            if !v.is_empty() {
                out.push_str(&format!("  {name}: {}\n", hjson_quote(v)));
            }
        };
        let entry_type = {
            let t = self.entry_type.trim();
            if t.is_empty() { "misc" } else { t }
        };
        push("key", &self.key);
        push("entry_type", entry_type);
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
        out.push_str("}\n");
        out
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
                // Escape braces — an unbalanced `}` in a field value (common in
                // messy imported .bib) would otherwise truncate the field and
                // corrupt every entry that follows, breaking the Typst build.
                // (Braces only: don't touch backslashes, which carry LaTeX accents.)
                let v = v.replace('{', "\\{").replace('}', "\\}");
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

/// SOURCES-2 — serialize entries to a CSL-JSON array (the inverse of the Research
/// Assistant's CSL-JSON import), for interop with Zotero / citation managers.
/// Returns `(json, count)`; the field/type mapping round-trips with the importer.
pub fn compile_csl_json(entries: &[BibEntry]) -> (String, usize) {
    use serde_json::{json, Map, Value};
    let valid: Vec<&BibEntry> = entries.iter().filter(|e| e.is_valid()).collect();
    let opt = |m: &mut Map<String, Value>, k: &str, v: &Option<String>| {
        if let Some(s) = v {
            if !s.trim().is_empty() {
                m.insert(k.into(), json!(s));
            }
        }
    };
    let items: Vec<Value> = valid
        .iter()
        .map(|e| {
            let mut m = Map::new();
            m.insert("id".into(), json!(e.key));
            m.insert(
                "type".into(),
                json!(match e.entry_type.as_str() {
                    "article" => "article-journal",
                    "book" => "book",
                    "incollection" => "chapter",
                    "inproceedings" => "paper-conference",
                    "phdthesis" | "mastersthesis" => "thesis",
                    _ => "document",
                }),
            );
            if !e.title.trim().is_empty() {
                m.insert("title".into(), json!(e.title));
            }
            let authors: Vec<Value> = e
                .author
                .split(" and ")
                .filter(|a| !a.trim().is_empty())
                .map(|a| match a.split_once(',') {
                    Some((fam, giv)) => json!({ "family": fam.trim(), "given": giv.trim() }),
                    None => json!({ "literal": a.trim() }),
                })
                .collect();
            if !authors.is_empty() {
                m.insert("author".into(), json!(authors));
            }
            if let Ok(y) = e.year.trim().parse::<i64>() {
                m.insert("issued".into(), json!({ "date-parts": [[y]] }));
            } else if !e.year.trim().is_empty() {
                m.insert("issued".into(), json!({ "literal": e.year }));
            }
            opt(&mut m, "container-title", &e.journal);
            opt(&mut m, "volume", &e.volume);
            opt(&mut m, "issue", &e.number);
            opt(&mut m, "page", &e.pages);
            opt(&mut m, "publisher", &e.publisher);
            opt(&mut m, "URL", &e.url);
            opt(&mut m, "DOI", &e.doi);
            opt(&mut m, "ISBN", &e.isbn);
            opt(&mut m, "note", &e.note);
            opt(&mut m, "edition", &e.edition);
            Value::Object(m)
        })
        .collect();
    let json = serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string());
    (json, valid.len())
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

/// Quote + escape a value for an HJSON string literal (`"…"`). Backslash and
/// double-quote are escaped; control chars are collapsed to spaces (values are
/// whitespace-normalised before this, so this is belt-and-braces).
fn hjson_quote(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('"');
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' | '\r' | '\t' => out.push(' '),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

/// Collapse internal whitespace runs (BibTeX values routinely span lines) to a
/// single space and trim the ends.
fn normalize_ws(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Extract Typst-style `@key` citation tokens from prose. A key starts with an
/// ASCII letter and continues with `[A-Za-z0-9_:-]`. The `@` must not be
/// preceded by an alphanumeric (so e-mail addresses like `a@b.com` are NOT
/// matched). Returns keys in first-seen order, with duplicates preserved (the
/// caller dedups as needed — call sites want per-occurrence reporting).
pub fn extract_cite_keys(prose: &str) -> Vec<String> {
    let chars: Vec<char> = prose.chars().collect();
    let mut keys = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '@' {
            let prev_ok = i == 0 || !chars[i - 1].is_alphanumeric();
            let starts_letter =
                i + 1 < chars.len() && chars[i + 1].is_ascii_alphabetic();
            if prev_ok && starts_letter {
                let mut j = i + 1;
                let mut key = String::new();
                while j < chars.len()
                    && (chars[j].is_ascii_alphanumeric()
                        || matches!(chars[j], '_' | ':' | '-'))
                {
                    key.push(chars[j]);
                    j += 1;
                }
                keys.push(key);
                i = j;
                continue;
            }
        }
        i += 1;
    }
    keys
}

/// A minimal, dependency-free BibTeX reader. Handles `@type{key, field = …}`
/// with brace-`{…}`, quote-`"…"`, and bare (numeric/word) values; nested
/// braces are balanced; `@comment` / `@string` / `@preamble` blocks are
/// skipped. String macros (`field = abbrev # " more"`) are NOT expanded — they
/// pass through as the literal token. Good enough to import real-world `.bib`
/// files exported by Zotero, JabRef, Google Scholar, etc.
pub fn parse_bibtex(input: &str) -> Vec<BibEntry> {
    let chars: Vec<char> = input.chars().collect();
    let mut entries = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '@' {
            i += 1;
            continue;
        }
        // entry type: the letters right after '@'.
        let mut j = i + 1;
        let mut etype = String::new();
        while j < chars.len() && chars[j].is_ascii_alphabetic() {
            etype.push(chars[j]);
            j += 1;
        }
        // Tolerate whitespace between the type and the opening delimiter
        // (`@article\n{key,…}` and `@article {key,…}` are valid BibTeX).
        while j < chars.len() && chars[j].is_whitespace() {
            j += 1;
        }
        if etype.is_empty() || j >= chars.len() || (chars[j] != '{' && chars[j] != '(') {
            i += 1;
            continue;
        }
        let open = chars[j];
        let close = if open == '{' { '}' } else { ')' };
        j += 1;
        let start = j;
        let mut depth = 1;
        while j < chars.len() && depth > 0 {
            if chars[j] == open {
                depth += 1;
            } else if chars[j] == close {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            j += 1;
        }
        let inner: String = chars[start..j.min(chars.len())].iter().collect();
        let etype = etype.trim().to_lowercase();
        if !matches!(etype.as_str(), "comment" | "string" | "preamble") {
            if let Some(e) = parse_bibtex_entry(&etype, &inner) {
                entries.push(e);
            }
        }
        i = j + 1;
    }
    entries
}

/// Parse the inside of one `@type{ … }` block: `key, name = value, …`.
fn parse_bibtex_entry(etype: &str, inner: &str) -> Option<BibEntry> {
    let chars: Vec<char> = inner.chars().collect();
    let mut i = 0;
    // Citation key — everything up to the first comma.
    let mut key = String::new();
    while i < chars.len() && chars[i] != ',' {
        key.push(chars[i]);
        i += 1;
    }
    let key = key.trim().to_string();
    if key.is_empty() {
        return None;
    }
    i += 1; // skip the comma

    let mut e = BibEntry {
        key,
        entry_type: etype.to_string(),
        ..Default::default()
    };

    while i < chars.len() {
        while i < chars.len() && (chars[i].is_whitespace() || chars[i] == ',') {
            i += 1;
        }
        if i >= chars.len() {
            break;
        }
        // Field name up to '='.
        let mut name = String::new();
        while i < chars.len() && chars[i] != '=' && chars[i] != ',' {
            name.push(chars[i]);
            i += 1;
        }
        if i >= chars.len() || chars[i] != '=' {
            break;
        }
        i += 1; // skip '='
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        // Value: {braced} | "quoted" | bare.
        let value = if i < chars.len() && chars[i] == '{' {
            let mut depth = 0;
            let mut v = String::new();
            while i < chars.len() {
                match chars[i] {
                    '{' => {
                        depth += 1;
                        if depth == 1 {
                            i += 1;
                            continue;
                        }
                    }
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                v.push(chars[i]);
                i += 1;
            }
            v
        } else if i < chars.len() && chars[i] == '"' {
            i += 1;
            let mut v = String::new();
            while i < chars.len() && chars[i] != '"' {
                v.push(chars[i]);
                i += 1;
            }
            if i < chars.len() {
                i += 1; // closing quote
            }
            v
        } else {
            let mut v = String::new();
            while i < chars.len() && chars[i] != ',' {
                v.push(chars[i]);
                i += 1;
            }
            v
        };
        let name = name.trim().to_lowercase();
        let value = normalize_ws(&value);
        if value.is_empty() {
            continue;
        }
        match name.as_str() {
            "author" => e.author = value,
            "title" => e.title = value,
            "year" => e.year = value,
            "date" if e.year.is_empty() => {
                // biblatex `date = {2024-01-02}` → take the year.
                e.year = value.split('-').next().unwrap_or("").to_string();
            }
            "journal" | "journaltitle" => e.journal = Some(value),
            "volume" => e.volume = Some(value),
            "number" | "issue" => e.number = Some(value),
            "pages" => e.pages = Some(value),
            "publisher" => e.publisher = Some(value),
            "booktitle" => e.booktitle = Some(value),
            "editor" => e.editor = Some(value),
            "edition" => e.edition = Some(value),
            "url" => e.url = Some(value),
            "doi" => e.doi = Some(value),
            "isbn" => e.isbn = Some(value),
            "note" => e.note = Some(value),
            "abstract" => e.abstract_ = Some(value),
            "keywords" => e.keywords = Some(value),
            _ => {}
        }
    }
    Some(e)
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
    fn to_bibtex_escapes_braces_in_values() {
        // R2: an unbalanced `}` in a field would otherwise truncate it and
        // corrupt every following entry / break the Typst build.
        let e = BibEntry::from_hjson("{\n  key: k\n  title: Repair of a } valve {x}\n}").unwrap();
        let bib = e.to_bibtex();
        assert!(bib.contains(r"title = {Repair of a \} valve \{x\}}"), "{bib}");
        // With the escaped braces removed, the structural braces balance.
        let structural = bib.replace("\\{", "").replace("\\}", "");
        assert_eq!(structural.matches('{').count(), structural.matches('}').count(), "{bib}");
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
    fn extract_cite_keys_finds_tokens_and_skips_emails() {
        let prose = "As shown by @smith2024 and @doe:2023b, but mail me at a@b.com. \
                     See also @nguyen-1999.";
        let keys = extract_cite_keys(prose);
        assert_eq!(keys, vec!["smith2024", "doe:2023b", "nguyen-1999"]);
        // `b@c` inside the email is not a citation (preceded by alnum).
        assert!(!keys.iter().any(|k| k == "b"));
    }

    #[test]
    fn extract_cite_keys_handles_unicode_prose() {
        // Russian prose with a citation — the @ must still be found.
        let keys = extract_cite_keys("Как показано в @ivanov2020, текст продолжается.");
        assert_eq!(keys, vec!["ivanov2020"]);
    }

    #[test]
    fn parse_bibtex_reads_braced_and_quoted_and_bare() {
        let bib = r#"
        @article{smith2024,
          author = {Smith, Jane and Doe, John},
          title  = "On {Nested} Braces",
          journal= {Journal of Things},
          year   = 2024,
          volume = 12,
          pages  = {1--9},
        }
        @comment{ this is ignored }
        @book{ulanov2021,
          author = {Ulánov, Владимир},
          title  = {Системы},
          year   = {2021}
        }
        "#;
        let entries = parse_bibtex(bib);
        assert_eq!(entries.len(), 2);
        let a = &entries[0];
        assert_eq!(a.key, "smith2024");
        assert_eq!(a.entry_type, "article");
        assert_eq!(a.author, "Smith, Jane and Doe, John");
        assert_eq!(a.title, "On {Nested} Braces");
        assert_eq!(a.year, "2024");
        assert_eq!(a.volume.as_deref(), Some("12"));
        assert_eq!(a.pages.as_deref(), Some("1--9"));
        let b = &entries[1];
        assert_eq!(b.key, "ulanov2021");
        assert_eq!(b.entry_type, "book");
        assert_eq!(b.author, "Ulánov, Владимир");
    }

    #[test]
    fn bibtex_import_round_trips_through_hjson() {
        // parse_bibtex → to_hjson → from_hjson must preserve the fields, even
        // with commas and colons in values.
        let bib = "@inproceedings{x:1, author = {Last, First}, title = {A, B: C}, year = 2020 }";
        let parsed = parse_bibtex(bib);
        assert_eq!(parsed.len(), 1);
        let hjson = parsed[0].to_hjson();
        let back = BibEntry::from_hjson(&hjson).expect("hjson round-trips");
        assert_eq!(back.key, "x:1");
        assert_eq!(back.entry_type, "inproceedings");
        assert_eq!(back.author, "Last, First");
        assert_eq!(back.title, "A, B: C");
        assert_eq!(back.year, "2020");
    }

    #[test]
    fn parse_bibtex_tolerates_whitespace_before_brace() {
        // A newline (or space) between the type and `{` is valid BibTeX; it used
        // to silently drop the entry.
        assert_eq!(parse_bibtex("@article\n{k, title = {T}, year = 2020}").len(), 1);
        assert_eq!(parse_bibtex("@book   {k2, title = {U}}").len(), 1);
        // A stray `@` with no type is still ignored.
        assert_eq!(parse_bibtex("email me @ home {not an entry}").len(), 0);
    }

    #[test]
    fn from_hjson_treats_null_field_as_absent() {
        // An explicit `null` used to error the whole entry; now it parses.
        let e = BibEntry::from_hjson("{\n  key: k\n  title: T\n  doi: null\n}").expect("null ok");
        assert_eq!(e.key, "k");
        assert_eq!(e.title, "T");
        assert!(e.doi.is_none());
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

    #[test]
    fn csl_json_export_shapes_entries() {
        let e = BibEntry {
            key: "smith2024".into(),
            entry_type: "article".into(),
            author: "Smith, Jane and Doe, John".into(),
            title: "A Study".into(),
            year: "2024".into(),
            journal: Some("Nature".into()),
            doi: Some("10.1/x".into()),
            ..Default::default()
        };
        let (json, count) = compile_csl_json(&[e]);
        assert_eq!(count, 1);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let item = &v[0];
        assert_eq!(item["id"], "smith2024");
        assert_eq!(item["type"], "article-journal");
        assert_eq!(item["title"], "A Study");
        assert_eq!(item["container-title"], "Nature");
        assert_eq!(item["DOI"], "10.1/x");
        assert_eq!(item["issued"]["date-parts"][0][0], 2024);
        assert_eq!(item["author"][0]["family"], "Smith");
        assert_eq!(item["author"][0]["given"], "Jane");
        assert_eq!(item["author"][1]["family"], "Doe");
    }
}
