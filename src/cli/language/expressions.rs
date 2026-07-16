//! `inkhaven language` idiom & metaphor surface: the expressions block
//! (idioms / metaphors), its load/save, and the add/list handlers. Split out of
//! the flat handler.

use crate::error::{Error, Result};

use super::*;

/// Load the `{ idioms: [...], metaphors: [...] }` block from the Grammar
/// chapter + the paragraph node that holds it.
pub(crate) fn load_expressions(
    store: &Store,
    hierarchy: &Hierarchy,
    lang_book: &crate::store::node::Node,
) -> Result<(crate::conlang::types::expression::Expressions, Option<crate::store::node::Node>)> {
    use crate::conlang::types::expression::Expressions;
    let Some(chapter) = hierarchy
        .children_of(Some(lang_book.id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Grammar"))
        .cloned()
    else {
        return Ok((Expressions::default(), None));
    };
    for para in hierarchy.children_of(Some(chapter.id)) {
        if para.kind != NodeKind::Paragraph {
            continue;
        }
        let Ok(Some(bytes)) = store.get_content(para.id) else { continue };
        if let Ok(Some(e)) = Expressions::from_hjson(&String::from_utf8_lossy(&bytes)) {
            return Ok((e, Some(para.clone())));
        }
    }
    Ok((Expressions::default(), None))
}

pub(crate) fn save_expressions(
    project: &Path,
    store: &Store,
    lang_book: &crate::store::node::Node,
    node: Option<crate::store::node::Node>,
    expr: &crate::conlang::types::expression::Expressions,
) -> Result<()> {
    let cfg = Config::load_layered(&ProjectLayout::new(project).config_path())?;
    let body = serde_json::to_string_pretty(expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, &cfg, lang_book, "expressions", node, &body)
}

/// LANG-1 P3.5 — add an idiom.
pub(crate) fn idiom_add(
    project: &Path,
    language: &str,
    form: &str,
    literal: Option<&str>,
    meaning: &str,
    register: Option<&str>,
) -> Result<()> {
    use crate::conlang::types::expression::Idiom;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (mut expr, node) = load_expressions(&store, &hierarchy, &lang_book)?;
    expr.idioms.push(Idiom {
        form: form.trim().to_string(),
        literal: literal.unwrap_or("").trim().to_string(),
        meaning: meaning.trim().to_string(),
        register: register.map(|r| vec![r.trim().to_string()]).unwrap_or_default(),
    });
    save_expressions(project, &store, &lang_book, node, &expr)?;
    eprintln!("{language}: added idiom `{}` ({} total)", form.trim(), expr.idioms.len());
    Ok(())
}

/// LANG-1 P3.5 — declare a conceptual metaphor.
pub(crate) fn metaphor_add(
    project: &Path,
    language: &str,
    source: &str,
    target: &str,
    example: Option<&str>,
) -> Result<()> {
    use crate::conlang::types::expression::Metaphor;
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (mut expr, node) = load_expressions(&store, &hierarchy, &lang_book)?;
    expr.metaphors.push(Metaphor {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
        examples: example.map(|e| vec![e.trim().to_string()]).unwrap_or_default(),
        note: String::new(),
    });
    save_expressions(project, &store, &lang_book, node, &expr)?;
    eprintln!(
        "{language}: declared metaphor {} → {} ({} total)",
        source.trim(),
        target.trim(),
        expr.metaphors.len()
    );
    Ok(())
}

/// LANG-1 P3.5 — list idioms + metaphors.
pub(crate) fn idioms_list(project: &Path, language: &str) -> Result<()> {
    let (store, hierarchy, lang_book) = open_lang_book(project, language)?;
    let (expr, _) = load_expressions(&store, &hierarchy, &lang_book)?;
    if expr.idioms.is_empty() && expr.metaphors.is_empty() {
        println!("{language}: no idioms or metaphors yet");
        return Ok(());
    }
    if !expr.idioms.is_empty() {
        println!("idioms ({}):", expr.idioms.len());
        for i in &expr.idioms {
            let reg = if i.register.is_empty() { String::new() } else { format!("  [{}]", i.register.join(",")) };
            println!("  {}  —  {}{}", i.form, i.meaning, reg);
            if !i.literal.trim().is_empty() {
                println!("      (lit. {})", i.literal);
            }
        }
    }
    if !expr.metaphors.is_empty() {
        println!("\nmetaphors ({}):", expr.metaphors.len());
        for m in &expr.metaphors {
            let ex = if m.examples.is_empty() { String::new() } else { format!("  e.g. {}", m.examples.join("; ")) };
            println!("  {} → {}{}", m.source, m.target, ex);
        }
    }
    Ok(())
}
/// Append an idiom to the language's expressions block (store-based).
pub(crate) fn add_idiom(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    form: &str,
    literal: &str,
    meaning: &str,
) -> Result<()> {
    use crate::conlang::types::expression::Idiom;
    let hierarchy = Hierarchy::load(store)?;
    let (mut expr, node) = load_expressions(store, &hierarchy, lang_book)?;
    expr.idioms.push(Idiom {
        form: form.trim().to_string(),
        literal: literal.trim().to_string(),
        meaning: meaning.trim().to_string(),
        register: Vec::new(),
    });
    let body = serde_json::to_string_pretty(&expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, cfg, lang_book, "expressions", node, &body)
}

/// Append a conceptual metaphor to the language's expressions block.
pub(crate) fn add_metaphor(
    store: &Store,
    cfg: &Config,
    lang_book: &crate::store::node::Node,
    source: &str,
    target: &str,
) -> Result<()> {
    use crate::conlang::types::expression::Metaphor;
    let hierarchy = Hierarchy::load(store)?;
    let (mut expr, node) = load_expressions(store, &hierarchy, lang_book)?;
    expr.metaphors.push(Metaphor {
        source: source.trim().to_string(),
        target: target.trim().to_string(),
        examples: Vec::new(),
        note: String::new(),
    });
    let body = serde_json::to_string_pretty(&expr)
        .map_err(|e| Error::Store(format!("serializing expressions: {e}")))?;
    upsert_grammar_paragraph(store, cfg, lang_book, "expressions", node, &body)
}
