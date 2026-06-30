use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget, Wrap};
use ratatui::Frame;

use crate::chat::{Chat, Role};
use crate::tui::App;

const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

/// Word-wrap a string into lines of at most `width` display columns
/// (a naive word-boundary wrap, good enough for our short chats).
pub fn wrap_text(content: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut out = Vec::new();
    for raw_line in content.split('\n') {
        if raw_line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in raw_line.split_whitespace() {
            if current.is_empty() {
                current = word.to_string();
            } else if current.len() + 1 + word.len() <= width {
                current.push(' ');
                current.push_str(word);
            } else {
                out.push(current);
                current = word.to_string();
            }
        }
        // handle words that overflow by themselves (very long token)
        while current.len() > width {
            let (head, tail) = current.split_at(width);
            out.push(head.to_string());
            current = tail.to_string();
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
        for w in wrap_text(&msg.content, width) {
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