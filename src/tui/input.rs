/// Single-line text input buffer for the top search bar and bottom AI prompt.
/// Stores the buffer plus a byte-cursor; rendering uses `chars()` so multibyte
/// characters (Cyrillic etc.) display correctly.
#[derive(Debug, Default, Clone)]
pub struct TextInput {
    buffer: String,
    /// Cursor position as a *character* index, not a byte index.
    cursor: usize,
}

impl TextInput {
    pub fn new() -> Self {
        Self::default()
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
        self.cursor = 0;
    }

    pub fn move_end(&mut self) {
        self.cursor = self.buffer.chars().count();
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
