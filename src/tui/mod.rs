use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io::{self, Stdout};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::chat::{Chat, FUNNY_LIMIT_MESSAGE};
use crate::provider::Provider;

pub mod input;
pub mod view;

use input::{Input, InputAction};

/// Shared slot the spawned provider task writes its result into.
type PendingResult = Arc<Mutex<Option<Result<String>>>>;

#[derive(Default)]
pub struct AppState {
    loading: bool,
}

impl AppState {
    pub fn is_loading(&self) -> bool {
        self.loading
    }
}

pub struct App {
    pub chat: Chat,
    pub provider: Arc<dyn Provider>,
    pub provider_name: &'static str,
    pub input: Input,
    pub state: AppState,
    pub pending: PendingResult,
    pub spinner: usize,
    pub should_quit: bool,
    pub locked: bool,
}

impl App {
    pub fn new(provider: Arc<dyn Provider>, initial_prompt: String) -> Self {
        let provider_name = provider.name();
        let mut app = Self {
            chat: Chat::new(),
            provider,
            provider_name,
            input: Input::new(),
            state: AppState::default(),
            pending: Arc::new(Mutex::new(None)),
            spinner: 0,
            should_quit: false,
            locked: false,
        };
        // Send the first prompt straight away.
        if app.chat.push_user(initial_prompt) {
            app.spawn_completion();
        }
        app
    }

    /// Spawn a tokio task that completes against the current history and
    /// writes the result into the shared slot.
    fn spawn_completion(&mut self) {
        self.state.loading = true;
        *self.pending.lock().unwrap() = None;
        let provider = Arc::clone(&self.provider);
        let messages = self.chat.messages.clone();
        let slot = Arc::clone(&self.pending);
        tokio::spawn(async move {
            let result = provider.complete(&messages).await;
            *slot.lock().unwrap() = Some(result);
        });
    }

    fn submit_input(&mut self) {
        if self.locked || self.state.is_loading() {
            return;
        }
        if self.chat.is_capped() {
            // Render the funny limit message once and lock further input.
            if !self.locked {
                self.chat.push_assistant(FUNNY_LIMIT_MESSAGE);
                self.locked = true;
            }
            self.input.clear();
            return;
        }
        let text = self.input.text();
        if text.trim().is_empty() {
            return;
        }
        if !self.chat.push_user(text) {
            // Edge case: the cap was reached by an error message path.
            self.chat.push_assistant(FUNNY_LIMIT_MESSAGE);
            self.locked = true;
            self.input.clear();
            return;
        }
        self.input.clear();
        self.spawn_completion();
    }

    fn take_pending(&mut self) {
        let taken = self.pending.lock().unwrap().take();
        if let Some(result) = taken {
            self.state.loading = false;
            match result {
                Ok(text) => self.chat.push_assistant(text),
                Err(e) => self.chat.push_assistant(format!("(error: {e})")),
            }
            // If we have hit the cap after the response, surface the funny
            // note so the user knows why next submit will be refused.
            if self.chat.is_capped() && !self.locked {
                self.chat.push_assistant(FUNNY_LIMIT_MESSAGE);
                self.locked = true;
            }
        }
    }

    fn handle_key(&mut self, key: &event::KeyEvent) {
        // Esc always quits.
        if key.code == KeyCode::Esc
            && key.kind == KeyEventKind::Press
        {
            self.should_quit = true;
            return;
        }
        if key.kind != KeyEventKind::Press {
            return;
        }
        if self.state.is_loading() || self.locked {
            // While loading or locked, ignore input edits.
            return;
        }
        let action = self.input.handle(key);
        if action == InputAction::Submit {
            self.submit_input();
        }
    }
}

/// Run the TUI for the given provider with the initial prompt already enqueued.
pub fn run(app: App) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = main_loop(&mut terminal, app);

    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)?;
    result
}

fn main_loop(terminal: &mut Terminal<CrosstermBackend<Stdout>>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|frame| view::render(frame, &app))?;

        // Poll input for ~50ms so we can still animate the spinner.
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(k) = event::read()? {
                app.handle_key(&k);
            }
        }

        if app.state.is_loading() {
            app.spinner = app.spinner.wrapping_add(1);
            app.take_pending();
        }

        if app.should_quit {
            break;
        }
    }
    Ok(())
}