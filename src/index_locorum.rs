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
/// maps a cite key to its source title. Sources are sorted by title
/// (case-insensitive, key as tiebreak); loci within a source sort naturally.
pub fn build(cites: &[LocusCitation], titles: &HashMap<String, String>) -> Vec<LocorumEntry> {
    // key -> (locus-order, locus -> chapter-order)
    let mut keys_order: Vec<String> = Vec::new();
    let mut by_key: HashMap<String, (Vec<String>, HashMap<String, Vec<String>>)> = HashMap::new();

    for c in cites {
        let Some(locus) = c.locus.as_ref().map(|l| l.trim()).filter(|l| !l.is_empty()) else {
            continue;
        };
        let entry = by_key.entry(c.key.clone()).or_insert_with(|| {
            keys_order.push(c.key.clone());
            (Vec::new(), HashMap::new())
        });
        let (locus_order, chapters) = entry;
        let chs = chapters.entry(locus.to_string()).or_insert_with(|| {
            locus_order.push(locus.to_string());
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
            let mut loci: Vec<LocusRow> = locus_order
                .into_iter()
                .map(|locus| {
                    let chs = chapters.remove(&locus).unwrap_or_default();
                    LocusRow { locus, chapters: chs }
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
        let idx = build(&cites, &titles());
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
        let idx = build(&[cite("quran", Some("2:255"), "Unity")], &HashMap::new());
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
        let idx = build(&[cite("bible", Some("John 3:16"), "Grace")], &titles());
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
        assert!(build(&[cite("bible", None, "X")], &titles()).is_empty());
    }
}
