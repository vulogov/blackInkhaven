//! LOCI (1.6.18+) — the **Index Locorum** builder. Pure: given the citations
//! harvested from the manuscript (`@key[locus]` pairs, via
//! [`crate::sources::extract_cite_loci`]) and a key→source-title map (from the
//! Sources book), it groups every cited *locus* under its source, sorts sources by
//! title and loci naturally (numeric-aware, so `3:2` precedes `3:16`), and renders
//! Typst / Markdown / JSON. No I/O — testable without a project.
//!
//! An index locorum ("index of places") lists every specific passage a work cites
//! across a source — the scholarly apparatus for theology, classics, and law.
//! Only citations that carry a locus contribute; a bare `@key` is a normal
//! citation and belongs in the bibliography, not here.

use std::collections::HashMap;

use crate::config::RefScheme;

/// A compiled reference scheme — the scheme name (which drives canonicalization),
/// the regex a locus must match, and the human format hint reported when it doesn't.
pub struct LocusScheme {
    name: String,
    re: regex::Regex,
    pub format: String,
}

impl LocusScheme {
    fn is_valid(&self, locus: &str) -> bool {
        self.re.is_match(locus.trim())
    }
    /// Canonicalize a locus under this scheme (built-in scripture schemes only;
    /// others return the locus unchanged).
    fn canonicalize(&self, locus: &str) -> String {
        canonicalize_locus(&self.name, locus)
    }
}

/// Canonicalize a locus under a named scheme: for the built-in scripture schemes,
/// normalize the chapter–verse separator to `:` and resolve the book to its
/// canonical English name (so `Jn 3.16`, `иоанна 3:16`, and `John 3:16` collapse);
/// every other scheme returns the locus with only whitespace collapsed.
fn canonicalize_locus(scheme_name: &str, locus: &str) -> String {
    let s: String = locus.split_whitespace().collect::<Vec<_>>().join(" ");
    match scheme_name {
        "bible" => match split_ref(&s) {
            Some((book, cv)) if !book.is_empty() => {
                match crate::research::scripture::canonical_bible_book(&book) {
                    Some(canon) => format!("{canon} {cv}"),
                    None => format!("{book} {cv}"), // separator fixed, book left as typed
                }
            }
            _ => s,
        },
        "quran" | "book-of-mormon" => match split_ref(&s) {
            Some((book, cv)) if !book.is_empty() => format!("{book} {cv}"),
            Some((_, cv)) => cv, // e.g. Qur'an "2.255" -> "2:255"
            None => s,
        },
        _ => s,
    }
}

/// Split a scripture reference into `(book, "ch:v[-r]")`. The trailing
/// whitespace-separated token must be a `chapter(:|.)verse[-range]`; `book` is
/// everything before it (empty when there is none, as in a bare `2:255`).
fn split_ref(s: &str) -> Option<(String, String)> {
    let (book, tail) = match s.rsplit_once(char::is_whitespace) {
        Some((b, t)) => (b.trim().to_string(), t),
        None => (String::new(), s),
    };
    Some((book, normalize_cv(tail)?))
}

/// `3:16` / `3.16` / `3:16-18` → `3:16` / `3:16-18`; `None` if not `digits(:|.)digits`.
fn normalize_cv(t: &str) -> Option<String> {
    let (ch, rest) = t.trim().split_once([':', '.'])?;
    let (v, range) = match rest.split_once('-') {
        Some((v, r)) => (v, Some(r)),
        None => (rest, None),
    };
    let numeric = |x: &str| !x.is_empty() && x.chars().all(|c| c.is_ascii_digit());
    if numeric(ch) && numeric(v) && range.map_or(true, numeric) {
        Some(match range {
            Some(r) => format!("{ch}:{v}-{r}"),
            None => format!("{ch}:{v}"),
        })
    } else {
        None
    }
}

/// Built-in reference schemes for the scripture-adapter keys, so their loci
/// validate with zero configuration. Book names allow Unicode letters (so the
/// Russian Synodal names validate), an optional leading `1`–`4` for numbered
/// books, and a `chapter:verse` (optionally a `-range`).
pub fn builtin_scheme(name: &str) -> Option<RefScheme> {
    let (pattern, format) = match name {
        "bible" | "book-of-mormon" => {
            (r"^([1-4]\s+)?\p{L}[\p{L}. ]*\s+\d+:\d+(-\d+)?$", "{book} {ch}:{v}")
        }
        "quran" => (r"^\d{1,3}:\d{1,3}(-\d+)?$", "{surah}:{ayah}"),
        _ => return None,
    };
    Some(RefScheme { pattern: pattern.to_string(), format: format.to_string() })
}

/// Resolve each cite key to a compiled locus scheme. `declared` maps a key to the
/// scheme *name* it declared (a source's `scheme:` field); a key with no explicit
/// declaration falls back to a built-in scheme of the same name (so scripture keys
/// validate unconfigured). `configured` is `sources.ref_schemes`. Keys with no
/// resolvable scheme are omitted (their loci are unvalidated). The returned error
/// list carries `(scheme-name, message)` for any pattern that failed to compile.
pub fn resolve_schemes(
    configured: &std::collections::BTreeMap<String, RefScheme>,
    declared: &HashMap<String, String>,
    keys: &[String],
) -> (HashMap<String, LocusScheme>, Vec<(String, String)>) {
    let mut out = HashMap::new();
    let mut errs = Vec::new();
    let mut seen_err: std::collections::HashSet<String> = std::collections::HashSet::new();
    for key in keys {
        // Explicit `scheme:` on the source, else the key itself (for built-ins).
        let name = declared.get(key).cloned().unwrap_or_else(|| key.clone());
        let Some(raw) = configured.get(&name).cloned().or_else(|| builtin_scheme(&name)) else {
            continue;
        };
        if raw.pattern.trim().is_empty() {
            continue;
        }
        match regex::Regex::new(&raw.pattern) {
            Ok(re) => {
                out.insert(key.clone(), LocusScheme { name: name.clone(), re, format: raw.format });
            }
            Err(e) => {
                if seen_err.insert(name.clone()) {
                    errs.push((name, e.to_string()));
                }
            }
        }
    }
    (out, errs)
}

/// One malformed locus — its source, the offending reference, and the scheme's
/// human format hint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Malformed {
    pub key: String,
    pub title: String,
    pub locus: String,
    pub expected: String,
}

/// Collect every locus that failed its source's scheme, for reporting.
pub fn malformed(entries: &[LocorumEntry], schemes: &HashMap<String, LocusScheme>) -> Vec<Malformed> {
    let mut out = Vec::new();
    for e in entries {
        for row in &e.loci {
            if !row.valid {
                let expected = schemes.get(&e.key).map(|s| s.format.clone()).unwrap_or_default();
                out.push(Malformed {
                    key: e.key.clone(),
                    title: e.title.clone(),
                    locus: row.locus.clone(),
                    expected,
                });
            }
        }
    }
    out
}

/// One harvested citation: a source key, its optional locus, and the chapter it
/// appears in.
#[derive(Debug, Clone)]
pub struct LocusCitation {
    pub key: String,
    pub locus: Option<String>,
    pub chapter: String,
}

/// One cited locus of a source + the chapters that cite it (first-seen order).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocusRow {
    pub locus: String,
    pub chapters: Vec<String>,
    /// Whether the locus matched its source's reference scheme (true when the
    /// source has no scheme — free-text loci are always valid).
    pub valid: bool,
}

/// One source's entry in the index locorum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocorumEntry {
    pub key: String,
    /// The resolved source title, or the key itself when unknown.
    pub title: String,
    pub loci: Vec<LocusRow>,
}

/// Build the index locorum. Only citations **with** a locus contribute. `titles`
/// maps a cite key to its source title; `schemes` maps a key to its compiled
/// reference scheme (see [`resolve_schemes`]) — a locus is marked invalid when it
/// fails its source's scheme, and a key with no scheme leaves its loci valid.
/// Sources are sorted by title (case-insensitive, key as tiebreak); loci within a
/// source sort naturally.
pub fn build(
    cites: &[LocusCitation],
    titles: &HashMap<String, String>,
    schemes: &HashMap<String, LocusScheme>,
) -> Vec<LocorumEntry> {
    // key -> (locus-order, locus -> chapter-order)
    let mut keys_order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, (Vec<String>, HashMap<String, Vec<String>>)> = HashMap::new();

    for c in cites {
        let Some(raw) = c.locus.as_ref().map(|l| l.trim()).filter(|l| !l.is_empty()) else {
            continue;
        };
        // Canonicalize under the source's scheme so variant spellings of one
        // passage (`Jn 3.16`, `John 3:16`) collapse into a single locus.
        let locus = schemes.get(&c.key).map_or_else(|| raw.to_string(), |s| s.canonicalize(raw));
        let entry = by_key.entry(c.key.clone()).or_insert_with(|| {
            keys_order.push(c.key.clone());
            (Vec::new(), HashMap::new())
        });
        let (locus_order, chapters) = entry;
        let chs = chapters.entry(locus.clone()).or_insert_with(|| {
            locus_order.push(locus.clone());
            Vec::new()
        });
        let ch = c.chapter.trim();
        if !ch.is_empty() && !chs.iter().any(|x| x == ch) {
            chs.push(ch.to_string());
        }
    }

    let mut entries: Vec<LocorumEntry> = keys_order
        .into_iter()
        .filter_map(|key| {
            let (locus_order, mut chapters) = by_key.remove(&key)?;
            let scheme = schemes.get(&key);
            let mut loci: Vec<LocusRow> = locus_order
                .into_iter()
                .map(|locus| {
                    let chs = chapters.remove(&locus).unwrap_or_default();
                    let valid = scheme.map_or(true, |s| s.is_valid(&locus));
                    LocusRow { locus, chapters: chs, valid }
                })
                .collect();
            loci.sort_by(|a, b| natural_key(&a.locus).cmp(&natural_key(&b.locus)));
            let title = titles.get(&key).cloned().unwrap_or_else(|| key.clone());
            Some(LocorumEntry { key, title, loci })
        })
        .collect();

    entries.sort_by(|a, b| {
        a.title.to_lowercase().cmp(&b.title.to_lowercase()).then_with(|| a.key.cmp(&b.key))
    });
    entries
}

/// A natural-sort key: digit runs become zero-padded (width 12) so numbers sort
/// numerically (`3:2` < `3:16` < `10:1`); other text is lowercased. Unicode-safe.
fn natural_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    let mut digits = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            digits.push(c);
        } else {
            if !digits.is_empty() {
                out.push_str(&format!("{:0>12}", digits));
                digits.clear();
            }
            out.extend(c.to_lowercase());
        }
    }
    if !digits.is_empty() {
        out.push_str(&format!("{:0>12}", digits));
    }
    out
}

/// The localized `Index Locorum` heading (the Latin term is standard in en/fr/es
/// scholarship; de/ru get native equivalents).
pub fn heading_for_language(lang: &str) -> &'static str {
    match lang.trim().to_lowercase().as_str() {
        "ru" | "russian" | "русский" => "Указатель мест",
        "de" | "german" | "deutsch" => "Stellenregister",
        // en / fr / es and any other keep the standard Latin term.
        _ => "Index Locorum",
    }
}

/// Render the index locorum as a Typst chapter (for `#include` in the built book).
pub fn render_typst(entries: &[LocorumEntry], heading: &str) -> String {
    let mut s = format!("= {heading}\n\n");
    for e in entries {
        s.push_str(&format!("== {} <indexlocorum-{}>\n\n", typst_escape(&e.title), e.key));
        for row in &e.loci {
            // A plain bullet list — colons in a locus need no escaping here
            // (unlike Typst's `/ term:` list, whose separator a locus colon breaks).
            let where_ = if row.chapters.is_empty() {
                String::new()
            } else {
                format!(" #h(1em) #text(gray)[{}]", typst_escape(&row.chapters.join(", ")))
            };
            s.push_str(&format!("- {}{}\n", typst_escape(&row.locus), where_));
        }
        s.push('\n');
    }
    s
}

/// Render as Markdown.
pub fn render_md(entries: &[LocorumEntry], heading: &str) -> String {
    let mut s = format!("# {heading}\n\n");
    for e in entries {
        s.push_str(&format!("## {} (`@{}`)\n\n", e.title, e.key));
        for row in &e.loci {
            if row.chapters.is_empty() {
                s.push_str(&format!("- {}\n", row.locus));
            } else {
                s.push_str(&format!("- {} — {}\n", row.locus, row.chapters.join(", ")));
            }
        }
        s.push('\n');
    }
    s
}

/// Render as JSON.
pub fn render_json(entries: &[LocorumEntry]) -> String {
    let arr: Vec<_> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "key": e.key,
                "title": e.title,
                "loci": e.loci.iter().map(|r| serde_json::json!({
                    "locus": r.locus,
                    "chapters": r.chapters,
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    let total: usize = entries.iter().map(|e| e.loci.len()).sum();
    serde_json::to_string_pretty(
        &serde_json::json!({ "index_locorum": arr, "sources": entries.len(), "loci": total }),
    )
    .unwrap_or_else(|_| "{}".into())
}

/// Escape the handful of Typst markup characters that would break a heading /
/// term-list line.
fn typst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '#' | '*' | '_' | '`' | '$' | '@' | '<' | '>' | '\\' | '[' | ']') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cite(key: &str, locus: Option<&str>, chapter: &str) -> LocusCitation {
        LocusCitation { key: key.into(), locus: locus.map(str::to_string), chapter: chapter.into() }
    }

    fn titles() -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("bible".into(), "The Holy Bible".into());
        m.insert("kant".into(), "Critique of Pure Reason".into());
        m
    }

    fn no_schemes() -> HashMap<String, LocusScheme> {
        HashMap::new()
    }

    /// Resolve the built-in `bible` scheme for the `bible` key (no config).
    fn bible_schemes() -> HashMap<String, LocusScheme> {
        let (s, errs) = resolve_schemes(
            &std::collections::BTreeMap::new(),
            &HashMap::new(),
            &["bible".to_string()],
        );
        assert!(errs.is_empty());
        s
    }

    #[test]
    fn groups_by_source_sorts_loci_naturally_and_dedupes_chapters() {
        let cites = vec![
            cite("bible", Some("John 3:16"), "Grace"),
            cite("bible", Some("John 3:2"), "Grace"),
            cite("bible", Some("John 3:16"), "Faith"), // same locus, new chapter
            cite("bible", Some("John 3:16"), "Grace"), // duplicate → not re-added
            cite("kant", Some("A51/B75"), "Reason"),
            cite("plato", None, "Forms"), // no locus → excluded
        ];
        let idx = build(&cites, &titles(), &no_schemes());
        assert_eq!(idx.len(), 2);
        // Sorted by title: "Critique…" (kant) before "The Holy Bible" (bible).
        assert_eq!(idx[0].key, "kant");
        assert_eq!(idx[1].key, "bible");
        let bible = &idx[1];
        // Natural sort: 3:2 before 3:16.
        assert_eq!(bible.loci[0].locus, "John 3:2");
        assert_eq!(bible.loci[1].locus, "John 3:16");
        // Chapters deduped, first-seen order.
        assert_eq!(bible.loci[1].chapters, vec!["Grace", "Faith"]);
    }

    #[test]
    fn unknown_key_falls_back_to_the_key_as_title() {
        let idx = build(&[cite("quran", Some("2:255"), "Unity")], &HashMap::new(), &no_schemes());
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].title, "quran");
    }

    #[test]
    fn natural_key_orders_numbers_numerically() {
        let mut v = vec!["10:1", "3:16", "3:2", "1:1"];
        v.sort_by(|a, b| natural_key(a).cmp(&natural_key(b)));
        assert_eq!(v, vec!["1:1", "3:2", "3:16", "10:1"]);
    }

    #[test]
    fn renders_all_three_formats() {
        let idx = build(&[cite("bible", Some("John 3:16"), "Grace")], &titles(), &no_schemes());
        let heading = heading_for_language("en");
        assert!(render_typst(&idx, heading).starts_with("= Index Locorum"));
        assert!(render_md(&idx, heading).contains("## The Holy Bible (`@bible`)"));
        let json = render_json(&idx);
        assert!(json.contains("\"loci\": 1"));
        assert!(json.contains("John 3:16"));
    }

    #[test]
    fn heading_localizes() {
        assert_eq!(heading_for_language("ru"), "Указатель мест");
        assert_eq!(heading_for_language("de"), "Stellenregister");
        assert_eq!(heading_for_language("fr"), "Index Locorum");
        assert_eq!(heading_for_language("en"), "Index Locorum");
    }

    #[test]
    fn empty_when_no_loci() {
        assert!(build(&[cite("bible", None, "X")], &titles(), &no_schemes()).is_empty());
    }

    #[test]
    fn builtin_bible_scheme_validates_and_flags() {
        let cites = vec![
            cite("bible", Some("John 3:16"), "I"),      // valid
            cite("bible", Some("1 Corinthians 13:4"), "I"), // numbered book, valid
            cite("bible", Some("John 3:sixteen"), "I"), // malformed
            cite("bible", Some("John 3"), "I"),         // missing verse, malformed
        ];
        let idx = build(&cites, &titles(), &bible_schemes());
        let bible = &idx[0];
        let valid: Vec<&str> = bible.loci.iter().filter(|r| r.valid).map(|r| r.locus.as_str()).collect();
        let bad: Vec<&str> = bible.loci.iter().filter(|r| !r.valid).map(|r| r.locus.as_str()).collect();
        assert!(valid.contains(&"John 3:16") && valid.contains(&"1 Corinthians 13:4"));
        assert!(bad.contains(&"John 3:sixteen") && bad.contains(&"John 3"));
    }

    #[test]
    fn malformed_reports_with_expected_format() {
        let idx = build(&[cite("bible", Some("John 3:sixteen"), "I")], &titles(), &bible_schemes());
        let bad = malformed(&idx, &bible_schemes());
        assert_eq!(bad.len(), 1);
        assert_eq!(bad[0].key, "bible");
        assert_eq!(bad[0].locus, "John 3:sixteen");
        assert_eq!(bad[0].expected, "{book} {ch}:{v}");
    }

    #[test]
    fn quran_scheme_matches_surah_ayah_only() {
        let (schemes, errs) = resolve_schemes(
            &std::collections::BTreeMap::new(),
            &HashMap::new(),
            &["quran".to_string()],
        );
        assert!(errs.is_empty());
        let idx = build(
            &[cite("quran", Some("2:255"), "I"), cite("quran", Some("Al-Baqara 255"), "I")],
            &HashMap::new(),
            &schemes,
        );
        let q = &idx[0];
        assert!(q.loci.iter().find(|r| r.locus == "2:255").unwrap().valid);
        assert!(!q.loci.iter().find(|r| r.locus == "Al-Baqara 255").unwrap().valid);
    }

    #[test]
    fn configured_scheme_via_declared_name() {
        // A source keyed `kant-cpr` declares it uses the user-defined `kant-ab` scheme.
        let mut cfg = std::collections::BTreeMap::new();
        cfg.insert(
            "kant-ab".to_string(),
            RefScheme { pattern: r"^A\d+(/B\d+)?$".into(), format: "A{n}/B{n}".into() },
        );
        let mut declared = HashMap::new();
        declared.insert("kant-cpr".to_string(), "kant-ab".to_string());
        let (schemes, errs) = resolve_schemes(&cfg, &declared, &["kant-cpr".to_string()]);
        assert!(errs.is_empty());
        let idx = build(
            &[cite("kant-cpr", Some("A51/B75"), "I"), cite("kant-cpr", Some("Ak. 5:122"), "I")],
            &HashMap::new(),
            &schemes,
        );
        let k = &idx[0];
        assert!(k.loci.iter().find(|r| r.locus == "A51/B75").unwrap().valid);
        // The Akademie citation does not match the A/B scheme.
        assert!(!k.loci.iter().find(|r| r.locus == "Ak. 5:122").unwrap().valid);
    }

    #[test]
    fn canonicalizes_and_merges_bible_variants() {
        let cites = vec![
            cite("bible", Some("John 3:16"), "I"),
            cite("bible", Some("Jn 3.16"), "II"),      // separator + (unknown) abbrev
            cite("bible", Some("иоанна 3:16"), "III"), // Russian → English, same passage
            cite("bible", Some("1 cor 13.4"), "IV"),   // abbrev + separator → 1 Corinthians 13:4
        ];
        let idx = build(&cites, &titles(), &bible_schemes());
        let bible = &idx[0];
        let loci: Vec<&str> = bible.loci.iter().map(|r| r.locus.as_str()).collect();
        // "John 3:16" and "иоанна 3:16" both canonicalize to "John 3:16" and merge,
        // carrying both chapters; "1 cor 13.4" → "1 Corinthians 13:4".
        let john = bible.loci.iter().find(|r| r.locus == "John 3:16").expect("John 3:16 row");
        assert!(john.chapters.contains(&"I".to_string()) && john.chapters.contains(&"III".to_string()));
        assert!(loci.contains(&"1 Corinthians 13:4"));
        // "Jn" is not resolvable by the table (2 chars) — the separator is still
        // fixed, but the abbreviation stays.
        assert!(loci.contains(&"Jn 3:16"));
        // All canonicalized forms are valid under the scheme.
        assert!(bible.loci.iter().all(|r| r.valid));
    }

    #[test]
    fn canonicalizes_quran_separator() {
        let idx = build(
            &[cite("quran", Some("2.255"), "I"), cite("quran", Some("2:255"), "II")],
            &HashMap::new(),
            &{
                let (s, _) = resolve_schemes(&std::collections::BTreeMap::new(), &HashMap::new(), &["quran".into()]);
                s
            },
        );
        // "2.255" and "2:255" both canonicalize to "2:255" → one row, two chapters.
        assert_eq!(idx[0].loci.len(), 1);
        assert_eq!(idx[0].loci[0].locus, "2:255");
        assert_eq!(idx[0].loci[0].chapters, vec!["I", "II"]);
    }

    #[test]
    fn no_scheme_leaves_loci_verbatim() {
        // Without a scheme, a Charaka-style `.` locus is untouched (no scripture
        // canonicalization mangles a tradition that legitimately uses dots).
        let idx = build(&[cite("charaka", Some("Sutra 1.24"), "I")], &HashMap::new(), &no_schemes());
        assert_eq!(idx[0].loci[0].locus, "Sutra 1.24");
    }

    #[test]
    fn no_scheme_leaves_loci_valid_and_bad_pattern_is_reported() {
        // Unknown key → no scheme → free-text, always valid.
        let idx = build(&[cite("plato", Some("Republic 514a"), "I")], &HashMap::new(), &no_schemes());
        assert!(idx[0].loci[0].valid);
        // A broken regex is reported, not panicked.
        let mut cfg = std::collections::BTreeMap::new();
        cfg.insert("broken".to_string(), RefScheme { pattern: "(".into(), format: "x".into() });
        let mut declared = HashMap::new();
        declared.insert("src".to_string(), "broken".to_string());
        let (schemes, errs) = resolve_schemes(&cfg, &declared, &["src".to_string()]);
        assert!(schemes.is_empty());
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].0, "broken");
    }
}
