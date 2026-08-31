/// Single-line text input buffer for the top search bar and bottom AI prompt.
/// Stores the buffer plus a byte-cursor; rendering uses `chars()` so multibyte
/// characters (Cyrillic etc.) display correctly.
#[derive(Debug, Default, Clone)]
pub struct TextInput {
    buffer: String,
    /// Cursor position as a *character* index, not a byte index.
    cursor: usize,
    /// 3.9 — when true the buffer may hold `\n` (the AI prompt compose box):
    /// paste preserves newlines, `insert_newline` works, and `move_up`/`down`
    /// + `Home`/`End` become line-aware. Default false keeps every single-line
    /// consumer (search bar, modal inputs) byte-for-byte unchanged.
    multiline: bool,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
    }

    /// 3.9 — a multi-line input (the AI prompt compose box).
    pub fn multiline() -> Self {
        Self {
            multiline: true,
            ..Self::default()
        }
    }

    /// Visual line count: `1 + '\n' count` for a multi-line input, always `1`
    /// for a single-line one. Drives the AI prompt box's expand/collapse.
    pub fn line_count(&self) -> usize {
        if !self.multiline {
            return 1;
        }
        self.buffer.chars().filter(|c| *c == '\n').count() + 1
    }

    /// 3.9 — insert a newline at the cursor (Shift/Alt+Enter). No-op for a
    /// single-line input, so Enter-to-submit callers can route here safely.
    pub fn insert_newline(&mut self) {
        if self.multiline {
            self.insert_char('\n');
        }
    }

    pub fn as_str(&self) -> &str {
        &self.buffer
    }

    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    /// Render the buffer for display with `cursor_char` placed at the actual
    /// cursor position. Use this everywhere we draw a single-line text input
    /// — otherwise the visual cursor lags behind the real position and edits
    /// in the middle of the buffer look like characters are being scrambled.
    pub fn render_with_cursor(&self, cursor_char: char) -> String {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut out = String::with_capacity(self.buffer.len() + 1);
        for (i, c) in chars.iter().enumerate() {
            if i == self.cursor {
                out.push(cursor_char);
            }
            out.push(*c);
        }
        if self.cursor >= chars.len() {
            out.push(cursor_char);
        }
        out
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
    }

    /// 1.2.8+ — replace the full buffer and set the cursor
    /// in one step (char-index).  Used by the shell pane's
    /// Tab autocomplete to swap a token for its completion.
    /// Clamps cursor to the new buffer's char length.
    pub fn set_with_cursor(&mut self, text: String, cursor_chars: usize) {
        let len = text.chars().count();
        self.buffer = text;
        self.cursor = cursor_chars.min(len);
    }

    pub fn insert_char(&mut self, c: char) {
        let byte_idx = self.byte_offset(self.cursor);
        self.buffer.insert(byte_idx, c);
        self.cursor += 1;
    }

    /// A1 — insert a whole string at the cursor (bracketed paste). For a
    /// single-line input, newlines collapse to single spaces so a multi-line
    /// paste lands as one line instead of submitting at the first `\n`. For a
    /// multi-line input (3.9 — the AI prompt), newlines are preserved (CRLF
    /// normalised to LF) so a pasted excerpt keeps its shape.
    pub fn insert_str(&mut self, s: &str) {
        let text: String = if self.multiline {
            s.replace("\r\n", "\n").replace('\r', "\n")
        } else {
            s.split(['\r', '\n'])
                .filter(|seg| !seg.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        };
        if text.is_empty() {
            return;
        }
        let byte_idx = self.byte_offset(self.cursor);
        self.buffer.insert_str(byte_idx, &text);
        self.cursor += text.chars().count();
    }

    pub fn backspace(&mut self) {
        if self.cursor == 0 {
            return;
        }
        let prev_byte = self.byte_offset(self.cursor - 1);
        let cur_byte = self.byte_offset(self.cursor);
        self.buffer.replace_range(prev_byte..cur_byte, "");
        self.cursor -= 1;
    }

    pub fn delete(&mut self) {
        let len = self.buffer.chars().count();
        if self.cursor >= len {
            return;
        }
        let cur_byte = self.byte_offset(self.cursor);
        let next_byte = self.byte_offset(self.cursor + 1);
        self.buffer.replace_range(cur_byte..next_byte, "");
    }

    pub fn move_left(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    pub fn move_right(&mut self) {
        let len = self.buffer.chars().count();
        if self.cursor < len {
            self.cursor += 1;
        }
    }

    pub fn move_home(&mut self) {
        if self.multiline {
            // Start of the current line (after the preceding '\n').
            let chars: Vec<char> = self.buffer.chars().collect();
            let mut i = self.cursor;
            while i > 0 && chars[i - 1] != '\n' {
                i -= 1;
            }
            self.cursor = i;
        } else {
            self.cursor = 0;
        }
    }

    pub fn move_end(&mut self) {
        if self.multiline {
            // End of the current line (before the next '\n').
            let chars: Vec<char> = self.buffer.chars().collect();
            let mut i = self.cursor;
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            self.cursor = i;
        } else {
            self.cursor = self.buffer.chars().count();
        }
    }

    /// 3.9 — move the cursor up/down one line, preserving the column (clamped to
    /// the target line's length). No-op for a single-line input. Returns whether
    /// it moved, so the AI-prompt handler can fall back to history recall when
    /// the cursor is already on the first/last line.
    pub fn move_up(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let (line, col) = self.cursor_line_col();
        if line == 0 {
            return false;
        }
        self.cursor = self.char_index_of(line - 1, col);
        true
    }

    pub fn move_down(&mut self) -> bool {
        if !self.multiline {
            return false;
        }
        let (line, col) = self.cursor_line_col();
        if line + 1 >= self.line_count() {
            return false;
        }
        self.cursor = self.char_index_of(line + 1, col);
        true
    }

    /// Cursor's `(line, column)` in the multi-line buffer (both 0-based).
    fn cursor_line_col(&self) -> (usize, usize) {
        let mut line = 0;
        let mut col = 0;
        for c in self.buffer.chars().take(self.cursor) {
            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    /// Char index of `(line, column)`, with `column` clamped to that line's
    /// length (so vertical moves land inside a shorter line, not past its end).
    fn char_index_of(&self, target_line: usize, target_col: usize) -> usize {
        let mut idx = 0;
        let mut line = 0;
        let mut col = 0;
        for c in self.buffer.chars() {
            if line == target_line && col == target_col {
                return idx;
            }
            if c == '\n' {
                if line == target_line {
                    // Reached end of the target line before target_col — clamp here.
                    return idx;
                }
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
            idx += 1;
        }
        idx
    }

    /// 1.2.8+ — kill from the cursor to the start of the
    /// buffer (readline Ctrl+U).  The deleted text is NOT
    /// captured into a yank ring — single-line prompts don't
    /// have the multi-stash workflow that justifies one.
    pub fn kill_to_start(&mut self) {
        let cur_byte = self.byte_offset(self.cursor);
        self.buffer.replace_range(0..cur_byte, "");
        self.cursor = 0;
    }

    /// 1.2.8+ — kill from the cursor to the end of the buffer
    /// (readline Ctrl+K).
    pub fn kill_to_end(&mut self) {
        let cur_byte = self.byte_offset(self.cursor);
        self.buffer.truncate(cur_byte);
    }

    /// 1.2.8+ — move the cursor backward to the start of the
    /// previous word.  Words are defined as runs of
    /// non-whitespace, non-punctuation chars (same convention
    /// as readline's `\b` / Alt+B).
    pub fn move_word_left(&mut self) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let mut i = self.cursor;
        // Skip any whitespace immediately before the cursor.
        while i > 0 && is_word_separator(chars[i - 1]) {
            i -= 1;
        }
        // Walk back through word chars.
        while i > 0 && !is_word_separator(chars[i - 1]) {
            i -= 1;
        }
        self.cursor = i;
    }

    /// 1.2.8+ — move the cursor forward to the end of the
    /// next word.  Mirrors `move_word_left`.
    pub fn move_word_right(&mut self) {
        let chars: Vec<char> = self.buffer.chars().collect();
        let len = chars.len();
        let mut i = self.cursor;
        // Walk forward through word chars first.
        while i < len && !is_word_separator(chars[i]) {
            i += 1;
        }
        // Then skip trailing separator(s) to land at the next
        // word's start.
        while i < len && is_word_separator(chars[i]) {
            i += 1;
        }
        self.cursor = i;
    }

    /// 1.2.8+ — kill the word immediately before the cursor
    /// (readline Ctrl+W / Alt+Backspace).  Uses the same word
    /// definition as `move_word_left`.
    pub fn kill_word_left(&mut self) {
        let start_cursor = self.cursor;
        self.move_word_left();
        let kill_start_byte = self.byte_offset(self.cursor);
        let kill_end_byte = self.byte_offset(start_cursor);
        self.buffer.replace_range(kill_start_byte..kill_end_byte, "");
    }

    fn byte_offset(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.buffer.len())
    }
}

/// 1.2.8+ — predicate used by `move_word_*` / `kill_word_*`.
/// Whitespace and ASCII punctuation chars break word runs;
/// everything else (letters, digits, underscores, non-ASCII
/// letters) is part of a word.  Matches readline + most
/// editors so Ctrl+W jumps to the start of the identifier
/// the cursor sits in, regardless of language.
fn is_word_separator(c: char) -> bool {
    c.is_whitespace()
        || matches!(
            c,
            '|' | ';' | '(' | ')' | '[' | ']' | '{' | '}'
            | ',' | '.' | ':' | '/' | '\\' | '"' | '\''
            | '`' | '<' | '>' | '!' | '?' | '*' | '&'
            | '=' | '+' | '~'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_insert_and_backspace() {
        let mut t = TextInput::new();
        t.insert_char('h');
        t.insert_char('i');
        assert_eq!(t.as_str(), "hi");
        t.backspace();
        assert_eq!(t.as_str(), "h");
        assert_eq!(t.cursor(), 1);
    }

    #[test]
    fn insert_str_flattens_newlines_and_advances_cursor() {
        // A1 — a multi-line paste lands as one line (newlines → single spaces),
        // so it never submits at the first `\n`; cursor ends past the inserted text.
        let mut t = TextInput::new();
        t.insert_str("line one\nline two\r\nthree");
        assert_eq!(t.as_str(), "line one line two three");
        assert_eq!(t.cursor(), "line one line two three".chars().count());
        // Inserts at the cursor, not just append.
        let mut u = TextInput::new();
        for c in "ac".chars() {
            u.insert_char(c);
        }
        u.move_left(); // between a and c
        u.insert_str("B");
        assert_eq!(u.as_str(), "aBc");
        // Unicode is char-counted, not byte-counted.
        let mut v = TextInput::new();
        v.insert_str("да\nнет");
        assert_eq!(v.as_str(), "да нет");
        assert_eq!(v.cursor(), 6);
    }

    #[test]
    fn unicode_insert_and_navigate() {
        let mut t = TextInput::new();
        for c in "утро".chars() {
            t.insert_char(c);
        }
        assert_eq!(t.as_str(), "утро");
        assert_eq!(t.cursor(), 4);
        t.move_left();
        t.move_left();
        t.backspace();
        assert_eq!(t.as_str(), "уро");
    }

    #[test]
    fn middle_insert_round_trip() {
        let mut t = TextInput::new();
        for c in "Hello".chars() {
            t.insert_char(c);
        }
        // Move cursor between 'e' and 'l'.
        t.move_home();
        t.move_right();
        t.move_right();
        t.insert_char('X');
        assert_eq!(t.as_str(), "HeXllo");
        assert_eq!(t.cursor(), 3);
    }

    #[test]
    fn render_with_cursor_in_middle() {
        let mut t = TextInput::new();
        for c in "Hi".chars() {
            t.insert_char(c);
        }
        // Cursor at end.
        assert_eq!(t.render_with_cursor('│'), "Hi│");
        // Cursor at start.
        t.move_home();
        assert_eq!(t.render_with_cursor('│'), "│Hi");
        // Cursor in middle.
        t.move_right();
        assert_eq!(t.render_with_cursor('│'), "H│i");
    }

    #[test]
    fn home_end_move_cursor() {
        let mut t = TextInput::new();
        for c in "hello world".chars() {
            t.insert_char(c);
        }
        assert_eq!(t.cursor(), 11);
        t.move_home();
        assert_eq!(t.cursor(), 0);
        assert_eq!(t.render_with_cursor('│'), "│hello world");
        t.move_end();
        assert_eq!(t.cursor(), 11);
        assert_eq!(t.render_with_cursor('│'), "hello world│");
    }

    #[test]
    fn delete_at_cursor() {
        let mut t = TextInput::new();
        for c in "abcde".chars() {
            t.insert_char(c);
        }
        t.move_home();
        t.move_right(); // cursor between a and b
        t.delete(); // removes 'b'
        assert_eq!(t.as_str(), "acde");
        assert_eq!(t.cursor(), 1);
    }

    #[test]
    fn kill_to_start_and_end() {
        let mut t = TextInput::new();
        for c in "hello world".chars() {
            t.insert_char(c);
        }
        // cursor at end → kill_to_start clears everything.
        t.kill_to_start();
        assert_eq!(t.as_str(), "");
        assert_eq!(t.cursor(), 0);

        for c in "hello world".chars() {
            t.insert_char(c);
        }
        // cursor between 'hello ' and 'world' → kill_to_end
        // leaves "hello ".
        t.move_home();
        for _ in 0..6 {
            t.move_right();
        }
        t.kill_to_end();
        assert_eq!(t.as_str(), "hello ");
        assert_eq!(t.cursor(), 6);
    }

    #[test]
    fn word_navigation_and_kill() {
        let mut t = TextInput::new();
        for c in "git status --short".chars() {
            t.insert_char(c);
        }
        // Hyphen is intentionally NOT a separator so `--short`
        // counts as one logical word (a CLI flag).  From the
        // end, three move_word_left jumps land on:
        //   `--short` start, `status` start, `git` start.
        t.move_word_left();
        assert_eq!(t.cursor(), "git status ".len());
        t.move_word_left();
        assert_eq!(t.cursor(), "git ".len());
        t.move_word_left();
        assert_eq!(t.cursor(), 0);

        // Forward: from start, jump past `git`, land on `status`.
        t.move_word_right();
        assert_eq!(t.cursor(), "git ".len());

        // Kill word left at end of buffer.
        let mut t2 = TextInput::new();
        for c in "git status".chars() {
            t2.insert_char(c);
        }
        t2.kill_word_left();
        assert_eq!(t2.as_str(), "git ");
    }

    #[test]
    fn multiline_preserves_newlines_and_counts_lines() {
        let mut t = TextInput::multiline();
        t.insert_str("line one\r\nline two\rthree");
        assert_eq!(t.as_str(), "line one\nline two\nthree");
        assert_eq!(t.line_count(), 3);
        // A single-line input still flattens (unchanged behaviour).
        let mut s = TextInput::new();
        s.insert_str("a\nb");
        assert_eq!(s.as_str(), "a b");
        assert_eq!(s.line_count(), 1);
    }

    #[test]
    fn multiline_insert_newline_and_no_op_when_single_line() {
        let mut t = TextInput::multiline();
        for c in "ab".chars() {
            t.insert_char(c);
        }
        t.insert_newline();
        for c in "cd".chars() {
            t.insert_char(c);
        }
        assert_eq!(t.as_str(), "ab\ncd");
        assert_eq!(t.line_count(), 2);
        // insert_newline is a no-op on a single-line input.
        let mut s = TextInput::new();
        s.insert_char('x');
        s.insert_newline();
        assert_eq!(s.as_str(), "x");
    }

    #[test]
    fn multiline_vertical_move_preserves_column_and_clamps() {
        let mut t = TextInput::multiline();
        t.insert_str("hello\nhi\nworld");
        // Cursor at end (on "world", col 5). Up → "hi" line, clamped to col 2.
        assert!(t.move_up());
        assert_eq!(t.cursor_line_col(), (1, 2));
        // Up again → "hello" line, col 2 (fits).
        assert!(t.move_up());
        assert_eq!(t.cursor_line_col(), (0, 2));
        // Already on the first line → no move, returns false (→ history recall).
        assert!(!t.move_up());
        // Down twice back to the last line.
        assert!(t.move_down());
        assert!(t.move_down());
        assert_eq!(t.cursor_line_col(), (2, 2));
        assert!(!t.move_down());
    }

    #[test]
    fn multiline_home_end_are_line_local() {
        let mut t = TextInput::multiline();
        t.insert_str("hello\nworld");
        // Cursor at end of "world"; Home → start of that line.
        t.move_home();
        assert_eq!(t.cursor_line_col(), (1, 0));
        t.move_end();
        assert_eq!(t.cursor_line_col(), (1, 5));
    }

    #[test]
    fn render_with_cursor_unicode() {
        let mut t = TextInput::new();
        for c in "утро".chars() {
            t.insert_char(c);
        }
        t.move_left();
        t.move_left();
        assert_eq!(t.render_with_cursor('│'), "ут│ро");
    }
}
