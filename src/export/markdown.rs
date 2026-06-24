//! Typst → Markdown converter.
//!
//! Covers the subset inkhaven itself emits via its `wrap_*` templates
//! and the paragraph bodies users typically write:
//!
//! * `= Heading` / `== Sub` / `=== SubSub` → `#` / `##` / `###`
//! * `*bold*` → `**bold**`, `_italic_` → `*italic*`
//! * Bullet lists (`- foo`) and ordered lists (`+ foo`) pass through
//! * `#image("path")` → `![](path)`, `#image("path", caption: "x")` →
//!   `![x](path)`
//! * Lines starting with `#` that we don't recognise are wrapped in
//!   `` `…` `` so the user can see the un-converted macro in the
//!   markdown without it bricking subsequent rendering.
//!
//! Out of scope: arbitrary Typst expressions, math, code blocks
//! (anything inside a `#raw(…)` block is dropped through verbatim
//! as a ` ``` ` fenced block).
//!
//! The converter is **lossy by design** — markdown can't represent
//! everything Typst can. The goal is "readable plain-text dump
//! good enough to share / paste / re-format", not round-trip
//! fidelity.

/// Single-pass line-by-line converter. Stateful only across:
///   * fenced raw blocks (`#raw(```…```)`) — we track open / close
///   * bullet vs ordered list — passes through unchanged
fn line_is_heading(line: &str) -> Option<(usize, &str)> {
    // Typst heading: `=`+ followed by space, then the rest.
    let bytes = line.as_bytes();
    let mut eq_count: usize = 0;
    while eq_count < bytes.len() && bytes[eq_count] == b'=' {
        eq_count += 1;
    }
    if eq_count == 0 || eq_count > 6 {
        return None;
    }
    if bytes.get(eq_count).copied() != Some(b' ') {
        return None;
    }
    let rest = line[eq_count + 1..].trim();
    Some((eq_count, rest))
}

/// Best-effort `#image("path")` → `![alt](path)` extractor. Returns
/// None if the line doesn't start with `#image(`.
fn convert_image_call(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("#image(") {
        return None;
    }
    let after = trimmed.trim_start_matches("#image(");
    // First quoted segment is the path.
    let (path, after_path) = match read_quoted(after) {
        Some(p) => p,
        None => return None,
    };
    // Look for `caption:` in the remaining args before the closing
    // paren. If present, use as alt text; otherwise alt is empty.
    let mut alt = String::new();
    if let Some(idx) = after_path.find("caption:") {
        let after_caption = &after_path[idx + "caption:".len()..];
        if let Some((cap, _)) = read_quoted(after_caption.trim_start()) {
            alt = cap;
        }
    }
    Some(format!("![{alt}]({path})"))
}

/// Read the next double-quoted string from `s`, returning the
/// payload and the remaining tail. Handles backslash escapes
/// (`\"` and `\\`). Returns None if `s` doesn't start with `"`.
fn read_quoted(s: &str) -> Option<(String, &str)> {
    let s = s.trim_start();
    let bytes = s.as_bytes();
    if bytes.first().copied() != Some(b'"') {
        return None;
    }
    let mut out = String::new();
    let mut i = 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if i + 1 < bytes.len() => {
                out.push(bytes[i + 1] as char);
                i += 2;
            }
            b'"' => return Some((out, &s[i + 1..])),
            c => {
                out.push(c as char);
                i += 1;
            }
        }
    }
    None
}

/// Inline-emphasis rewrite. Typst uses `*bold*` and `_italic_`;
/// markdown wants `**bold**` and `*italic*`. We only touch
/// well-balanced runs to avoid mangling stray asterisks inside
/// code-ish content.
fn convert_emphasis(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '*' => {
                // Greedy: read until next un-escaped '*' on the same
                // line. If we don't find one, treat the '*' as literal.
                let mut body = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == '*' {
                        closed = true;
                        break;
                    }
                    body.push(d);
                }
                if closed && !body.is_empty() {
                    out.push_str("**");
                    out.push_str(&body);
                    out.push_str("**");
                } else {
                    out.push('*');
                    out.push_str(&body);
                }
            }
            '_' => {
                let mut body = String::new();
                let mut closed = false;
                for d in chars.by_ref() {
                    if d == '_' {
                        closed = true;
                        break;
                    }
                    body.push(d);
                }
                if closed && !body.is_empty() {
                    out.push('*');
                    out.push_str(&body);
                    out.push('*');
                } else {
                    out.push('_');
                    out.push_str(&body);
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// If `line` is a complete single-line `#raw(...)` — parentheses
/// balanced within the line — return its inner text with one pair of
/// surrounding quotes stripped. Returns `None` when the `(` opens an
/// unbalanced (multi-line) block, which the caller renders as a fence.
fn single_line_raw_inner(line: &str) -> Option<String> {
    let open = line.find('(')?;
    let mut depth = 0i32;
    let mut close = None;
    for (i, c) in line.char_indices().skip_while(|&(i, _)| i < open) {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    let close = close?;
    let inner = line[open + 1..close].trim();
    let inner = inner
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .unwrap_or(inner);
    Some(inner.to_string())
}

/// Public entry. See module docs for the supported subset.
pub fn typst_to_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 64);
    let mut in_raw_block = false;
    for raw_line in input.lines() {
        // Preserve raw-content blocks. The most common pattern is a
        // line containing `#raw(` followed by ` ``` …``` ` on its
        // own. We pass these straight through, just stripping the
        // surrounding `#raw(` / `)` wrapper.
        let trimmed = raw_line.trim();
        if !in_raw_block && (trimmed.starts_with("#raw(") || trimmed == "#raw(block:true)") {
            // M2 — a *self-contained* single-line `#raw("…")` (parens
            // balanced on this line) must NOT open a fenced block: the
            // old code did, and since the close only matched a bare `)`
            // line, every following chapter was swallowed into one code
            // block. Render it as an inline span instead; only a genuine
            // multi-line opener (`#raw(` / `#raw(block:true)`) enters
            // block mode.
            if let Some(inner) = single_line_raw_inner(trimmed) {
                if inner.contains('`') {
                    // A backtick in the content would close the span
                    // early (markdown injection) — fence it wider.
                    out.push_str("`` ");
                    out.push_str(&inner);
                    out.push_str(" ``\n");
                } else {
                    out.push('`');
                    out.push_str(&inner);
                    out.push_str("`\n");
                }
                continue;
            }
            in_raw_block = true;
            out.push_str("```\n");
            continue;
        }
        if in_raw_block && trimmed == ")" {
            in_raw_block = false;
            out.push_str("```\n");
            continue;
        }
        if in_raw_block {
            out.push_str(raw_line);
            out.push('\n');
            continue;
        }

        // Headings.
        if let Some((level, rest)) = line_is_heading(raw_line) {
            for _ in 0..level {
                out.push('#');
            }
            out.push(' ');
            out.push_str(&convert_emphasis(rest));
            out.push('\n');
            continue;
        }

        // Images.
        if let Some(img) = convert_image_call(raw_line) {
            out.push_str(&img);
            out.push('\n');
            continue;
        }

        // Bullet / ordered lists pass through.
        if let Some(rest) = raw_line.strip_prefix("- ") {
            out.push_str("- ");
            out.push_str(&convert_emphasis(rest));
            out.push('\n');
            continue;
        }
        if let Some(rest) = raw_line.strip_prefix("+ ") {
            out.push_str("1. ");
            out.push_str(&convert_emphasis(rest));
            out.push('\n');
            continue;
        }

        // Unknown directive line — preserve verbatim inside an
        // inline code span so the reader sees the macro source
        // without it perturbing surrounding flow.
        if raw_line.trim_start().starts_with('#') && !raw_line.trim_start().starts_with("#!") {
            out.push('`');
            out.push_str(raw_line);
            out.push('`');
            out.push('\n');
            continue;
        }

        out.push_str(&convert_emphasis(raw_line));
        out.push('\n');
    }
    if in_raw_block {
        // Unclosed raw block — close it so the markdown is valid.
        out.push_str("```\n");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line_raw_does_not_swallow_following_content() {
        // M2 regression — a self-contained `#raw("…")` must render as an
        // inline span and NOT open a fence that eats the next heading.
        let md = typst_to_markdown("#raw(\"x = 1\")\n= Chapter Two\nbody\n");
        assert!(md.contains("`x = 1`"), "raw should be inline: {md:?}");
        assert!(md.contains("# Chapter Two"), "heading must survive: {md:?}");
        assert!(!md.contains("```"), "no fence should open: {md:?}");
    }

    #[test]
    fn single_line_raw_escapes_inner_backtick() {
        let md = typst_to_markdown("#raw(\"a`b\")\n");
        assert!(md.contains("`` a`b ``"), "backtick must be fenced wider: {md:?}");
    }

    #[test]
    fn multiline_raw_block_still_fences() {
        // A bare `#raw(` opener (unbalanced on the line) keeps block mode.
        let md = typst_to_markdown("#raw(\ncode line\n)\n");
        assert!(md.contains("```"), "multi-line raw should fence: {md:?}");
        assert!(md.contains("code line"));
    }

    #[test]
    fn headings_three_levels() {
        let md = typst_to_markdown("= H1\n== H2\n=== H3\n");
        assert!(md.contains("# H1"));
        assert!(md.contains("## H2"));
        assert!(md.contains("### H3"));
    }

    #[test]
    fn bold_and_italic() {
        let md = typst_to_markdown("*bold* and _italic_ words.\n");
        assert!(md.contains("**bold**"));
        assert!(md.contains("*italic*"));
    }

    #[test]
    fn image_with_caption() {
        let md = typst_to_markdown("#image(\"img/foo.png\", caption: \"Foo\")\n");
        assert!(md.contains("![Foo](img/foo.png)"));
    }

    #[test]
    fn unknown_directive_quoted() {
        let md = typst_to_markdown("#set page(width: 10cm)\n");
        assert!(md.contains("`#set page(width: 10cm)`"));
    }
}
