//! TDOC-4 — render a node's content to an HTML fragment. Structure comes from the
//! `para:*` subtype metadata; prose goes through `typst_to_markdown` → our
//! `markdown_to_html`. Heading levels inside a paragraph are demoted so the chapter
//! title stays the sole `<h1>`.

use std::collections::BTreeMap;

use crate::export::markdown::typst_to_markdown;

use super::markdown_html::{escape_html, markdown_to_html};

/// Render a single paragraph body to HTML, applying single-sourcing variables and
/// branching on the `para:*` structural subtype. Headings are demoted one level so
/// the enclosing chapter title stays the sole `<h1>`.
pub fn render_body(tags: &[String], body: &str, variables: &BTreeMap<String, String>) -> String {
    let mut body = body.to_string();
    for (k, v) in variables {
        body = body.replace(&format!("{{{{{k}}}}}"), v);
    }

    if let Some(kind) = tags.iter().find_map(|t| t.strip_prefix("para:admonition-")) {
        return render_admonition(kind, &body);
    }
    if tags.iter().any(|t| t == "para:code") {
        return render_code_listing(&body);
    }
    if tags.iter().any(|t| t == "para:math") {
        let inner = body.trim().trim_matches('$').trim();
        return format!("<div class=\"math-block\">{}</div>\n", super::math_html::render_block(inner));
    }
    if tags.iter().any(|t| t == "para:table") {
        return render_table(&body);
    }
    if tags.iter().any(|t| t == "para:procedure") {
        // Typst `+ item` numbered list → markdown ordered list.
        let md: String = body
            .lines()
            .map(|l| {
                l.strip_prefix("+ ")
                    .map(|rest| format!("1. {rest}"))
                    .unwrap_or_else(|| l.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n");
        return markdown_to_html(&demote_headings(&md, 1));
    }

    let md = typst_to_markdown(&body);
    markdown_to_html(&demote_headings(&md, 1))
}

/// Render a `para:admonition-<kind>` block: pull the content out of the seed
/// `#block(…)[ … ]` and style it as an `<aside>`.
fn render_admonition(kind: &str, body: &str) -> String {
    let inner = admonition_inner(body);
    let title = kind
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + &kind[c.len_utf8()..])
        .unwrap_or_else(|| kind.to_string());
    let html = markdown_to_html(&demote_headings(&typst_to_markdown(inner), 2));
    format!(
        "<aside class=\"admonition {kind}\"><p class=\"admonition-title\">{}</p>\n{html}</aside>\n",
        escape_html(&title)
    )
}

/// Extract the content inside the outer `)[ … ]` of a `#block(...)[...]` seed.
fn admonition_inner(body: &str) -> &str {
    if let Some(open) = body.find(")[") {
        let after = &body[open + 2..];
        if let Some(close) = after.rfind(']') {
            return after[..close].trim();
        }
    }
    body.trim()
}

/// Render a `para:code` listing: the fenced block → `<pre><code>` inside a
/// `<figure>` carrying the caption if present.
fn render_code_listing(body: &str) -> String {
    let (lang, code) = extract_first_fence(body);
    let caption = extract_caption(body);
    let class = if lang.is_empty() {
        String::new()
    } else {
        format!(" class=\"language-{}\"", escape_html(&lang))
    };
    let cap_html = if caption.is_empty() {
        String::new()
    } else {
        format!("<figcaption>{}</figcaption>", escape_html(&caption))
    };
    format!(
        "<figure class=\"listing\"><pre><code{class}>{}</code></pre>{cap_html}</figure>\n",
        escape_html(&code)
    )
}

fn extract_first_fence(body: &str) -> (String, String) {
    let mut lines = body.lines();
    for line in lines.by_ref() {
        if let Some(info) = line.trim_start().strip_prefix("```") {
            let lang = info.split_whitespace().next().unwrap_or("").to_string();
            let mut code = String::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(l);
                code.push('\n');
            }
            return (lang, code);
        }
    }
    (String::new(), body.to_string())
}

fn extract_caption(body: &str) -> String {
    if let Some(i) = body.find("caption:") {
        let after = &body[i + "caption:".len()..];
        if let Some(open) = after.find('[') {
            let rest = &after[open + 1..];
            if let Some(close) = rest.find(']') {
                return rest[..close].trim().to_string();
            }
        }
    }
    String::new()
}

/// Render a `para:table` (`#table(columns: …, table.header(…), [cell]…)`) as an
/// HTML `<table>`. Degrades to the normal converter if it can't be parsed.
fn render_table(body: &str) -> String {
    let Some(inner) = extract_call(body, "#table") else {
        return markdown_to_html(&typst_to_markdown(body));
    };
    let ncols = count_columns(inner).max(1);
    let (header, rest) = split_header(inner);
    let header_cells = parse_cells(&header);
    let body_cells = parse_cells(&rest);

    let mut out = String::from("<table>");
    if !header_cells.is_empty() {
        out.push_str("<thead><tr>");
        for c in &header_cells {
            out.push_str(&format!("<th>{}</th>", render_cell(c)));
        }
        out.push_str("</tr></thead>");
    }
    out.push_str("<tbody>");
    for row in body_cells.chunks(ncols) {
        out.push_str("<tr>");
        for c in row {
            out.push_str(&format!("<td>{}</td>", render_cell(c)));
        }
        out.push_str("</tr>");
    }
    out.push_str("</tbody></table>\n");
    out
}

/// The content inside the outermost `(...)` following `name` (e.g. `#table`).
fn extract_call<'a>(body: &'a str, name: &str) -> Option<&'a str> {
    let start = body.find(name)?;
    let after = &body[start + name.len()..];
    let open = after.find('(')?;
    let mut depth = 0i32;
    for (idx, ch) in after.char_indices() {
        if idx < open {
            continue;
        }
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[open + 1..idx]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Column count from the `columns:` value (a tuple's non-empty entries, or a number).
fn count_columns(inner: &str) -> usize {
    let Some(p) = inner.find("columns:") else { return 1 };
    let after = inner[p + "columns:".len()..].trim_start();
    if after.starts_with('(') {
        if let Some(tuple) = extract_call(after, "") {
            return split_top_level(tuple, ',').iter().filter(|s| !s.trim().is_empty()).count().max(1);
        }
    }
    after.chars().take_while(|c| c.is_ascii_digit()).collect::<String>().parse().unwrap_or(1)
}

/// Split off the `table.header(...)` cells; return (header-region, the rest).
fn split_header(inner: &str) -> (String, String) {
    if let Some(start) = inner.find("table.header") {
        let after = &inner[start..];
        if let Some(hdr) = extract_call(after, "table.header") {
            let hdr = hdr.to_string();
            // Everything before + after the header call is the body region.
            let call_len = "table.header".len()
                + after["table.header".len()..].find('(').map(|o| o + 1).unwrap_or(0)
                + hdr.len()
                + 1;
            let mut rest = inner[..start].to_string();
            rest.push_str(&inner[start + call_len.min(after.len())..]);
            return (hdr, rest);
        }
    }
    (String::new(), inner.to_string())
}

/// Top-level `[...]` cells (bracket-depth aware).
fn parse_cells(s: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' {
            let mut depth = 1;
            let start = i + 1;
            i += 1;
            while i < chars.len() && depth > 0 {
                match chars[i] {
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                i += 1;
            }
            cells.push(chars[start..i].iter().collect());
        }
        i += 1;
    }
    cells
}

/// Split a string on `sep` at bracket/paren depth zero.
fn split_top_level(s: &str, sep: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth = 0i32;
    let mut cur = String::new();
    for ch in s.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            c if c == sep && depth == 0 => {
                parts.push(std::mem::take(&mut cur));
                continue;
            }
            _ => {}
        }
        cur.push(ch);
    }
    parts.push(cur);
    parts
}

/// A table cell's Typst content → inline HTML (stripping a wrapping `<p>`).
fn render_cell(cell: &str) -> String {
    let html = markdown_to_html(&typst_to_markdown(cell.trim()));
    let h = html.trim();
    let h = h.strip_prefix("<p>").unwrap_or(h);
    let h = h.strip_suffix("</p>").unwrap_or(h);
    h.trim().to_string()
}

/// Demote ATX headings by `by` levels (so a chapter page keeps a single `<h1>`).
fn demote_headings(md: &str, by: u8) -> String {
    md.lines()
        .map(|line| {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            if hashes >= 1 && hashes <= 6 && line.as_bytes().get(hashes) == Some(&b' ') {
                let new = (hashes + by as usize).min(6);
                format!("{} {}", "#".repeat(new), &line[hashes + 1..])
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admonition_renders_aside() {
        let body = "#block(\n  stroke: 0.5pt + blue,\n)[\nNote: mind the gap.\n]\n";
        let html = render_body(&["para:admonition-note".to_string()], body, &BTreeMap::new());
        assert!(html.contains("<aside class=\"admonition note\">"));
        assert!(html.contains("admonition-title\">Note</p>"));
        assert!(html.contains("mind the gap"));
    }

    #[test]
    fn code_listing_renders_figure() {
        let body = "#figure(\n  caption: [A greeting.],\n)[\n```rust\nfn main() {}\n```\n]\n";
        let html = render_body(&["para:code".to_string()], body, &BTreeMap::new());
        assert!(html.contains("<figure class=\"listing\">"));
        assert!(html.contains("<code class=\"language-rust\">fn main() {}"));
        assert!(html.contains("<figcaption>A greeting.</figcaption>"));
    }

    #[test]
    fn table_renders_html_grid() {
        let body = "#table(\n  columns: (auto, 1fr),\n  table.header([*A*], [*B*],),\n  [1], [2],\n  [3], [4],\n)\n";
        let html = render_body(&["para:table".to_string()], body, &BTreeMap::new());
        assert!(html.contains("<table><thead><tr><th>"), "got: {html}");
        assert!(html.contains("<td>1</td><td>2</td></tr>"), "got: {html}");
        assert!(html.contains("<td>3</td><td>4</td></tr>"));
    }

    #[test]
    fn math_renders_mathml() {
        let html = render_body(&["para:math".to_string()], "$\n  x^2\n$\n", &BTreeMap::new());
        assert!(html.contains("<math display=\"block\""), "got: {html}");
        assert!(html.contains("<msup>"));
    }

    #[test]
    fn variables_resolve_and_headings_demote() {
        let html = render_body(&[], "= Overview\n\n{{product}} is here.\n", &BTreeMap::from([("product".to_string(), "Inkhaven".to_string())]));
        assert!(html.contains("<h2 id=\"overview\">Overview</h2>"), "got: {html}");
        assert!(html.contains("Inkhaven is here."));
    }
}
