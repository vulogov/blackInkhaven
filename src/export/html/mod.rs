//! TDOC-4.1 — the HTML static-site exporter. A third consumer of the node tree:
//! one page per chapter + an index, rendered through overridable `minijinja`
//! templates (the `functional/` machinery vs `theme/` look split), localised to the
//! book's language, with images copied and TDOC-3 profiles/variables applied.

mod assets;
mod companions;
mod labels;
mod markdown_html;
mod math_html;
mod render;
mod search;
mod templates;
mod toc;
mod world_html;

use std::collections::{BTreeSet, HashSet};
use std::path::Path;

use anyhow::{bail, Result};
use uuid::Uuid;

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use labels::Labels;

/// Write the bundled default templates to `dir` so a user can customise from them.
pub use templates::eject as eject_templates;

/// Export a user book as a self-contained HTML site into `out_dir`.
///
/// `templates_override` (from `--templates <dir>`) wins over the config
/// `docs.html.template_dir`; either may be absolute or project-relative.
#[allow(clippy::too_many_arguments)]
pub fn export_html(
    layout: &ProjectLayout,
    store: &crate::store::Store,
    h: &Hierarchy,
    cfg: &Config,
    root_id: Option<Uuid>,
    profiles: &[(String, String)],
    status_floor: Option<usize>,
    out_dir: &Path,
    templates_override: Option<&Path>,
) -> Result<()> {
    let book = resolve_book(h, root_id)?;
    let hcfg = &cfg.docs.html;

    let timeline_ids: HashSet<Uuid> = h
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Chapter
                && n.system_tag.as_deref() == Some(crate::store::SYSTEM_TAG_BOOK_TIMELINE)
        })
        .map(|n| n.id)
        .collect();

    let model = toc::build(h, book.id, &timeline_ids);
    // `--templates <dir>` wins; else `<project>/<docs.html.template_dir>`. Both
    // resolve absolute paths as-is (Path::join replaces on an absolute rhs).
    let template_base = match templates_override {
        Some(p) => layout.root.join(p),
        None => layout.root.join(&hcfg.template_dir),
    };
    let tmpl = templates::load(&template_base)?;
    let site = load_site_vars(layout, &hcfg.variables_file);
    let labels = Labels::for_language(&cfg.language);
    let html_lang = Labels::html_lang(&cfg.language);
    let variables = &cfg.docs.variables;
    let book_title = hcfg.site_title.clone().unwrap_or_else(|| book.title.clone());
    let book_lang = cfg.language.clone();

    std::fs::create_dir_all(out_dir)?;
    std::fs::write(out_dir.join("theme.css"), &tmpl.css)?;

    // Assemble each page's content HTML first.
    struct Page {
        file: String,
        title: String,
        content: String,
        is_index: bool,
    }
    let mut pages: Vec<Page> = Vec::new();

    // Index — book-level front matter.
    let mut index_html = String::new();
    for id in &model.front_matter {
        let Some(node) = h.get(*id) else { continue };
        if !super::profile_matches(&node.tags, profiles) || below_floor(node, status_floor) {
            continue;
        }
        if let Some(body) = read_body(layout, node) {
            index_html.push_str(&render::render_body(&node.tags, &body, variables));
        }
    }
    pages.push(Page {
        file: "index.html".into(),
        title: labels.home.into(),
        content: index_html,
        is_index: true,
    });

    // Chapter pages.
    for ch in &model.chapters {
        let mut content = format!(
            "<h1 id=\"{}\">{}</h1>\n",
            markdown_html::slugify(&ch.title),
            markdown_html::escape_html(&ch.title)
        );
        for cnode in &ch.content {
            let Some(n) = h.get(cnode.id) else { continue };
            if !super::profile_matches(&n.tags, profiles) {
                continue;
            }
            match cnode.kind {
                NodeKind::Subchapter => {
                    let anchor = cnode.anchor.clone().unwrap_or_default();
                    content.push_str(&format!(
                        "<h2 id=\"{anchor}\">{}</h2>\n",
                        markdown_html::escape_html(&cnode.title)
                    ));
                }
                NodeKind::Paragraph => {
                    if below_floor(n, status_floor) {
                        continue;
                    }
                    if let Some(body) = read_body(layout, n) {
                        content.push_str(&render::render_body(&n.tags, &body, variables));
                    }
                }
                _ => {}
            }
        }
        pages.push(Page {
            file: ch.file.clone(),
            title: ch.title.clone(),
            content,
            is_index: false,
        });
    }

    // TDOC-4.3 — enabled companion books (Sources, Glossary, Places, the Language,
    // the World, …) become appendix pages of the same site.
    let companion_pages = companions::build(layout, store, h, cfg);
    for cp in companion_pages {
        pages.push(Page {
            file: cp.file,
            title: cp.title,
            content: cp.content,
            is_index: false,
        });
    }

    // INDEX-1 — a back-of-book index page, with each location a real anchor link.
    if hcfg.include.index {
        if let Some(content) = build_index_page(layout, store, h, cfg, &model) {
            pages.push(Page {
                file: "appendix-index.html".into(),
                title: "Index".into(),
                content,
                is_index: false,
            });
        }
    }

    // The nav lists every page (chapters, with their subsection anchors, then the
    // companion appendices) — one website, one contents list.
    let mut base_nav: Vec<toc::NavItem> = toc::nav(&model.chapters, "");
    for p in pages.iter().skip(1 + model.chapters.len()) {
        base_nav.push(toc::NavItem {
            title: p.title.clone(),
            href: p.file.clone(),
            current: false,
            sections: Vec::new(),
        });
    }

    // Render each page through the template.
    let mut collected = BTreeSet::new();
    let mut search_docs: Vec<search::SearchDoc> = Vec::new();
    for i in 0..pages.len() {
        let content = assets::rewrite_images(&pages[i].content, &mut collected);
        if hcfg.search {
            search_docs.push(search::SearchDoc {
                title: pages[i].title.clone(),
                url: pages[i].file.clone(),
                text: search::strip_html(&content),
            });
        }
        let nav: Vec<toc::NavItem> = base_nav
            .iter()
            .map(|it| toc::NavItem { current: it.href == pages[i].file, ..it.clone() })
            .collect();
        let prev = if i > 0 {
            Some(minijinja::context! { title => pages[i - 1].title, href => pages[i - 1].file })
        } else {
            None
        };
        let next = pages.get(i + 1).map(|p| minijinja::context! { title => p.title, href => p.file });
        let ctx = minijinja::context! {
            lang => html_lang,
            book => minijinja::context! { title => book_title, language => book_lang },
            site => site,
            vars => variables,
            labels => labels,
            search => hcfg.search,
            nav => nav,
            page => minijinja::context! {
                title => pages[i].title,
                content => content,
                root => "",
                is_index => pages[i].is_index,
                prev => prev,
                next => next,
            },
        };
        let rendered = tmpl.render_page(ctx)?;
        std::fs::write(out_dir.join(&pages[i].file), rendered)?;
    }

    // TDOC-4.2 — the client-side search corpus + script (self-contained).
    if hcfg.search {
        std::fs::write(out_dir.join("search-index.js"), search::build_index_js(&search_docs))?;
        std::fs::write(out_dir.join("search.js"), search::SEARCH_JS)?;
    }

    assets::copy_assets(&layout.root, out_dir, &collected)?;
    Ok(())
}

/// Resolve the user book to export: the scope root if it is a book, else the sole
/// user book.
fn resolve_book(h: &Hierarchy, root_id: Option<Uuid>) -> Result<&Node> {
    if let Some(id) = root_id {
        if let Some(n) = h.get(id) {
            if n.kind == NodeKind::Book {
                return Ok(n);
            }
        }
    }
    let books: Vec<&Node> = h
        .children_of(None)
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book && n.system_tag.is_none())
        .collect();
    match books.len() {
        1 => Ok(books[0]),
        0 => bail!("export html: no user book to export"),
        _ => bail!("export html: multiple user books — pass --book-name"),
    }
}

fn read_body(layout: &ProjectLayout, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    std::fs::read_to_string(layout.root.join(rel)).ok()
}

/// Whether a paragraph sits below the `--status` floor (so the export skips it).
fn below_floor(node: &Node, floor: Option<usize>) -> bool {
    match floor {
        Some(f) => super::status_ladder_index(node.status.as_deref()) < f,
        None => false,
    }
}

/// INDEX-1 — build the back-of-book index page HTML (each location an anchor link),
/// or `None` when there are no terms or no matches.
fn build_index_page(
    layout: &ProjectLayout,
    store: &crate::store::Store,
    h: &Hierarchy,
    cfg: &Config,
    model: &toc::SiteModel,
) -> Option<String> {
    let icfg = &cfg.docs.index;
    let mut terms: Vec<String> = Vec::new();
    let mut see_refs: Vec<(String, String)> = Vec::new();
    if icfg.from_glossary {
        for e in crate::glossary::glossary_entries_from_store(store, h, None) {
            for syn in &e.synonyms {
                see_refs.push((syn.clone(), e.term.clone()));
            }
            terms.push(e.term);
        }
    }
    terms.extend(icfg.terms.iter().cloned());
    if terms.is_empty() {
        return None;
    }

    let mut units: Vec<crate::book_index::IndexUnit> = Vec::new();
    for ch in &model.chapters {
        for cnode in &ch.content {
            if cnode.kind != NodeKind::Paragraph {
                continue;
            }
            let Some(n) = h.get(cnode.id) else { continue };
            if let Some(body) = read_body(layout, n) {
                units.push(crate::book_index::IndexUnit {
                    chapter: ch.title.clone(),
                    anchor: ch.file.clone(),
                    text: crate::audiobook::typst_to_plain(&body),
                });
            }
        }
    }
    let entries = crate::book_index::build(&terms, &see_refs, &units);
    if entries.is_empty() {
        return None;
    }

    let mut s = String::from("<h1>Index</h1>\n<div class=\"book-index\">\n");
    for e in &entries {
        s.push_str(&format!(
            "<p class=\"idx\"><span class=\"idx-term\">{}</span> — ",
            markdown_html::escape_html(&e.term)
        ));
        if let Some(canonical) = &e.see {
            s.push_str(&format!("<em>see</em> {}", markdown_html::escape_html(canonical)));
        } else {
            let links: Vec<String> = e
                .locations
                .iter()
                .map(|l| {
                    format!(
                        "<a href=\"{}\">{}</a>",
                        markdown_html::escape_html(&l.anchor),
                        markdown_html::escape_html(&l.chapter)
                    )
                })
                .collect();
            s.push_str(&links.join(", "));
        }
        s.push_str("</p>\n");
    }
    s.push_str("</div>\n");
    Some(s)
}

/// Parse the site variables file (`html.hjson`) into a JSON value for templates.
fn load_site_vars(layout: &ProjectLayout, file: &str) -> serde_json::Value {
    let path = layout.root.join(file);
    match std::fs::read_to_string(&path) {
        Ok(raw) => serde_hjson::from_str::<serde_json::Value>(&raw).unwrap_or_else(|e| {
            tracing::warn!(target: "inkhaven::html", "html: `{file}` did not parse ({e}) — ignored");
            serde_json::json!({})
        }),
        Err(_) => serde_json::json!({}),
    }
}
