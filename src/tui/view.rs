use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;

use crate::chat::{Chat, Role};
use crate::tui::App;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Word-wrap a string into lines of at most `width` columns, counting Unicode
/// scalar values rather than bytes (a naive word-boundary wrap, good enough for
/// our short chats). Lengths are measured in `chars()` so a hard split never
/// lands mid-codepoint — `split_at` on a byte offset would otherwise panic on
/// multibyte content (CJK, emoji, accents).
pub fn wrap_text(content: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for raw_line in content.split('\n') {
        if raw_line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        let mut current_len = 0; // length of `current` in chars
        for word in raw_line.split_whitespace() {
            let word_len = word.chars().count();
            if current.is_empty() {
                current = word.to_string();
                current_len = word_len;
            } else if current_len + 1 + word_len <= width {
                current.push(' ');
                current.push_str(word);
                current_len += 1 + word_len;
            } else {
                out.push(current);
                current = word.to_string();
                current_len = word_len;
            }
        }
        // handle words that overflow by themselves (very long token); split on
        // a char boundary at `width` chars, not `width` bytes.
        while current_len > width {
            let split_byte = current
                .char_indices()
                .nth(width)
                .map(|(i, _)| i)
                .unwrap_or(current.len());
            let tail = current.split_off(split_byte);
            out.push(current);
            current = tail;
            current_len -= width;
        }
        out.push(current);
    }
    if out.is_empty() {
        out.push(String::new());
    }
    out
}

pub fn render(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let chunks = Layout::vertical([Constraint::Min(3), Constraint::Length(1), Constraint::Length(8)])
        .spacing(1)
        .split(area);

    render_history(frame, app, chunks[0]);
    render_status_bar(frame, app, chunks[1]);
    render_input(frame, app, chunks[2]);
}

fn render_history(frame: &mut Frame, app: &App, area: Rect) {
    let inner = Block::default()
        .borders(Borders::TOP)
        .title(Span::styled(
            " dude — short AI chats in the terminal ",
            Style::default().fg(Color::Cyan),
        ))
        .inner(area);

    let width = inner.width as usize;

    // Build all content lines for every message in the chat.
    let mut all_lines: Vec<Line> = Vec::new();
    for msg in &app.chat.messages {
        // header line
        let header_style = match msg.role {
            Role::User => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            Role::Assistant => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
        };
        all_lines.push(Line::styled(format!("{}:", msg.role.label()), header_style));
        // Display-only flavor: Walter always opens with "Dude, ...", and The
        // Dude replies with "Walter, ...". The raw text is what is sent to /
        // received from the model.
        let display_content = match msg.role {
            Role::User => format!("Dude, {}", msg.content),
            Role::Assistant => format!("Walter, {}", msg.content),
        };
        for w in wrap_text(&display_content, width) {
            let style = match msg.role {
                Role::User => Style::default().fg(Color::DarkGray),
                Role::Assistant => Style::default(),
            };
            all_lines.push(Line::styled(w, style));
        }
        all_lines.push(Line::raw(""));
    }

    let visible = all_lines.len().saturating_sub(inner.height as usize);
    let offset = visible.min(u16::MAX as usize) as u16;
    Paragraph::new(all_lines)
        .scroll((offset, 0))
        .render(inner, frame.buffer_mut());
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mut spans = Vec::new();
    spans.push(Span::styled(
        format!(" {} ", app.provider_name),
        Style::default().fg(Color::Magenta),
    ));
    spans.push(Span::raw("•"));
    spans.push(Span::raw(format!(
        " {} turn(s) left ",
        app.chat.remaining()
    )));
    spans.push(Span::raw("•"));
    if app.state.is_loading() {
        let spinner = SPINNER[app.spinner % SPINNER.len()];
        spans.push(Span::styled(
            format!(" {} Loading… ", spinner),
            Style::default().fg(Color::Cyan),
        ));
        spans.push(Span::raw("•"));
    }
    if app.locked {
        spans.push(Span::styled(
            " session closed ",
            Style::default().fg(Color::Red),
        ));
        spans.push(Span::raw("•"));
    }
    spans.push(Span::styled(
        " Enter: send · Alt/Shift+Enter: newline · Esc: quit ",
        Style::default().fg(Color::DarkGray),
    ));
    Paragraph::new(Line::from(spans)).render(area, frame.buffer_mut());
}

fn render_input(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if app.state.is_loading() {
            Style::default().fg(Color::DarkGray)
        } else if app.locked {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Cyan)
        })
        .title(" prompt (multi-line) ");

    let lines: Vec<Line> = app
        .input
        .lines
        .iter()
        .map(|l| Line::raw(l.clone()))
        .collect();
    Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .render(area, frame.buffer_mut());
}

#[allow(dead_code)]
fn _unused_chat(_c: &Chat) {}

#[cfg(test)]
mod tests {
    use super::wrap_text;

    #[test]
    fn long_multibyte_token_does_not_panic() {
        // A long unbroken CJK run (no spaces) is a single "word" longer than
        // the width; previously this split mid-codepoint and panicked.
        let s = "技".repeat(50);
        let lines = wrap_text(&s, 10);
        assert!(lines.iter().all(|l| l.chars().count() <= 10));
        assert_eq!(lines.concat(), s);
    }

    #[test]
    fn emoji_token_does_not_panic() {
        let s = "🦀".repeat(20);
        let lines = wrap_text(&s, 5);
        assert!(lines.iter().all(|l| l.chars().count() <= 5));
        assert_eq!(lines.concat(), s);
    }

    #[test]
    fn ascii_word_wrap_unchanged() {
        let lines = wrap_text("the quick brown fox", 9);
        assert_eq!(lines, vec!["the quick", "brown fox"]);
    }
}