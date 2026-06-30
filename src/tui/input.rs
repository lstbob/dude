/// A tiny multi-line text editor for the input box.
///
/// Keys:
///   - printable char: insert
///   - Enter:          submit (caller turns this into a send)
///   - Alt+Enter / Shift+Enter: insert newline
///   - Backspace:      delete char before cursor (merge lines at col 0)
///   - Left/Right/Up/Down: move the cursor
///   - Ctrl+U:         clear all
#[derive(Default)]
pub struct Input {
    pub lines: Vec<String>,
    /// Byte offset within `lines[cursor_line]`.
    pub line: usize,
    pub col: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputAction {
    /// User hit plain Enter with non-empty text.
    Submit,
    /// App should keep editing.
    Editing,
}

impl Input {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            line: 0,
            col: 0,
        }
    }

    /// Returns the joined text using `\n` between lines.
    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.iter().all(|l| l.is_empty())
    }

    /// Reset editor contents to a single empty line.
    pub fn clear(&mut self) {
        self.lines = vec![String::new()];
        self.line = 0;
        self.col = 0;
    }

    /// Handle a crossterm key event. Returns the resulting action.
    pub fn handle(
        &mut self,
        key: &crossterm::event::KeyEvent,
    ) -> InputAction {
        use crossterm::event::{KeyCode, KeyModifiers};

        let alt = key.modifiers.contains(KeyModifiers::ALT);
        let shift = key.modifiers.contains(KeyModifiers::SHIFT);
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter => {
                if alt || shift {
                    self.insert_char('\n');
                    InputAction::Editing
                } else {
                    if self.is_empty() {
                        InputAction::Editing
                    } else {
                        InputAction::Submit
                    }
                }
            }
            // Ctrl+U: clear the whole input. Ctrl+W: delete the previous word.
            // Other Ctrl+letter combos are ignored so they don't insert junk.
            KeyCode::Char('u') if ctrl => {
                self.clear();
                InputAction::Editing
            }
            KeyCode::Char('w') if ctrl => {
                self.delete_word_back();
                InputAction::Editing
            }
            KeyCode::Char(c) if ctrl => {
                // Swallow other control combos (Ctrl+C / Ctrl+D are handled
                // at the app level, not here).
                let _ = c;
                InputAction::Editing
            }
            // Many terminals send Backspace as the DEL char (U+007F) or the
            // BS char (U+0008) instead of KeyCode::Backspace; treat those as
            // backspace too.
            KeyCode::Char('\u{7f}') | KeyCode::Char('\u{8}') => {
                self.backspace();
                InputAction::Editing
            }
            KeyCode::Char(c) => {
                self.insert_char(c);
                InputAction::Editing
            }
            KeyCode::Backspace => {
                self.backspace();
                InputAction::Editing
            }
            KeyCode::Delete => {
                self.delete_forward();
                InputAction::Editing
            }
            KeyCode::Left => {
                self.move_left();
                InputAction::Editing
            }
            KeyCode::Right => {
                self.move_right();
                InputAction::Editing
            }
            KeyCode::Up => {
                self.move_up();
                InputAction::Editing
            }
            KeyCode::Down => {
                self.move_down();
                InputAction::Editing
            }
            _ => InputAction::Editing,
        }
    }

    fn insert_char(&mut self, c: char) {
        if c == '\n' {
            // split current line at cursor
            let cur = self.lines[self.line].clone();
            let (left, right) = cur.split_at(self.col);
            let left = left.to_string();
            let right = right.to_string();
            self.lines[self.line] = left;
            self.lines.insert(self.line + 1, right);
            self.line += 1;
            self.col = 0;
        } else {
            self.lines[self.line].insert(self.col, c);
            self.col += c.len_utf8();
        }
    }

    fn backspace(&mut self) {
        if self.col == 0 {
            if self.line == 0 {
                return;
            }
            // merge with previous line
            let prev_len = self.lines[self.line - 1].len();
            let cur = self.lines.remove(self.line);
            self.line -= 1;
            self.lines[self.line].push_str(&cur);
            self.col = prev_len;
        } else {
            let line = &mut self.lines[self.line];
            // Find the byte index where the character immediately before the
            // cursor starts. self.col is always a char boundary, so we step
            // back first, then walk over any continuation bytes.
            let mut idx = self.col;
            loop {
                idx -= 1;
                if line.is_char_boundary(idx) {
                    break;
                }
            }
            line.replace_range(idx..self.col, "");
            self.col = idx;
        }
    }

    /// Delete the word to the left of the cursor (readline-style Ctrl+W):
    /// first skip any trailing whitespace, then delete one run of non-space.
    /// If at the start of a line, the newline is consumed first.
    fn delete_word_back(&mut self) {
        if self.col == 0 {
            if self.line == 0 {
                return;
            }
            // merge with previous line, then keep deleting the word
            let cur = self.lines.remove(self.line);
            self.line -= 1;
            let prev_len = self.lines[self.line].len();
            self.lines[self.line].push_str(&cur);
            self.col = prev_len;
        }

        let old_col = self.col;
        if old_col == 0 {
            return;
        }
        let bytes = self.lines[self.line].as_bytes().to_vec();
        let mut start = old_col;
        // skip trailing whitespace before the cursor
        while let Some(prev) = char_start_before(&bytes, start) {
            if bytes[prev].is_ascii_whitespace() {
                start = prev;
            } else {
                break;
            }
        }
        // skip one word of non-whitespace before the cursor
        while let Some(prev) = char_start_before(&bytes, start) {
            if !bytes[prev].is_ascii_whitespace() {
                start = prev;
            } else {
                break;
            }
        }
        if start != old_col {
            self.lines[self.line].replace_range(start..old_col, "");
            self.col = start;
        }
    }

    /// Delete the character to the right of the cursor (Delete key).
    fn delete_forward(&mut self) {
        let line_len = self.lines[self.line].len();
        if self.col >= line_len {
            // join with the next line
            if self.line + 1 < self.lines.len() {
                let next = self.lines.remove(self.line + 1);
                self.lines[self.line].push_str(&next);
            }
            return;
        }
        let s = self.lines[self.line].as_str();
        let mut end = self.col + 1;
        while end < line_len && !s.is_char_boundary(end) {
            end += 1;
        }
        self.lines[self.line].replace_range(self.col..end, "");
    }

    fn move_left(&mut self) {
        if self.col > 0 {
            let mut idx = self.col;
            while !self.lines[self.line].is_char_boundary(idx - 1) {
                idx -= 1;
            }
            self.col = idx - 1;
        } else if self.line > 0 {
            self.line -= 1;
            self.col = self.lines[self.line].len();
        }
    }

    fn move_right(&mut self) {
        let line_len = self.lines[self.line].len();
        if self.col < line_len {
            let s = self.lines[self.line].as_str();
            let mut idx = self.col;
            while idx < line_len && !s.is_char_boundary(idx + 1) {
                idx += 1;
            }
            self.col = idx + 1;
        } else if self.line + 1 < self.lines.len() {
            self.line += 1;
            self.col = 0;
        }
    }

    fn move_up(&mut self) {
        if self.line > 0 {
            self.line -= 1;
            self.col = self.col.min(self.lines[self.line].len());
        }
    }

    fn move_down(&mut self) {
        if self.line + 1 < self.lines.len() {
            self.line += 1;
            self.col = self.col.min(self.lines[self.line].len());
        }
    }
}

/// Given the bytes of the current line and a byte index `idx`, return the byte
/// index where the character immediately preceding `idx` starts, or `None`
/// if `idx == 0`. Walks back over any continuation bytes to the previous lead
/// byte.
fn char_start_before(bytes: &[u8], idx: usize) -> Option<usize> {
    if idx == 0 {
        return None;
    }
    let mut k = idx;
    loop {
        k -= 1;
        // A lead byte in UTF-8 is any byte that is NOT a continuation byte
        // (0x80..=0xBF). 0 is always a boundary, guaranteeing termination.
        if !(0x80..=0xBF).contains(&bytes[k]) {
            return Some(k);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn from(s: &str) -> Input {
        let mut i = Input::new();
        i.lines = vec![s.to_string()];
        i.col = s.len();
        i.line = 0;
        i
    }

    #[test]
    fn delete_word_back_eats_word_and_trailing_space() {
        let mut i = from("hello world");
        i.delete_word_back();
        assert_eq!(i.text(), "hello ");
        assert_eq!(i.col, 6);
    }

    #[test]
    fn delete_word_back_skips_trailing_whitespace_first() {
        let mut i = from("foo   ");
        i.delete_word_back();
        assert_eq!(i.text(), "");
        assert_eq!(i.col, 0);
    }

    #[test]
    fn delete_word_back_at_line_start_merges_lines() {
        let mut i = Input::new();
        i.lines = vec!["alpha".to_string(), "beta".to_string()];
        i.line = 1;
        i.col = 0;
        i.delete_word_back();
        // newline consumed, then the word before the cursor ("alpha") is
        // deleted, leaving "beta".
        assert_eq!(i.text(), "beta");
        assert_eq!(i.line, 0);
    }

    #[test]
    fn delete_word_back_handles_multibyte() {
        let mut i = from("café au lait");
        i.col = "café au lait".len();
        i.delete_word_back();
        assert_eq!(i.text(), "café au ");
    }

    #[test]
    fn delete_forward_removes_char_right_of_cursor() {
        let mut i = from("hello");
        i.col = 2; // after 'e'
        i.delete_forward();
        assert_eq!(i.text(), "helo");
        assert_eq!(i.col, 2);
    }

    #[test]
    fn backspace_removes_char_before_cursor() {
        let mut i = from("hello");
        i.backspace();
        assert_eq!(i.text(), "hell");
        assert_eq!(i.col, 4);
    }

    #[test]
    fn backspace_mid_string() {
        let mut i = from("hello");
        i.col = 2; // between 'e' and 'l'
        i.backspace();
        assert_eq!(i.text(), "hllo");
        assert_eq!(i.col, 1);
    }

    #[test]
    fn backspace_handles_multibyte() {
        let mut i = from("café");
        i.backspace();
        assert_eq!(i.text(), "caf");
        assert_eq!(i.col, 3);
    }

    #[test]
    fn backspace_at_col_zero_merges_lines() {
        let mut i = Input::new();
        i.lines = vec!["foo".to_string(), "bar".to_string()];
        i.line = 1;
        i.col = 0;
        i.backspace();
        assert_eq!(i.text(), "foobar");
        assert_eq!(i.line, 0);
        assert_eq!(i.col, 3);
    }
}