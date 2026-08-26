//! Typst → Markdown converter.
//!
//! Covers the subset inkhaven itself emits via its `wrap_*` templates
//! and the paragraph bodies users typically write:
//!
//! * `= Heading` / `== Sub` / `=== SubSub` → `#` / `##` / `###`
//! * `*bold*` → `**bold**`, `_italic_` → `*italic*`
//! * Bullet lists (`- foo`) and ordered lists (`+ foo`) pass through
//! * `#image("path")` → `![](path)`, `#image("path", caption: "x")` →
//!   `![x](path)`, and a wrapped `#figure(image("path"), caption: [x])`
//!   (single-line) → the same
//! * `#footnote[body]` (inline, anywhere in a line) → a `[^N]` marker plus a
//!   `[^N]: body` definition list emitted at the end; balanced-bracket aware
//! * `@key` / `@key[locus]` references → pandoc-style `[@key]` / `[@key, locus]`
//!   (a `@` only starts a ref at a word boundary)
//! * Lines starting with `#` that we don't recognise are wrapped in
//!   `` `…` `` so the user can see the un-converted macro in the
//!   markdown without it bricking subsequent rendering.
//!
//! Out of scope: arbitrary Typst expressions, math, tables, and multi-line
//! `#figure(…)` blocks; code blocks (anything inside a `#raw(…)` block is
//! dropped through verbatim as a ` ``` ` fenced block).
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
    // Iterate by `char`, not by byte: `bytes[i] as char` is a Latin-1 decode that
    // mangles every multi-byte UTF-8 char (Cyrillic/accented captions → mojibake).
    // The delimiters (`"`, `\`) are ASCII, so char-iteration handles them cleanly.
    let mut chars = s.char_indices();
    if !matches!(chars.next(), Some((_, '"'))) {
        return None;
    }
    let mut out = String::new();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                if let Some((_, next)) = chars.next() {
                    out.push(next);
                }
            }
            '"' => return Some((out, &s[i + 1..])),
            other => out.push(other),
        }
    }
    None
}

/// Inline-emphasis rewrite. Typst uses `*bold*` and `_italic_`; markdown wants
/// `**bold**` and `*italic*`. A7 — only a balanced pair whose inner content is
/// non-empty and not whitespace-adjacent is converted, so stray `*` in prose
/// (arithmetic like `3 * 4 * 5`, a literal glyph) survives — the same guard the
/// HTML exporter's `replace_pair` uses. Bold runs first so the single `*` the
/// italic pass emits isn't re-paired into bold.
fn convert_emphasis(line: &str) -> String {
    let bold = replace_delim(line, '*', "**", "**");
    replace_delim(&bold, '_', "*", "*")
}

/// Replace balanced `delim … delim` pairs with `open … close`, skipping any pair
/// whose inner content is empty or starts/ends with a space (a stray delimiter).
/// On a skip only the first delimiter is emitted literally, leaving the rest —
/// including its potential closer — to be reconsidered as a later opener.
fn replace_delim(s: &str, delim: char, open: &str, close: &str) -> String {
    let d = delim.len_utf8();
    let mut out = String::with_capacity(s.len() + 8);
    let mut rest = s;
    while let Some(start) = rest.find(delim) {
        let after = &rest[start + d..];
        if let Some(end) = after.find(delim) {
            let inner = &after[..end];
            if !inner.is_empty() && !inner.starts_with(' ') && !inner.ends_with(' ') {
                out.push_str(&rest[..start]);
                out.push_str(open);
                out.push_str(inner);
                out.push_str(close);
                rest = &after[end + d..];
                continue;
            }
        }
        // No valid close (or a stray delimiter) — emit it literally and move on.
        out.push_str(&rest[..start + d]);
        rest = &rest[start + d..];
    }
    out.push_str(rest);
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

/// `#figure(image("path"), caption: [Cap])` (or `caption: "Cap"`) → `![Cap](path)`.
/// The plain `#image(...)` case is [`convert_image_call`]; this catches the wrapped
/// figure form the old converter leaked as literal code. Single-line only.
fn convert_figure_image(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if !trimmed.starts_with("#figure(") {
        return None;
    }
    let img_idx = trimmed.find("image(")?;
    let after = &trimmed[img_idx + "image(".len()..];
    let (path, _) = read_quoted(after)?;
    // Caption may be a string (`caption: "…"`) or content (`caption: [ … ]`).
    let mut alt = String::new();
    if let Some(idx) = trimmed.find("caption:") {
        let after_cap = trimmed[idx + "caption:".len()..].trim_start();
        if let Some((cap, _)) = read_quoted(after_cap) {
            alt = cap;
        } else if let Some(rest) = after_cap.strip_prefix('[') {
            if let Some((cap, _)) = read_bracketed(rest) {
                alt = cap.trim().to_string();
            }
        }
    }
    Some(format!("![{}]({})", alt.trim(), path))
}

/// Read a `[…]`-delimited body from `s` (which begins **after** the opening `[`),
/// respecting nested brackets. Returns `(body, tail-after-])`. `None` if unbalanced.
fn read_bracketed(s: &str) -> Option<(String, &str)> {
    let mut depth = 1i32;
    for (i, c) in s.char_indices() {
        match c {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some((s[..i].to_string(), &s[i + c.len_utf8()..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Replace each inline `#footnote[body]` with a markdown `[^N]` marker, collecting
/// the (emphasis-/ref-converted) body into `notes` for a definition list emitted at
/// the end. Balanced-bracket aware, so a body may itself contain `[…]`; an
/// unbalanced `#footnote[` is left literal rather than eating the rest of the text.
fn extract_footnotes(text: &str, notes: &mut Vec<String>) -> String {
    const OPEN: &str = "#footnote[";
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(pos) = rest.find(OPEN) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + OPEN.len()..];
        match read_bracketed(after) {
            Some((body, tail)) => {
                notes.push(convert_emphasis(&convert_refs(&body)));
                out.push_str(&format!("[^{}]", notes.len()));
                rest = tail;
            }
            None => {
                out.push_str(&rest[pos..pos + OPEN.len()]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn is_ref_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_' | ':')
}

/// Convert Typst references / citations to pandoc-style markdown citations:
/// `@key` → `[@key]`, `@key[locus]` → `[@key, locus]`. A `@` only starts a ref at a
/// word boundary (start, or after whitespace / an opening bracket-quote), so a stray
/// `@` inside prose is left alone. Pandoc understands both the cite and the label
/// form; imperfect for figure cross-refs but far better than leaking literal `@key`.
fn convert_refs(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len() + 8);
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let boundary =
            i == 0 || matches!(chars[i - 1], ' ' | '\t' | '(' | '[' | '"' | '\u{201c}' | '\u{ab}');
        if c == '@' && boundary && chars.get(i + 1).is_some_and(|d| d.is_alphabetic()) {
            let mut j = i + 1;
            while j < chars.len() && is_ref_char(chars[j]) {
                j += 1;
            }
            let key: String = chars[i + 1..j].iter().collect();
            // Optional `[locus]`.
            if chars.get(j) == Some(&'[') {
                if let Some(close) = chars[j + 1..].iter().position(|&d| d == ']') {
                    let locus: String = chars[j + 1..j + 1 + close].iter().collect();
                    out.push_str(&format!("[@{key}, {}]", locus.trim()));
                    i = j + 1 + close + 1;
                    continue;
                }
            }
            out.push_str(&format!("[@{key}]"));
            i = j;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The full inline transform for a line of prose (or a heading / list body):
/// footnotes → refs → emphasis, in that order (footnotes and refs first so the
/// emphasis rewrite never mangles their brackets). Footnote bodies are collected
/// into `notes`.
fn convert_inline(text: &str, notes: &mut Vec<String>) -> String {
    let no_fn = extract_footnotes(text, notes);
    convert_emphasis(&convert_refs(&no_fn))
}

/// Public entry. See module docs for the supported subset.
pub fn typst_to_markdown(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 64);
    let mut in_raw_block = false;
    let mut footnotes: Vec<String> = Vec::new();
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
            out.push_str(&convert_inline(rest, &mut footnotes));
            out.push('\n');
            continue;
        }

        // Images — plain `#image(...)` or a wrapped `#figure(image(...))`.
        if let Some(img) = convert_image_call(raw_line).or_else(|| convert_figure_image(raw_line)) {
            out.push_str(&img);
            out.push('\n');
            continue;
        }

        // Bullet / ordered lists pass through.
        if let Some(rest) = raw_line.strip_prefix("- ") {
            out.push_str("- ");
            out.push_str(&convert_inline(rest, &mut footnotes));
            out.push('\n');
            continue;
        }
        if let Some(rest) = raw_line.strip_prefix("+ ") {
            out.push_str("1. ");
            out.push_str(&convert_inline(rest, &mut footnotes));
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

        out.push_str(&convert_inline(raw_line, &mut footnotes));
        out.push('\n');
    }
    if in_raw_block {
        // Unclosed raw block — close it so the markdown is valid.
        out.push_str("```\n");
    }
    // Emit the collected footnote definitions (markdown's `[^N]: …` list).
    if !footnotes.is_empty() {
        out.push('\n');
        for (i, body) in footnotes.iter().enumerate() {
            out.push_str(&format!("[^{}]: {body}\n", i + 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_quoted_preserves_non_ascii() {
        // Audit regression — must not Latin-1-decode multi-byte UTF-8 to mojibake.
        let (s, rest) = read_quoted("\"café Москва\" tail").unwrap();
        assert_eq!(s, "café Москва");
        assert_eq!(rest, " tail");
        // Escapes still work and keep following non-ASCII intact.
        let (s2, _) = read_quoted("\"a\\\"b é\"").unwrap();
        assert_eq!(s2, "a\"b é");
    }

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
    fn stray_asterisks_are_not_emphasis() {
        // A7 — arithmetic / literal `*` (whitespace-adjacent) survives verbatim
        // instead of being mangled into bold.
        let md = typst_to_markdown("multiply 3 * 4 * 5 now.\n");
        assert!(md.contains("3 * 4 * 5"), "stray asterisks preserved: {md}");
        assert!(!md.contains("**"), "no bold introduced: {md}");
        // A real emphasis pair adjacent to a stray one still converts.
        let md2 = typst_to_markdown("a *bold* b * c\n");
        assert!(md2.contains("**bold**") && md2.contains("b * c"), "got: {md2}");
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

    #[test]
    fn figure_image_with_content_caption() {
        // XP-1 — a wrapped figure used to leak as literal code.
        let md = typst_to_markdown("#figure(image(\"img/map.png\"), caption: [The Reach])\n");
        assert!(md.contains("![The Reach](img/map.png)"), "{md:?}");
        assert!(!md.contains('`'), "no literal-code leak: {md:?}");
        // The string-caption form too.
        let md2 = typst_to_markdown("#figure(image(\"a.png\"), caption: \"Cap\")\n");
        assert!(md2.contains("![Cap](a.png)"), "{md2:?}");
    }

    #[test]
    fn inline_footnote_becomes_marker_plus_definition() {
        // XP-1 — inline #footnote[…] used to leak verbatim.
        let md = typst_to_markdown("The bell rang#footnote[At dawn.] once.\n");
        assert!(md.contains("The bell rang[^1] once."), "marker in place: {md:?}");
        assert!(md.contains("[^1]: At dawn."), "definition emitted: {md:?}");
        assert!(!md.contains("#footnote"), "no literal footnote source: {md:?}");
    }

    #[test]
    fn footnote_body_may_contain_brackets_and_emphasis() {
        let md = typst_to_markdown("x#footnote[see _note_ [ok]] y\n");
        assert!(md.contains("x[^1] y"), "{md:?}");
        // Body keeps its inner bracket and gets emphasis-converted.
        assert!(md.contains("[^1]: see *note* [ok]"), "{md:?}");
    }

    #[test]
    fn references_become_pandoc_citations() {
        // XP-1 — @key / @key[locus] used to pass through literal.
        let md = typst_to_markdown("As in @einstein1905[p. 4] and see @fig-map here.\n");
        assert!(md.contains("[@einstein1905, p. 4]"), "cite with locus: {md:?}");
        assert!(md.contains("[@fig-map]"), "bare ref: {md:?}");
        // A stray non-boundary `@` is left alone.
        let md2 = typst_to_markdown("email a@b later\n");
        assert!(md2.contains("a@b"), "non-boundary @ untouched: {md2:?}");
    }
}
