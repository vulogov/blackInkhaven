//! TDOC-4.3 — companion-book inclusion. When `docs.html.include.<book>` is on, that
//! system book is rendered as an appendix page on the site. Sources gets a formatted
//! bibliography; every other book renders its entries — prose through the normal
//! path, HJSON entries as readable field lists.

use crate::config::Config;
use crate::project::ProjectLayout;
use crate::store::hierarchy::Hierarchy;
use crate::store::node::{Node, NodeKind};

use super::markdown_html::{escape_html, slugify};
use super::render;

/// One appendix page rendered from a companion book.
pub struct CompanionPage {
    pub file: String,
    pub title: String,
    pub content: String,
}

/// Build an appendix page for each enabled companion book, in a stable order.
pub fn build(layout: &ProjectLayout, h: &Hierarchy, cfg: &Config) -> Vec<CompanionPage> {
    use crate::store::{
        SYSTEM_TAG_CHARACTERS, SYSTEM_TAG_GLOSSARY, SYSTEM_TAG_LANGUAGES, SYSTEM_TAG_MYTHOLOGY,
        SYSTEM_TAG_NOTES, SYSTEM_TAG_PLACES, SYSTEM_TAG_SOURCES, SYSTEM_TAG_WORLD,
    };
    let inc = &cfg.docs.html.include;
    let specs: &[(bool, &str, &str)] = &[
        (inc.sources, SYSTEM_TAG_SOURCES, "Sources"),
        (inc.glossary, SYSTEM_TAG_GLOSSARY, "Glossary"),
        (inc.characters, SYSTEM_TAG_CHARACTERS, "Characters"),
        (inc.places, SYSTEM_TAG_PLACES, "Places"),
        (inc.language, SYSTEM_TAG_LANGUAGES, "Language"),
        (inc.world, SYSTEM_TAG_WORLD, "World"),
        (inc.mythology, SYSTEM_TAG_MYTHOLOGY, "Mythology"),
        (inc.notes, SYSTEM_TAG_NOTES, "Notes"),
    ];

    let mut out = Vec::new();
    for (enabled, tag, title) in specs {
        if !enabled {
            continue;
        }
        let Some(book) = h.children_of(None).into_iter().find(|n| {
            n.kind == NodeKind::Book && n.system_tag.as_deref() == Some(*tag)
        }) else {
            continue;
        };
        let content = if *tag == SYSTEM_TAG_SOURCES {
            render_sources(layout, h, book.id)
        } else if *tag == SYSTEM_TAG_LANGUAGES {
            render_language(layout, h, book.id)
        } else if *tag == SYSTEM_TAG_WORLD {
            render_world(layout, h, book.id)
        } else {
            render_generic(layout, h, book.id, title)
        };
        // Skip a book with no real content (the render fns return "" when empty).
        if content.trim().is_empty() {
            continue;
        }
        out.push(CompanionPage {
            file: format!("appendix-{}.html", slugify(title)),
            title: title.to_string(),
            content,
        });
    }
    out
}

fn read_body(layout: &ProjectLayout, node: &Node) -> Option<String> {
    let rel = node.file.as_ref()?;
    std::fs::read_to_string(layout.root.join(rel)).ok()
}

/// Generic companion rendering: walk the book, emit a section per entry. Returns
/// `""` when the book has no renderable entries.
fn render_generic(layout: &ProjectLayout, h: &Hierarchy, book_id: uuid::Uuid, title: &str) -> String {
    let mut body = String::new();
    let mut has_entry = false;
    for id in h.collect_subtree(book_id) {
        if id == book_id {
            continue;
        }
        let Some(n) = h.get(id) else { continue };
        match n.kind {
            NodeKind::Chapter | NodeKind::Subchapter => {
                body.push_str(&format!(
                    "<h2 id=\"{}\">{}</h2>\n",
                    slugify(&n.title),
                    escape_html(&n.title)
                ));
            }
            NodeKind::Paragraph => {
                let Some(raw) = read_body(layout, n) else { continue };
                let entry = render_entry(&n.title, n.content_type.as_deref(), &raw);
                if !entry.trim().is_empty() {
                    has_entry = true;
                }
                body.push_str(&entry);
            }
            _ => {}
        }
    }
    if !has_entry {
        return String::new();
    }
    format!("<h1>{}</h1>\n{body}", escape_html(title))
}

/// TDOC-4.4 — the Language book: a sortable/filterable lexicon table per invented
/// language, plus any grammar / sample prose. Returns `""` when nothing is defined.
fn render_language(layout: &ProjectLayout, h: &Hierarchy, lang_root_id: uuid::Uuid) -> String {
    let mut body = String::new();
    let mut any_table = false;
    for lang in h
        .children_of(Some(lang_root_id))
        .into_iter()
        .filter(|n| n.kind == NodeKind::Book)
    {
        let entries = load_dictionary_from_disk(layout, h, lang.id);
        if !entries.is_empty() {
            body.push_str(&lexicon_table(&lang.title, &entries));
            any_table = true;
        }

        // Grammar / Phonology / Sample-text chapters — rendered as prose/HJSON.
        for ch in h
            .children_of(Some(lang.id))
            .into_iter()
            .filter(|n| n.kind == NodeKind::Chapter)
        {
            let t = ch.title.to_ascii_lowercase();
            if t == "dictionary" || t == "meta" {
                continue;
            }
            let mut sec = String::new();
            for id in h.collect_subtree(ch.id) {
                let Some(n) = h.get(id) else { continue };
                if n.kind != NodeKind::Paragraph {
                    continue;
                }
                let Some(raw) = read_body(layout, n) else { continue };
                sec.push_str(&render_entry(&n.title, n.content_type.as_deref(), &raw));
            }
            if !sec.trim().is_empty() {
                body.push_str(&format!(
                    "<h2 id=\"{}\">{} — {}</h2>\n{sec}",
                    slugify(&format!("{}-{}", lang.title, ch.title)),
                    escape_html(&lang.title),
                    escape_html(&ch.title)
                ));
            }
        }
    }
    if body.trim().is_empty() {
        return String::new();
    }
    if any_table {
        body.push_str(LEXICON_SCRIPT);
    }
    format!("<h1>Language</h1>\n{body}")
}

/// A sortable, filterable HTML table for one language's dictionary.
fn lexicon_table(lang: &str, entries: &[crate::language_entry::DictionaryEntry]) -> String {
    let mut s = format!(
        "<h2 id=\"{}\">{} — Dictionary</h2>\n<p class=\"lexicon-count\">{} entries · click a column to sort, type to filter</p>\n",
        slugify(&format!("{lang}-dictionary")),
        escape_html(lang),
        entries.len()
    );
    s.push_str("<input type=\"search\" class=\"lexicon-filter\" placeholder=\"Filter the dictionary…\" aria-label=\"Filter dictionary\">\n");
    s.push_str("<table class=\"lexicon\"><thead><tr>");
    for head in ["Word", "Part of speech", "Meaning", "Registers", "Domain", "Era", "Etymology"] {
        s.push_str(&format!("<th>{head}</th>"));
    }
    s.push_str("</tr></thead><tbody>\n");
    for e in entries {
        s.push_str("<tr>");
        s.push_str(&td(&e.word));
        s.push_str(&td(&e.pos));
        s.push_str(&td(&e.translation));
        s.push_str(&td(&e.registers.join(", ")));
        s.push_str(&td(&e.domain.join(", ")));
        s.push_str(&td(e.era.as_deref().unwrap_or("")));
        s.push_str(&td(e.etymology.as_deref().unwrap_or("")));
        s.push_str("</tr>\n");
    }
    s.push_str("</tbody></table>\n");
    s
}

fn td(text: &str) -> String {
    format!("<td>{}</td>", escape_html(text))
}

/// Self-contained (inline) vanilla-JS: click a header to sort, type in the filter
/// box to narrow. No external dependency — the page stays self-contained.
const LEXICON_SCRIPT: &str = r#"<script>
(function(){
  function cell(row,i){return (row.cells[i].textContent||'').trim().toLowerCase();}
  document.querySelectorAll('table.lexicon').forEach(function(table){
    var tbody=table.tBodies[0];
    var input=table.previousElementSibling;
    if(input&&input.classList&&input.classList.contains('lexicon-filter')){
      input.addEventListener('input',function(){
        var q=input.value.toLowerCase();
        Array.prototype.forEach.call(tbody.rows,function(row){
          row.style.display=row.textContent.toLowerCase().indexOf(q)>=0?'':'none';
        });
      });
    }
    Array.prototype.forEach.call(table.tHead.rows[0].cells,function(th,i){
      var dir=0;
      th.addEventListener('click',function(){
        dir=dir<=0?1:-1;
        var rows=Array.prototype.slice.call(tbody.rows);
        rows.sort(function(a,b){return cell(a,i).localeCompare(cell(b,i))*dir;});
        rows.forEach(function(r){tbody.appendChild(r);});
        Array.prototype.forEach.call(table.tHead.rows[0].cells,function(c){c.removeAttribute('aria-sort');});
        th.setAttribute('aria-sort',dir>0?'ascending':'descending');
      });
    });
  });
})();
</script>
"#;

/// TDOC-4.5 — the World book: a narrative guide compiled from `world.hjson`, falling
/// back to the materialised paragraphs if no world definition is present.
fn render_world(layout: &ProjectLayout, h: &Hierarchy, book_id: uuid::Uuid) -> String {
    let world_path = layout.root.join("world.hjson");
    if let Ok(raw) = std::fs::read_to_string(&world_path) {
        if let Ok(def) = crate::world::types::WorldDefinition::from_hjson(&raw) {
            let narrative = super::world_html::render(&def);
            if !narrative.trim().is_empty() {
                return narrative;
            }
        }
    }
    render_generic(layout, h, book_id, "World")
}

/// Load every parseable `DictionaryEntry` under a language's `Dictionary` chapter,
/// reading bodies from disk (the exporter never touches the store).
fn load_dictionary_from_disk(
    layout: &ProjectLayout,
    h: &Hierarchy,
    lang_book_id: uuid::Uuid,
) -> Vec<crate::language_entry::DictionaryEntry> {
    let Some(chapter) = h
        .children_of(Some(lang_book_id))
        .into_iter()
        .find(|n| n.kind == NodeKind::Chapter && n.title.eq_ignore_ascii_case("Dictionary"))
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for id in h.collect_subtree(chapter.id) {
        let Some(n) = h.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(body) = read_body(layout, n) else { continue };
        if let Ok(Some(entry)) = crate::language_entry::parse(&body) {
            out.push(entry);
        }
    }
    out
}

/// One entry: an HJSON body becomes a titled field list; prose goes through the
/// normal renderer.
fn render_entry(title: &str, content_type: Option<&str>, body: &str) -> String {
    if content_type == Some("hjson") {
        match serde_hjson::from_str::<serde_json::Value>(body) {
            Ok(value) => format!(
                "<section class=\"entry\"><h3 id=\"{}\">{}</h3>\n{}</section>\n",
                slugify(title),
                escape_html(title),
                value_to_html(&value)
            ),
            Err(_) => String::new(),
        }
    } else {
        let html = render::render_body(&[], body, &std::collections::BTreeMap::new());
        format!("<section class=\"entry\">{html}</section>\n")
    }
}

/// Render a parsed HJSON/JSON value as readable HTML. Objects become field lists;
/// a list of objects becomes a series of named cards (so a world's settlements or a
/// gazetteer's places read as entries, not raw nesting); a list of scalars becomes
/// an inline comma list.
fn value_to_html(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Object(map) => {
            let mut s = String::from("<dl class=\"fields\">");
            for (k, val) in map {
                s.push_str(&format!("<dt>{}</dt><dd>{}</dd>", escape_html(k), value_to_html(val)));
            }
            s.push_str("</dl>");
            s
        }
        serde_json::Value::Array(arr) if !arr.is_empty() && arr.iter().all(|x| x.is_object()) => {
            let mut s = String::new();
            for item in arr {
                let name = item
                    .get("name")
                    .or_else(|| item.get("title"))
                    .and_then(|x| x.as_str());
                let head = name
                    .map(|n| format!("<h4>{}</h4>", escape_html(n)))
                    .unwrap_or_default();
                s.push_str(&format!("<div class=\"card\">{head}{}</div>", value_to_html(item)));
            }
            s
        }
        serde_json::Value::Array(arr)
            if arr.iter().all(|x| x.is_string() || x.is_number() || x.is_boolean()) =>
        {
            let items: Vec<String> = arr.iter().map(scalar_text).collect();
            format!("<p>{}</p>", items.join(", "))
        }
        serde_json::Value::Array(arr) => {
            let mut s = String::from("<ul>");
            for item in arr {
                s.push_str(&format!("<li>{}</li>", value_to_html(item)));
            }
            s.push_str("</ul>");
            s
        }
        serde_json::Value::String(s) => escape_html(s),
        serde_json::Value::Null => String::new(),
        other => escape_html(&other.to_string()),
    }
}

fn scalar_text(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => escape_html(s),
        other => escape_html(&other.to_string()),
    }
}

/// Sources → a formatted, author-sorted bibliography.
fn render_sources(layout: &ProjectLayout, h: &Hierarchy, book_id: uuid::Uuid) -> String {
    let mut entries: Vec<crate::sources::BibEntry> = Vec::new();
    for id in h.collect_subtree(book_id) {
        let Some(n) = h.get(id) else { continue };
        if n.kind != NodeKind::Paragraph {
            continue;
        }
        let Some(body) = read_body(layout, n) else { continue };
        if let Some(e) = crate::sources::BibEntry::from_hjson(&body) {
            entries.push(e);
        }
    }
    if entries.is_empty() {
        return String::new();
    }
    entries.sort_by(|a, b| a.author.cmp(&b.author).then(a.year.cmp(&b.year)));

    let mut out = String::from("<h1>Sources</h1>\n<div class=\"bibliography\">\n");
    for e in &entries {
        out.push_str(&format_reference(e));
    }
    out.push_str("</div>\n");
    out
}

fn format_reference(e: &crate::sources::BibEntry) -> String {
    let mut s = format!("<p class=\"ref\" id=\"{}\">", escape_html(&e.key));
    if !e.author.is_empty() {
        s.push_str(&format!("<span class=\"ref-author\">{}</span> ", escape_html(&e.author)));
    }
    if !e.year.is_empty() {
        s.push_str(&format!("({}). ", escape_html(&e.year)));
    }
    if !e.title.is_empty() {
        s.push_str(&format!("<span class=\"ref-title\">{}</span>. ", escape_html(&e.title)));
    }
    if let Some(j) = e.journal.as_deref().filter(|s| !s.is_empty()) {
        s.push_str(&format!("<em>{}</em>. ", escape_html(j)));
    } else if let Some(p) = e.publisher.as_deref().filter(|s| !s.is_empty()) {
        s.push_str(&format!("{}. ", escape_html(p)));
    }
    if let Some(url) = e.url.as_deref().filter(|s| !s.is_empty()) {
        s.push_str(&format!("<a href=\"{}\">{}</a>", escape_html(url), escape_html(url)));
    }
    s.push_str("</p>\n");
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hjson_value_renders_field_list() {
        let v = serde_json::json!({"kind": "marsh", "notable": "the drowned tower"});
        let html = value_to_html(&v);
        assert!(html.contains("<dt>kind</dt><dd>marsh</dd>"), "got: {html}");
        assert!(html.contains("<dt>notable</dt><dd>the drowned tower</dd>"));
    }

    #[test]
    fn array_of_objects_renders_cards() {
        let v = serde_json::json!({
            "settlements": [
                {"name": "Fenhold", "population": 1200},
                {"name": "Tidemarsh", "population": 800}
            ],
            "climate": ["cold", "wet"]
        });
        let html = value_to_html(&v);
        assert!(html.contains("<div class=\"card\"><h4>Fenhold</h4>"), "got: {html}");
        assert!(html.contains("<h4>Tidemarsh</h4>"));
        // A list of scalars becomes an inline comma list, not nested bullets.
        assert!(html.contains("<p>cold, wet</p>"));
    }

    #[test]
    fn bibentry_formats_as_reference() {
        let e = crate::sources::BibEntry {
            key: "smith2020".into(),
            author: "Smith, J.".into(),
            title: "A Study".into(),
            year: "2020".into(),
            url: Some("https://x.example".into()),
            ..Default::default()
        };
        let s = format_reference(&e);
        assert!(s.contains("id=\"smith2020\""));
        assert!(s.contains("<span class=\"ref-author\">Smith, J.</span>"));
        assert!(s.contains("(2020)."));
        assert!(s.contains("<a href=\"https://x.example\">"));
    }
}
