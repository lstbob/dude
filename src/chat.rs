use serde::{Deserialize, Serialize};

/// Maximum number of *user* turns allowed in a single `dude` session.
/// The initial prompt counts as turn 1; 5 follow-ups bring us to 6 total.
/// After turn 6 the conversation is capped and the funny error is shown.
pub const MAX_USER_TURNS: usize = 6;

pub const FUNNY_LIMIT_MESSAGE: &str =
    "This is supposed to be a short conversation... but you just hit the 6-question \
     limit. Start a new `dude` session if you need to keep going.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    User,
    Assistant,
}

impl Role {
    pub fn label(self) -> &'static str {
        match self {
            Role::User => "Walter",
            Role::Assistant => "The Dude",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
}

impl Message {
    pub fn user(content: impl Into<String>) -> Self {
        Self { role: Role::User, content: content.into() }
    }
    pub fn assistant(content: impl Into<String>) -> Self {
        Self { role: Role::Assistant, content: content.into() }
    }
}

/// Conversation state with a hard cap on user turns.
#[derive(Default)]
pub struct Chat {
    pub messages: Vec<Message>,
    pub user_turns: usize,
}

impl Chat {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a user turn. Returns `false` (and does not append) if the user
    /// has already used all `MAX_USER_TURNS` turns; the caller should render
    /// `FUNNY_LIMIT_MESSAGE` instead.
    pub fn push_user(&mut self, content: impl Into<String>) -> bool {
        if self.user_turns >= MAX_USER_TURNS {
            return false;
        }
        self.messages.push(Message::user(content));
        self.user_turns += 1;
        true
    }

    pub fn push_assistant(&mut self, content: impl Into<String>) {
        self.messages.push(Message::assistant(content));
    }

    /// Remaining user turns (counting the loop display). Zero means capped.
    pub fn remaining(&self) -> usize {
        MAX_USER_TURNS.saturating_sub(self.user_turns)
    }

    pub fn is_capped(&self) -> bool {
        self.user_turns >= MAX_USER_TURNS
    }
}