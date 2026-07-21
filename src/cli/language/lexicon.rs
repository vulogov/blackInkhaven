//! `inkhaven language` lexicon surface: dictionary-entry creation (manual +
//! imported), the `add-word`/`query`/`gaps`-adjacent handlers, AI lexicon
//! generation, manuscript scan, and the alphabet-bucketing + seed-body helpers.
//! Split out of the flat handler; the loaders + hjson helpers live in the parent.

use std::path::Path;

use crate::config::Config;
use crate::error::{Error, Result};
use crate::store::Store;

use super::*;

/// LANG-1 P2.7 — scan the manuscript for candidate undefined conlang words.
pub(crate) fn scan_manuscript(project: &Path, language: &str, json: bool) -> Result<()> {
    use std::collections::HashSet;
    use unicode_segmentation::UnicodeSegmentation;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block — the scan needs the inventory to tell \
             conlang words from prose"
        ))
    })?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let known: HashSet<String> = entries
        .iter()
        .flat_map(|e| e.surface_forms().into_iter().map(|s| s.to_lowercase()))
        .collect();
    if known.is_empty() {
        eprintln!("note: {language} has no dictionary entries yet — nothing anchors the scan");
    }

    // Every user-book paragraph as a word list (system books are reference
    // material, not manuscript prose).
    let mut paragraphs: Vec<Vec<String>> = Vec::new();
    for node in hierarchy.iter() {
        if node.kind != NodeKind::Paragraph {
            continue;
        }
        let mut cursor = Some(node.id);
        let mut is_system = false;
        while let Some(id) = cursor {
            match hierarchy.get(id) {
                Some(n) if n.system_tag.is_some() => {
                    is_system = true;
                    break;
                }
                Some(n) => cursor = n.parent_id,
                None => break,
            }
        }
        if is_system {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(node.id) else { continue };
        let Ok(body) = std::str::from_utf8(&bytes) else { continue };
        paragraphs.push(body.unicode_words().map(String::from).collect());
    }

    let report = crate::conlang::lexicon::scan_undefined(&phonology, &known, &paragraphs);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&report)
                .map_err(|e| Error::Store(format!("serializing scan: {e}")))?
        );
        return Ok(());
    }

    println!(
        "scan {language} · {} paragraph(s), {} in a conlang context",
        report.paragraphs_scanned, report.conlang_paragraphs
    );
    if report.candidates.is_empty() {
        println!("  ✓ no undefined conlang words found");
        return Ok(());
    }
    println!("\n  candidate undefined words ({}):", report.candidates.len());
    for c in &report.candidates {
        println!("    {:<16} ×{}", c.word, c.count);
    }
    eprintln!("\n(heuristic — `add-word` the real ones, fix the typos)");
    Ok(())
}

/// LANG-1 P2.6 — list Places + Characters linked to a language.
pub(crate) fn speakers(project: &Path, language: &str) -> Result<()> {
    use crate::conlang::links::ConlangLinks;
    let (store, _hierarchy, lang_book) = open_lang_book(project, language)?;
    let links = ConlangLinks::load(store.project_root()).map_err(Error::Io)?;
    let (places, characters) = links.speakers_of(&lang_book.title);

    println!("speakers of {}", lang_book.title);
    if places.is_empty() && characters.is_empty() {
        println!("  (none linked yet — see `inkhaven language link-place` / `link-character`)");
        return Ok(());
    }
    if !places.is_empty() {
        println!("\n  places ({}):", places.len());
        for p in &places {
            println!("    {p}");
        }
    }
    if !characters.is_empty() {
        println!("\n  characters ({}):", characters.len());
        for (name, level) in &characters {
            println!("    {name:<20} {level}");
        }
    }
    Ok(())
}

/// LANG-1 P2.4 — query the dictionary by the rich entry fields.
#[allow(clippy::too_many_arguments)]
pub(crate) fn query(
    project: &Path,
    language: &str,
    register: Option<&str>,
    domain: Option<&str>,
    era: Option<&str>,
    pos: Option<&str>,
    text: Option<&str>,
    json: bool,
) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let f = crate::conlang::lexicon::Filter { register, domain, era, pos, text };
    let matches = crate::conlang::lexicon::filter(&entries, &f);

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&matches)
                .map_err(|e| Error::Store(format!("serializing query: {e}")))?
        );
        return Ok(());
    }

    println!("{} / {} entr(y/ies) match", matches.len(), entries.len());
    for e in &matches {
        let mut tags = Vec::new();
        if !e.registers.is_empty() {
            tags.push(format!("[{}]", e.registers.join(",")));
        }
        if !e.domain.is_empty() {
            tags.push(format!("{{{}}}", e.domain.join(",")));
        }
        if let Some(era) = &e.era {
            tags.push(format!("<{era}>"));
        }
        let pos = if e.pos.trim().is_empty() { String::new() } else { format!(" ({})", e.pos) };
        println!(
            "  {:<16} {}{}{}",
            e.word,
            e.translation,
            pos,
            if tags.is_empty() { String::new() } else { format!("  {}", tags.join(" ")) }
        );
    }
    Ok(())
}

/// LANG-1 P2.2 — AI-assisted dictionary generation behind the dedup gate.
#[allow(clippy::too_many_arguments)]
pub(crate) fn generate_lexicon(
    project: &Path,
    language: &str,
    topic: Option<&str>,
    count: usize,
    era: Option<&str>,
    register: Option<&str>,
    provider: Option<&str>,
    semantic: bool,
    semantic_threshold: f32,
    yes: bool,
) -> Result<()> {
    use crate::conlang::generate::lexicon as lexgen;

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let phonology = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no phoneme block — add `phonemes` / `classes` / `templates` \
             HJSON under its `Phonology` chapter first"
        ))
    })?;
    if phonology.templates_for(crate::conlang::TemplateRole::Root).is_empty() {
        return Err(Error::Config(format!(
            "language `{language}` declares no `root` templates — needed to generate forms"
        )));
    }
    let existing = load_dictionary(&store, &hierarchy, &lang_book)?;

    let pool = lexgen::build_pool(&phonology, &existing, count);
    if pool.is_empty() {
        return Err(Error::Config(
            "could not generate any valid candidate forms — loosen the phonotactic constraints".into(),
        ));
    }

    let ai = crate::ai::AiClient::from_config(&cfg.llm)?;
    let (model, _env) = ai.resolve_provider(&cfg.llm, provider)?;
    let work_lang = if cfg.language.trim().is_empty() { "english" } else { cfg.language.trim() };
    eprintln!(
        "inkhaven language generate-lexicon · {language} · model: {model} · glosses in {work_lang}"
    );

    let prompt = build_lexgen_prompt(language, topic, count, era, register, work_lang, &pool);
    let raw = crate::ai::stream::collect_blocking(
        ai.client.clone(),
        model.to_string(),
        Some(LEXGEN_SYSTEM.to_string()),
        prompt,
    )
    .map_err(|e| Error::Store(format!("inference error: {e}")))?;

    let proposals = match lexgen::parse_proposals(&raw) {
        Ok(p) => p,
        Err(why) => {
            eprintln!("could not parse model reply: {why}\n---- raw ----\n{raw}\n---- end ----");
            return Ok(());
        }
    };
    let (mut kept, rejected) = lexgen::dedup(&phonology, &existing, proposals);

    // Semantic half of the dedup gate: reject near-synonyms by gloss
    // embedding (catches "stone" vs "rock" the string check misses).
    let mut near_synonyms: Vec<(lexgen::LexProposal, f32)> = Vec::new();
    if semantic && !kept.is_empty() {
        let existing_glosses: Vec<&str> = existing
            .iter()
            .map(|e| e.translation.trim())
            .filter(|g| !g.is_empty())
            .collect();
        let kept_glosses: Vec<&str> = kept.iter().map(|p| p.gloss.trim()).collect();
        let existing_vecs = if existing_glosses.is_empty() {
            Vec::new()
        } else {
            store.embed_batch(&existing_glosses)?
        };
        let kept_vecs = store.embed_batch(&kept_glosses)?;
        let (sem_kept, sem_rejected) =
            lexgen::semantic_filter(kept, &existing_vecs, &kept_vecs, semantic_threshold);
        kept = sem_kept;
        near_synonyms = sem_rejected;
    }

    println!(
        "proposed {} entr(y/ies) for {language}{} ({} rejected by the dedup gate):",
        kept.len(),
        topic.map(|t| format!(" · topic: {t}")).unwrap_or_default(),
        rejected.len()
    );
    for p in &kept {
        let pos = if p.pos.trim().is_empty() { "?" } else { p.pos.trim() };
        println!("  {:<16} {} ({})", p.form, p.gloss, pos);
    }
    if !rejected.is_empty() {
        eprintln!("\nrejected:");
        for (p, reason) in &rejected {
            eprintln!("  {:<16} {} — {}", p.form, p.gloss, reason.as_str());
        }
    }
    if !near_synonyms.is_empty() {
        eprintln!("\nrejected (near-synonyms, cosine > {semantic_threshold:.2}):");
        for (p, sim) in &near_synonyms {
            eprintln!("  {:<16} {} — too close ({sim:.2})", p.form, p.gloss);
        }
    }

    if yes {
        let mut added = 0usize;
        for p in &kept {
            // Commit through the rich-import path so the AI's register /
            // domain tags + the batch era land on the entry (P2.5).
            let entry = ImportEntry {
                word: p.form.trim().to_string(),
                pos: if p.pos.trim().is_empty() { "noun".into() } else { p.pos.trim().to_string() },
                translation: p.gloss.trim().to_string(),
                example: p.example.trim().to_string(),
                register: p.register.trim().to_string(),
                domain: p.domain.iter().map(|d| d.trim().to_string()).filter(|d| !d.is_empty()).collect(),
                era: era.unwrap_or("").trim().to_string(),
                ..Default::default()
            };
            match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
                Ok(_) => added += 1,
                Err(e) => eprintln!("  skipped {}: {e}", p.form),
            }
        }
        eprintln!("\nadded {added} entr(y/ies) to {language}'s Dictionary");
    } else {
        eprintln!(
            "\n(dry run — re-run with --yes to add the {} kept entr(y/ies))",
            kept.len()
        );
        // PANE-1 P3 — surface the advisory proposals in the Output pane with a
        // promote-to-Dictionary action (no-op outside the TUI / active pane).
        emit_lexicon_proposal(language, topic, era, &kept);
    }
    Ok(())
}

pub(crate) fn build_lexgen_prompt(
    language: &str,
    topic: Option<&str>,
    count: usize,
    era: Option<&str>,
    register: Option<&str>,
    work_lang: &str,
    pool: &[String],
) -> String {
    let domain = topic.unwrap_or("core everyday life");
    let candidates = pool
        .iter()
        .map(|f| format!("\"{f}\""))
        .collect::<Vec<_>>()
        .join(", ");
    let mut constraints = format!(
        "Language: {language}. Produce {count} dictionary entries for the semantic domain: {domain}."
    );
    if let Some(e) = era {
        constraints.push_str(&format!(" In-world era: {e}."));
    }
    if let Some(r) = register {
        constraints.push_str(&format!(" Register: {r}."));
    }
    format!(
        "{constraints}\n\n\
         Pick a coherent set of {count} concepts a culture needs for this domain, then assign each \
         a distinct `form` chosen ONLY from the candidate list below. Write every `gloss` and \
         `example` in {work_lang}. Do not repeat a meaning. Keep `pos` a short lowercase tag. Tag \
         each entry with a `register` and one or two `domain` tags appropriate to its concept.\n\n\
         Candidate forms (choose from these): [{candidates}]\n\n\
         Reply with the JSON object only."
    )
}
/// `inkhaven language add-word`.
/// Resolves the target language sub-book by case-
/// insensitive title; finds its Dictionary chapter;
/// derives the alphabet bucket for the new word from
/// the first character (auto-creates the subchapter
/// when missing); rejects duplicate words.
pub(crate) fn add_word(
    project: &Path,
    language: &str,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
    stress: Option<usize>,
) -> Result<()> {
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
        .ok_or_else(|| {
            Error::Store(
                "Language system book missing — re-open the project to seed it"
                    .into(),
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

    let (entry, bucket) = add_dictionary_entry_impl(
        &store,
        &cfg,
        &lang_book,
        word,
        pos,
        translation,
        example,
        stress,
    )?;
    let _ = entry;
    eprintln!(
        "added `{word}` to `{language}/Dictionary/{bucket}` ({pos} · {translation})"
    );
    Ok(())
}

/// shared "add dictionary
/// entry" implementation used by:
///   * the CLI `add-word` subcommand (above);
///   * the in-TUI tree-pane Add Paragraph (`+`) commit
///     handler when the cursor sits anywhere under
///     `Language/<lang>/Dictionary`.
///
/// Caller supplies the per-language Book; we look up
/// the Dictionary chapter, derive the alphabet bucket
/// (consulting Meta/overview first, first-char
/// uppercase as fallback), find-or-create the bucket
/// subchapter, reject duplicates, create the entry
/// paragraph, and seed its body with the HJSON
/// template (POS / translation / example fields are
/// left empty in the TUI flow — the author fills them
/// in by editing the paragraph).
///
/// Returns `(entry_node, bucket_name)` so callers can
/// surface a status message or move the tree cursor.
pub(crate) fn add_dictionary_entry_impl(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
    stress: Option<usize>,
) -> Result<(crate::store::node::Node, String)> {
    let body = seed_dictionary_entry_body(word, pos, translation, example, stress);
    create_dictionary_entry(store, cfg, lang_book, word, &body)
}

/// fully-populated entry record
/// used by the CSV import path.  Distinct from the
/// `language_entry::DictionaryEntry` parser type
/// because we own this one (mutable builder) and the
/// parser one is immutable (deserialised view).
#[derive(Debug, Default, Clone)]
pub(crate) struct ImportEntry {
    pub word: String,
    pub pos: String,
    pub translation: String,
    pub example: String,
    pub pronunciation: String,
    pub etymology: String,
    pub related: Vec<String>,
    pub inflection: std::collections::BTreeMap<String, String>,
    pub examples: Vec<String>,
    pub register: String,
    pub era: String,
    pub notes: String,
    /// LANG-1 P2.4/P2.5 — semantic-domain tags.
    pub domain: Vec<String>,
}

/// Add a fully-populated dictionary entry from an
/// import row.  Bypasses the verbose commented seed
/// template and writes compact HJSON with only the
/// populated fields.  Shares the bucket-derivation +
/// duplicate-check + persistence machinery with the
/// interactive `add_dictionary_entry_impl`.
pub(crate) fn add_imported_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    entry: &ImportEntry,
) -> Result<(crate::store::node::Node, String)> {
    let body = build_imported_entry_body(entry);
    create_dictionary_entry(store, cfg, lang_book, &entry.word, &body)
}

/// Shared bucket-derivation + duplicate-check +
/// node-creation + disk/bdslib persistence for both
/// the interactive and bulk-import paths.  Body is
/// passed verbatim — callers pick whether they want
/// the verbose commented template or a compact
/// concrete entry.
pub(crate) fn create_dictionary_entry(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    word: &str,
    body: &str,
) -> Result<(crate::store::node::Node, String)> {
    let hierarchy = Hierarchy::load(store)?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| {
            n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary")
        })
        .cloned()
        .ok_or_else(|| {
            Error::Config(format!(
                "language `{}` has no `Dictionary` chapter — likely scaffolded with a pre-Phase-A inkhaven",
                lang_book.title
            ))
        })?;
    let bucket = derive_alphabet_bucket(store, &hierarchy, lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .ok_or_else(|| {
            Error::Config(format!("could not derive alphabet bucket from `{word}`"))
        })?;
    let dictionary_kids = hierarchy.children_of(Some(dictionary.id));
    let subchapter = match dictionary_kids
        .iter()
        .find(|n| n.kind == NodeKind::Subchapter && n.title == bucket)
        .cloned()
    {
        Some(existing) => existing.clone(),
        None => {
            let hierarchy = Hierarchy::load(store)?;
            store.create_node(
                cfg,
                &hierarchy,
                NodeKind::Subchapter,
                &bucket,
                Some(&dictionary),
                None,
                InsertPosition::End,
            )?
        }
    };
    let hierarchy = Hierarchy::load(store)?;
    if hierarchy
        .children_of(Some(subchapter.id))
        .iter()
        .any(|n| n.title.eq_ignore_ascii_case(word))
    {
        return Err(Error::Config(format!(
            "word `{word}` already defined under `{}/Dictionary/{bucket}`",
            lang_book.title
        )));
    }
    let hierarchy = Hierarchy::load(store)?;
    let mut entry = store.create_node(
        cfg,
        &hierarchy,
        NodeKind::Paragraph,
        word,
        Some(&subchapter),
        None,
        InsertPosition::End,
    )?;
    entry.content_type = Some("hjson".to_string());
    if let Some(rel) = &entry.file {
        let abs = store.project_root().join(rel);
        std::fs::write(&abs, body.as_bytes())
            .map_err(|e| Error::Store(format!("write entry: {e}")))?;
    }
    store
        .update_paragraph_content(&mut entry, body.as_bytes())
        .map_err(|e| Error::Store(format!("seed entry: {e}")))?;
    Ok((entry, bucket))
}

/// compact concrete HJSON for an
/// imported entry.  Emits ONLY the fields the import
/// row actually populated; skips empty optional
/// fields entirely so the resulting paragraph reads
/// cleanly when the author opens it.
pub(crate) fn build_imported_entry_body(entry: &ImportEntry) -> String {
    let mut out = String::from("{\n");
    out.push_str(&format!("  word:         \"{}\"\n", escape_hjson(&entry.word)));
    out.push_str(&format!("  type:         \"{}\"\n", escape_hjson(&entry.pos)));
    out.push_str(&format!(
        "  translation:  \"{}\"\n",
        escape_hjson(&entry.translation)
    ));
    if !entry.example.is_empty() {
        out.push_str(&format!(
            "  example:      \"{}\"\n",
            escape_hjson(&entry.example)
        ));
    }
    if !entry.examples.is_empty() {
        out.push_str("  examples: [\n");
        for ex in &entry.examples {
            out.push_str(&format!("    \"{}\"\n", escape_hjson(ex)));
        }
        out.push_str("  ]\n");
    }
    if !entry.pronunciation.is_empty() {
        out.push_str(&format!(
            "  pronunciation: \"{}\"\n",
            escape_hjson(&entry.pronunciation)
        ));
    }
    if !entry.etymology.is_empty() {
        out.push_str(&format!(
            "  etymology:    \"{}\"\n",
            escape_hjson(&entry.etymology)
        ));
    }
    if !entry.related.is_empty() {
        let items: Vec<String> = entry
            .related
            .iter()
            .map(|r| format!("\"{}\"", escape_hjson(r)))
            .collect();
        out.push_str(&format!("  related:      [{}]\n", items.join(", ")));
    }
    if !entry.inflection.is_empty() {
        out.push_str("  inflection: {\n");
        for (k, v) in &entry.inflection {
            out.push_str(&format!(
                "    {}: \"{}\"\n",
                k,
                escape_hjson(v)
            ));
        }
        out.push_str("  }\n");
    }
    if !entry.register.is_empty() {
        out.push_str(&format!(
            "  register:     \"{}\"\n",
            escape_hjson(&entry.register)
        ));
    }
    if !entry.era.is_empty() {
        out.push_str(&format!("  era:          \"{}\"\n", escape_hjson(&entry.era)));
    }
    if !entry.notes.is_empty() {
        out.push_str(&format!(
            "  notes:        \"{}\"\n",
            escape_hjson(&entry.notes)
        ));
    }
    if !entry.domain.is_empty() {
        let items = entry
            .domain
            .iter()
            .map(|d| format!("\"{}\"", escape_hjson(d)))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  domain:       [{items}]\n"));
    }
    out.push_str("}\n");
    out
}

/// seed body for a grammar
/// rule paragraph created in the TUI.  Mirrors the
/// proposal §4 schema so future Phase D.2 work
/// (`--format grammar` exporter, `language define-rule`
/// CLI) can parse it the same way the dictionary entry
/// parser handles entries today.  Authors edit the
/// HJSON to fill in `category`, `applies_when`, etc.
pub(crate) const GRAMMAR_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier the AI translation prompt references
  // in applied-rules lists.  Lowercase + hyphens.
  // Example: \"noun-case-system\",
  // \"verb-tense-aspect\", \"reduplication\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 grammar export
  // sectioning AND the in-prompt grouping.
  //   morphology   — word-formation, inflection
  //   syntax       — clause structure, word order
  //   phonology    — sound rules
  //   orthography  — spelling conventions
  //   semantics    — meaning relationships
  //   pragmatics   — usage / discourse rules
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // Plain text inside an HJSON multi-line string;
  // tabular layouts work fine.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  This text is fed
    verbatim to the AI translator at translation
    time, so be explicit:

      * State the input → output transformation.
      * Show the morpheme boundaries (- or .).
      * Show ALL exceptions inline so the LLM
        doesn't have to guess.

    Example layout for a case system:

      NOM: zero suffix.   aran     (king)
      ACC: -n.             aran → aranin
      DAT: -en.            aran → aranen
      GEN: -o.             aran → arano
  '''

  // ──────────────────────────────────────────────────
  // FEW-SHOT EXAMPLES — bundled into the translation
  // prompt envelope so the LLM sees the rule applied.
  // ──────────────────────────────────────────────────

  examples: [
    // { source: \"\",  target: \"\",  gloss: \"\" }
    // { source: \"\",  target: \"\",  gloss: \"\" }
  ]

  // ──────────────────────────────────────────────────
  // RAG TRIGGERING — when this rule should be
  // included in the translation prompt envelope.
  // ──────────────────────────────────────────────────

  // Plain-language condition the LLM evaluates
  // against the source sentence.  Tight applies_when
  // keeps the prompt focused (Phase C envelope
  // includes only matching rules; default cap is 6).
  applies_when: \"\"

  // Sibling rules this one builds on, by rule_id.
  // The RAG layer pulls dependent rules
  // automatically.  Example: a verb-conjugation
  // rule depends on the stem-formation rule.
  depends_on:   []

  // Rules that conflict with this one — only one
  // should fire per translation pass.  Phase D.2
  // `language doctor` will surface conflicting
  // pairs that lack an `applies_when` disambiguator.
  conflicts_with: []

  // ──────────────────────────────────────────────────
  // METADATA / NOTES
  // ──────────────────────────────────────────────────

  // Productivity — how broadly the rule applies.
  // \"core\"        — fires on most sentences
  // \"common\"      — fires on a recognisable
  //                  subset of constructions
  // \"specialised\" — narrow / register-bound
  // \"vestigial\"   — historical residue only
  productivity: \"\"

  // Register / style restrictions, if any:
  // formal | informal | literary | sacred | archaic.
  register:     \"\"

  // Author's notes — historical motivation,
  // worldbuilding rationale, comparison to natural-
  // language analogues.  Not read by the LLM.
  notes:        \"\"
}
";

/// seed body for a
/// phonology rule paragraph.  Lighter than the
/// grammar template because phonology rules tend to
/// be more declarative (allowed onsets, vowel
/// harmony patterns) than triggered.
pub(crate) const PHONOLOGY_RULE_SEED_BODY: &str = "{
  // ──────────────────────────────────────────────────
  // IDENTITY
  // ──────────────────────────────────────────────────

  // Identifier — lowercase + hyphens.  Referenced by
  // grammar rules' `depends_on` field and by the
  // phonotactic generator (`Ctrl+B Shift+W` in the
  // Language book — Phase D.2).
  // Examples: \"consonant-inventory\",
  // \"vowel-harmony\", \"syllable-template\",
  // \"intervocalic-voicing\".
  rule_id:      \"\"

  // Human-readable title for the rule card renderer.
  title:        \"\"

  // Category — drives Phase D.2 phonology export
  // sectioning AND the phonotactic generator's
  // weighting.
  //   consonants     — IPA inventory of consonants
  //   vowels         — IPA inventory of vowels
  //   phonotactics   — allowed onset / nucleus / coda
  //   syllable       — syllable template (CV, CVC, …)
  //   stress         — stress placement rule
  //   tone           — tonal system / pitch rules
  //   sound-changes  — historical or allophonic shifts
  //   prosody        — intonation / rhythm patterns
  category:     \"\"

  // ──────────────────────────────────────────────────
  // RULE BODY — read by both the LLM and the human.
  // ──────────────────────────────────────────────────

  rule:         '''
    Describe the rule here.  Use IPA inside
    /slashes/ for phonemic and [brackets] for
    phonetic.

    Example layouts:

      Phonotactic template:
        ONSET: zero | C | CC (only stop+liquid)
        NUCLEUS: V | VV (long vowels)
        CODA: zero | C | CC (limited to /s, n, r, l/)

      Sound change:
        /s/ → [z] / V_V (intervocalic voicing)

      Vowel harmony:
        Front vowels {i, e} co-occur in roots;
        back vowels {a, o, u} co-occur in roots;
        suffixes harmonise with the root.
  '''

  // ──────────────────────────────────────────────────
  // INVENTORIES — for consonants / vowels categories.
  // ──────────────────────────────────────────────────

  // List of phonemes (IPA strings).  Optional; used
  // by the phonotactic generator to constrain output.
  // phonemes:     []

  // Allophonic variants by environment.  Map of
  // phoneme → list of (environment, realisation).
  // allophones:   {}

  // ──────────────────────────────────────────────────
  // ENVIRONMENT — for sound-changes / allophony.
  // ──────────────────────────────────────────────────

  // Where the rule applies (LLM evaluates against the
  // source's phonetic context).
  // environment:  \"\"

  // ──────────────────────────────────────────────────
  // EXAMPLES — IPA pairs showing the rule in action.
  // ──────────────────────────────────────────────────

  examples: [
    // { input: \"\", output: \"\", gloss: \"\" }
  ]

  // Known exceptions — words / morphemes where the
  // rule does NOT apply.
  exceptions: []

  // ──────────────────────────────────────────────────
  // NOTES
  // ──────────────────────────────────────────────────

  // Register / style restrictions, if any.
  register:     \"\"

  // Author's notes — historical motivation, source
  // dialect, comparison to natural-language analogues.
  notes:        \"\"
}
";

/// Derive the alphabet-bucket subchapter name for a
/// word.  Uses the first non-whitespace character,
/// uppercased.  Returns `None` only if the input is
/// entirely whitespace — alphanumeric, Cyrillic,
/// Greek, hyphen / apostrophe-prefix all map to
/// their leading letter or symbol.
pub(crate) fn alphabet_bucket(word: &str) -> Option<String> {
    let ch = word.chars().find(|c| !c.is_whitespace())?;
    Some(ch.to_uppercase().to_string())
}

/// Consult the language sub-book's `Meta/overview`
/// HJSON for the alphabet-bucket name.  The author's
/// declared groupings override the naive first-char
/// uppercase (Phase B's fallback).  Returns:
///   * `Ok(Some(bucket))` — declared alphabet covers
///     the word's first character.
///   * `Ok(None)` — Meta chapter missing, overview
///     paragraph missing, HJSON block absent, alphabet
///     list empty, or first char not in any declared
///     entry.  Caller falls back to `alphabet_bucket`.
///   * `Err` — HJSON parse failure or store IO error.
///     Surfaced rather than swallowed so a malformed
///     overview is noisy enough to fix.
pub(crate) fn derive_alphabet_bucket(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    word: &str,
) -> Result<Option<String>> {
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
    let body = std::str::from_utf8(&bytes).map_err(|e| {
        Error::Config(format!("Meta/overview body is not UTF-8: {e}"))
    })?;
    let meta = match crate::language_entry::parse_meta_overview(body)
        .map_err(Error::Config)?
    {
        Some(m) => m,
        None => return Ok(None),
    };
    Ok(meta.bucket_for_word(word).map(|s| s.to_string()))
}

/// Build the seeded body for a freshly-added
/// dictionary entry.  Pure HJSON — no Typst wrappers
/// — so the editor renders with HJSON syntax
/// highlighting.  The paragraph's `content_type` is
/// set to `"hjson"` at create time.
///
/// switched FROM Typst-
/// with-fenced-HJSON to pure HJSON.  The translation
/// prompt composer + parser handle both formats; new
/// entries use pure HJSON.
pub(crate) fn seed_dictionary_entry_body(
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
    stress: Option<usize>,
) -> String {
    let example_value = example.unwrap_or("").trim();
    // 1.8.2 — emit a real `stress` field only when the author gave one (lexical
    // stress, for verse scansion); otherwise leave it out.
    let stress_line = match stress {
        Some(n) => format!(
            "// Stressed syllable (1-based) — lexical stress for verse scansion.\n  \
             stress:       {n}\n  \n  "
        ),
        None => String::new(),
    };
    format!(
        "{{\n  \
         // ──────────────────────────────────────────────────\n  \
         // CORE — required for the entry to function as a\n  \
         // lexicon-overlay target + translation-prompt source.\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         word:         \"{word}\"\n  \
         \n  \
         // Part of speech.  Free-form string; the\n  \
         // proposal suggests: noun | verb | adjective |\n  \
         // adverb | pronoun | preposition | conjunction |\n  \
         // interjection | particle.  Language-specific\n  \
         // categories (\"classifier\", \"evidential\",\n  \
         // \"applicative\") are fine.\n  \
         type:         \"{pos}\"\n  \
         \n  \
         // Working-language gloss — what this word\n  \
         // means in the project's `language` (the value\n  \
         // the AI translator maps to/from).\n  \
         translation:  \"{translation}\"\n  \
         \n  \
         // Canonical sample sentence the author wants\n  \
         // frozen into the entry.  Becomes few-shot\n  \
         // anchor data in the translation prompt.\n  \
         example:      \"{example}\"\n  \
         \n  \
         // ──────────────────────────────────────────────────\n  \
         // OPTIONAL — uncomment and fill the ones you need.\n  \
         // Each is consumed by either the translation\n  \
         // prompt envelope (Phase C) or the future\n  \
         // dictionary card renderer (Phase D.2).\n  \
         // ──────────────────────────────────────────────────\n  \
         \n  \
         // Additional example sentences beyond the\n  \
         // canonical one.  Phase C translation flow\n  \
         // uses every example as few-shot data.\n  \
         // examples:     [\n  \
         //   \"\"\n  \
         //   \"\"\n  \
         // ]\n  \
         \n  \
         // IPA transcription (between slashes for\n  \
         // phonemic, brackets for phonetic).\n  \
         // pronunciation: \"\"\n  \
         \n  \
         // Etymology / derivation.  Plain text or\n  \
         // [[wikilink]] style cross-reference to a\n  \
         // proto-form entry.\n  \
         // etymology:    \"\"\n  \
         \n  \
         // Cross-references to sibling entries — other\n  \
         // words in this language that share roots,\n  \
         // contrast in register, or commonly co-occur.\n  \
         // related:      []\n  \
         \n  \
         // Paradigm forms.  Every VALUE here gets\n  \
         // added to the lexicon overlay so inflected\n  \
         // words light up in prose alongside the\n  \
         // lemma.  KEY names are free-form and feed\n  \
         // the translation prompt as paradigm hints.\n  \
         // inflection:   {{\n  \
         //   plural:     \"\"\n  \
         //   genitive:   \"\"\n  \
         //   accusative: \"\"\n  \
         //   dative:     \"\"\n  \
         //   ablative:   \"\"\n  \
         // }}\n  \
         \n  \
         // Register / style: formal | informal |\n  \
         // archaic | literary | colloquial | sacred.\n  \
         // register:     \"\"\n  \
         \n  \
         // Era — when the word entered the language.\n  \
         // Useful for historical-fiction projects.\n  \
         // era:          \"\"\n  \
         \n  \
         // Auto-tracked count of mentions in the\n  \
         // manuscript.  Phase D.2 `language doctor`\n  \
         // updates this; leave 0 for now.\n  \
         // frequency:    0\n  \
         \n  \
         // Free-form usage notes — register cues,\n  \
         // taboos, mnemonic etymology, whatever\n  \
         // helps you remember the word.\n  \
         {stress_line}\
         notes:        \"\"\n\
         }}\n",
        word = escape_hjson(word),
        pos = escape_hjson(pos),
        translation = escape_hjson(translation),
        example = escape_hjson(example_value),
        stress_line = stress_line,
    )
}
