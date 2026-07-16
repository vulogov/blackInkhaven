//! `inkhaven language export` + the dictionary/grammar/phrasebook renderers
//! (JSON, Anki, two-column Typst, CSV). Split out of the flat handler; the
//! shared loaders and string helpers live in the parent module.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::Store;

use super::*;

/// export a language's content
/// to a portable artefact.  Three formats land in
/// Phase D; `grammar` and `phrasebook` from the
/// proposal §12 are deferred to D.2.
pub(crate) fn export(
    project: &Path,
    language: &str,
    format: LanguageExportFormat,
    output: Option<&Path>,
) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout, &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?;
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found"
            ))
        })?;

    // Collect data once; per-format renderers fan
    // out from a single walk.
    let chapters = hierarchy.children_of(Some(lang_book.id));
    let mut entries: Vec<(String, crate::language_entry::DictionaryEntry)> = Vec::new();
    let mut meta: Option<crate::language_entry::MetaOverview> = None;
    let mut grammar_bodies: Vec<(String, String)> = Vec::new();
    let mut phonology_bodies: Vec<(String, String)> = Vec::new();
    let mut sample_bodies: Vec<(String, String)> = Vec::new();
    for chapter in &chapters {
        let title_lc = chapter.title.to_lowercase();
        // For Dictionary, walk the subtree (entries
        // live one level deeper, under the alphabet
        // subchapter).  For the flat chapters
        // (Grammar / Phonology / Sample texts / Meta),
        // a children_of(chapter) is enough.
        match title_lc.as_str() {
            "dictionary" => {
                for id in hierarchy.collect_subtree(chapter.id) {
                    let Some(n) = hierarchy.get(id) else { continue; };
                    if n.kind != NodeKind::Paragraph {
                        continue;
                    }
                    let Ok(Some(bytes)) = store.get_content(n.id) else { continue; };
                    let Ok(body) = std::str::from_utf8(&bytes) else { continue; };
                    if let Ok(Some(e)) = crate::language_entry::parse(body) {
                        entries.push((n.title.clone(), e));
                    }
                }
            }
            "grammar" | "phonology" | "sample texts" => {
                let bucket = match title_lc.as_str() {
                    "grammar" => &mut grammar_bodies,
                    "phonology" => &mut phonology_bodies,
                    _ => &mut sample_bodies,
                };
                for n in hierarchy
                    .children_of(Some(chapter.id))
                    .into_iter()
                    .filter(|n| n.kind == NodeKind::Paragraph)
                {
                    if let Ok(Some(bytes)) = store.get_content(n.id) {
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            bucket.push((n.title.clone(), body.to_string()));
                        }
                    }
                }
            }
            "meta" => {
                if let Some(overview) = hierarchy
                    .children_of(Some(chapter.id))
                    .into_iter()
                    .find(|n| {
                        n.kind == NodeKind::Paragraph
                            && n.title.eq_ignore_ascii_case("overview")
                    })
                {
                    if let Ok(Some(bytes)) = store.get_content(overview.id) {
                        if let Ok(body) = std::str::from_utf8(&bytes) {
                            if let Ok(Some(m)) =
                                crate::language_entry::parse_meta_overview(body)
                            {
                                meta = Some(m);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    // Sort entries by lemma so every format renders
    // in a stable order.
    entries.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));

    let rendered: Vec<u8> = match format {
        LanguageExportFormat::Json => render_json(
            &lang_book.title,
            meta.as_ref(),
            &entries,
            &grammar_bodies,
            &phonology_bodies,
            &sample_bodies,
        )?,
        LanguageExportFormat::Anki => render_anki(&entries)?,
        LanguageExportFormat::DictionaryTwocol => render_dictionary_twocol(
            &lang_book.title,
            meta.as_ref(),
            &entries,
        ),
        // 1.2.16+ Phase P.5 — three new formats.
        LanguageExportFormat::Csv => render_csv(&entries),
        LanguageExportFormat::Grammar => render_grammar(
            &lang_book.title,
            &grammar_bodies,
            &phonology_bodies,
        ),
        LanguageExportFormat::Phrasebook => render_phrasebook(
            &lang_book.title,
            &sample_bodies,
        ),
        // 1.3.19 LANG-1 P6 — interchange formats (pure renderers in
        // conlang::interchange). XLIFF keys its source language off the
        // project working language; the IPA chart needs the phoneme model.
        LanguageExportFormat::Xliff => crate::conlang::interchange::xliff(
            &lang_book.title,
            &cfg.language,
            &entries,
        )
        .into_bytes(),
        LanguageExportFormat::Linguex => {
            crate::conlang::interchange::linguex(&lang_book.title, &entries).into_bytes()
        }
        LanguageExportFormat::IpaChart => {
            let phon =
                load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
            crate::conlang::interchange::ipa_chart(&lang_book.title, &phon).into_bytes()
        }
    };

    match (output, format) {
        (Some(path), _) => {
            // 1.2.15+ Phase S.4 — atomic write so
            // an interrupted export doesn't leave
            // a half-written file.
            crate::io_atomic::write(path, &rendered).map_err(|e| {
                Error::Config(format!("write {}: {e}", path.display()))
            })?;
            eprintln!("wrote {} bytes to {}", rendered.len(), path.display());
        }
        (None, LanguageExportFormat::DictionaryTwocol)
        | (None, LanguageExportFormat::Grammar)
        | (None, LanguageExportFormat::Phrasebook) => {
            return Err(Error::Config(
                "this export format needs --output <path.typ> — \
                 the Typst renderer doesn't stream to stdout"
                    .into(),
            ));
        }
        (None, _) => {
            use std::io::Write;
            std::io::stdout()
                .write_all(&rendered)
                .map_err(|e| Error::Config(format!("stdout write: {e}")))?;
        }
    }
    Ok(())
}

pub(crate) fn render_json(
    language_name: &str,
    meta: Option<&crate::language_entry::MetaOverview>,
    entries: &[(String, crate::language_entry::DictionaryEntry)],
    grammar: &[(String, String)],
    phonology: &[(String, String)],
    samples: &[(String, String)],
) -> Result<Vec<u8>> {
    use serde_json::{json, Map, Value};
    let mut root = Map::new();
    root.insert("language".into(), Value::String(language_name.to_string()));
    if let Some(m) = meta {
        root.insert("meta".into(), json!({
            "name": m.name,
            "language_kind": m.language_kind,
            "family": m.family,
            "iso_code": m.iso_code,
            "alphabet": m.alphabet,
            "reading_direction": m.reading_direction,
            "stemmer": m.stemmer,
            "example_corpus_ref": m.example_corpus_ref,
        }));
    }
    let entries_json: Vec<Value> = entries
        .iter()
        .map(|(title, e)| {
            json!({
                "title": title,
                "word": e.word,
                "type": e.pos,
                "translation": e.translation,
                "example": e.example,
                "inflection": e.inflection,
            })
        })
        .collect();
    root.insert("dictionary".into(), Value::Array(entries_json));
    root.insert(
        "grammar".into(),
        Value::Array(
            grammar
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    root.insert(
        "phonology".into(),
        Value::Array(
            phonology
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    root.insert(
        "sample_texts".into(),
        Value::Array(
            samples
                .iter()
                .map(|(t, b)| json!({ "title": t, "body": b }))
                .collect(),
        ),
    );
    let mut buf = serde_json::to_vec_pretty(&Value::Object(root))
        .map_err(|e| Error::Config(format!("json serialise: {e}")))?;
    buf.push(b'\n');
    Ok(buf)
}

pub(crate) fn render_anki(
    entries: &[(String, crate::language_entry::DictionaryEntry)],
) -> Result<Vec<u8>> {
    // CSV columns: word, translation, type, example,
    // inflection.  Anki / SuperMemo / Mochi all parse
    // comma-separated; quoting handled by the
    // standard escape rules.  Header row included so
    // the user can map columns in the import wizard.
    let mut out = String::new();
    out.push_str("word,translation,type,example,inflection\n");
    for (_, e) in entries {
        let infl: String = e
            .inflection
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join("; ");
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            csv_field(&e.word),
            csv_field(&e.translation),
            csv_field(&e.pos),
            csv_field(&e.example),
            csv_field(&infl),
        ));
    }
    Ok(out.into_bytes())
}

/// Standard RFC 4180-style CSV quoting: wrap the
/// field in `"…"` and double any embedded `"` when
/// the field contains comma / newline / quote;
/// otherwise emit verbatim.
pub(crate) fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub(crate) fn render_dictionary_twocol(
    language_name: &str,
    meta: Option<&crate::language_entry::MetaOverview>,
    entries: &[(String, crate::language_entry::DictionaryEntry)],
) -> Vec<u8> {
    // Group entries by alphabet bucket.  Use the
    // first character of the entry's title
    // (uppercased) as the bucket key — same logic as
    // the add-word fallback.  Authors with non-
    // Latin alphabets get sensible grouping for free.
    let mut by_bucket: std::collections::BTreeMap<String, Vec<&(String, crate::language_entry::DictionaryEntry)>> =
        std::collections::BTreeMap::new();
    for entry in entries {
        let bucket = entry
            .0
            .chars()
            .find(|c| !c.is_whitespace())
            .map(|c| c.to_uppercase().to_string())
            .unwrap_or_else(|| "?".into());
        by_bucket.entry(bucket).or_default().push(entry);
    }

    let mut s = String::new();
    s.push_str(&format!("#set page(paper: \"a4\", columns: 2)\n"));
    s.push_str("#set text(font: \"New Computer Modern\", size: 10pt)\n");
    s.push_str("#set par(justify: true)\n");
    s.push('\n');
    s.push_str(&format!("#align(center)[= {} dictionary]\n", language_name));
    if let Some(m) = meta {
        if !m.language_kind.is_empty() || !m.family.is_empty() {
            s.push_str("#align(center)[#text(style: \"italic\")[");
            if !m.language_kind.is_empty() {
                s.push_str(&m.language_kind);
            }
            if !m.family.is_empty() {
                if !m.language_kind.is_empty() {
                    s.push_str(" · ");
                }
                s.push_str(&m.family);
            }
            s.push_str("]]\n");
        }
    }
    s.push('\n');
    for (bucket, group) in &by_bucket {
        s.push_str(&format!(
            "#align(center)[#text(size: 14pt, weight: \"bold\")[— {bucket} —]]\n"
        ));
        s.push('\n');
        for (title, e) in group {
            s.push_str(&format!(
                "*{title}*  #text(style: \"italic\")[{}]  {}\n",
                typst_escape(&e.pos),
                typst_escape(&e.translation),
            ));
            if !e.example.trim().is_empty() {
                s.push_str(&format!(
                    "  #pad(left: 2em)[#text(style: \"italic\")[{}]]\n",
                    typst_escape(e.example.trim()),
                ));
            }
            if !e.inflection.is_empty() {
                let pretty: Vec<String> = e
                    .inflection
                    .iter()
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect();
                s.push_str(&format!(
                    "  #pad(left: 2em)[#text(size: 8pt)[forms — {}]]\n",
                    typst_escape(&pretty.join(", ")),
                ));
            }
            s.push('\n');
        }
    }
    s.into_bytes()
}

/// Minimal Typst-content escape: `*`, `_`, `#`, `[`,
/// `]`, `\` are the only markup-bearing
/// characters in body-text context.  Sufficient for
/// dictionary-entry content; authors with
/// adversarial input (raw Typst inside translations)
/// should use the `json` format instead.
pub(crate) fn typst_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '*' | '_' | '#' | '[' | ']' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// 1.2.16+ Phase P.5 — render a dictionary as a
/// round-trip-compatible CSV that the `--import`
/// path can re-ingest.  Five columns matching the
/// in-memory `DictionaryEntry` shape: `word`,
/// `type` (pos), `translation`, `example`,
/// `inflection`.
///
/// Richer per-paragraph fields (`pronunciation`,
/// `etymology`, `related`, `register`, `era`,
/// `notes`) survive in the original HJSON
/// paragraph bodies but are not parsed into
/// `DictionaryEntry` so they don't appear here.
/// For full preservation across machines use the
/// `--format json` export (which serialises every
/// raw paragraph body verbatim) or — better —
/// `inkhaven backup` of the whole project.
pub(crate) fn render_csv(entries: &[(String, crate::language_entry::DictionaryEntry)]) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("word,type,translation,example,inflection\n");
    for (_lemma, e) in entries {
        out.push_str(&csv_field(&e.word));
        out.push(',');
        out.push_str(&csv_field(&e.pos));
        out.push(',');
        out.push_str(&csv_field(&e.translation));
        out.push(',');
        out.push_str(&csv_field(&e.example));
        out.push(',');
        out.push_str(&csv_field(&join_inflection(&e.inflection)));
        out.push('\n');
    }
    out.into_bytes()
}

pub(crate) fn join_inflection(inflection: &std::collections::BTreeMap<String, String>) -> String {
    let mut parts: Vec<String> =
        inflection.iter().map(|(k, v)| format!("{k}={v}")).collect();
    parts.sort();
    parts.join(";")
}

/// 1.2.16+ Phase P.5 — render a typst grammar
/// reference.  Walks the Grammar and Phonology
/// chapter bodies (each is HJSON-shaped); groups
/// by `category` field; emits a sectioned typst
/// document with examples tables.
pub(crate) fn render_grammar(
    language_title: &str,
    grammar_bodies: &[(String, String)],
    phonology_bodies: &[(String, String)],
) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("#set page(paper: \"a4\", margin: 2cm)\n");
    out.push_str("#set heading(numbering: \"1.\")\n");
    out.push_str("#set text(font: (\"New Computer Modern\", \"DejaVu Serif\"), size: 11pt)\n");
    out.push_str(&format!(
        "#align(center)[#text(20pt, weight: \"bold\")[{} — grammar reference]]\n\n",
        typst_escape(language_title),
    ));
    out.push_str("#outline()\n\n");
    out.push_str("#pagebreak()\n\n");

    let mut by_category: std::collections::BTreeMap<String, Vec<&(String, String)>> =
        std::collections::BTreeMap::new();
    for entry in grammar_bodies {
        let cat = extract_hjson_string_field(&entry.1, "category")
            .unwrap_or_else(|| "Uncategorised".to_string());
        by_category.entry(cat).or_default().push(entry);
    }

    out.push_str("= Grammar rules\n\n");
    for (cat, rules) in &by_category {
        out.push_str(&format!("== {}\n\n", typst_escape(cat)));
        for (title, body) in rules {
            out.push_str(&format!("=== {}\n\n", typst_escape(title)));
            if let Some(rule) = extract_hjson_string_field(body, "rule") {
                out.push_str(&format!("*Rule:* {}\n\n", typst_escape(&rule)));
            }
            if let Some(examples_block) =
                extract_hjson_examples(body)
            {
                if !examples_block.is_empty() {
                    out.push_str("*Examples:*\n\n");
                    for ex in &examples_block {
                        out.push_str(&format!("- {}\n", typst_escape(ex)));
                    }
                    out.push('\n');
                }
            }
        }
    }

    if !phonology_bodies.is_empty() {
        out.push_str("\n= Phonology rules\n\n");
        for (title, body) in phonology_bodies {
            out.push_str(&format!("== {}\n\n", typst_escape(title)));
            if let Some(rule) = extract_hjson_string_field(body, "rule") {
                out.push_str(&format!("*Rule:* {}\n\n", typst_escape(&rule)));
            }
            if let Some(pattern) = extract_hjson_string_field(body, "pattern") {
                out.push_str(&format!("*Pattern:* `{}`\n\n", pattern));
            }
        }
    }

    out.into_bytes()
}

/// 1.2.16+ Phase P.5 — render a typst phrasebook
/// from the Sample texts chapter.  Two-column
/// layout via typst's `grid`; gloss left,
/// invented-language sample right.  Sample bodies
/// are expected to contain a `gloss:` and
/// `original:` HJSON field; falls back to the
/// raw body when either is missing.
pub(crate) fn render_phrasebook(
    language_title: &str,
    sample_bodies: &[(String, String)],
) -> Vec<u8> {
    let mut out = String::new();
    out.push_str("#set page(paper: \"a4\", margin: 2cm)\n");
    out.push_str("#set text(font: (\"New Computer Modern\", \"DejaVu Serif\"), size: 11pt)\n");
    out.push_str(&format!(
        "#align(center)[#text(20pt, weight: \"bold\")[{} — phrasebook]]\n\n",
        typst_escape(language_title),
    ));
    if sample_bodies.is_empty() {
        out.push_str("_No sample texts in the project yet._\n");
        return out.into_bytes();
    }
    for (title, body) in sample_bodies {
        let gloss = extract_hjson_string_field(body, "gloss")
            .or_else(|| extract_hjson_string_field(body, "translation"));
        let original = extract_hjson_string_field(body, "original")
            .or_else(|| extract_hjson_string_field(body, "text"));
        out.push_str(&format!("== {}\n\n", typst_escape(title)));
        out.push_str("#grid(columns: (1fr, 1fr), gutter: 1em,\n");
        out.push_str(&format!(
            "  [#text(weight: \"semibold\")[Gloss]\\\n{}],\n",
            typst_escape(gloss.as_deref().unwrap_or(body)),
        ));
        out.push_str(&format!(
            "  [#text(weight: \"semibold\")[Original]\\\n{}],\n",
            typst_escape(original.as_deref().unwrap_or("(no original supplied)")),
        ));
        out.push_str(")\n\n");
    }
    out.into_bytes()
}
