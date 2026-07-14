//! LEXICON (1.6.21+) — the **Index Verborum** builder. The term-level twin of the
//! Index Locorum: given the scholarly-lexicon terms (from the Glossary — each with
//! its original-language forms and distinct senses) and where each appears in the
//! manuscript, it renders the apparatus a critical edition carries at the back —
//! every key term, its source-language form, its senses, and the chapters that use
//! it. Pure: no I/O, testable without a project.
//!
//! Only lexicon terms that actually appear in the prose contribute — like the Index
//! Locorum, it cannot flatter you with a term you defined but never used.

/// One declared sense of a term (label + gloss), flattened for the index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenseRow {
    pub label: String,
    pub gloss: String,
}

/// One scholarly-lexicon term to index.
#[derive(Debug, Clone)]
pub struct LexTerm {
    pub term: String,
    pub original_forms: Vec<String>,
    pub senses: Vec<SenseRow>,
}

/// One harvested use of a term: the canonical term and the chapter it appears in.
#[derive(Debug, Clone)]
pub struct TermUsage {
    pub term: String,
    pub chapter: String,
}

/// One term's entry in the index verborum.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbumEntry {
    pub term: String,
    pub original_forms: Vec<String>,
    pub senses: Vec<SenseRow>,
    /// The chapters that use the term (first-seen order, deduped).
    pub chapters: Vec<String>,
}

/// Build the index verborum. A lexicon term with no recorded usage is dropped.
/// Entries are sorted by term (case-insensitive); chapters keep first-seen order.
pub fn build(lexicon: &[LexTerm], usages: &[TermUsage]) -> Vec<VerbumEntry> {
    use std::collections::HashMap;
    // term -> chapters (first-seen order, deduped)
    let mut chapters: HashMap<&str, Vec<String>> = HashMap::new();
    for u in usages {
        let ch = u.chapter.trim();
        if ch.is_empty() {
            continue;
        }
        let list = chapters.entry(u.term.as_str()).or_default();
        if !list.iter().any(|c| c == ch) {
            list.push(ch.to_string());
        }
    }

    let mut entries: Vec<VerbumEntry> = lexicon
        .iter()
        .filter_map(|lt| {
            let chs = chapters.get(lt.term.as_str())?;
            if chs.is_empty() {
                return None;
            }
            Some(VerbumEntry {
                term: lt.term.clone(),
                original_forms: lt.original_forms.clone(),
                senses: lt.senses.clone(),
                chapters: chs.clone(),
            })
        })
        .collect();
    entries.sort_by(|a, b| a.term.to_lowercase().cmp(&b.term.to_lowercase()));
    entries
}

/// The localized `Index Verborum` heading (the Latin term is standard in en/fr/es
/// scholarship; de/ru get native equivalents).
pub fn heading_for_language(lang: &str) -> &'static str {
    match lang.trim().to_lowercase().as_str() {
        "ru" | "russian" | "русский" => "Указатель терминов",
        "de" | "german" | "deutsch" => "Wortregister",
        _ => "Index Verborum",
    }
}

/// Render as a Typst chapter (for `#include` in the built book).
pub fn render_typst(entries: &[VerbumEntry], heading: &str) -> String {
    let mut s = format!("= {heading}\n\n");
    for e in entries {
        let forms = if e.original_forms.is_empty() {
            String::new()
        } else {
            format!(" #h(0.6em) #text(style: \"italic\")[{}]", typst_escape(&e.original_forms.join(", ")))
        };
        s.push_str(&format!("== {}{forms} <indexverborum-{}>\n\n", typst_escape(&e.term), slug(&e.term)));
        for sr in &e.senses {
            let label = if sr.label.trim().is_empty() {
                String::new()
            } else {
                format!(" *{}* — ", typst_escape(sr.label.trim()))
            };
            s.push_str(&format!("+ {label}{}\n", typst_escape(sr.gloss.trim())));
        }
        if !e.chapters.is_empty() {
            s.push_str(&format!(
                "\n#text(gray)[{}]\n",
                typst_escape(&e.chapters.join(", "))
            ));
        }
        s.push('\n');
    }
    s
}

/// Render as Markdown.
pub fn render_md(entries: &[VerbumEntry], heading: &str) -> String {
    let mut s = format!("# {heading}\n\n");
    for e in entries {
        let forms = if e.original_forms.is_empty() {
            String::new()
        } else {
            format!(" — *{}*", e.original_forms.join(", "))
        };
        s.push_str(&format!("## {}{forms}\n\n", e.term));
        for (i, sr) in e.senses.iter().enumerate() {
            let label = if sr.label.trim().is_empty() { String::new() } else { format!("**{}** — ", sr.label.trim()) };
            s.push_str(&format!("{}. {label}{}\n", i + 1, sr.gloss.trim()));
        }
        if !e.chapters.is_empty() {
            s.push_str(&format!("\n*Used in:* {}\n", e.chapters.join(", ")));
        }
        s.push('\n');
    }
    s
}

/// Render as JSON.
pub fn render_json(entries: &[VerbumEntry]) -> String {
    let arr: Vec<_> = entries
        .iter()
        .map(|e| {
            serde_json::json!({
                "term": e.term,
                "original_forms": e.original_forms,
                "senses": e.senses.iter().map(|s| serde_json::json!({ "label": s.label, "gloss": s.gloss })).collect::<Vec<_>>(),
                "chapters": e.chapters,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::json!({ "index_verborum": arr, "terms": entries.len() }))
        .unwrap_or_else(|_| "{}".into())
}

/// A safe Typst label slug for a term (`[A-Za-z0-9_-]`).
fn slug(term: &str) -> String {
    term.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect()
}

/// Escape the Typst markup characters that would break a heading / list line.
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

    fn lt(term: &str, forms: &[&str], senses: &[(&str, &str)]) -> LexTerm {
        LexTerm {
            term: term.into(),
            original_forms: forms.iter().map(|s| s.to_string()).collect(),
            senses: senses.iter().map(|(l, g)| SenseRow { label: l.to_string(), gloss: g.to_string() }).collect(),
        }
    }
    fn use_(term: &str, chapter: &str) -> TermUsage {
        TermUsage { term: term.into(), chapter: chapter.into() }
    }

    #[test]
    fn indexes_used_terms_with_forms_senses_and_chapters() {
        let lexicon = vec![
            lt("reason", &["Vernunft"], &[("Vernunft", "the unconditioned"), ("Verstand", "concepts")]),
            lt("grace", &["gratia"], &[("charis", "unmerited favor")]),
        ];
        let usages = vec![
            use_("reason", "On Reason"),
            use_("reason", "On Freedom"),
            use_("reason", "On Reason"), // dup chapter → not re-added
            // "grace" never used → dropped
        ];
        let idx = build(&lexicon, &usages);
        assert_eq!(idx.len(), 1);
        assert_eq!(idx[0].term, "reason");
        assert_eq!(idx[0].original_forms, vec!["Vernunft"]);
        assert_eq!(idx[0].senses.len(), 2);
        assert_eq!(idx[0].chapters, vec!["On Reason", "On Freedom"]);
    }

    #[test]
    fn sorts_by_term_and_drops_unused() {
        let lexicon = vec![
            lt("being", &[], &[("existence", "that it is")]),
            lt("act", &[], &[("energeia", "actuality")]),
            lt("unused", &[], &[]),
        ];
        let usages = vec![use_("being", "II"), use_("act", "I")];
        let idx = build(&lexicon, &usages);
        assert_eq!(idx.iter().map(|e| e.term.as_str()).collect::<Vec<_>>(), vec!["act", "being"]);
    }

    #[test]
    fn renders_all_formats() {
        let idx = build(
            &[lt("reason", &["Vernunft"], &[("Vernunft", "the unconditioned")])],
            &[use_("reason", "On Reason")],
        );
        let h = heading_for_language("en");
        assert!(render_typst(&idx, h).starts_with("= Index Verborum"));
        assert!(render_md(&idx, h).contains("## reason — *Vernunft*"));
        let json = render_json(&idx);
        assert!(json.contains("\"terms\": 1") && json.contains("Vernunft"));
    }

    #[test]
    fn heading_localizes() {
        assert_eq!(heading_for_language("ru"), "Указатель терминов");
        assert_eq!(heading_for_language("de"), "Wortregister");
        assert_eq!(heading_for_language("fr"), "Index Verborum");
        assert_eq!(heading_for_language("en"), "Index Verborum");
    }
}
