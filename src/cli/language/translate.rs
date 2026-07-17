//! `inkhaven language` translation surface: forward/reverse/cross translation,
//! translation memory (remember/list), corpus build + eval, and the Output-pane
//! emit helpers shared with the Bund `ink.lang.*` verbs. Split out of the flat
//! handler.

use std::path::Path;

use crate::error::{Error, Result};

use super::*;

/// Parse a `root` or `root:gloss` argument into a syntax word.
pub(crate) fn parse_word(s: &str) -> crate::conlang::syntax::Word {
    use crate::conlang::syntax::Word;
    match s.split_once(':') {
        Some((root, gloss)) => Word { root: root.trim().to_string(), gloss: gloss.trim().to_string() },
        None => Word { root: s.trim().to_string(), gloss: s.trim().to_string() },
    }
}

/// 1.3.24 PANE-1 P2 — route a translation into the Output pane. A no-op unless an
/// Build the `{ trace, alternatives }` extra metadata for a forward translation,
/// so the Output pane's `o` expansion can show the per-word derivation.
pub(crate) fn translation_trace_json(
    t: &crate::conlang::translate::Translation,
) -> serde_json::Value {
    use crate::conlang::translate::Decision;
    let trace: Vec<serde_json::Value> = t
        .trace
        .iter()
        .map(|e| {
            let decision = match &e.decision {
                Decision::LexiconLookup { word, pos } => {
                    serde_json::json!({ "kind": "lexicon", "word": word, "pos": pos })
                }
                Decision::Untranslatable => serde_json::json!({ "kind": "untranslatable" }),
            };
            serde_json::json!({
                "source": e.source, "role": e.role, "target": e.target,
                "confidence": e.confidence, "decision": decision,
            })
        })
        .collect();
    let alternatives: Vec<serde_json::Value> = t
        .alternatives
        .iter()
        .map(|a| {
            serde_json::json!({
                "text": a.text, "source": a.source,
                "confidence": a.confidence, "rationale": a.rationale,
            })
        })
        .collect();
    serde_json::json!({ "trace": trace, "alternatives": alternatives })
}

/// Output store is active (i.e. running in the TUI), so the shell CLI is
/// unaffected. Emits a `translation_result`, plus a
/// `translation_uncovered_word_report` when the lexicon missed words.
pub(crate) fn emit_translation_output(
    language: &str,
    source: &str,
    target: &str,
    confidence: f32,
    direction: &str,
    unresolved: &[String],
    extra: serde_json::Value,
) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity};
    let mut meta = serde_json::json!({
        "text": format!("{source}  →  {target}"),
        "source": source,
        "target": target,
        "confidence": confidence,
        "direction": direction,
        "language": language,
    });
    // Merge in any kind-specific extra (the forward path passes trace +
    // alternatives, so the pane's `o` expansion has detail to show).
    if let (Some(obj), Some(ex)) = (meta.as_object_mut(), extra.as_object()) {
        for (k, v) in ex {
            obj.insert(k.clone(), v.clone());
        }
    }
    let msg = Message::new(kinds::TRANSLATION_RESULT, Severity::Info, Lifetime::Session(500), meta)
        .with_source_language(language);
    crate::pane::output::emit(&msg);

    if !unresolved.is_empty() {
        let report = Message::new(
            kinds::TRANSLATION_UNCOVERED_WORD_REPORT,
            Severity::Warning,
            Lifetime::UntilActedOn,
            serde_json::json!({
                "text": format!("{} word(s) uncovered: {}", unresolved.len(), unresolved.join(", ")),
                "uncovered_words": unresolved,
                "source_text": source,
                "language": language,
            }),
        )
        .with_source_language(language);
        crate::pane::output::emit(&report);
    }
}

/// PANE-1 P3 — route a `generate-lexicon` dry-run's surviving proposals into
/// the Output pane as a single advisory `lexicon_proposal` message. The whole
/// proposal list rides in `metadata.proposals` (form / gloss / pos / example /
/// register / domain / era) so the pane's `Enter`/promote action can commit the
/// batch to the Dictionary without re-running the model. A no-op when no Output
/// store is installed (plain CLI) — it only surfaces in the TUI or a Bund run
/// that has the pane active.
pub(crate) fn emit_lexicon_proposal(
    language: &str,
    topic: Option<&str>,
    era: Option<&str>,
    proposals: &[crate::conlang::generate::lexicon::LexProposal],
) {
    use crate::pane::output::{kinds, ActionId, Lifetime, Message, Severity};
    if proposals.is_empty() {
        return;
    }
    let items: Vec<serde_json::Value> = proposals
        .iter()
        .map(|p| {
            serde_json::json!({
                "form": p.form.trim(),
                "gloss": p.gloss.trim(),
                "pos": if p.pos.trim().is_empty() { "noun" } else { p.pos.trim() },
                "example": p.example.trim(),
                "register": p.register.trim(),
                "domain": p.domain.iter()
                    .map(|d| d.trim().to_string())
                    .filter(|d| !d.is_empty())
                    .collect::<Vec<_>>(),
            })
        })
        .collect();
    let preview = proposals
        .iter()
        .take(4)
        .map(|p| format!("{} ({})", p.form.trim(), p.gloss.trim()))
        .collect::<Vec<_>>()
        .join(", ");
    let more = if proposals.len() > 4 {
        format!(" +{} more", proposals.len() - 4)
    } else {
        String::new()
    };
    let topic_s = topic.map(|t| format!(" · {t}")).unwrap_or_default();
    let meta = serde_json::json!({
        "text": format!("{} proposed word(s){topic_s}: {preview}{more}", proposals.len()),
        "language": language,
        "topic": topic.unwrap_or(""),
        "era": era.unwrap_or(""),
        "count": proposals.len(),
        "proposals": items,
    });
    let msg = Message::new(
        kinds::LEXICON_PROPOSAL,
        Severity::Info,
        Lifetime::UntilActedOn,
        meta,
    )
    .with_source_language(language)
    // Promote (accept-into-Dictionary) leads; the defaults follow.
    .with_actions(vec![
        ActionId::Promote,
        ActionId::Dismiss,
        ActionId::Expand,
        ActionId::Pin,
    ]);
    crate::pane::output::emit(&msg);
}

/// PANE-1 P3 — route a `lect` (render-in-variety) result into the Output pane as
/// a `variety_rendering` message. Each base→variety pair rides in
/// `metadata.renderings`; `o` expands the full list. A no-op outside the TUI /
/// an active-pane Bund run.
pub(crate) fn emit_variety_rendering(
    language: &str,
    variety: &str,
    summary: &str,
    items: &[(String, String)],
) {
    use crate::pane::output::{kinds, Lifetime, Message, Severity};
    if items.is_empty() {
        return;
    }
    let renderings: Vec<serde_json::Value> = items
        .iter()
        .map(|(base, rendered)| serde_json::json!({ "base": base, "rendered": rendered }))
        .collect();
    let preview = items
        .iter()
        .map(|(b, r)| if b == r { b.clone() } else { format!("{b}→{r}") })
        .collect::<Vec<_>>()
        .join("  ");
    let meta = serde_json::json!({
        "text": format!("{language} · {variety}: {preview}"),
        "language": language,
        "variety": variety,
        "summary": summary,
        "renderings": renderings,
    });
    let msg = Message::new(
        kinds::VARIETY_RENDERING,
        Severity::Info,
        Lifetime::Session(500),
        meta,
    )
    .with_source_language(language);
    crate::pane::output::emit(&msg);
}

/// 1.3.23 LANG-3 P0 — translate English into the conlang (Tier 1, RBMT).
pub(crate) fn translate(project: &Path, language: &str, text: &str, trace: bool, json: bool) -> Result<()> {
    use crate::conlang::translate::{self, Decision};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    // Phonology is only needed for inflection/allophony; without it the engine
    // still orders and concatenates the mapped roots.
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    // Tier 2 (retrieval): layer the translation memory over the rule-based result.
    // A non-empty memory gets a semantic query embedding (an exact hit needs none).
    let mem = translate::memory::TranslationMemory::load(project, language)
        .map_err(|e| Error::Store(format!("loading translation memory: {e}")))?;
    let query_embedding: Option<Vec<f32>> = if mem.is_empty() {
        None
    } else {
        store.embed_batch(&[text]).ok().and_then(|mut v| (!v.is_empty()).then(|| v.remove(0)))
    };
    let t = translate::apply_memory(
        translate::translate(&phon, &morph, &grammar_spec.grammar, &entries, text),
        &mem,
        query_embedding.as_deref(),
    );
    emit_translation_output(language, text, &t.target, t.confidence, "forward", &t.unresolved, translation_trace_json(&t));

    if json {
        let trace_json: Vec<serde_json::Value> = t
            .trace
            .iter()
            .map(|e| {
                let decision = match &e.decision {
                    Decision::LexiconLookup { word, pos } => {
                        serde_json::json!({ "kind": "lexicon", "word": word, "pos": pos })
                    }
                    Decision::Untranslatable => serde_json::json!({ "kind": "untranslatable" }),
                };
                serde_json::json!({
                    "source": e.source, "role": e.role, "target": e.target,
                    "confidence": e.confidence, "decision": decision,
                })
            })
            .collect();
        let alternatives_json: Vec<serde_json::Value> = t
            .alternatives
            .iter()
            .map(|a| {
                serde_json::json!({
                    "text": a.text, "source": a.source,
                    "confidence": a.confidence, "rationale": a.rationale,
                })
            })
            .collect();
        let out = serde_json::json!({
            "source": t.source,
            "target": t.target,
            "literal": t.literal,
            "confidence": t.confidence,
            "unresolved": t.unresolved,
            "alternatives": alternatives_json,
            "trace": trace_json,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing translation: {e}")))?
        );
        return Ok(());
    }

    let order = grammar_spec.grammar.get("word_order").map(String::as_str).unwrap_or("svo");
    println!(
        "{}  ({} · {} · confidence {:.2})",
        t.target,
        order.to_uppercase(),
        t.tier.label(),
        t.confidence
    );

    // Interlinear: surface words over their glosses.
    if !t.words.is_empty() {
        let widths: Vec<usize> =
            t.words.iter().map(|(w, g)| w.chars().count().max(g.chars().count()) + 2).collect();
        let mut line1 = String::from("  ");
        let mut line2 = String::from("  ");
        for (i, (w, g)) in t.words.iter().enumerate() {
            line1.push_str(&format!("{:<width$}", w, width = widths[i]));
            line2.push_str(&format!("{:<width$}", g, width = widths[i]));
        }
        println!("{line1}");
        println!("{line2}");
    }
    println!("  '{}'", t.literal);

    for a in &t.alternatives {
        println!("  alt: {}  ({})", a.text, a.rationale);
    }
    if !t.unresolved.is_empty() {
        eprintln!(
            "\nnot in {language}'s lexicon: {} — coin them, or add with `language add-word`",
            t.unresolved.join(", ")
        );
    }
    if trace {
        eprintln!("\ntrace:");
        for e in &t.trace {
            let how = match &e.decision {
                Decision::LexiconLookup { word, pos } => format!("lexicon → {word} ({pos})"),
                Decision::Untranslatable => "untranslatable (passed through)".to_string(),
            };
            eprintln!("  {:<8} {:<12} {how}  [{:.2}]", e.role, e.source, e.confidence);
        }
    }
    Ok(())
}

/// 1.3.23 LANG-3 P0 — reverse a conlang sentence back into English (Tier 1).
pub(crate) fn reverse_translate(project: &Path, language: &str, text: &str, json: bool) -> Result<()> {
    use crate::conlang::translate::reverse;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let r = reverse::reverse(&phon, &morph, &grammar_spec.grammar, &entries, text);
    emit_translation_output(language, text, &r.english, r.confidence, "reverse", &r.unresolved, serde_json::Value::Null);

    if json {
        let out = serde_json::json!({
            "source": r.source, "english": r.english,
            "confidence": r.confidence, "unresolved": r.unresolved,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing reverse: {e}")))?
        );
        return Ok(());
    }

    println!("{}  (Tier 1 RBMT · confidence {:.2})", r.english, r.confidence);
    let gl: Vec<String> = r.words.iter().map(|(w, g)| format!("{w}={g}")).collect();
    println!("  {}", gl.join("  "));
    if !r.unresolved.is_empty() {
        eprintln!("\nnot in {language}'s lexicon: {}", r.unresolved.join(", "));
    }
    Ok(())
}

/// 1.3.23 LANG-3 P0 — cross-translate conlang `from` → conlang `to` via English.
pub(crate) fn cross_translate(project: &Path, from: &str, to: &str, text: &str, json: bool) -> Result<()> {
    use crate::conlang::translate::reverse::{self, LangCtx};
    // Open the project once; resolve both language books from the same store.
    let (store, hierarchy, from_book) = open_lang_book(project, from)?;
    let to_book = find_language_book(&hierarchy, to)?;

    let f_phon = load_phonology(&store, &hierarchy, &from_book)?.unwrap_or_default();
    let f_morph = load_morphology(&store, &hierarchy, &from_book)?.unwrap_or_default();
    let (f_spec, _) = load_grammar_spec(&store, &hierarchy, &from_book)?;
    let f_entries = load_dictionary(&store, &hierarchy, &from_book)?;

    let t_phon = load_phonology(&store, &hierarchy, &to_book)?.unwrap_or_default();
    let t_morph = load_morphology(&store, &hierarchy, &to_book)?.unwrap_or_default();
    let (t_spec, _) = load_grammar_spec(&store, &hierarchy, &to_book)?;
    let t_entries = load_dictionary(&store, &hierarchy, &to_book)?;

    let fromctx =
        LangCtx { phon: &f_phon, morph: &f_morph, typology: &f_spec.grammar, entries: &f_entries };
    let toctx =
        LangCtx { phon: &t_phon, morph: &t_morph, typology: &t_spec.grammar, entries: &t_entries };
    let c = reverse::cross(&fromctx, &toctx, text);
    emit_translation_output(from, text, &c.target, c.confidence, "cross", &c.unresolved, serde_json::Value::Null);

    if json {
        let out = serde_json::json!({
            "source": c.source, "english": c.english, "target": c.target,
            "confidence": c.confidence, "unresolved": c.unresolved,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing cross: {e}")))?
        );
        return Ok(());
    }

    println!("{from} → {to}  (via English · confidence {:.2})", c.confidence);
    println!("  {}  ({})", c.source, from);
    println!("  ↓ {}", c.english);
    println!("  {}  ({})", c.target, to);
    if !c.unresolved.is_empty() {
        eprintln!("\nlost in pivot: {}", c.unresolved.join(", "));
    }
    Ok(())
}

/// Compute and cache embeddings for any memory pairs missing one, via the
/// store's embedder. Best-effort: an embedding failure leaves the lexical path
/// intact (semantic retrieval is an upgrade, never a hard dependency).
pub(crate) fn embed_pending_memory(
    store: &Store,
    mem: &mut crate::conlang::translate::memory::TranslationMemory,
) {
    let need = mem.needs_embeddings();
    if need.is_empty() {
        return;
    }
    let refs: Vec<&str> = need.iter().map(String::as_str).collect();
    if let Ok(vecs) = store.embed_batch(&refs) {
        for (en, v) in need.iter().zip(vecs) {
            mem.set_embedding(en, v);
        }
    }
}

/// 1.3.23 LANG-3 P1 — remember a confirmed translation (the correction loop).
pub(crate) fn remember(project: &Path, language: &str, english: &str, conlang: &str) -> Result<()> {
    use crate::conlang::translate::memory::TranslationMemory;
    let (store, _hierarchy, _lang_book) = open_lang_book(project, language)?;
    let mut mem = TranslationMemory::load(project, language)
        .map_err(|e| Error::Store(format!("loading translation memory: {e}")))?;
    mem.add(english, conlang);
    embed_pending_memory(&store, &mut mem);
    mem.save(project, language)
        .map_err(|e| Error::Store(format!("saving translation memory: {e}")))?;
    println!("remembered: {english}  →  {conlang}");
    eprintln!("({} translation(s) in {language}'s memory)", mem.len());
    Ok(())
}

/// 1.3.23 LANG-3 P1 — list a language's translation memory.
pub(crate) fn memory_list(project: &Path, language: &str) -> Result<()> {
    use crate::conlang::translate::memory::TranslationMemory;
    let _ = open_lang_book(project, language)?;
    let mem = TranslationMemory::load(project, language)
        .map_err(|e| Error::Store(format!("loading translation memory: {e}")))?;
    if mem.is_empty() {
        println!(
            "{language} has no remembered translations yet — add one with \
             `language remember {language} --english … --conlang …`."
        );
        return Ok(());
    }
    println!("{language} · {} remembered translation(s):", mem.len());
    for (en, con) in mem.entries() {
        println!("  {en}  →  {con}");
    }
    Ok(())
}

/// 1.3.23 LANG-3 P1 — generate a synthetic corpus and (with --yes) seed memory.
pub(crate) fn corpus(
    project: &Path,
    language: &str,
    pool_path: Option<&str>,
    limit: Option<usize>,
    commit: bool,
    json: bool,
) -> Result<()> {
    use crate::conlang::translate::{corpus as corpusmod, memory::TranslationMemory};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let pool_text = match pool_path {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| Error::Config(format!("reading pool `{p}`: {e}")))?,
        None => corpusmod::BUNDLED_POOL.to_string(),
    };
    let mut pool = corpusmod::parse_pool(&pool_text);
    if let Some(n) = limit {
        pool.truncate(n);
    }
    let report = corpusmod::generate(&phon, &morph, &grammar_spec.grammar, &entries, &pool);

    if commit && !report.accepted.is_empty() {
        let mut mem = TranslationMemory::load(project, language)
            .map_err(|e| Error::Store(format!("loading translation memory: {e}")))?;
        for (en, con) in &report.accepted {
            mem.add(en, con);
        }
        embed_pending_memory(&store, &mut mem);
        mem.save(project, language)
            .map_err(|e| Error::Store(format!("saving translation memory: {e}")))?;
    }

    if json {
        let out = serde_json::json!({
            "scanned": report.scanned,
            "accepted": report.accepted.len(),
            "acceptance_rate": report.acceptance_rate(),
            "top_missing": report.top_missing(10),
            "committed": commit,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing corpus report: {e}")))?
        );
        return Ok(());
    }

    println!(
        "{language} · corpus: {}/{} accepted ({:.0}% coverage)",
        report.accepted.len(),
        report.scanned,
        report.acceptance_rate() * 100.0
    );
    for (en, con) in report.accepted.iter().take(6) {
        println!("  {en}  →  {con}");
    }
    if report.accepted.len() > 6 {
        println!("  … and {} more", report.accepted.len() - 6);
    }
    let missing = report.top_missing(8);
    if !missing.is_empty() {
        let list: Vec<String> = missing.iter().map(|(w, n)| format!("{w} ({n})")).collect();
        eprintln!("\ntop missing words: {}", list.join(", "));
        eprintln!("add them with `language add-word` to raise coverage.");
    }
    if commit {
        eprintln!("\nseeded {} pair(s) into {language}'s translation memory.", report.accepted.len());
    } else {
        eprintln!("\n(preview — re-run with --yes to seed the translation memory)");
    }
    Ok(())
}

/// 1.3.23 LANG-3 P3 — export the translation system as a portable `.itm` bundle.
pub(crate) fn export_translation(project: &Path, language: &str, out: Option<&str>) -> Result<()> {
    use crate::conlang::translate::{export, memory::TranslationMemory};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let mem = TranslationMemory::load(project, language)
        .map_err(|e| Error::Store(format!("loading translation memory: {e}")))?;

    let memory_pairs: Vec<(String, String)> =
        mem.entries().map(|(e, c)| (e.to_string(), c.to_string())).collect();
    let lexicon: Vec<(String, String, String)> = entries
        .iter()
        .map(|e| (e.word.clone(), e.pos.clone(), e.translation.clone()))
        .collect();

    let meta = export::BundleMeta {
        language: language.to_string(),
        exported: chrono::Utc::now().format("%Y-%m-%d").to_string(),
        inkhaven_version: env!("CARGO_PKG_VERSION").to_string(),
        memory_pairs: memory_pairs.len(),
        lexicon_entries: lexicon.len(),
    };
    let bytes = export::bundle(&meta, &memory_pairs, &lexicon)
        .map_err(|e| Error::Store(format!("building translation bundle: {e}")))?;

    let out_path = match out {
        Some(p) => std::path::PathBuf::from(p),
        None => std::path::PathBuf::from(format!("{}-translation.itm", language.to_lowercase())),
    };
    crate::io_atomic::write(&out_path, &bytes)
        .map_err(|e| Error::Store(format!("writing {}: {e}", out_path.display())))?;
    println!("exported {language}'s translation pack → {}", out_path.display());
    eprintln!("({} memory pair(s), {} lexicon entr(y/ies))", meta.memory_pairs, meta.lexicon_entries);
    Ok(())
}

/// Cosine similarity of two embedding vectors.
pub(crate) fn cosine_sim(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for i in 0..a.len() {
        dot += a[i] * b[i];
        na += a[i] * a[i];
        nb += b[i] * b[i];
    }
    if na == 0.0 || nb == 0.0 {
        0.0
    } else {
        dot / (na.sqrt() * nb.sqrt())
    }
}

/// 1.3.23 LANG-3 P2 — evaluate translation quality (round-trip + coverage).
pub(crate) fn eval(
    project: &Path,
    language: &str,
    test_set: Option<&str>,
    limit: Option<usize>,
    json: bool,
) -> Result<()> {
    use crate::conlang::translate::{corpus as corpusmod, eval as evalmod};
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;

    let pool_text = match test_set {
        Some(p) => std::fs::read_to_string(p)
            .map_err(|e| Error::Config(format!("reading test set `{p}`: {e}")))?,
        None => corpusmod::BUNDLED_POOL.to_string(),
    };
    let mut sentences = corpusmod::parse_pool(&pool_text);
    if let Some(n) = limit {
        sentences.truncate(n);
    }

    let items = evalmod::round_trip_all(&phon, &morph, &grammar_spec.grammar, &entries, &sentences);
    let coverage = evalmod::coverage(&items);
    let covered: Vec<&evalmod::RoundTrip> = items.iter().filter(|i| i.covered).collect();

    // Round-trip similarity: embed each source and its recovered English, cosine.
    let round_trip: Option<f32> = if covered.is_empty() {
        None
    } else {
        let mut texts: Vec<&str> = Vec::with_capacity(covered.len() * 2);
        for i in &covered {
            texts.push(i.english.as_str());
            texts.push(i.recovered.as_str());
        }
        match store.embed_batch(&texts) {
            Ok(emb) => {
                let sum: f32 = (0..covered.len()).map(|k| cosine_sim(&emb[2 * k], &emb[2 * k + 1])).sum();
                Some(sum / covered.len() as f32)
            }
            Err(_) => None,
        }
    };

    if json {
        let out = serde_json::json!({
            "sentences": items.len(),
            "covered": covered.len(),
            "coverage": coverage,
            "round_trip_similarity": round_trip,
        });
        println!(
            "{}",
            serde_json::to_string_pretty(&out)
                .map_err(|e| Error::Store(format!("serializing eval: {e}")))?
        );
        return Ok(());
    }

    println!("{language} · evaluation · {} sentence(s)", items.len());
    println!("  coverage:    {:.0}%  ({}/{} fully translatable)", coverage * 100.0, covered.len(), items.len());
    match round_trip {
        Some(s) => println!("  round-trip:  {s:.2}  (mean source↔recovered similarity over the {} covered)", covered.len()),
        None => println!("  round-trip:  n/a  (no covered sentences to round-trip)"),
    }
    Ok(())
}

/// LANG-1 syntax — assemble a sentence from its parts and print the clause.
#[allow(clippy::too_many_arguments)]
pub(crate) fn sentence(
    project: &Path,
    language: &str,
    subject: Option<&str>,
    subject_number: &str,
    subject_person: &str,
    subject_adj: Option<&str>,
    verb: Option<&str>,
    object: Option<&str>,
    object_number: &str,
    object_adj: Option<&str>,
    noun_paradigm: &str,
    verb_paradigm: &str,
    negate: bool,
    negator: Option<&str>,
    question: bool,
    q_particle: Option<&str>,
) -> Result<()> {
    use crate::conlang::syntax::{self, Clause, NounPhrase};

    if subject.is_none() && verb.is_none() && object.is_none() {
        return Err(Error::Config("give at least a --subject and a --verb".into()));
    }

    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block"))
    })?;
    let morph = load_morphology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let (grammar_spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;

    let np = |w: Option<&str>, number: &str, adj: Option<&str>| -> Option<NounPhrase> {
        w.map(|w| NounPhrase {
            head: parse_word(w),
            number: number.to_string(),
            adjective: adj.map(parse_word),
        })
    };

    let clause = Clause {
        subject: np(subject, subject_number, subject_adj),
        verb: verb.map(parse_word),
        verb_person: subject_person.to_string(),
        object: np(object, object_number, object_adj),
        noun_paradigm: noun_paradigm.to_string(),
        verb_paradigm: verb_paradigm.to_string(),
        negated: negate,
        negator: negator.map(parse_word),
        question,
        question_particle: q_particle.map(parse_word),
    };

    let rendered = syntax::assemble(&phon, &morph, &grammar_spec.grammar, &clause);

    let order = grammar_spec.grammar.get("word_order").map(String::as_str).unwrap_or("svo");
    println!("{} ({} order)", rendered.surface, order.to_uppercase());
    // Interlinear: surface words over their glosses.
    let surf: Vec<&str> = rendered.words.iter().map(|(w, _)| w.as_str()).collect();
    let gl: Vec<&str> = rendered.words.iter().map(|(_, g)| g.as_str()).collect();
    let widths: Vec<usize> =
        rendered.words.iter().map(|(w, g)| w.chars().count().max(g.chars().count()) + 2).collect();
    let mut line1 = String::from("  ");
    let mut line2 = String::from("  ");
    for (i, w) in surf.iter().enumerate() {
        line1.push_str(&format!("{:<width$}", w, width = widths[i]));
        line2.push_str(&format!("{:<width$}", gl[i], width = widths[i]));
    }
    println!("{line1}");
    println!("{line2}");
    println!("  '{}'", rendered.literal);
    Ok(())
}
