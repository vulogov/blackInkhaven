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
                .map_err(|e| Error::Store(format!("serialising scan: {e}")))?
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
                .map_err(|e| Error::Store(format!("serialising query: {e}")))?
        );
        return Ok(());
    }

    println!("{} / {} match", matches.len(), super::wording::entries(entries.len()));
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
        "proposed {} for {language}{} ({} rejected by the dedup gate):",
        super::wording::entries(kept.len()),
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
        let mut writer = DictionaryWriter::open(&store, &cfg, &lang_book)?;
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
            match writer.add_imported(&entry) {
                Ok(_) => added += 1,
                Err(e) => eprintln!("  skipped {}: {e}", p.form),
            }
        }
        writer.finish()?;
        eprintln!("\nadded {} to {language}'s Dictionary", super::wording::entries(added));
    } else {
        eprintln!(
            "\n(dry run — re-run with --yes to add the {} kept)",
            super::wording::entries(kept.len())
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
// Mirrors the clap `AddWord` arguments one-to-one; a struct would only move
// the same eight names behind one more indirection.
#[allow(clippy::too_many_arguments)]
pub(crate) fn add_word(
    project: &Path,
    language: &str,
    word: &str,
    pos: &str,
    translation: &str,
    example: Option<&str>,
    stress: Option<usize>,
    force: bool,
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
        .ok_or_else(|| super::wording::language_not_found(language))?;

    // LB-4: the same spelling check the CSV import runs, so the two doors
    // agree. `--force` is the escape hatch, as on import.
    if !force {
        use crate::conlang::lexicon::headword::HeadwordError;
        if let Err(e) = HeadwordChecker::load(&store, &hierarchy, &lang_book)?.check(word) {
            let hint = match e {
                HeadwordError::UnknownSegment(_) | HeadwordError::UnknownLetter(_) => {
                    " — fix the spelling, extend the declaration, or pass --force"
                }
                _ => "",
            };
            return Err(Error::Config(format!("`{word}` {e}{hint}")));
        }
    }

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
    let mut writer = DictionaryWriter::open(store, cfg, lang_book)?;
    let out = writer.add_seeded(word, pos, translation, example, stress)?;
    writer.finish()?;
    Ok(out)
}

/// The spelling check every interactive door runs on a headword (LB-4 /
/// LB-5): the language's `Phonology` inventory when one is declared, else
/// its `Meta/overview.alphabet`. Load once, check many (the CSV pre-flight
/// walks every row with one checker).
pub(crate) struct HeadwordChecker {
    pub(crate) alphabet: Vec<String>,
    pub(crate) phonology: Option<crate::conlang::Phonology>,
}

impl HeadwordChecker {
    pub(crate) fn load(
        store: &Store,
        hierarchy: &Hierarchy,
        lang_book: &crate::store::node::Node,
    ) -> Result<Self> {
        let alphabet = read_meta_overview(store, hierarchy, lang_book)?
            .map(|m| m.alphabet)
            .unwrap_or_default();
        // A block that does not parse at all must not lock the author out
        // of their own dictionary: warn and fall back to the alphabet.
        let phonology = match load_phonology(store, hierarchy, lang_book) {
            Ok(p) => p,
            Err(e) => {
                eprintln!("warning: {e} — headwords checked against the alphabet only");
                None
            }
        };
        Ok(Self { alphabet, phonology })
    }

    /// Which declaration `check` consults, for messages and summaries.
    pub(crate) fn authority(&self) -> &'static str {
        if self.phonology.as_ref().is_some_and(|p| !p.phonemes.is_empty()) {
            "Phonology inventory"
        } else if !self.alphabet.is_empty() {
            "Meta/overview.alphabet"
        } else {
            "shape only (no alphabet or Phonology inventory declared)"
        }
    }

    pub(crate) fn check(
        &self,
        word: &str,
    ) -> std::result::Result<(), crate::conlang::lexicon::headword::HeadwordError> {
        crate::conlang::lexicon::headword::validate_headword(
            word,
            &self.alphabet,
            self.phonology.as_ref(),
        )
    }
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
    let mut writer = DictionaryWriter::open(store, cfg, lang_book)?;
    let out = writer.add_imported(entry)?;
    writer.finish()?;
    Ok(out)
}


/// 3.18 LI-3 — one batch of dictionary writes.
///
/// Before this, every entry paid three full hierarchy loads, a re-read and
/// re-parse of `Meta/overview` for its bucket, a template write + embed
/// followed by the real body's write + re-embed, and a full flush of the
/// vector index. The writer loads the hierarchy, the alphabet and the
/// spelling checker once, keeps its hierarchy snapshot current with the
/// nodes it creates ([`Hierarchy::insert`]), writes each entry's final
/// body up front ([`Store::create_paragraph_with_body`]: one embedding
/// batch per row), and holds a [`crate::store::SyncBatch`] so the index is
/// flushed once in [`Self::finish`]. The checks are unchanged: LB-6 shape
/// and NFC keying, the alphabet bucket (declared alphabet first, base
/// letter fallback), the homograph rule (same spelling + different `pos` is
/// titled `word (N)`; same or empty `pos` is the duplicate the error names).
pub(crate) struct DictionaryWriter<'a> {
    store: &'a Store,
    cfg: &'a Config,
    lang_book: crate::store::node::Node,
    dictionary: crate::store::node::Node,
    hierarchy: Hierarchy,
    /// `Meta/overview`, parsed once (a malformed overview is an error, as
    /// `derive_alphabet_bucket` always treated it).
    meta: Option<crate::language_entry::MetaOverview>,
    checker: HeadwordChecker,
    /// `pos` of the entries this batch created, so the homograph check
    /// does not read their bodies back.
    batch_pos: std::collections::HashMap<uuid::Uuid, String>,
    batch: Option<crate::store::SyncBatch<'a>>,
    /// Buckets this batch added entries to, and whether it created a bucket:
    /// the parents [`Self::finish`] re-sorts into alphabet order (LI-5).
    touched_buckets: std::collections::BTreeSet<uuid::Uuid>,
    added_bucket: bool,
}

impl<'a> DictionaryWriter<'a> {
    /// Load everything a batch needs: one hierarchy, the Dictionary
    /// chapter, the overview and the spelling checker; and start deferring
    /// index flushes.
    pub(crate) fn open(
        store: &'a Store,
        cfg: &'a Config,
        lang_book: &crate::store::node::Node,
    ) -> Result<Self> {
        let hierarchy = Hierarchy::load(store)?;
        let dictionary = hierarchy
            .children_of(Some(lang_book.id))
            .into_iter()
            .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
            .cloned()
            .ok_or_else(|| {
                Error::Config(format!(
                    "language `{}` has no `Dictionary` chapter — likely scaffolded with a pre-Phase-A inkhaven",
                    lang_book.title
                ))
            })?;
        let meta = load_meta_overview_strict(store, &hierarchy, lang_book)?;
        let checker = HeadwordChecker::load(store, &hierarchy, lang_book)?;
        Ok(Self {
            store,
            cfg,
            lang_book: lang_book.clone(),
            dictionary,
            hierarchy,
            meta,
            checker,
            batch_pos: std::collections::HashMap::new(),
            batch: Some(store.begin_batch()),
            touched_buckets: std::collections::BTreeSet::new(),
            added_bucket: false,
        })
    }

    /// The spelling check every door runs (LB-4 / LB-5).
    pub(crate) fn check(
        &self,
        word: &str,
    ) -> std::result::Result<(), crate::conlang::lexicon::headword::HeadwordError> {
        self.checker.check(word)
    }

    /// Which declaration [`Self::check`] consults.
    pub(crate) fn authority(&self) -> &'static str {
        self.checker.authority()
    }

    /// Add an import row (compact HJSON body; LB-3 NFC at the door).
    pub(crate) fn add_imported(
        &mut self,
        entry: &ImportEntry,
    ) -> Result<(crate::store::node::Node, String)> {
        let word = crate::conlang::types::norm::nfc(&entry.word);
        let body = if word == entry.word {
            build_imported_entry_body(entry)
        } else {
            build_imported_entry_body(&ImportEntry { word: word.clone(), ..entry.clone() })
        };
        self.add(&word, &entry.pos, &body)
    }

    /// Add an interactively seeded entry (the commented template body).
    pub(crate) fn add_seeded(
        &mut self,
        word: &str,
        pos: &str,
        translation: &str,
        example: Option<&str>,
        stress: Option<usize>,
    ) -> Result<(crate::store::node::Node, String)> {
        let word = crate::conlang::types::norm::nfc(word);
        let body = seed_dictionary_entry_body(&word, pos, translation, example, stress);
        self.add(&word, pos, &body)
    }

    /// Add one entry with `body` verbatim. Returns the entry node and its
    /// bucket. A failed row leaves the writer unchanged.
    pub(crate) fn add(
        &mut self,
        word: &str,
        pos: &str,
        body: &str,
    ) -> Result<(crate::store::node::Node, String)> {
        use crate::conlang::lexicon::headword;
        use crate::conlang::types::norm::fold;
        if let Err(e) = headword::validate_shape(word) {
            return Err(Error::Config(format!("headword `{}` {e}", word.trim())));
        }
        // Idempotent for callers that already normalised; keeps the bucket, the
        // duplicate check and the node title on the NFC headword (LB-3).
        let word = crate::conlang::types::norm::nfc(word.trim());
        let bucket = self
            .meta
            .as_ref()
            .and_then(|m| m.bucket_for_word(&word).map(str::to_string))
            .or_else(|| alphabet_bucket(&word))
            .ok_or_else(|| Error::Config(format!("could not derive alphabet bucket from `{word}`")))?;
        let subchapter = match self
            .hierarchy
            .children_of(Some(self.dictionary.id))
            .into_iter()
            .find(|n| n.kind == NodeKind::Subchapter && n.title == bucket)
            .cloned()
        {
            Some(existing) => existing,
            None => {
                let created = self.store.create_node(
                    self.cfg,
                    &self.hierarchy,
                    NodeKind::Subchapter,
                    &bucket,
                    Some(&self.dictionary),
                    None,
                    InsertPosition::End,
                )?;
                self.hierarchy.insert(created.clone());
                self.added_bucket = true;
                created
            }
        };
        let key = fold(&word);
        let same_spelling: Vec<crate::store::node::Node> = self
            .hierarchy
            .children_of(Some(subchapter.id))
            .into_iter()
            .filter(|n| n.kind == NodeKind::Paragraph && fold(homograph_base(&n.title)) == key)
            .cloned()
            .collect();
        let mut title = word.clone();
        if !same_spelling.is_empty() {
            let pos_key = fold(pos.trim());
            let mut conflict = pos_key.is_empty();
            for existing in &same_spelling {
                let existing_pos = match self.batch_pos.get(&existing.id) {
                    Some(p) => fold(p.trim()),
                    None => self
                        .store
                        .get_content(existing.id)?
                        .and_then(|bytes| {
                            crate::language_entry::parse(&String::from_utf8_lossy(&bytes)).ok().flatten()
                        })
                        .map(|e| fold(e.pos.trim()))
                        .unwrap_or_default(),
                };
                if existing_pos.is_empty() || existing_pos == pos_key {
                    conflict = true;
                }
            }
            if conflict {
                let as_pos = if pos.trim().is_empty() {
                    String::new()
                } else {
                    format!(" as {}", pos.trim())
                };
                return Err(Error::Config(format!(
                    "word `{word}` already defined{as_pos} under `{}/Dictionary/{bucket}`",
                    self.lang_book.title
                )));
            }
            title = format!("{word} ({})", same_spelling.len() + 1);
        }
        let entry = self.store.create_paragraph_with_body(
            self.cfg,
            &self.hierarchy,
            &title,
            Some(&subchapter),
            InsertPosition::End,
            body.as_bytes(),
            Some("hjson"),
        )?;
        self.hierarchy.insert(entry.clone());
        self.batch_pos.insert(entry.id, pos.to_string());
        self.touched_buckets.insert(subchapter.id);
        Ok((entry, bucket))
    }

    /// End the batch: put every touched bucket (and the bucket list, when
    /// one was created) into alphabet order with one rename pass each
    /// (LI-5), then flush the vector index once. Entries are appended while
    /// the batch runs and sorted here, so a batch never pays a hierarchy
    /// reload per middle insert.
    pub(crate) fn finish(mut self) -> Result<()> {
        let alphabet: Vec<String> = self.meta.as_ref().map(|m| m.alphabet.clone()).unwrap_or_default();
        let dictionary_id = self.dictionary.id;
        let plans: Vec<SortPlan> = plan_dictionary_sort(&self.hierarchy, &self.dictionary, &alphabet)
            .into_iter()
            .filter(|p| (p.parent.id == dictionary_id && self.added_bucket) || self.touched_buckets.contains(&p.parent.id))
            .collect();
        for plan in &plans {
            self.store.reorder_children(&mut self.hierarchy, Some(plan.parent.id), &plan.order)?;
        }
        if let Some(batch) = self.batch.take() {
            batch.finish()?;
        }
        Ok(())
    }
}

/// The reorder `sort-dictionary` would apply under one parent (LI-5): the
/// children in alphabet order, and the moves that implies.
#[derive(Debug, Clone)]
pub(crate) struct SortPlan {
    pub parent: crate::store::node::Node,
    /// Every current child, in the new order.
    pub order: Vec<uuid::Uuid>,
    /// `(title, from, to)` — 1-based positions of the children that move.
    pub moves: Vec<(String, usize, usize)>,
}

/// Plan the dictionary's alphabet order: one plan per bucket whose entries
/// are out of order (first, while the bucket directories still have their
/// names), then one for the bucket list itself. Buckets order by declared
/// alphabet position, undeclared buckets after them; entries by
/// [`Alphabet::sort_key`]. Children of another kind (a stray note under
/// the chapter) keep their slots; only the keyed siblings permute.
pub(crate) fn plan_dictionary_sort(
    hierarchy: &Hierarchy,
    dictionary: &crate::store::node::Node,
    alphabet: &[String],
) -> Vec<SortPlan> {
    use crate::conlang::lexicon::headword::Alphabet;
    let alpha = Alphabet::new(alphabet);
    let mut plans = Vec::new();
    let buckets: Vec<&crate::store::node::Node> = hierarchy
        .children_of(Some(dictionary.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Subchapter)
        .collect();
    for bucket in &buckets {
        if let Some(plan) = plan_children(hierarchy, bucket, |n| {
            (n.kind == NodeKind::Paragraph).then(|| alpha.sort_key(&n.title))
        }) {
            plans.push(plan);
        }
    }
    if let Some(plan) = plan_children(hierarchy, dictionary, |n| {
        (n.kind == NodeKind::Subchapter).then(|| alpha.bucket_sort_key(&n.title))
    }) {
        plans.push(plan);
    }
    plans
}

/// Sort the keyed children of `parent` among their own slots; `None` when
/// nothing would move.
fn plan_children<K: Ord>(
    hierarchy: &Hierarchy,
    parent: &crate::store::node::Node,
    key: impl Fn(&crate::store::node::Node) -> Option<K>,
) -> Option<SortPlan> {
    let children = hierarchy.children_of(Some(parent.id));
    let keyed: Vec<(usize, K)> = children.iter().enumerate().filter_map(|(i, n)| key(n).map(|k| (i, k))).collect();
    let mut sorted: Vec<&(usize, K)> = keyed.iter().collect();
    sorted.sort_by(|a, b| a.1.cmp(&b.1));
    let mut order: Vec<uuid::Uuid> = children.iter().map(|n| n.id).collect();
    let mut moves = Vec::new();
    for (slot, (from, _)) in keyed.iter().map(|(slot, _)| slot).zip(sorted.iter().map(|(from, k)| (from, k))) {
        order[*slot] = children[*from].id;
        if from != slot {
            moves.push((children[*from].title.clone(), from + 1, slot + 1));
        }
    }
    (!moves.is_empty()).then(|| SortPlan { parent: parent.clone(), order, moves })
}

/// `inkhaven language sort-dictionary <lang> [--dry-run]` (LI-5): put the
/// bucket subchapters and the entries inside each into declared alphabet
/// order, reporting every move. With no alphabet declared, entries sort by
/// folded code point and buckets by title.
pub(crate) fn sort_dictionary(project: &Path, language: &str, dry_run: bool) -> Result<()> {
    let (store, mut hierarchy, lang_book) = open_lang_book(project, language)?;
    let dictionary = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
        .cloned()
        .ok_or_else(|| Error::Config(format!("language `{}` has no `Dictionary` chapter", lang_book.title)))?;
    let alphabet: Vec<String> = load_meta_overview_strict(&store, &hierarchy, &lang_book)?
        .map(|m| m.alphabet)
        .unwrap_or_default();
    if alphabet.is_empty() {
        eprintln!("note: {} declares no Meta/overview.alphabet — sorting by letter", lang_book.title);
    }
    let plans = plan_dictionary_sort(&hierarchy, &dictionary, &alphabet);
    if plans.is_empty() {
        println!("{}/Dictionary is already in alphabet order", lang_book.title);
        return Ok(());
    }
    let (mut buckets_moved, mut entries_moved) = (0usize, 0usize);
    for plan in &plans {
        let label = if plan.parent.id == dictionary.id {
            buckets_moved += plan.moves.len();
            "Dictionary".to_string()
        } else {
            entries_moved += plan.moves.len();
            format!("Dictionary/{}", plan.parent.title)
        };
        for (title, from, to) in &plan.moves {
            println!("  {label}: `{title}` {from} → {to}");
        }
    }
    if dry_run {
        println!("(dry run — {} and {} would move)", super::wording::plural(buckets_moved, "bucket", "buckets"), super::wording::entries(entries_moved));
        return Ok(());
    }
    let batch = store.begin_batch();
    for plan in &plans {
        store.reorder_children(&mut hierarchy, Some(plan.parent.id), &plan.order)?;
    }
    batch.finish()?;
    eprintln!("sorted {}/Dictionary: {} and {} moved", lang_book.title, super::wording::plural(buckets_moved, "bucket", "buckets"), super::wording::entries(entries_moved));
    Ok(())
}

/// `Meta/overview` for bucket derivation, with `derive_alphabet_bucket`'s
/// strictness: a missing chapter / paragraph / block is `None`; a malformed
/// block or non-UTF-8 body is an error, so it is noisy enough to fix.
fn load_meta_overview_strict(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<Option<crate::language_entry::MetaOverview>> {
    let Some(meta_chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Meta"))
    else {
        return Ok(None);
    };
    let Some(overview) = hierarchy
        .children_of(Some(meta_chapter.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Paragraph && n.title.eq_ignore_ascii_case("overview"))
    else {
        return Ok(None);
    };
    let Some(bytes) = store.get_content(overview.id)? else {
        return Ok(None);
    };
    let body = std::str::from_utf8(&bytes)
        .map_err(|e| Error::Config(format!("Meta/overview body is not UTF-8: {e}")))?;
    crate::language_entry::parse_meta_overview(body).map_err(Error::Config)
}

/// Whether a dictionary entry title names `word`: NFC + case-folded, with
/// the ` (N)` homograph suffix ignored unless the query itself carries one
/// (`pata` matches `pata` and `pata (2)`; `pata (2)` matches only itself).
pub(crate) fn entry_title_matches(title: &str, word: &str) -> bool {
    use crate::conlang::types::norm::fold;
    let word = word.trim();
    if homograph_base(word) != word {
        fold(title.trim()) == fold(word)
    } else {
        fold(homograph_base(title)) == fold(word)
    }
}

/// Locate a dictionary entry by headword: the bucket the current rule
/// derives is searched first, then every bucket under `Dictionary` — an
/// entry filed before 3.17 (first-character buckets such as `É`), or under
/// an alphabet that has since changed, is still found by name.  Returns
/// `(bucket subchapter, entry paragraph)`.
pub(crate) fn find_dictionary_entry(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
    word: &str,
) -> Result<Option<(crate::store::node::Node, crate::store::node::Node)>> {
    use crate::conlang::types::norm::fold;
    let Some(dictionary) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
        .cloned()
    else {
        return Ok(None);
    };
    let buckets: Vec<crate::store::node::Node> = hierarchy
        .children_of(Some(dictionary.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Subchapter)
        .cloned()
        .collect();
    let derived = derive_alphabet_bucket(store, hierarchy, lang_book, word)?
        .or_else(|| alphabet_bucket(word))
        .map(|b| fold(&b));
    let search = |bucket: &crate::store::node::Node| {
        hierarchy
            .children_of(Some(bucket.id))
            .into_iter()
            .find(|n| n.kind == NodeKind::Paragraph && entry_title_matches(&n.title, word))
            .cloned()
    };
    if let Some(d) = &derived {
        if let Some(bucket) = buckets.iter().find(|b| fold(&b.title) == *d) {
            if let Some(entry) = search(bucket) {
                return Ok(Some((bucket.clone(), entry)));
            }
        }
    }
    for bucket in &buckets {
        if let Some(entry) = search(bucket) {
            return Ok(Some((bucket.clone(), entry)));
        }
    }
    Ok(None)
}

/// The spelling behind an entry title: `pata (2)` → `pata`.  Homograph
/// titles carry a ` (N)` suffix (LB-6); everything else is returned as is.
pub(crate) fn homograph_base(title: &str) -> &str {
    crate::conlang::lexicon::headword::split_homograph(title).0
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
/// word when `Meta/overview.alphabet` does not cover
/// it: the base letter of the first grapheme cluster,
/// uppercased (`ą́tal` → `A`, `ŋa` → `Ŋ`, `'ata` → `A`;
/// LB-3 / LB-6).  Returns `None` only when the input
/// has no letters at all.
pub(crate) fn alphabet_bucket(word: &str) -> Option<String> {
    crate::conlang::lexicon::headword::fallback_bucket(word)
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

#[cfg(test)]
mod sort_tests {
    use super::*;
    use crate::store::node::Node;
    use uuid::Uuid;

    fn node(id: Uuid, kind: &str, title: &str, parent: Option<Uuid>, order: u32) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": kind, "title": title,
            "slug": title.to_lowercase().replace(' ', "-"),
            "path": [], "parent_id": parent, "order": order, "file": null,
            "modified_at": "2026-01-01T00:00:00Z",
        }))
        .unwrap()
    }

    /// LI-5: a scrambled dictionary plans into alphabet order — entries
    /// inside each bucket first, then the buckets; a stray paragraph under
    /// the chapter keeps its slot; an already-sorted bucket has no plan.
    #[test]
    fn plan_orders_buckets_by_alphabet_and_entries_by_collation() {
        let (dict, b_d, b_ch, b_a, b_ng, note) =
            (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let (e1, e2, e3, e4) = (Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7(), Uuid::now_v7());
        let nodes = vec![
            node(dict, "chapter", "Dictionary", None, 1),
            node(b_d, "subchapter", "D", Some(dict), 1),
            node(note, "paragraph", "notes", Some(dict), 2),
            node(b_ng, "subchapter", "\u{014A}", Some(dict), 3),
            node(b_ch, "subchapter", "Ch", Some(dict), 4),
            node(b_a, "subchapter", "A", Some(dict), 5),
            node(e1, "paragraph", "chata (2)", Some(b_ch), 1),
            node(e2, "paragraph", "chata", Some(b_ch), 2),
            node(e3, "paragraph", "cha", Some(b_ch), 3),
            node(e4, "paragraph", "ad", Some(b_a), 1),
        ];
        let h = Hierarchy::from_nodes_for_test(nodes.clone());
        let alphabet: Vec<String> = ["A", "B", "Ch", "D"].iter().map(|s| s.to_string()).collect();
        let plans = plan_dictionary_sort(&h, &nodes[0], &alphabet);
        assert_eq!(plans.len(), 2, "{plans:?}");
        // Entries of Ch first (bucket directories still carry their names).
        assert_eq!(plans[0].parent.id, b_ch);
        assert_eq!(plans[0].order, vec![e3, e2, e1]);
        // Moves are listed by destination slot.
        assert_eq!(plans[0].moves, vec![("cha".to_string(), 3, 1), ("chata (2)".to_string(), 1, 3)]);
        // Then the buckets: A, Ch, D declared in that order, Ŋ after; the
        // stray note keeps slot 2.
        assert_eq!(plans[1].parent.id, dict);
        assert_eq!(plans[1].order, vec![b_a, note, b_ch, b_d, b_ng]);
        let moved: Vec<&str> = plans[1].moves.iter().map(|(t, _, _)| t.as_str()).collect();
        assert_eq!(moved, vec!["A", "Ch", "D", "\u{014A}"]);
        // Sorted input plans nothing.
        let sorted = Hierarchy::from_nodes_for_test(vec![
            node(dict, "chapter", "Dictionary", None, 1),
            node(b_a, "subchapter", "A", Some(dict), 1),
            node(e4, "paragraph", "ab", Some(b_a), 1),
            node(e1, "paragraph", "ad", Some(b_a), 2),
        ]);
        assert!(plan_dictionary_sort(&sorted, &nodes[0], &alphabet).is_empty());
    }
}

#[cfg(test)]
mod writer_tests {
    use super::*;
    use crate::project::ProjectLayout;
    use crate::store::node::Node;

    /// LI-5: a batch added in scrambled order lands sorted, and
    /// `sort-dictionary` repairs a tree scrambled by hand: files renamed
    /// with new `NN-` prefixes, descendants' `file` paths rewritten, a
    /// fresh load in alphabet order, and `export` following it.
    #[test]
    #[ignore = "opens a real Store (embedding-model download); run locally with --ignored"]
    fn writer_sorts_touched_buckets_and_sort_dictionary_repairs_a_scrambled_tree() {
        let tmp = tempfile::tempdir().unwrap();
        crate::cli::init::run(tmp.path(), true, "empty").unwrap();
        let layout = ProjectLayout::new(tmp.path());
        let cfg = Config::load_layered(&layout.config_path()).unwrap();
        let store = Store::open(layout.clone(), &cfg).unwrap();
        let book = super::super::init_language(&store, &cfg, "Sortlang").unwrap();

        // Scrambled batch: buckets T, K, A appear in that order; entries too.
        let mut writer = DictionaryWriter::open(&store, &cfg, &book).unwrap();
        for (w, g) in [("tika", "see"), ("kata", "stone"), ("ata", "one"), ("kala", "bird"), ("ta", "two"), ("aka", "three")] {
            writer.add_imported(&row(w, "noun", g)).unwrap();
        }
        writer.finish().unwrap();
        let titles = |h: &Hierarchy| -> Vec<(String, Vec<String>)> {
            let dict = h.children_of(Some(book.id)).into_iter().find(|n| n.title == "Dictionary").unwrap();
            h.children_of(Some(dict.id))
                .into_iter()
                .filter(|n| n.kind == NodeKind::Subchapter)
                .map(|b| (b.title.clone(), h.children_of(Some(b.id)).iter().map(|e| e.title.clone()).collect()))
                .collect()
        };
        let h = Hierarchy::load(&store).unwrap();
        assert_eq!(
            titles(&h),
            vec![
                ("A".to_string(), vec!["aka".to_string(), "ata".to_string()]),
                ("K".to_string(), vec!["kala".to_string(), "kata".to_string()]),
                ("T".to_string(), vec!["ta".to_string(), "tika".to_string()]),
            ]
        );
        // Every entry's stored `file` exists on disk after the bucket renames.
        for n in h.iter().filter(|n| n.kind == NodeKind::Paragraph && n.parent_id.is_some()) {
            if let Some(f) = &n.file {
                if f.contains("dictionary") {
                    assert!(tmp.path().join(f).exists(), "{f} missing on disk");
                }
            }
        }

        // Scramble by hand: swap the A and T buckets and two entries in K.
        let dict = h.children_of(Some(book.id)).into_iter().find(|n| n.title == "Dictionary").unwrap().clone();
        let buckets: Vec<Node> = h.children_of(Some(dict.id)).into_iter().cloned().collect();
        store.swap_siblings(&h, buckets[0].id, buckets[2].id).unwrap();
        let h = Hierarchy::load(&store).unwrap();
        let k = h.children_of(Some(dict.id)).into_iter().find(|n| n.title == "K").unwrap().clone();
        let ks: Vec<Node> = h.children_of(Some(k.id)).into_iter().cloned().collect();
        store.swap_siblings(&h, ks[0].id, ks[1].id).unwrap();
        let h = Hierarchy::load(&store).unwrap();
        assert_eq!(titles(&h)[0].0, "T");
        assert_eq!(titles(&h)[1].1, vec!["kata".to_string(), "kala".to_string()]);

        // Plan + apply (what `sort-dictionary` does without --dry-run).
        let plans = plan_dictionary_sort(&h, &dict, &[]);
        assert_eq!(plans.len(), 2);
        let batch = store.begin_batch();
        let mut moved = 0;
        let mut live = h;
        for plan in &plans {
            moved += store.reorder_children(&mut live, Some(plan.parent.id), &plan.order).unwrap();
        }
        batch.finish().unwrap();
        assert_eq!(moved, 4, "A and T buckets, kala and kata");
        // The primitive kept its hierarchy current: it matches a fresh load.
        let h = Hierarchy::load(&store).unwrap();
        assert_eq!(titles(&live), titles(&h));
        assert_eq!(
            titles(&h),
            vec![
                ("A".to_string(), vec!["aka".to_string(), "ata".to_string()]),
                ("K".to_string(), vec!["kala".to_string(), "kata".to_string()]),
                ("T".to_string(), vec!["ta".to_string(), "tika".to_string()]),
            ]
        );
        let dict = h.children_of(Some(book.id)).into_iter().find(|n| n.title == "Dictionary").unwrap();
        let a = h.children_of(Some(dict.id))[0];
        assert_eq!(a.order, 1);
        assert!(tmp.path().join(h.fs_path(a, &layout)).is_dir(), "A bucket directory renamed to 01-");
        for n in h.iter().filter(|n| n.kind == NodeKind::Paragraph) {
            if let Some(f) = &n.file {
                if f.contains("dictionary") {
                    assert!(tmp.path().join(f).exists(), "{f} missing on disk after sort");
                    // The stored path sits in the bucket's renamed directory
                    // with the entry's new `NN-` prefix.
                    let dir = tmp.path().join(h.fs_path(h.get(n.parent_id.unwrap()).unwrap(), &layout));
                    assert_eq!(tmp.path().join(f).parent().unwrap(), dir, "{f} lives in its bucket's current directory");
                    assert!(f.rsplit('/').next().unwrap().starts_with(&format!("{:02}-", n.order)), "{f} carries order {}", n.order);
                }
            }
        }
        assert!(plan_dictionary_sort(&h, dict, &[]).is_empty(), "sorted tree plans nothing");
        // Export follows the same order.
        let entries = load_dictionary(&store, &h, &book).unwrap();
        let words: Vec<&str> = entries.iter().map(|e| e.word.as_str()).collect();
        assert_eq!(words, vec!["aka", "ata", "kala", "kata", "ta", "tika"]);
    }

    fn row(word: &str, pos: &str, gloss: &str) -> ImportEntry {
        ImportEntry {
            word: word.into(),
            pos: pos.into(),
            translation: gloss.into(),
            ..Default::default()
        }
    }

    /// 3.18 LI-3 — a batch through one `DictionaryWriter` lands the same tree
    /// the per-row path produced: alphabet buckets, NFC titles, `word (2)`
    /// homographs, the duplicate error, `hjson` content type, final body in
    /// the store and on disk — and the vector index is flushed once at
    /// `finish`, so a fresh open of the project finds it populated.
    #[test]
    #[ignore = "opens a real Store (embedding-model download); run locally with --ignored"]
    fn writer_batch_lands_the_per_row_tree_and_flushes_once() {
        let tmp = tempfile::tempdir().unwrap();
        crate::cli::init::run(tmp.path(), true, "empty").unwrap();
        let layout = ProjectLayout::new(tmp.path());
        let cfg = Config::load_layered(&layout.config_path()).unwrap();
        let store = Store::open(layout.clone(), &cfg).unwrap();
        let book = super::super::init_language(&store, &cfg, "Testlang").unwrap();

        let syl = ["ka", "ta", "pa", "ma", "na", "la"];
        let mut rows: Vec<ImportEntry> = (0..30)
            .map(|i| row(&format!("{}{}", syl[i / 6], syl[i % 6]), "noun", &format!("sense {i}")))
            .collect();
        rows.push(row("roka", "noun", "stone"));
        rows.push(row("roka", "verb", "to throw")); // homograph → `roka (2)`
        rows.push(row("roka", "noun", "again"));    // duplicate → error
        rows.push(row("e\u{0301}le", "noun", "nfc")); // NFC → `éle`

        let mut writer = DictionaryWriter::open(&store, &cfg, &book).unwrap();
        let mut ok = 0;
        let mut errors = Vec::new();
        for r in &rows {
            match writer.add_imported(r) {
                Ok(_) => ok += 1,
                Err(e) => errors.push(e.to_string()),
            }
        }
        writer.finish().unwrap();
        assert_eq!(errors.len(), 1, "{errors:?}");
        assert!(errors[0].contains("already defined as noun"), "{}", errors[0]);
        assert_eq!(ok, rows.len() - 1, "every non-duplicate row lands: {errors:?}");

        let h = Hierarchy::load(&store).unwrap();
        let dict = h
            .children_of(Some(book.id))
            .into_iter()
            .find(|n| n.title == "Dictionary")
            .unwrap();
        let buckets: Vec<String> = h
            .children_of(Some(dict.id))
            .iter()
            .filter(|n| n.kind == NodeKind::Subchapter)
            .map(|n| n.title.clone())
            .collect();
        // 3.18 LI-5: the batch lands in alphabet order, not insertion order.
        assert_eq!(buckets, ["E", "K", "M", "N", "P", "R", "T"]);
        let r = h.children_of(Some(dict.id)).into_iter().find(|n| n.title == "R").unwrap();
        let titles: Vec<String> = h.children_of(Some(r.id)).iter().map(|n| n.title.clone()).collect();
        assert_eq!(titles, ["roka", "roka (2)"]);
        let e = h.children_of(Some(dict.id)).into_iter().find(|n| n.title == "E").unwrap();
        let ele = h.children_of(Some(e.id))[0];
        assert_eq!(ele.title, "éle");
        assert_eq!(ele.content_type.as_deref(), Some("hjson"));
        let body = store.get_content(ele.id).unwrap().unwrap();
        let parsed = crate::language_entry::parse(&String::from_utf8_lossy(&body)).unwrap().unwrap();
        assert_eq!(parsed.word, "éle");
        let on_disk = std::fs::read(tmp.path().join(ele.file.as_ref().unwrap())).unwrap();
        assert_eq!(on_disk, body, "the body is written once, as the final text");

        // The index was flushed at finish: a fresh open sees every vector.
        let paragraphs = h.iter().filter(|n| n.kind == NodeKind::Paragraph).count();
        drop(store);
        let reopened = Store::open(layout, &cfg).unwrap();
        assert!(!reopened.vector_index_appears_stale());
        assert!(reopened.vector_count().unwrap() >= 2 * paragraphs);
    }
}
