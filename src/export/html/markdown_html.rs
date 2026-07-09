//! TDOC-4 — a small Markdown→HTML renderer over exactly the subset
//! `export::markdown::typst_to_markdown` emits (headings, emphasis, inline code,
//! links, images, lists, fenced code, blockquotes, rules). Because we own the
//! intermediate, this stays small and needs no external crate.

/// HTML-escape text content.
pub fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// A URL-safe slug for heading anchors.
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if c.is_whitespace() || c == '-' || c == '_' {
            if !prev_dash && !out.is_empty() {
                out.push('-');
                prev_dash = true;
            }
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        out.push_str("section");
    }
    out
}

/// Render a Markdown string (our subset) to an HTML fragment.
pub fn markdown_to_html(md: &str) -> String {
    let mut out = String::new();
    let mut lines = md.lines().peekable();
    while let Some(raw) = lines.next() {
        let had_break = raw.ends_with("  "); // markdown hard line break
        let line = raw.trim_end();
        let trimmed = line.trim_start();

        // Fenced code.
        if let Some(info) = trimmed.strip_prefix("```") {
            let lang = info.split_whitespace().next().unwrap_or("");
            let mut code = String::new();
            for l in lines.by_ref() {
                if l.trim_start().starts_with("```") {
                    break;
                }
                code.push_str(&escape_html(l));
                code.push('\n');
            }
            let class = if lang.is_empty() {
                String::new()
            } else {
                format!(" class=\"language-{}\"", escape_html(lang))
            };
            out.push_str(&format!("<pre><code{class}>{code}</code></pre>\n"));
            continue;
        }

        if trimmed.is_empty() {
            continue;
        }

        // Headings.
        if let Some((level, text)) = parse_heading(trimmed) {
            out.push_str(&format!(
                "<h{level} id=\"{}\">{}</h{level}>\n",
                slugify(text),
                inline(text)
            ));
            continue;
        }

        // Horizontal rule.
        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            out.push_str("<hr>\n");
            continue;
        }

        // Blockquote.
        if let Some(first) = trimmed.strip_prefix("> ").or_else(|| trimmed.strip_prefix(">")) {
            let mut body = inline(first.trim());
            while let Some(next) = lines.peek() {
                let t = next.trim_start();
                if let Some(q) = t.strip_prefix("> ").or_else(|| t.strip_prefix(">")) {
                    body.push(' ');
                    body.push_str(&inline(q.trim()));
                    lines.next();
                } else {
                    break;
                }
            }
            out.push_str(&format!("<blockquote><p>{body}</p></blockquote>\n"));
            continue;
        }

        // Lists.
        if let Some((ordered, content)) = list_item(trimmed) {
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>\n<li>{}</li>\n", inline(content)));
            while let Some(next) = lines.peek() {
                let t = next.trim_start();
                match list_item(t) {
                    Some((_, c)) => {
                        out.push_str(&format!("<li>{}</li>\n", inline(c)));
                        lines.next();
                    }
                    None => break,
                }
            }
            out.push_str(&format!("</{tag}>\n"));
            continue;
        }

        // A lone image becomes a figure.
        if let Some(fig) = lone_image(trimmed) {
            out.push_str(&fig);
            out.push('\n');
            continue;
        }

        // Paragraph — gather consecutive non-structural lines, honouring markdown
        // hard breaks (a line ending in two spaces → `<br>`).
        let mut segs: Vec<(String, bool)> = vec![(trimmed.to_string(), had_break)];
        while let Some(next) = lines.peek() {
            let next_break = next.ends_with("  ");
            let t = next.trim();
            if t.is_empty() || is_block_start(t) {
                break;
            }
            segs.push((t.to_string(), next_break));
            lines.next();
        }
        let mut html = String::new();
        for (idx, (seg, _)) in segs.iter().enumerate() {
            if idx > 0 {
                html.push_str(if segs[idx - 1].1 { "<br>\n" } else { " " });
            }
            html.push_str(&inline(seg));
        }
        out.push_str(&format!("<p>{html}</p>\n"));
    }
    out
}

fn parse_heading(s: &str) -> Option<(u8, &str)> {
    let hashes = s.chars().take_while(|c| *c == '#').count();
    if (1..=6).contains(&hashes) && s.as_bytes().get(hashes) == Some(&b' ') {
        Some((hashes as u8, s[hashes + 1..].trim()))
    } else {
        None
    }
}

fn list_item(s: &str) -> Option<(bool, &str)> {
    if let Some(rest) = s.strip_prefix("- ").or_else(|| s.strip_prefix("* ")).or_else(|| s.strip_prefix("+ ")) {
        return Some((false, rest.trim()));
    }
    // Ordered: "1. " / "12. "
    let digits = s.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && s[digits..].starts_with(". ") {
        return Some((true, s[digits + 2..].trim()));
    }
    None
}

fn is_block_start(s: &str) -> bool {
    s.starts_with("```")
        || s.starts_with("#")
        || s.starts_with("> ")
        || s == "---"
        || list_item(s).is_some()
}

fn lone_image(s: &str) -> Option<String> {
    let s = s.trim();
    if !s.starts_with("![") || !s.ends_with(')') {
        return None;
    }
    let (alt, src) = parse_image(s)?;
    // Only treat as a figure if the whole line is the image.
    let caption = if alt.is_empty() {
        String::new()
    } else {
        format!("<figcaption>{}</figcaption>", inline(&alt))
    };
    Some(format!(
        "<figure><img src=\"{}\" alt=\"{}\">{caption}</figure>",
        escape_html(&src),
        escape_html(&alt)
    ))
}

/// Parse `![alt](src)` → (alt, src).
fn parse_image(s: &str) -> Option<(String, String)> {
    let rest = s.strip_prefix("![")?;
    let close_alt = rest.find("](")?;
    let alt = &rest[..close_alt];
    let after = &rest[close_alt + 2..];
    let close = after.find(')')?;
    Some((alt.to_string(), after[..close].to_string()))
}

/// Inline markup → HTML. Input is escaped first, then span markers applied.
fn inline(s: &str) -> String {
    let escaped = escape_html(s);
    let with_media = replace_media(&escaped);
    let with_code = replace_code_spans(&with_media);
    let with_strong = replace_pair(&with_code, "**", "<strong>", "</strong>");
    let with_em = replace_pair(&with_strong, "_", "<em>", "</em>");
    replace_pair(&with_em, "*", "<em>", "</em>")
}

/// Replace `![alt](src)` (image) and `[text](url)` (link). Runs before other
/// spans so bracket/paren punctuation isn't consumed by them.
fn replace_media(s: &str) -> String {
    let mut out = String::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        let is_img = s[i..].starts_with("![");
        let is_link = bytes[i] == b'[';
        if is_img || is_link {
            let start = if is_img { i + 2 } else { i + 1 };
            if let Some(rel_close) = s[start..].find("](") {
                let label = &s[start..start + rel_close];
                let after = &s[start + rel_close + 2..];
                if let Some(rel_paren) = after.find(')') {
                    let target = &after[..rel_paren];
                    if is_img {
                        out.push_str(&format!("<img src=\"{target}\" alt=\"{label}\">"));
                    } else {
                        out.push_str(&format!("<a href=\"{target}\">{label}</a>"));
                    }
                    i = start + rel_close + 2 + rel_paren + 1;
                    continue;
                }
            }
        }
        // advance one char
        let ch_len = s[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn replace_code_spans(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find('`') {
        let after = &rest[start + 1..];
        if let Some(end) = after.find('`') {
            out.push_str(&rest[..start]);
            out.push_str("<code>");
            out.push_str(&after[..end]);
            out.push_str("</code>");
            rest = &after[end + 1..];
        } else {
            out.push_str(&rest[..start + 1]);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

/// Replace matched `delim … delim` pairs. Skips a pair whose inner content is
/// empty or starts/ends with whitespace (avoids eating stray `*`).
fn replace_pair(s: &str, delim: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find(delim) {
        let after = &rest[start + delim.len()..];
        if let Some(end) = after.find(delim) {
            let inner = &after[..end];
            if !inner.is_empty() && !inner.starts_with(' ') && !inner.ends_with(' ') {
                out.push_str(&rest[..start]);
                out.push_str(open);
                out.push_str(inner);
                out.push_str(close);
                rest = &after[end + delim.len()..];
                continue;
            }
        }
        // No valid close — emit the delimiter literally and move on.
        out.push_str(&rest[..start + delim.len()]);
        rest = &rest[start + delim.len()..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_and_slug() {
        assert_eq!(escape_html("a<b>&\"c"), "a&lt;b&gt;&amp;&quot;c");
        assert_eq!(slugify("The Tide That Remembers!"), "the-tide-that-remembers");
        assert_eq!(slugify("  --Weird__Name  "), "weird-name");
    }

    #[test]
    fn headings_and_paragraphs() {
        let html = markdown_to_html("# Title\n\nA paragraph with **bold** and *italic*.\n");
        assert!(html.contains("<h1 id=\"title\">Title</h1>"));
        assert!(html.contains("<p>A paragraph with <strong>bold</strong> and <em>italic</em>.</p>"));
    }

    #[test]
    fn code_fence_and_span() {
        let html = markdown_to_html("Use `x < y` inline.\n\n```rust\nlet a = 1 < 2;\n```\n");
        assert!(html.contains("<code>x &lt; y</code>"));
        assert!(html.contains("<pre><code class=\"language-rust\">let a = 1 &lt; 2;\n</code></pre>"));
    }

    #[test]
    fn links_images_lists() {
        let html = markdown_to_html("- one\n- two\n\nSee [docs](https://x.example) and ![a cat](cat.png).\n");
        assert!(html.contains("<ul>\n<li>one</li>\n<li>two</li>\n</ul>"));
        assert!(html.contains("<a href=\"https://x.example\">docs</a>"));
        assert!(html.contains("<img src=\"cat.png\" alt=\"a cat\">"));
    }

    #[test]
    fn hard_breaks_become_br() {
        // Two trailing spaces = a markdown hard break (used by the dictionary).
        let html = markdown_to_html("**ka** /ka/  \nwater  \n*Example:* foo\n");
        assert!(html.contains("<strong>ka</strong> /ka/<br>\nwater<br>\n<em>Example:</em> foo"), "got: {html}");
    }

    #[test]
    fn lone_image_becomes_figure() {
        let html = markdown_to_html("![The map](map.png)\n");
        assert!(html.contains("<figure><img src=\"map.png\" alt=\"The map\"><figcaption>The map</figcaption></figure>"));
    }
}
