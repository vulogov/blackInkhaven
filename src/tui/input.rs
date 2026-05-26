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

    fn byte_offset(&self, char_idx: usize) -> usize {
        self.buffer
            .char_indices()
            .nth(char_idx)
            .map(|(b, _)| b)
            .unwrap_or(self.buffer.len())
    }
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
