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

    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }

    #[allow(dead_code)]
    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn clear(&mut self) {
        self.buffer.clear();
        self.cursor = 0;
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
}
