//! 1.2.10+ — hand-rolled HJSON walker that builds a
//! `path → byte-range` index over the source text.
//!
//! Phase 2 foundation: every later save / inspect /
//! comment-render path needs to know **where** each
//! leaf value lives in the source bytes so we can
//! splice in a new value without disturbing comments,
//! unknown fields, or whitespace.
//!
//! The walker is *intentionally* not a full HJSON
//! parser.  `serde_hjson` has already done semantic
//! validation by the time we get here — our job is to
//! map the parsed shape back onto source spans.  We
//! tolerate any HJSON that round-trips through
//! `serde_hjson`; anything richer (e.g. exotic
//! number formats) we still recognise structurally
//! because we only ever record byte ranges, never
//! interpret values.
//!
//! What it produces:
//!
//!   * `leaves[path] = LeafSpan { value_range,
//!     leading_comments_range, .. }` for every scalar
//!     / array leaf (i.e. anything that isn't a
//!     stanza).
//!   * `stanzas[path] = StanzaSpan { open_brace,
//!     close_brace, .. }` for every nested object.
//!   * `top_level_object_body` — the byte range of
//!     the implicit-or-explicit top-level object's
//!     contents (for appending new fields at the
//!     root).
//!
//! What it doesn't do (in Phase 2):
//!
//!   * Validate semantic HJSON correctness.  Garbage
//!     in → garbage out; we trust the upstream
//!     `serde_hjson` pass.
//!   * Locate spans for individual elements inside
//!     arrays.  The whole array is one span.

use std::collections::BTreeMap;
use std::ops::Range;

#[derive(Debug, Clone)]
pub struct LeafSpan {
    /// Byte range of the value text (exclusive of any
    /// trailing whitespace / comma).
    pub value_range: Range<usize>,
    /// Byte range of leading comments attached to
    /// this leaf — the contiguous block of `#` / `//`
    /// / `/* */` comments immediately preceding the
    /// key, separated from the key by at most
    /// horizontal whitespace + a single newline.
    /// Surfaced by the Phase 3 `Ctrl+B i` comment
    /// inspector; not read by the Phase 2 save path.
    #[allow(dead_code)]
    pub leading_comments_range: Option<Range<usize>>,
}

#[derive(Debug, Clone)]
pub struct StanzaSpan {
    /// Byte index of the `{` opening the stanza.
    /// Reserved for Phase 3's inspector pane.
    #[allow(dead_code)]
    pub open_brace: usize,
    /// Byte index of the `}` closing the stanza.
    /// Used by the save pipeline as the append
    /// insertion point.
    pub close_brace: usize,
    /// Byte range of the stanza's leading comments.
    /// Reserved for Phase 3.
    #[allow(dead_code)]
    pub leading_comments_range: Option<Range<usize>>,
}

#[derive(Debug, Clone)]
pub struct HjsonIndex {
    /// The source text the spans index into.
    pub source: String,
    pub leaves: BTreeMap<String, LeafSpan>,
    pub stanzas: BTreeMap<String, StanzaSpan>,
    /// Insertion point at the top of the file (just
    /// inside the outer `{`, or at byte 0 when the
    /// file is implicit-object style).  Reserved for
    /// future top-of-file appends (Phase 2 appends
    /// happen at `top_level_body_end`).
    #[allow(dead_code)]
    pub top_level_body_start: usize,
    /// Closing-side insertion point.  Just before the
    /// final `}` if present, else end of file.
    pub top_level_body_end: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("unexpected EOF at byte {pos}")]
    UnexpectedEof { pos: usize },
    #[error("unexpected `{ch}` at byte {pos}")]
    Unexpected { ch: char, pos: usize },
    #[error("unterminated string starting at byte {pos}")]
    UnterminatedString { pos: usize },
    /// Reserved — current walker tolerates
    /// unterminated block comments by consuming to
    /// EOF.  Kept as a variant so future stricter
    /// parsing can switch in without an enum bump.
    #[allow(dead_code)]
    #[error("unterminated block comment starting at byte {pos}")]
    UnterminatedBlockComment { pos: usize },
}

pub type Result<T> = std::result::Result<T, ParseError>;

pub fn parse(source: &str) -> Result<HjsonIndex> {
    let mut w = Walker::new(source);
    w.parse_top_level()?;
    Ok(HjsonIndex {
        source: source.to_string(),
        leaves: w.leaves,
        stanzas: w.stanzas,
        top_level_body_start: w.top_level_body_start,
        top_level_body_end: w.top_level_body_end,
    })
}

struct Walker<'a> {
    src: &'a [u8],
    pos: usize,
    leaves: BTreeMap<String, LeafSpan>,
    stanzas: BTreeMap<String, StanzaSpan>,
    top_level_body_start: usize,
    top_level_body_end: usize,
    /// Span of the contiguous block of `#` / `//` /
    /// `/* */` comments that immediately precede the
    /// next key.  Reset every time we skip a blank
    /// line between comments and key, or after we
    /// attach the comments to a leaf.
    pending_leading_comments: Option<Range<usize>>,
}

impl<'a> Walker<'a> {
    fn new(src: &'a str) -> Self {
        Self {
            src: src.as_bytes(),
            pos: 0,
            leaves: BTreeMap::new(),
            stanzas: BTreeMap::new(),
            top_level_body_start: 0,
            top_level_body_end: src.len(),
            pending_leading_comments: None,
        }
    }

    fn parse_top_level(&mut self) -> Result<()> {
        self.skip_trivia();
        if self.peek() == Some(b'{') {
            self.advance();
            self.top_level_body_start = self.pos;
            self.parse_object_body("")?;
            self.skip_trivia();
            if self.peek() == Some(b'}') {
                self.top_level_body_end = self.pos;
                self.advance();
            } else {
                self.top_level_body_end = self.pos;
            }
        } else {
            // Implicit top-level object.
            self.top_level_body_start = self.pos;
            self.parse_object_body("")?;
            self.top_level_body_end = self.pos;
        }
        Ok(())
    }

    fn parse_object_body(&mut self, parent_path: &str) -> Result<()> {
        loop {
            self.skip_trivia();
            match self.peek() {
                None => return Ok(()),
                Some(b'}') => return Ok(()),
                _ => {}
            }
            let leading = self.pending_leading_comments.take();
            // Parse key.
            let key = self.read_key()?;
            self.skip_trivia();
            // Optional `:` separator (HJSON requires
            // it but is forgiving about whitespace).
            if self.peek() == Some(b':') {
                self.advance();
            }
            // Inline whitespace + comments between
            // `:` and value belong to the value, not
            // the next field — so we DON'T let them
            // bleed into the next pending-leading.
            self.skip_inline_trivia();
            let path = if parent_path.is_empty() {
                key
            } else {
                format!("{parent_path}.{key}")
            };
            let value_start = self.pos;
            let value_kind = self.parse_value(&path)?;
            let value_end = self.pos;
            match value_kind {
                ValueKind::Object { open, close } => {
                    self.stanzas.insert(
                        path,
                        StanzaSpan {
                            open_brace: open,
                            close_brace: close,
                            leading_comments_range: leading,
                        },
                    );
                }
                _ => {
                    self.leaves.insert(
                        path,
                        LeafSpan {
                            value_range: value_start..value_end,
                            leading_comments_range: leading,
                        },
                    );
                }
            }
            // Optional trailing comma + whitespace
            // before next key.  Pending leading
            // comments reset here in case the value
            // ended on a trailing inline comment.
            self.skip_trivia();
            if self.peek() == Some(b',') {
                self.advance();
            }
        }
    }

    fn read_key(&mut self) -> Result<String> {
        match self.peek() {
            Some(b'"') => {
                let s = self.read_quoted_string(b'"')?;
                Ok(s)
            }
            Some(b'\'') => {
                let s = self.read_quoted_string(b'\'')?;
                Ok(s)
            }
            Some(_) => {
                // Unquoted key: read until `:` or
                // whitespace or `,`.
                let start = self.pos;
                while let Some(c) = self.peek() {
                    if c == b':'
                        || c == b','
                        || c.is_ascii_whitespace()
                        || c == b'{'
                        || c == b'['
                        || c == b'}'
                    {
                        break;
                    }
                    self.advance();
                }
                let end = self.pos;
                let key = std::str::from_utf8(&self.src[start..end])
                    .map_err(|_| ParseError::Unexpected {
                        ch: '?',
                        pos: start,
                    })?
                    .to_string();
                if key.is_empty() {
                    return Err(ParseError::Unexpected {
                        ch: self.peek().map(|b| b as char).unwrap_or('\0'),
                        pos: self.pos,
                    });
                }
                Ok(key)
            }
            None => Err(ParseError::UnexpectedEof { pos: self.pos }),
        }
    }

    fn parse_value(&mut self, path: &str) -> Result<ValueKind> {
        match self.peek() {
            Some(b'{') => {
                let open = self.pos;
                self.advance();
                self.parse_object_body(path)?;
                self.skip_trivia();
                let close = self.pos;
                if self.peek() == Some(b'}') {
                    self.advance();
                } else {
                    return Err(ParseError::UnexpectedEof { pos: self.pos });
                }
                Ok(ValueKind::Object { open, close })
            }
            Some(b'[') => {
                self.skip_array()?;
                Ok(ValueKind::Array)
            }
            Some(b'"') => {
                self.skip_quoted_string(b'"')?;
                Ok(ValueKind::String)
            }
            Some(b'\'') => {
                if self.peek_n(3) == Some(b"'''".as_ref()) {
                    self.skip_triple_string()?;
                } else {
                    self.skip_quoted_string(b'\'')?;
                }
                Ok(ValueKind::String)
            }
            Some(_) => {
                // Unquoted scalar: read until newline,
                // comma, `}`, `]`, or `#` / `//`
                // comment start.
                self.skip_unquoted_scalar();
                Ok(ValueKind::Scalar)
            }
            None => Err(ParseError::UnexpectedEof { pos: self.pos }),
        }
    }

    fn skip_array(&mut self) -> Result<()> {
        // Consume `[`.
        self.advance();
        let mut depth = 1usize;
        while depth > 0 {
            self.skip_trivia();
            match self.peek() {
                None => {
                    return Err(ParseError::UnexpectedEof { pos: self.pos });
                }
                Some(b'[') => {
                    self.advance();
                    depth += 1;
                }
                Some(b']') => {
                    self.advance();
                    depth -= 1;
                }
                Some(b'"') => {
                    self.skip_quoted_string(b'"')?;
                }
                Some(b'\'') => {
                    if self.peek_n(3) == Some(b"'''".as_ref()) {
                        self.skip_triple_string()?;
                    } else {
                        self.skip_quoted_string(b'\'')?;
                    }
                }
                Some(b'{') => {
                    self.advance();
                    self.parse_object_body("")?;
                    self.skip_trivia();
                    if self.peek() == Some(b'}') {
                        self.advance();
                    }
                }
                Some(_) => {
                    self.advance();
                }
            }
        }
        Ok(())
    }

    fn skip_quoted_string(&mut self, quote: u8) -> Result<()> {
        let start = self.pos;
        // Skip opening quote.
        self.advance();
        while let Some(c) = self.peek() {
            if c == b'\\' {
                self.advance();
                if self.peek().is_some() {
                    self.advance();
                }
            } else if c == quote {
                self.advance();
                return Ok(());
            } else {
                self.advance();
            }
        }
        Err(ParseError::UnterminatedString { pos: start })
    }

    fn read_quoted_string(&mut self, quote: u8) -> Result<String> {
        let start = self.pos;
        self.skip_quoted_string(quote)?;
        // Strip outer quotes; don't bother with escape
        // processing for keys (HJSON keys are rarely
        // escaped).
        let inner_start = start + 1;
        let inner_end = self.pos.saturating_sub(1);
        Ok(std::str::from_utf8(&self.src[inner_start..inner_end])
            .map_err(|_| ParseError::Unexpected { ch: '?', pos: start })?
            .to_string())
    }

    fn skip_triple_string(&mut self) -> Result<()> {
        let start = self.pos;
        // Consume opening `'''`.
        self.advance();
        self.advance();
        self.advance();
        while self.pos < self.src.len() {
            if self.peek_n(3) == Some(b"'''".as_ref()) {
                self.advance();
                self.advance();
                self.advance();
                return Ok(());
            }
            self.advance();
        }
        Err(ParseError::UnterminatedString { pos: start })
    }

    fn skip_unquoted_scalar(&mut self) {
        // HJSON unquoted scalars run to the end of
        // the line (newline) or to a `,` / `}` / `]`
        // at the same nesting level.  Inline `#` /
        // `//` start a trailing comment and terminate
        // the value.
        while let Some(c) = self.peek() {
            if c == b'\n' || c == b'\r' || c == b',' || c == b'}' || c == b']' {
                break;
            }
            // Trailing-comment terminator: only when
            // it's at the start of a comment marker.
            if c == b'#' {
                break;
            }
            if c == b'/' && self.peek_at(1) == Some(b'/') {
                break;
            }
            if c == b'/' && self.peek_at(1) == Some(b'*') {
                break;
            }
            self.advance();
        }
        // Trim trailing horizontal whitespace from
        // the value so the splice replaces exactly
        // the value text.
        while self.pos > 0 {
            let prev = self.src[self.pos - 1];
            if prev == b' ' || prev == b'\t' {
                self.pos -= 1;
            } else {
                break;
            }
        }
    }

    /// Skip whitespace + comments.  Tracks
    /// `pending_leading_comments` as the contiguous
    /// block of comments immediately preceding the
    /// current position.  A blank line (two newlines)
    /// between comments and the next field discards
    /// the pending block — those comments are
    /// detached and don't attach to the field below.
    fn skip_trivia(&mut self) {
        let mut current_block: Option<Range<usize>> = None;
        // Count consecutive newlines since the last
        // non-whitespace character.  Two-or-more
        // means a blank line — discards the block.
        let mut consecutive_newlines: u32 = 0;
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\r' {
                self.advance();
            } else if c == b'\n' {
                self.advance();
                consecutive_newlines += 1;
                if consecutive_newlines >= 2 && current_block.is_some() {
                    // Blank line after a comment
                    // block — detach it.
                    current_block = None;
                }
            } else if c == b'#' {
                // `#` line comment.  After consuming
                // its trailing newline (via
                // `advance_line`), we've "spent" one
                // newline; the next newline in the
                // stream is the SECOND, i.e. a blank
                // line.  Seed consecutive_newlines
                // with 1 to capture that.
                let start = self.pos;
                self.advance_line();
                let end = self.pos;
                current_block = Some(extend_or_new(current_block, start, end));
                consecutive_newlines = 1;
            } else if c == b'/' && self.peek_at(1) == Some(b'/') {
                let start = self.pos;
                self.advance_line();
                let end = self.pos;
                current_block = Some(extend_or_new(current_block, start, end));
                consecutive_newlines = 1;
            } else if c == b'/' && self.peek_at(1) == Some(b'*') {
                consecutive_newlines = 0;
                let start = self.pos;
                // Consume `/*`.
                self.advance();
                self.advance();
                while self.pos < self.src.len() {
                    if self.src[self.pos] == b'*'
                        && self.pos + 1 < self.src.len()
                        && self.src[self.pos + 1] == b'/'
                    {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
                let end = self.pos;
                current_block = Some(extend_or_new(current_block, start, end));
            } else {
                break;
            }
        }
        if current_block.is_some() {
            self.pending_leading_comments = current_block;
        }
    }

    /// Skip whitespace + comments but DON'T update
    /// `pending_leading_comments` — used between a key
    /// and its value, where any trailing/inline
    /// comment shouldn't attach to the NEXT key.
    fn skip_inline_trivia(&mut self) {
        while let Some(c) = self.peek() {
            if c == b' ' || c == b'\t' || c == b'\r' {
                self.advance();
            } else if c == b'#' {
                self.advance_line();
            } else if c == b'/' && self.peek_at(1) == Some(b'/') {
                self.advance_line();
            } else if c == b'/' && self.peek_at(1) == Some(b'*') {
                self.advance();
                self.advance();
                while self.pos < self.src.len() {
                    if self.src[self.pos] == b'*'
                        && self.pos + 1 < self.src.len()
                        && self.src[self.pos + 1] == b'/'
                    {
                        self.advance();
                        self.advance();
                        break;
                    }
                    self.advance();
                }
            } else {
                break;
            }
        }
    }

    fn advance_line(&mut self) {
        while let Some(c) = self.peek() {
            self.advance();
            if c == b'\n' {
                break;
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    fn peek_n(&self, n: usize) -> Option<&[u8]> {
        if self.pos + n <= self.src.len() {
            Some(&self.src[self.pos..self.pos + n])
        } else {
            None
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
    }
}

fn extend_or_new(prev: Option<Range<usize>>, start: usize, end: usize) -> Range<usize> {
    match prev {
        Some(range) => range.start..end,
        None => start..end,
    }
}

#[derive(Debug, Clone, Copy)]
enum ValueKind {
    Object { open: usize, close: usize },
    Array,
    String,
    Scalar,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_object_records_every_leaf() {
        let src = "{\n  a: 1\n  b: hello\n  c: true\n}";
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("a"));
        assert!(idx.leaves.contains_key("b"));
        assert!(idx.leaves.contains_key("c"));
        let a = idx.leaves.get("a").unwrap();
        assert_eq!(&src[a.value_range.clone()], "1");
        let b = idx.leaves.get("b").unwrap();
        assert_eq!(&src[b.value_range.clone()], "hello");
        let c = idx.leaves.get("c").unwrap();
        assert_eq!(&src[c.value_range.clone()], "true");
    }

    #[test]
    fn nested_object_records_stanza_and_inner_leaves() {
        let src = "{\n  outer: {\n    inner: 42\n  }\n}";
        let idx = parse(src).unwrap();
        assert!(idx.stanzas.contains_key("outer"));
        assert!(idx.leaves.contains_key("outer.inner"));
        let inner = idx.leaves.get("outer.inner").unwrap();
        assert_eq!(&src[inner.value_range.clone()], "42");
    }

    #[test]
    fn leading_line_comments_attach_to_next_key() {
        let src = "{\n  // first comment\n  // second comment\n  key: 1\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("key").unwrap();
        let range = leaf
            .leading_comments_range
            .clone()
            .expect("leading comments should attach");
        let text = &src[range];
        assert!(text.contains("first comment"));
        assert!(text.contains("second comment"));
    }

    #[test]
    fn hash_comment_recognised() {
        let src = "{\n  # hash style\n  key: 1\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("key").unwrap();
        let range = leaf.leading_comments_range.clone().unwrap();
        assert!(src[range].contains("hash style"));
    }

    #[test]
    fn quoted_string_value_records_quotes_inside_range() {
        let src = "{\n  key: \"hello world\"\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("key").unwrap();
        assert_eq!(&src[leaf.value_range.clone()], "\"hello world\"");
    }

    #[test]
    fn string_with_internal_braces_doesnt_confuse_parser() {
        let src = r#"{
  greeting: "Hello, {name}!"
  ok: true
}"#;
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("greeting"));
        assert!(idx.leaves.contains_key("ok"));
        let greeting = idx.leaves.get("greeting").unwrap();
        assert_eq!(&src[greeting.value_range.clone()], "\"Hello, {name}!\"");
    }

    #[test]
    fn array_value_is_one_span() {
        let src = "{\n  list: [\n    \"a\",\n    \"b\",\n    \"c\",\n  ]\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("list").unwrap();
        let text = &src[leaf.value_range.clone()];
        assert!(text.starts_with('['));
        assert!(text.ends_with(']'));
        assert!(text.contains("\"a\""));
    }

    #[test]
    fn unquoted_scalar_stops_at_newline() {
        let src = "{\n  language: english\n  another: 5\n}";
        let idx = parse(src).unwrap();
        let lang = idx.leaves.get("language").unwrap();
        assert_eq!(&src[lang.value_range.clone()], "english");
        let another = idx.leaves.get("another").unwrap();
        assert_eq!(&src[another.value_range.clone()], "5");
    }

    #[test]
    fn trailing_inline_comment_doesnt_eat_value() {
        let src = "{\n  port: 8080  // server port\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("port").unwrap();
        assert_eq!(&src[leaf.value_range.clone()], "8080");
    }

    #[test]
    fn implicit_top_level_object_works() {
        // No outer braces — also valid HJSON.
        let src = "a: 1\nb: 2\n";
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("a"));
        assert!(idx.leaves.contains_key("b"));
        assert_eq!(idx.top_level_body_start, 0);
    }

    #[test]
    fn blank_line_resets_pending_comments() {
        let src = "{\n  // header explaining the file\n\n  key: 1\n}";
        let idx = parse(src).unwrap();
        let leaf = idx.leaves.get("key").unwrap();
        // The header comment shouldn't attach because
        // a blank line separates it.
        assert!(leaf.leading_comments_range.is_none(),
            "expected no leading comments after blank line; got: {:?}",
            leaf.leading_comments_range
        );
    }

    #[test]
    fn block_comment_recognised() {
        let src = "{\n  /* block\n     comment */ key: 1\n}";
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("key"));
    }

    #[test]
    fn deep_nesting_paths_use_dot_separator() {
        let src = "{ a: { b: { c: { d: 1 } } } }";
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("a.b.c.d"));
        assert!(idx.stanzas.contains_key("a"));
        assert!(idx.stanzas.contains_key("a.b"));
        assert!(idx.stanzas.contains_key("a.b.c"));
    }

    #[test]
    fn stanza_braces_recorded_correctly() {
        let src = "{ a: { b: 1 } }";
        let idx = parse(src).unwrap();
        let span = idx.stanzas.get("a").unwrap();
        assert_eq!(src.as_bytes()[span.open_brace], b'{');
        assert_eq!(src.as_bytes()[span.close_brace], b'}');
    }

    #[test]
    fn realistic_inkhaven_hjson_round_trips() {
        let src = r#"// inkhaven project config
{
  language: english

  embeddings: {
    model: MultilingualE5Small
    chunk_size: 800
    chunk_overlap: 0.15
  }

  llm: {
    default: deepseek
    providers: {
      gemini: {
        model: gemini-2.5-pro
        api_key_env: GEMINI_API_KEY
      }
    }
  }
}"#;
        let idx = parse(src).unwrap();
        assert!(idx.leaves.contains_key("language"));
        assert!(idx.leaves.contains_key("embeddings.model"));
        assert!(idx.leaves.contains_key("embeddings.chunk_size"));
        assert!(idx.leaves.contains_key("embeddings.chunk_overlap"));
        assert!(idx.leaves.contains_key("llm.default"));
        assert!(idx.leaves.contains_key("llm.providers.gemini.model"));
        assert!(idx.leaves.contains_key("llm.providers.gemini.api_key_env"));
        assert!(idx.stanzas.contains_key("embeddings"));
        assert!(idx.stanzas.contains_key("llm"));
        assert!(idx.stanzas.contains_key("llm.providers"));
        assert!(idx.stanzas.contains_key("llm.providers.gemini"));
    }
}
