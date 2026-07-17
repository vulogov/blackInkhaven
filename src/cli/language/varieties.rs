//! `inkhaven language` varieties & contact surface: declared varieties, lect
//! rendering, dialect proposal, loan/borrowing, and areal-feature checks. Split
//! out of the flat handler (the loaders + proposers live in the parent module).

use crate::error::{Error, Result};

use super::*;

/// LANG-2 P1 — list a language's declared varieties.
pub(crate) fn varieties(project: &Path, language: &str) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let vs = load_varieties(&store, &hierarchy, &lang_book)?;
    if vs.varieties.is_empty() {
        println!(
            "{language} declares no varieties — add a `{{ varieties: [ … ] }}` block to its \
             Grammar chapter (see Documentation/CONLANG.md)."
        );
        return Ok(());
    }
    println!("{language} · {} variet(y/ies):", vs.varieties.len());
    for v in &vs.varieties {
        println!("  {:<14} {}", v.id, v.summary());
        if let Some(note) = &v.note {
            println!("  {:<14} {}", "", note);
        }
    }
    Ok(())
}

/// LANG-2 P1 — render a form / text in a variety.
pub(crate) fn lect(
    project: &Path,
    language: &str,
    variety: &str,
    word: Option<&str>,
    text: Option<&str>,
) -> Result<()> {
    use crate::conlang::variety as varengine;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let vs = load_varieties(&store, &hierarchy, &lang_book)?;
    let v = vs.get(variety).ok_or_else(|| {
        Error::Config(format!(
            "language `{language}` has no variety `{variety}` (try `inkhaven language varieties {language}`)"
        ))
    })?;
    println!("{language} · {} ({})", v.id, v.summary());
    let mut items: Vec<(String, String)> = Vec::new();
    if let Some(w) = word {
        let rendered = varengine::render_form(&phon, v, w);
        let mark = if rendered == w { "  (unchanged)" } else { "" };
        println!("  {w}  →  {rendered}{mark}");
        items.push((w.to_string(), rendered));
    }
    if let Some(t) = text {
        let rendered = varengine::render_text(&phon, v, t);
        println!("  base    {t}");
        println!("  {:<7} {}", v.id, rendered);
        items.push((t.to_string(), rendered));
    }
    if word.is_none() && text.is_none() {
        return Err(Error::Config("give --word <form> or --text \"…\"".into()));
    }
    // PANE-1 P3 — mirror the rendering into the Output pane.
    emit_variety_rendering(language, &v.id, &v.summary(), &items);
    Ok(())
}

/// LANG-2 P1 — a dialect-comparison table across every declared variety.
pub(crate) fn dialects(project: &Path, language: &str, count: usize) -> Result<()> {
    use crate::conlang::variety as varengine;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.unwrap_or_default();
    let vs = load_varieties(&store, &hierarchy, &lang_book)?;
    if vs.varieties.is_empty() {
        println!("{language} declares no varieties to compare.");
        return Ok(());
    }
    let entries = load_dictionary(&store, &hierarchy, &lang_book)?;
    let sample: Vec<_> = entries.iter().take(count).collect();
    if sample.is_empty() {
        println!("{language} has no dictionary entries to compare.");
        return Ok(());
    }

    // Column header: gloss | base | <each variety id>.
    let mut header = vec!["gloss".to_string(), "base".to_string()];
    header.extend(vs.varieties.iter().map(|v| v.id.clone()));
    // Build rows, tracking the widest cell per column for alignment.
    let mut rows: Vec<Vec<String>> = Vec::new();
    for e in &sample {
        let mut row = vec![e.translation.clone(), e.word.clone()];
        for v in &vs.varieties {
            let (form, overridden) = varengine::render_concept(&phon, v, &e.translation, &e.word);
            row.push(if overridden { format!("{form}*") } else { form });
        }
        rows.push(row);
    }
    let cols = header.len();
    let widths: Vec<usize> = (0..cols)
        .map(|c| {
            header[c]
                .chars()
                .count()
                .max(rows.iter().map(|r| r[c].chars().count()).max().unwrap_or(0))
        })
        .collect();
    let fmt_row = |r: &[String]| {
        r.iter()
            .enumerate()
            .map(|(c, cell)| format!("{:<width$}", cell, width = widths[c]))
            .collect::<Vec<_>>()
            .join("  ")
    };
    println!("{language} · dialect comparison ({} entries, * = word override):", sample.len());
    println!("  {}", fmt_row(&header));
    for r in &rows {
        println!("  {}", fmt_row(r));
    }
    Ok(())
}

/// LANG-2 P2 — borrow (nativise) a donor form into a recipient language.
pub(crate) fn borrow(
    project: &Path,
    language: &str,
    form: &str,
    from: Option<&str>,
    gloss: Option<&str>,
    pos: &str,
    commit: bool,
) -> Result<()> {
    use crate::conlang::contact;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let phon = load_phonology(&store, &hierarchy, &lang_book)?.ok_or_else(|| {
        Error::Config(format!("language `{language}` has no phoneme block to borrow into"))
    })?;
    let loan = load_loan_phonology(&store, &hierarchy, &lang_book)?;
    let a = contact::adapt(&phon, &loan, form);

    let donor_lang = from.map(|f| format!(" from {f}")).unwrap_or_default();
    println!("{language} borrows{donor_lang}: {form}  →  {}", a.adapted);
    if !a.ipa.is_empty() {
        println!("  /{}/", a.ipa.join(""));
    }
    if a.steps.is_empty() {
        println!("  (already legal — no repair needed)");
    } else {
        for s in &a.steps {
            println!("  · {s}");
        }
    }

    if commit {
        let g = gloss.ok_or_else(|| {
            Error::Config("--yes needs --gloss (the loanword's meaning)".into())
        })?;
        let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
        let etymology = match from {
            Some(f) => format!("borrowed from {f} ({form})"),
            None => format!("borrowed ({form})"),
        };
        let entry = ImportEntry {
            word: a.adapted.clone(),
            pos: pos.to_string(),
            translation: g.to_string(),
            etymology,
            ..Default::default()
        };
        match add_imported_dictionary_entry(&store, &cfg, &lang_book, &entry) {
            Ok(_) => eprintln!("\nadded `{}` ({g}) to {language}'s Dictionary", a.adapted),
            Err(e) => eprintln!("\nnot added: {e}"),
        }
    } else {
        eprintln!("\n(advisory — re-run with --yes --gloss <meaning> to add it)");
    }
    Ok(())
}

/// LANG-2 P3 — areal (Sprachbund) convergence: per-language overlay, or the
/// whole-world regional view when no language is named.
pub(crate) fn areal(project: &Path, language: Option<&str>) -> Result<()> {
    use crate::conlang::contact::{converge, ArealStatus};
    let mark = |s: ArealStatus| match s {
        ArealStatus::Converged => "✓",
        ArealStatus::Shift => "→",
        ArealStatus::Adopt => "+",
    };

    match language {
        // ── per-language convergence overlay ──────────────────────────────
        Some(lang) => {
            let (store, hierarchy, lang_book) = open_lang_book(project, lang)?;
            let Some(contact) = load_contact(&store, &hierarchy, &lang_book)? else {
                println!(
                    "{lang} declares no `contact` block — add `{{ contact: {{ region, with, \
                     areal_features }} }}` to its Grammar chapter."
                );
                return Ok(());
            };
            let (spec, _) = load_grammar_spec(&store, &hierarchy, &lang_book)?;
            println!("{lang} · contact: {}", if contact.region.is_empty() { "(unnamed area)" } else { &contact.region });
            if !contact.with.is_empty() {
                println!("  in contact with: {}", contact.with.join(", "));
            }
            if contact.areal_features.is_empty() {
                return Ok(());
            }
            println!("  areal features (advisory overlay — grammar is unchanged):");
            for c in converge(&spec.grammar, &contact.areal_features) {
                let detail = match c.status {
                    ArealStatus::Converged => format!("already {}", c.areal_value),
                    ArealStatus::Shift => format!(
                        "would shift {} → {}",
                        c.current.as_deref().unwrap_or("?"),
                        c.areal_value
                    ),
                    ArealStatus::Adopt => format!("would adopt {} (currently unset)", c.areal_value),
                };
                println!("    {} {:<16} {detail}", mark(c.status), c.feature);
            }
        }
        // ── regional Sprachbund view across every language ────────────────
        None => {
            let layout = ProjectLayout::new(project);
            layout.require_initialized()?;
            let cfg = Config::load_layered(&layout.config_path())?;
            let store = Store::open(layout, &cfg)?;
            let hierarchy = Hierarchy::load(&store)?;
            // region → (members, merged areal_features)
            let mut regions: std::collections::BTreeMap<
                String,
                (Vec<crate::store::node::Node>, std::collections::BTreeMap<String, String>),
            > = std::collections::BTreeMap::new();
            for book in all_language_books(&hierarchy) {
                if let Some(c) = load_contact(&store, &hierarchy, &book)? {
                    let key = if c.region.is_empty() { "(unnamed area)".to_string() } else { c.region.clone() };
                    let entry = regions.entry(key).or_default();
                    entry.0.push(book);
                    for (f, v) in c.areal_features {
                        entry.1.entry(f).or_insert(v);
                    }
                }
            }
            if regions.is_empty() {
                println!("no contact areas declared — add a `contact` block to a language's Grammar.");
                return Ok(());
            }
            for (region, (members, features)) in &regions {
                let names: Vec<&str> = members.iter().map(|m| m.title.as_str()).collect();
                println!("{region} — {}", names.join(", "));
                if features.is_empty() {
                    continue;
                }
                for (f, av) in features {
                    let cells: Vec<String> = members
                        .iter()
                        .map(|m| {
                            let (spec, _) = load_grammar_spec(&store, &hierarchy, m).unwrap_or_default();
                            let mut one = std::collections::BTreeMap::new();
                            one.insert(f.clone(), av.clone());
                            let status = converge(&spec.grammar, &one)[0].status;
                            format!("{} {}", m.title, mark(status))
                        })
                        .collect();
                    println!("  {f} = {av:<18}  {}", cells.join("  "));
                }
            }
        }
    }
    Ok(())
}
