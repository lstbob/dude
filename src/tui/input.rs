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
            KeyCode::Char('u') if ctrl => {
                self.clear();
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
            // find previous char boundary
            let mut idx = self.col;
            while !line.is_char_boundary(idx) {
                idx -= 1;
            }
            line.replace_range(idx..self.col, "");
            self.col = idx;
        }
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