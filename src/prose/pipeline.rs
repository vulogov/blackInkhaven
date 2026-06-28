//! NARR-1 — the compute pipeline: extract a book's prose by chapter, compute
//! voice profiles, and persist them to `prose.duckdb` with content-hash
//! laziness and language-change invalidation.

use anyhow::Result;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::NodeKind;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::Node;

use super::profile::{self, VoiceProfile, VoiceScope};
use super::store::ProseStore;
use super::{CompiledLexicon, ProseLanguage, resolve_prose_language};

/// A chapter's prose as one stripped blob: every non-Jinja paragraph in the
/// chapter subtree, run through the Typst→plain stripper. STRUCT-1 Jinja
/// templates are excluded entirely; STRUCT-2 structural paragraphs are reduced
/// to their prose content by the stripper.
pub(crate) fn chapter_prose_text(layout: &ProjectLayout, h: &Hierarchy, chapter_id: uuid::Uuid) -> String {
    let mut out = String::new();
    for id in h.collect_subtree(chapter_id) {
        let Some(p) = h.get(id) else { continue };
        if p.kind != NodeKind::Paragraph {
            continue;
        }
        if p.content_type.as_deref() == Some("jinja") {
            continue;
        }
        let Some(rel) = p.file.as_ref() else { continue };
        if let Ok(raw) = std::fs::read_to_string(layout.root.join(rel)) {
            out.push_str(&crate::audiobook::typst_to_plain(&raw));
            out.push('\n');
        }
    }
    out
}

/// Recompute (lazily) every chapter + the book aggregate for `book`, persisting
/// to the store. Skips any scope whose stored hash already matches the current
/// stripped text. `now` is the RFC3339 timestamp stamped on recomputed rows.
/// Returns the current set of stored profiles.
pub(crate) fn refresh_book(
    store: &ProseStore,
    layout: &ProjectLayout,
    h: &Hierarchy,
    cfg: &Config,
    book: &Node,
    explicit_lang: Option<&str>,
    deep: bool,
    mattr_window: usize,
    now: &str,
) -> Result<Vec<VoiceProfile>> {
    let (lang, _note) = resolve_prose_language(explicit_lang, &cfg.language);
    // Language change → mark rows in a different language stale.
    store.mark_language_stale(&book.slug, &lang)?;

    // One compiled lexicon for the whole refresh, with the project's extras.
    let lx = CompiledLexicon::for_language_with(
        &lang,
        &cfg.prose.extra_modal_tokens,
        &cfg.prose.extra_interiority_phrases,
    );

    let chapters: Vec<&Node> = h
        .children_of(Some(book.id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Chapter)
        .collect();

    let mut full = String::new();
    for (idx, ch) in chapters.iter().enumerate() {
        let text = chapter_prose_text(layout, h, ch.id);
        full.push_str(&text);
        full.push('\n');
        refresh_scope(
            store,
            &book.slug,
            VoiceScope::Chapter((idx + 1) as u32),
            &text,
            &lang,
            &lx,
            deep,
            mattr_window,
            now,
        )?;
    }
    refresh_scope(store, &book.slug, VoiceScope::Book, &full, &lang, &lx, deep, mattr_window, now)?;

    store.get_all(&book.slug)
}

#[allow(clippy::too_many_arguments)]
fn refresh_scope(
    store: &ProseStore,
    book_slug: &str,
    scope: VoiceScope,
    text: &str,
    lang: &ProseLanguage,
    lx: &CompiledLexicon,
    deep: bool,
    mattr_window: usize,
    now: &str,
) -> Result<()> {
    let hash = profile::hash_text(text);
    if store.stored_hash(book_slug, &scope.as_str())? == Some(hash) {
        return Ok(()); // unchanged since last compute
    }
    let p = profile::compute_profile_with(text, scope, lang, lx, deep, mattr_window);
    store.upsert(book_slug, &p, now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::node::NodeKind;

    fn mk(kind: NodeKind, slug: &str, parent: Option<uuid::Uuid>, file: Option<String>) -> Node {
        serde_json::from_value(serde_json::json!({
            "id": uuid::Uuid::new_v4(), "kind": format!("{kind:?}").to_lowercase(),
            "title": slug, "slug": slug, "path": [], "parent_id": parent,
            "order": 0, "file": file, "modified_at": "2026-01-01T00:00:00Z",
        }))
        .expect("node")
    }

    #[test]
    fn extraction_excludes_jinja_and_strips() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        let layout = ProjectLayout::new(root);
        std::fs::create_dir_all(root.join("books/ch")).unwrap();
        std::fs::write(root.join("books/ch/01-p.typ"), "= Title\n\nThe sky was *blue* today.").unwrap();
        std::fs::write(root.join("books/ch/02-t.jinja"), "{{ title }} template body").unwrap();

        let chap = mk(NodeKind::Chapter, "ch", None, None);
        let chap_id = chap.id;
        let prose = Node { parent_id: Some(chap_id), file: Some("books/ch/01-p.typ".into()), ..mk(NodeKind::Paragraph, "p", Some(chap_id), Some("books/ch/01-p.typ".into())) };
        let mut jinja = mk(NodeKind::Paragraph, "t", Some(chap_id), Some("books/ch/02-t.jinja".into()));
        jinja.content_type = Some("jinja".into());

        let h = Hierarchy::from_nodes_for_test(vec![chap, prose, jinja]);
        let text = chapter_prose_text(&layout, &h, chap_id);
        assert!(text.contains("blue"), "{text:?}");
        assert!(!text.contains("template body"), "jinja leaked: {text:?}");
    }
}
