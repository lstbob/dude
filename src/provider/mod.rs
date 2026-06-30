use anyhow::Result;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::chat::Message;

pub mod anthropic;
pub mod gemini;
pub mod groq;
pub mod openai;

/// A single completion against a chat history. Implementations translate the
/// shared `Vec<Message>` into the provider's native request shape, make an
/// async HTTP POST, and return the assistant's text reply. The boxed future
/// borrows `&self` and the message slice, which is enough for our use.
pub type Fut<'a> =
    Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

pub trait Provider: Send + Sync {
    fn name(&self) -> &'static str;
    fn complete<'a>(&'a self, messages: &'a [Message]) -> Fut<'a>;
}

/// Construct the right provider for `cfg`, applying the per-provider default
/// model when `cfg.model` is empty, or the user's override otherwise.
pub fn from_config(cfg: &crate::config::Config) -> Result<Arc<dyn Provider>> {
    let model = if cfg.model.is_empty() {
        None
    } else {
        Some(cfg.model.as_str())
    };
    match cfg.llm_provider.as_str() {
        "gemini" => Ok(Arc::new(gemini::Gemini::new(
            cfg.gemini_api_key.clone(),
            model,
        )?)),
        "openai" => Ok(Arc::new(openai::OpenAi::new(
            cfg.openai_api_key.clone(),
            model,
        )?)),
        "anthropic" => Ok(Arc::new(anthropic::Anthropic::new(
            cfg.anthropic_api_key.clone(),
            model,
        )?)),
        "groq" => Ok(Arc::new(groq::Groq::new(
            cfg.groq_api_key.clone(),
            model,
        )?)),
        other => anyhow::bail!("unknown llm_provider in config: {other}"),
    }
}