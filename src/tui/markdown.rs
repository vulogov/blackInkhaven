//! Tiny markdown renderer for the AI pane and a markdown→Typst converter
//! for the r/i/t/b "apply to editor" flow.
//!
//! The renderer is intentionally minimal — the AI streams partial markdown
//! and we re-render the full buffer every frame, so this needs to be cheap
//! and tolerant of half-written input. Pulldown-cmark's event-driven API
//! handles trailing-open structures gracefully; we just translate events to
//! styled ratatui `Line`s.
//!
//! Supported: headings (h1–h6), paragraphs, bold, italic, inline code,
//! code fences, bullet / numbered lists (one level of nesting cosmetics),
//! blockquotes, soft / hard breaks, link text inline.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Render `src` (a possibly-incomplete markdown string) into a vector of
/// styled `Line`s. The output never panics on malformed input — partial
/// fences, unclosed emphasis, and orphan list items all produce reasonable
/// approximations.
pub fn render(src: &str) -> Vec<Line<'static>> {
    if src.is_empty() {
        return Vec::new();
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, options);

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_spans: Vec<Span<'static>> = Vec::new();
    let mut style_stack: Vec<Style> = vec![Style::default()];
    let mut in_code_block = false;
    let mut list_stack: Vec<Option<u64>> = Vec::new(); // None = bullet, Some(n) = ordered current number
    let mut blockquote_depth: usize = 0;
    let mut pending_block_break = false;

    let flush_line =
        |lines: &mut Vec<Line<'static>>, spans: &mut Vec<Span<'static>>| {
            if spans.is_empty() {
                lines.push(Line::from(""));
            } else {
                lines.push(Line::from(std::mem::take(spans)));
            }
        };

    let cur_style = |stack: &Vec<Style>| -> Style {
        *stack.last().unwrap_or(&Style::default())
    };

    let push_text =
        |spans: &mut Vec<Span<'static>>, text: &str, style: Style, in_code_block: bool| {
            // In code blocks, preserve newlines by splitting into multiple
            // spans. For inline code or prose, keep newlines as-is — the
            // event loop emits SoftBreak/HardBreak separately.
            if in_code_block {
                for (i, line) in text.split('\n').enumerate() {
                    if i > 0 {
                        // Caller flushes lines for code blocks below.
                    }
                    if !line.is_empty() {
                        spans.push(Span::styled(line.to_string(), style));
                    }
                }
            } else if !text.is_empty() {
                spans.push(Span::styled(text.to_string(), style));
            }
        };

    let bq_prefix = |depth: usize| -> Span<'static> {
        if depth == 0 {
            Span::raw("")
        } else {
            Span::styled(
                "│ ".repeat(depth),
                Style::default().fg(Color::DarkGray),
            )
        }
    };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if pending_block_break {
                        flush_line(&mut lines, &mut current_spans);
                        pending_block_break = false;
                    }
                    if blockquote_depth > 0 {
                        current_spans.push(bq_prefix(blockquote_depth));
                    }
                }
                Tag::Heading { level, .. } => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    let (color, prefix) = match level {
                        HeadingLevel::H1 => (Color::Cyan, "# "),
                        HeadingLevel::H2 => (Color::LightCyan, "## "),
                        HeadingLevel::H3 => (Color::Green, "### "),
                        HeadingLevel::H4 => (Color::Yellow, "#### "),
                        _ => (Color::Magenta, "##### "),
                    };
                    let style = Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD);
                    style_stack.push(style);
                    current_spans.push(Span::styled(prefix.to_string(), style));
                }
                Tag::BlockQuote(_) => {
                    blockquote_depth += 1;
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    let style = Style::default()
                        .fg(Color::DarkGray)
                        .add_modifier(Modifier::ITALIC);
                    style_stack.push(style);
                }
                Tag::CodeBlock(_) => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    in_code_block = true;
                    style_stack.push(
                        Style::default()
                            .fg(Color::Yellow)
                            .bg(Color::Indexed(236)),
                    );
                }
                Tag::List(start) => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    list_stack.push(start);
                }
                Tag::Item => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    if blockquote_depth > 0 {
                        current_spans.push(bq_prefix(blockquote_depth));
                    }
                    let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                    let bullet = match list_stack.last_mut() {
                        Some(Some(n)) => {
                            let s = format!("{}{}. ", indent, n);
                            *n += 1;
                            s
                        }
                        _ => format!("{}• ", indent),
                    };
                    current_spans.push(Span::styled(
                        bullet,
                        Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD),
                    ));
                }
                Tag::Emphasis => {
                    let s = cur_style(&style_stack).add_modifier(Modifier::ITALIC);
                    style_stack.push(s);
                }
                Tag::Strong => {
                    let s = cur_style(&style_stack).add_modifier(Modifier::BOLD);
                    style_stack.push(s);
                }
                Tag::Strikethrough => {
                    let s =
                        cur_style(&style_stack).add_modifier(Modifier::CROSSED_OUT);
                    style_stack.push(s);
                }
                Tag::Link { .. } => {
                    let s = cur_style(&style_stack)
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED);
                    style_stack.push(s);
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    pending_block_break = true;
                }
                TagEnd::Heading(_) => {
                    style_stack.pop();
                    flush_line(&mut lines, &mut current_spans);
                    pending_block_break = false;
                }
                TagEnd::BlockQuote(_) => {
                    style_stack.pop();
                    blockquote_depth = blockquote_depth.saturating_sub(1);
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    pending_block_break = true;
                }
                TagEnd::CodeBlock => {
                    style_stack.pop();
                    in_code_block = false;
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                    pending_block_break = true;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    pending_block_break = true;
                }
                TagEnd::Item => {
                    if !current_spans.is_empty() {
                        flush_line(&mut lines, &mut current_spans);
                    }
                }
                TagEnd::Emphasis
                | TagEnd::Strong
                | TagEnd::Strikethrough
                | TagEnd::Link => {
                    style_stack.pop();
                }
                _ => {}
            },
            Event::Text(t) => {
                let style = cur_style(&style_stack);
                if in_code_block {
                    // Code-block text may include newlines; split and flush
                    // each line with the code style + bg.
                    let mut first = true;
                    for line in t.split('\n') {
                        if !first {
                            flush_line(&mut lines, &mut current_spans);
                        }
                        first = false;
                        if !line.is_empty() {
                            current_spans.push(Span::styled(line.to_string(), style));
                        }
                    }
                } else {
                    push_text(&mut current_spans, &t, style, false);
                }
            }
            Event::Code(t) => {
                let style = Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD);
                current_spans.push(Span::styled(format!("`{t}`"), style));
            }
            Event::SoftBreak => {
                current_spans.push(Span::raw(" "));
            }
            Event::HardBreak => {
                flush_line(&mut lines, &mut current_spans);
            }
            Event::Rule => {
                if !current_spans.is_empty() {
                    flush_line(&mut lines, &mut current_spans);
                }
                lines.push(Line::from(Span::styled(
                    "────────────────────────",
                    Style::default().fg(Color::DarkGray),
                )));
            }
            _ => {}
        }
    }
    // Flush trailing partial line (common during streaming).
    if !current_spans.is_empty() {
        flush_line(&mut lines, &mut current_spans);
    }
    lines
}

/// Translate markdown to Typst markup. Used when the user transfers an AI
/// result into the editor (r / i / t / b). Anything we don't know how to
/// convert is passed through verbatim — Typst tolerates most plain prose.
///
/// Conversions:
/// * `# H1` … `###### H6`  →  `= H1` … `====== H6`
/// * `**bold**`             →  `*bold*`
/// * `*italic*` / `_italic_` → `_italic_`
/// * `` `inline code` ``    →  `` `inline code` ``   (Typst raw-text)
/// * Fenced code blocks     →  ```` ```lang … ``` ```` (same syntax)
/// * `- item` / `* item`    →  `- item`
/// * `1. item`              →  `+ item`
/// * `[text](url)`          →  `#link("url")[text]`
/// * `> quote`              →  `#quote[quote]`
/// * `---`                  →  `#line(length: 100%)`
pub fn markdown_to_typst(src: &str) -> String {
    if src.is_empty() {
        return String::new();
    }
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    let parser = Parser::new_ext(src, options);

    let mut out = String::new();
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    let mut code_block_lang: Option<String> = None;
    let mut pending_link_url: Vec<String> = Vec::new();
    let mut blockquote_depth: usize = 0;
    let mut at_line_start = true;

    let ensure_newline = |out: &mut String, at_line_start: &mut bool| {
        if !out.ends_with('\n') {
            out.push('\n');
        }
        *at_line_start = true;
    };

    for event in parser {
        match event {
            Event::Start(tag) => match tag {
                Tag::Paragraph => {
                    if !at_line_start {
                        out.push('\n');
                    }
                    at_line_start = true;
                }
                Tag::Heading { level, .. } => {
                    if !at_line_start {
                        out.push('\n');
                    }
                    let depth = match level {
                        HeadingLevel::H1 => 1,
                        HeadingLevel::H2 => 2,
                        HeadingLevel::H3 => 3,
                        HeadingLevel::H4 => 4,
                        HeadingLevel::H5 => 5,
                        HeadingLevel::H6 => 6,
                    };
                    out.push_str(&"=".repeat(depth));
                    out.push(' ');
                    at_line_start = false;
                }
                Tag::BlockQuote(_) => {
                    blockquote_depth += 1;
                    if !at_line_start {
                        out.push('\n');
                    }
                    out.push_str("#quote[");
                    at_line_start = false;
                }
                Tag::CodeBlock(kind) => {
                    if !at_line_start {
                        out.push('\n');
                    }
                    in_code_block = true;
                    if let pulldown_cmark::CodeBlockKind::Fenced(lang) = kind {
                        let lang_str = lang.to_string();
                        if !lang_str.trim().is_empty() {
                            code_block_lang = Some(lang_str);
                        }
                    }
                    out.push_str("```");
                    if let Some(lang) = &code_block_lang {
                        out.push_str(lang);
                    }
                    out.push('\n');
                    at_line_start = true;
                }
                Tag::List(start) => {
                    list_stack.push(start);
                    if !at_line_start {
                        out.push('\n');
                    }
                }
                Tag::Item => {
                    if !at_line_start {
                        out.push('\n');
                    }
                    let indent = "  ".repeat(list_stack.len().saturating_sub(1));
                    match list_stack.last_mut() {
                        Some(Some(n)) => {
                            out.push_str(&indent);
                            // Typst orderered lists use "+" — the actual
                            // numbering is implicit. We drop the original
                            // number on purpose.
                            out.push_str("+ ");
                            *n += 1;
                        }
                        _ => {
                            out.push_str(&indent);
                            out.push_str("- ");
                        }
                    }
                    at_line_start = false;
                }
                Tag::Emphasis => {
                    out.push('_');
                    at_line_start = false;
                }
                Tag::Strong => {
                    out.push('*');
                    at_line_start = false;
                }
                Tag::Strikethrough => {
                    out.push_str("#strike[");
                    at_line_start = false;
                }
                Tag::Link { dest_url, .. } => {
                    pending_link_url.push(dest_url.to_string());
                    out.push_str("#link(\"");
                    out.push_str(&dest_url);
                    out.push_str("\")[");
                    at_line_start = false;
                }
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Paragraph => {
                    ensure_newline(&mut out, &mut at_line_start);
                    out.push('\n');
                }
                TagEnd::Heading(_) => {
                    ensure_newline(&mut out, &mut at_line_start);
                    out.push('\n');
                }
                TagEnd::BlockQuote(_) => {
                    blockquote_depth = blockquote_depth.saturating_sub(1);
                    out.push(']');
                    ensure_newline(&mut out, &mut at_line_start);
                    out.push('\n');
                }
                TagEnd::CodeBlock => {
                    in_code_block = false;
                    code_block_lang = None;
                    ensure_newline(&mut out, &mut at_line_start);
                    out.push_str("```\n\n");
                    at_line_start = true;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    ensure_newline(&mut out, &mut at_line_start);
                    if list_stack.is_empty() {
                        out.push('\n');
                    }
                }
                TagEnd::Item => {
                    // List item content already ended with prose; the next
                    // Start(Item) or End(List) will add the newline.
                }
                TagEnd::Emphasis => out.push('_'),
                TagEnd::Strong => out.push('*'),
                TagEnd::Strikethrough => out.push(']'),
                TagEnd::Link => {
                    pending_link_url.pop();
                    out.push(']');
                }
                _ => {}
            },
            Event::Text(t) => {
                out.push_str(&t);
                at_line_start = t.ends_with('\n');
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&t);
                out.push('`');
                at_line_start = false;
            }
            Event::SoftBreak => {
                if in_code_block {
                    out.push('\n');
                    at_line_start = true;
                } else {
                    out.push(' ');
                }
            }
            Event::HardBreak => {
                out.push_str(" \\\n");
                at_line_start = true;
            }
            Event::Rule => {
                if !at_line_start {
                    out.push('\n');
                }
                out.push_str("#line(length: 100%)\n\n");
                at_line_start = true;
            }
            _ => {}
        }
    }
    // Trim trailing blank lines but keep one terminator.
    while out.ends_with("\n\n\n") {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_renders() {
        let lines = render("# Hello\n\nworld");
        assert!(lines.len() >= 2, "{lines:?}");
    }

    #[test]
    fn typst_heading_h1() {
        let out = markdown_to_typst("# Title\n\nbody");
        assert!(out.starts_with("= Title\n"), "got: {out}");
        assert!(out.contains("body"));
    }

    #[test]
    fn typst_heading_h3() {
        let out = markdown_to_typst("### Sub\n");
        assert!(out.starts_with("=== Sub"), "got: {out}");
    }

    #[test]
    fn typst_bold_italic() {
        let out = markdown_to_typst("**bold** and *italic* and _emph_");
        // Bold → *…*, italic/emph → _…_
        assert!(out.contains("*bold*"), "got: {out}");
        assert!(out.contains("_italic_"));
        assert!(out.contains("_emph_"));
    }

    #[test]
    fn typst_unordered_list() {
        let out = markdown_to_typst("- one\n- two\n");
        assert!(out.contains("- one"), "got: {out}");
        assert!(out.contains("- two"));
    }

    #[test]
    fn typst_ordered_list() {
        let out = markdown_to_typst("1. first\n2. second\n");
        assert!(out.contains("+ first"), "got: {out}");
        assert!(out.contains("+ second"));
    }

    #[test]
    fn typst_inline_code() {
        let out = markdown_to_typst("Use `Ctrl+S` to save.");
        assert!(out.contains("`Ctrl+S`"), "got: {out}");
    }

    #[test]
    fn typst_link() {
        let out = markdown_to_typst("see [docs](https://example.com)");
        assert!(out.contains(r#"#link("https://example.com")[docs]"#), "got: {out}");
    }

    #[test]
    fn render_partial_streaming() {
        // Trailing-open bold should not panic and produce some output.
        let lines = render("Streaming **half-bold");
        assert!(!lines.is_empty());
    }

    #[test]
    fn empty_inputs_are_safe() {
        assert!(render("").is_empty());
        assert!(markdown_to_typst("").is_empty());
    }
}
