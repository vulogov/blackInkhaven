//! `inkhaven language import` — foreign-dictionary ingestion (Polyglot XML,
//! CSV) plus the manuscript/inventory scan helpers it shares with it. Split out
//! of the flat handler; the dictionary-entry builders and loaders live in the
//! parent module.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// `inkhaven language list`.
/// Walks the `Language` system book and emits one
/// row per language with summary counts.  Quick
/// at-a-glance complement to `language doctor`.
/// `inkhaven language add-word
/// <lang> --import <path.csv>`.  Bulk-load a CSV
/// dictionary.  Format described in the CLI variant
/// docstring; mechanically:
///   * RFC 4180 quoting (`"…"` for fields with
///     commas / quotes / newlines; `""` for embedded
///     quotes).
///   * Header row maps column NAMES to row positions
///     so the CSV's columns can appear in any order
///     and any subset.
///   * Complex fields parsed inside the row:
///       - `inflection`: `;`-separated `key=value` pairs
///       - `examples`:   `|`-separated sentences
///       - `related`:    `;`-separated word slugs
///   * Skip rules: empty `word` cell + `word` starting
///     with `#` both treated as skip-this-row; duplicate
///     `word` (already in the dictionary) skipped with
///     warning.
///   * Tally printed at end (imported / skipped /
///     failed counts).
/// 1.3.19 LANG-1 P6 — import a dictionary from a foreign conlang/linguistics
/// tool (Toolbox/MDF SFM, PolyGlot). Parses the file into neutral lexemes
/// (`conlang::interchange`), previews them by default, and writes them into the
/// Dictionary only with `--yes`. Deterministic format conversion — no AI — but
/// non-committal by default so an author reviews before the book changes.
pub(crate) fn import_foreign(
    project: &Path,
    language: &str,
    file: &Path,
    format: crate::cli::LanguageImportFormat,
    commit: bool,
) -> Result<()> {
    use crate::cli::LanguageImportFormat;
    use crate::conlang::interchange;

    let (store, _hierarchy, lang_book) = open_lang_book(project, language)?;

    let lexemes = match format {
        LanguageImportFormat::Toolbox => {
            let raw = std::fs::read_to_string(file).map_err(|e| {
                Error::Config(format!("could not read {}: {e}", file.display()))
            })?;
            interchange::parse_toolbox(&raw)
        }
        LanguageImportFormat::Polyglot => {
            let xml = read_polyglot_xml(file)?;
            interchange::parse_polyglot(&xml).map_err(Error::Config)?
        }
    };

    if lexemes.is_empty() {
        eprintln!(
            "no entries found in {} — is it a {} file?",
            file.display(),
            match format {
                LanguageImportFormat::Toolbox => "Toolbox/SFM",
                LanguageImportFormat::Polyglot => "PolyGlot",
            }
        );
        return Ok(());
    }

    if !commit {
        eprintln!(
            "{} entr{} parsed from {} (preview — pass --yes to import):\n",
            lexemes.len(),
            if lexemes.len() == 1 { "y" } else { "ies" },
            file.display()
        );
        for lx in lexemes.iter().take(20) {
            let pos = if lx.pos.is_empty() {
                String::new()
            } else {
                format!("  [{}]", lx.pos)
            };
            println!("  {:<20} {}{}", lx.word, lx.translation, pos);
        }
        if lexemes.len() > 20 {
            println!("  … and {} more", lexemes.len() - 20);
        }
        return Ok(());
    }

    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let (mut added, mut skipped) = (0usize, 0usize);
    for lx in &lexemes {
        let entry = ImportEntry {
            word: lx.word.clone(),
            pos: lx.pos.clone(),
            translation: lx.translation.clone(),
            example: lx.example.clone(),
            pronunciation: lx.pronunciation.clone(),
            etymology: lx.etymology.clone(),
            notes: lx.notes.clone(),
            ..Default::default()
        };
        match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
            Ok(_) => added += 1,
            Err(e) => {
                skipped += 1;
                eprintln!("  skipped {}: {e}", lx.word);
            }
        }
    }
    eprintln!("\nimported {added} entr(y/ies) into {language}'s Dictionary ({skipped} skipped)");
    Ok(())
}

/// Read PolyGlot dictionary XML from either the native `.pgd` ZIP archive
/// (extracting `PGDictionary.xml`) or a raw `.xml` file. The archive member
/// name has varied across PolyGlot versions, so fall back to the first
/// `*.xml` entry when the canonical name is absent.
pub(crate) fn read_polyglot_xml(file: &Path) -> Result<String> {
    let bytes = std::fs::read(file)
        .map_err(|e| Error::Config(format!("could not read {}: {e}", file.display())))?;
    // ZIP archives start with the local-file-header magic `PK\x03\x04`.
    let is_zip = bytes.starts_with(b"PK\x03\x04");
    if !is_zip {
        return String::from_utf8(bytes)
            .map_err(|e| Error::Config(format!("{} is not valid UTF-8: {e}", file.display())));
    }
    let reader = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(reader)
        .map_err(|e| Error::Config(format!("{} is not a valid .pgd archive: {e}", file.display())))?;
    // Prefer the canonical member; else the first .xml in the archive.
    let names: Vec<String> = (0..zip.len())
        .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
        .collect();
    let target = names
        .iter()
        .find(|n| n.eq_ignore_ascii_case("PGDictionary.xml"))
        .or_else(|| names.iter().find(|n| n.to_ascii_lowercase().ends_with(".xml")))
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "no XML dictionary found inside {} (members: {})",
                file.display(),
                names.join(", ")
            ))
        })?;
    let mut member = zip
        .by_name(&target)
        .map_err(|e| Error::Config(format!("could not read {target} from archive: {e}")))?;
    let mut xml = String::new();
    std::io::Read::read_to_string(&mut member, &mut xml)
        .map_err(|e| Error::Config(format!("could not decode {target}: {e}")))?;
    Ok(xml)
}

pub(crate) fn import_dictionary_csv(
    project: &Path,
    language: &str,
    csv_path: &Path,
    new: bool,
    force: bool,
) -> Result<()> {
    use crate::store::node::NodeKind;
    let layout = ProjectLayout::new(project);
    layout.require_initialized()?;
    let cfg = Config::load_layered(&layout.config_path())?;
    let store = Store::open(layout.clone(), &cfg)?;
    let hierarchy = Hierarchy::load(&store)?;

    let lang_root = hierarchy
        .iter()
        .find(|n| {
            n.kind == NodeKind::Book
                && n.system_tag.as_deref() == Some(SYSTEM_TAG_LANGUAGES)
        })
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it".into(),
            )
        })?
        .clone();
    let lang_book = hierarchy
        .children_of(Some(lang_root.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Book && n.title.eq_ignore_ascii_case(language)
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` not found — run `inkhaven language init {language}` first"
            ))
        })?;

    let raw = std::fs::read_to_string(csv_path).map_err(|e| {
        Error::Config(format!(
            "could not read CSV file {}: {e}",
            csv_path.display()
        ))
    })?;
    let rows = parse_csv(&raw)
        .map_err(|e| Error::Config(format!("CSV parse error: {e}")))?;
    let mut rows = rows.into_iter();
    let header = rows
        .next()
        .ok_or_else(|| Error::Config("CSV is empty (no header row)".into()))?;
    let columns = resolve_csv_columns(&header)?;

    // Materialise the data rows so we can do the
    // pre-flight pass + the actual import pass.
    let data_rows: Vec<Vec<String>> = rows.collect();

    // ── Pre-flight validation ─────────────────────
    //
    // Walk every CSV row's `word`, collect every
    // non-whitespace character, and verify against
    // the language's declared alphabet +
    // phonology-rule phoneme inventories.  Aborts
    // the import before ANY writes if there's a
    // violation, so a partial import doesn't leave
    // the dictionary in a confused state.  --force
    // skips this; --new wipes before importing so
    // the validation also pre-empts a destructive
    // wipe on a CSV that wouldn't have imported
    // cleanly anyway.
    if !force {
        let meta = read_meta_overview(&store, &hierarchy, &lang_book)?;
        let phoneme_inventories =
            collect_phonology_inventories(&store, &hierarchy, &lang_book)?;
        let alphabet: Vec<String> = meta
            .as_ref()
            .map(|m| m.alphabet.clone())
            .unwrap_or_default();
        let mut violations: Vec<String> = Vec::new();
        for (row_idx, row) in data_rows.iter().enumerate() {
            let display_row = row_idx + 2;
            let word = row
                .get(columns.word)
                .cloned()
                .unwrap_or_default()
                .trim()
                .to_string();
            if word.is_empty() || word.starts_with('#') {
                continue;
            }
            if !alphabet.is_empty() {
                if let Some(bad) = first_unknown_letter(&word, &alphabet) {
                    violations.push(format!(
                        "row {display_row}: `{word}` contains `{bad}` not in Meta/overview.alphabet"
                    ));
                    continue; // skip phonology check for already-flagged word
                }
            }
            if !phoneme_inventories.is_empty() {
                if let Some(bad) = first_unknown_letter(&word, &phoneme_inventories) {
                    violations.push(format!(
                        "row {display_row}: `{word}` contains `{bad}` not in any Phonology inventory"
                    ));
                }
            }
        }
        if !violations.is_empty() {
            eprintln!(
                "Pre-flight validation failed — {} violation(s) found:\n",
                violations.len()
            );
            for v in &violations {
                eprintln!("  · {v}");
            }
            eprintln!(
                "\nFix by either:\n  \
                 · updating Meta/overview.alphabet to include the missing characters, OR\n  \
                 · updating a Phonology rule's `phonemes` list to include them, OR\n  \
                 · correcting the CSV, OR\n  \
                 · re-running with --force to bypass validation."
            );
            return Err(Error::Config(format!(
                "import aborted — {} alphabet/phonology violation(s)",
                violations.len()
            )));
        }
    }

    // ── --new wipe ────────────────────────────────
    //
    // Validation passed, --new requested → delete
    // every paragraph + bucket subchapter under the
    // Dictionary chapter (preserving the Dictionary
    // chapter itself so the subsequent import lands
    // in a known place).
    if new {
        wipe_dictionary(&store, &hierarchy, &lang_book, language)?;
    }

    let mut imported = 0usize;
    let mut skipped_blank = 0usize;
    let mut skipped_comment = 0usize;
    let mut skipped_duplicate = 0usize;
    let mut failed = 0usize;

    for (row_idx, row) in data_rows.into_iter().enumerate() {
        // Row 1 in user terms = header; data starts at row 2.
        let display_row = row_idx + 2;
        let entry = match build_import_entry_from_row(&columns, &row) {
            Ok(e) => e,
            Err(e) => {
                eprintln!("row {display_row}: {e} — skipped");
                failed += 1;
                continue;
            }
        };
        let trimmed = entry.word.trim();
        if trimmed.is_empty() {
            skipped_blank += 1;
            continue;
        }
        if trimmed.starts_with('#') {
            skipped_comment += 1;
            continue;
        }
        match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
            Ok((_, bucket)) => {
                eprintln!("imported `{}` → {language}/Dictionary/{bucket}", entry.word);
                imported += 1;
            }
            Err(e) => {
                let msg = e.to_string();
                // The duplicate-detect message comes from
                // `create_dictionary_entry`; surface as a
                // skip rather than a failure so an
                // idempotent re-import doesn't tally the
                // pre-existing entries as errors.
                if msg.contains("already defined") {
                    eprintln!("row {display_row}: `{}` already exists — skipped", entry.word);
                    skipped_duplicate += 1;
                } else {
                    eprintln!("row {display_row}: import `{}` failed: {msg}", entry.word);
                    failed += 1;
                }
            }
        }
    }

    eprintln!();
    eprintln!("Import summary for `{language}`");
    eprintln!("  imported:        {imported}");
    if skipped_blank > 0 {
        eprintln!("  skipped (blank): {skipped_blank}");
    }
    if skipped_comment > 0 {
        eprintln!("  skipped (#):     {skipped_comment}");
    }
    if skipped_duplicate > 0 {
        eprintln!("  skipped (dup):   {skipped_duplicate}");
    }
    if failed > 0 {
        eprintln!("  failed:          {failed}");
    }
    Ok(())
}

/// Column-name → index mapping.  Built from the
/// CSV's header row so columns can appear in any
/// order and any subset (required columns enforced
/// here).
pub(crate) struct CsvColumns {
    pub(crate) word: usize,
    pub(crate) pos: usize,
    pub(crate) translation: usize,
    pub(crate) example: Option<usize>,
    pub(crate) pronunciation: Option<usize>,
    pub(crate) etymology: Option<usize>,
    pub(crate) related: Option<usize>,
    pub(crate) inflection: Option<usize>,
    pub(crate) examples: Option<usize>,
    pub(crate) register: Option<usize>,
    pub(crate) era: Option<usize>,
    pub(crate) notes: Option<usize>,
}

pub(crate) fn resolve_csv_columns(header: &[String]) -> Result<CsvColumns> {
    let lookup = |name: &str| -> Option<usize> {
        header.iter().position(|h| h.trim().eq_ignore_ascii_case(name))
    };
    let word = lookup("word").ok_or_else(|| {
        Error::Config("CSV missing required column `word`".into())
    })?;
    let pos = lookup("type").ok_or_else(|| {
        Error::Config("CSV missing required column `type`".into())
    })?;
    let translation = lookup("translation").ok_or_else(|| {
        Error::Config("CSV missing required column `translation`".into())
    })?;
    Ok(CsvColumns {
        word,
        pos,
        translation,
        example: lookup("example"),
        pronunciation: lookup("pronunciation"),
        etymology: lookup("etymology"),
        related: lookup("related"),
        inflection: lookup("inflection"),
        examples: lookup("examples"),
        register: lookup("register"),
        era: lookup("era"),
        notes: lookup("notes"),
    })
}

pub(crate) fn build_import_entry_from_row(
    cols: &CsvColumns,
    row: &[String],
) -> std::result::Result<ImportEntry, String> {
    let get = |idx: usize| -> String {
        row.get(idx).cloned().unwrap_or_default()
    };
    let opt = |maybe_idx: Option<usize>| -> String {
        maybe_idx.map(get).unwrap_or_default()
    };
    let inflection_raw = opt(cols.inflection);
    let inflection = parse_inflection_field(&inflection_raw);
    let examples_raw = opt(cols.examples);
    let examples = split_pipe(&examples_raw);
    let related_raw = opt(cols.related);
    let related = split_semicolon(&related_raw);
    Ok(ImportEntry {
        word: get(cols.word).trim().to_string(),
        pos: get(cols.pos).trim().to_string(),
        translation: get(cols.translation).trim().to_string(),
        example: opt(cols.example).trim().to_string(),
        pronunciation: opt(cols.pronunciation).trim().to_string(),
        etymology: opt(cols.etymology).trim().to_string(),
        related,
        inflection,
        examples,
        register: opt(cols.register).trim().to_string(),
        era: opt(cols.era).trim().to_string(),
        notes: opt(cols.notes).trim().to_string(),
        domain: Vec::new(),
    })
}

/// `nominative=atal;genitive=atale;plural=atatal`
/// → BTreeMap.  Bad entries (no `=`) are silently
/// skipped — the import is best-effort row-by-row.
pub(crate) fn parse_inflection_field(
    raw: &str,
) -> std::collections::BTreeMap<String, String> {
    let mut out = std::collections::BTreeMap::new();
    for pair in raw.split(';') {
        let pair = pair.trim();
        if pair.is_empty() {
            continue;
        }
        if let Some(eq) = pair.find('=') {
            let key = pair[..eq].trim().to_string();
            let value = pair[eq + 1..].trim().to_string();
            if !key.is_empty() && !value.is_empty() {
                out.insert(key, value);
            }
        }
    }
    out
}

pub(crate) fn split_pipe(raw: &str) -> Vec<String> {
    raw.split('|')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub(crate) fn split_semicolon(raw: &str) -> Vec<String> {
    raw.split(';')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Minimal RFC 4180-style CSV reader.  Handles:
///   * Quoted fields with embedded `,`, `\n`, `"`
///     (`""` doubles to a single `"`).
///   * Unquoted fields with neither.
///   * CRLF + bare LF row separators.
/// Returns `Vec<Vec<String>>` — one Vec per row.
/// Errors only on truly malformed input (unclosed
/// quote at end of file).
/// read + parse the language
/// sub-book's `Meta/overview` body.  Returns `None`
/// when the chapter / paragraph is missing or the
/// body has no parseable HJSON (pre-Phase-A
/// scaffolds).  Errors only on store I/O failures.
pub(crate) fn read_meta_overview(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::language_entry::MetaOverview>> {
    use crate::store::node::NodeKind;
    let Some(meta_chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(overview) = hierarchy
        .children_of(Some(meta_chapter.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case("overview")
        })
        .cloned()
    else {
        return Ok(None);
    };
    let Some(bytes) = store.get_content(overview.id)? else {
        return Ok(None);
    };
    let body = match std::str::from_utf8(&bytes) {
        Ok(s) => s,
        Err(_) => return Ok(None),
    };
    Ok(crate::language_entry::parse_meta_overview(body)
        .map_err(Error::Config)?)
}

/// collect the union of every
/// Phonology rule's `phonemes` field as a single
/// list of allowed grapheme strings.  Used as the
/// reference inventory the CSV import validates
/// every word against.  Returns an empty list when
/// no Phonology rule declares `phonemes` — in that
/// case the validator skips the phonology check
/// (the alphabet check still runs).
///
/// Note: phonemes are technically sounds and word
/// characters are graphemes — we treat them as
/// interchangeable here because for most invented
/// languages with Latin / Cyrillic orthography the
/// author writes phonemes using single-character
/// graphemes.  Authors with more complex
/// orthography-to-phonology mappings can run with
/// --force.
pub(crate) fn collect_phonology_inventories(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Vec<String>> {
    use crate::store::node::NodeKind;
    use serde::Deserialize;
    #[derive(Deserialize)]
    struct PhonologyRule {
        #[serde(default)]
        phonemes: Vec<String>,
    }
    let Some(phonology) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Phonology")
        })
        .cloned()
    else {
        return Ok(Vec::new());
    };
    let mut out: Vec<String> = Vec::new();
    for id in hierarchy.collect_subtree(phonology.id) {
        let Some(node) = hierarchy.get(id) else { continue; };
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(id) else { continue; };
        let Ok(body) = std::str::from_utf8(&bytes) else { continue; };
        // Try whole-body HJSON first (the new
        // content_type=hjson format), fall back to
        // fenced extraction for legacy bodies.
        // Same parse strategy as
        // `language_entry::parse_with`.
        let parsed: Option<PhonologyRule> = serde_hjson::from_str(body)
            .ok()
            .or_else(|| {
                // Reuse the fence extractor by parsing
                // the wrapping body shape — but the
                // public extract_hjson_block helper
                // isn't exported.  For phonology rules
                // authored on the new template, the
                // whole-body parse covers us; legacy
                // fenced bodies will have to be
                // re-saved by the author (or hit via
                // --force).
                None
            });
        if let Some(rule) = parsed {
            out.extend(rule.phonemes);
        }
    }
    Ok(out)
}

/// find the first character in
/// `word` that doesn't match any entry in `inventory`.
/// Returns the offending character so the error
/// message can name it.  Case-insensitive: `'a'`
/// matches both `'A'` and `'a'` in the inventory.
/// Whitespace and ASCII punctuation are always
/// accepted (sentences may contain hyphens,
/// apostrophes, etc.).
pub(crate) fn first_unknown_letter(word: &str, inventory: &[String]) -> Option<char> {
    let inventory_lower: Vec<String> = inventory
        .iter()
        .map(|s| s.to_lowercase())
        .collect();
    for c in word.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() {
            continue;
        }
        let c_lower = c.to_lowercase().collect::<String>();
        let found = inventory_lower
            .iter()
            .any(|entry| entry.contains(&c_lower));
        if !found {
            return Some(c);
        }
    }
    None
}

/// `--new` wipe.  Deletes every
/// paragraph + bucket subchapter under the
/// language's Dictionary chapter, preserving the
/// Dictionary chapter itself so the subsequent
/// import has a known parent.  Walks the bucket
/// subchapters in reverse-order so each
/// `delete_subtree` call sees a stable hierarchy
/// (deleting in forward order shifts every
/// remaining sibling's `order` field).
pub(crate) fn wipe_dictionary(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    language: &str,
) -> Result<()> {
    use crate::store::node::NodeKind;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{language}` has no Dictionary chapter to wipe"
            ))
        })?;
    let buckets: Vec<_> =
        hierarchy.children_of(Some(dictionary.id)).into_iter().cloned().collect();
    let bucket_count = buckets.len();
    let mut entry_count = 0usize;
    // `Hierarchy::fs_path` ignores its layout
    // argument (returns a project-root-relative
    // path); pass a dummy.  Reverse order so
    // deletes don't shift remaining siblings'
    // on-disk `NN-slug` prefixes — the rename pass
    // would otherwise multiply the work.
    let dummy_layout = ProjectLayout::new(store.project_root());
    for bucket in buckets.into_iter().rev() {
        let fresh = Hierarchy::load(store)?;
        let ids = fresh.collect_subtree(bucket.id);
        entry_count += ids.len().saturating_sub(1);
        let Some(refreshed_bucket) = fresh.get(bucket.id) else { continue; };
        let fs_rel = fresh.fs_path(refreshed_bucket, &dummy_layout);
        store
            .delete_subtree(&fs_rel, &ids)
            .map_err(|e| Error::Store(format!("wipe bucket `{}`: {e}", bucket.title)))?;
    }
    eprintln!(
        "--new: wiped {entry_count} existing entries across {bucket_count} buckets from `{language}/Dictionary`"
    );
    Ok(())
}

pub(crate) fn parse_csv(raw: &str) -> std::result::Result<Vec<Vec<String>>, String> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quoted = false;
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if in_quoted {
            match c {
                '"' => {
                    // `""` inside a quoted field = one literal quote.
                    if chars.peek() == Some(&'"') {
                        chars.next();
                        field.push('"');
                    } else {
                        in_quoted = false;
                    }
                }
                _ => field.push(c),
            }
        } else {
            match c {
                '"' => in_quoted = true,
                ',' => {
                    row.push(std::mem::take(&mut field));
                }
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                '\n' => {
                    row.push(std::mem::take(&mut field));
                    rows.push(std::mem::take(&mut row));
                }
                _ => field.push(c),
            }
        }
    }
    if in_quoted {
        return Err("unclosed quote at end of file".into());
    }
    // Flush the trailing field/row when the file
    // doesn't end with a newline.
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    Ok(rows)
}
