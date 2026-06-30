use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::{Message, Role};
use crate::provider::{Fut, Provider};

const DEFAULT_MODEL: &str = "claude-3-5-haiku-20241022";
const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const MAX_TOKENS: u32 = 1024;

pub struct Anthropic {
    api_key: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
    max_tokens: u32,
    messages: Vec<MessageRef<'a>>,
}

#[derive(Serialize)]
struct MessageRef<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(default)]
    text: String,
}

impl Anthropic {
    pub fn new(api_key: String, model: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("anthropic api key is empty");
        }
        Ok(Self {
            api_key,
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            client: Client::new(),
        })
    }
}

impl Provider for Anthropic {
    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn complete<'a>(&'a self, messages: &'a [Message]) -> Fut<'a> {
        Box::pin(async move {
            let req = Request {
                model: &self.model,
                max_tokens: MAX_TOKENS,
                messages: messages
                    .iter()
                    .map(|m| MessageRef {
                        role: match m.role {
                            Role::User => "user",
                            Role::Assistant => "assistant",
                        },
                        content: &m.content,
                    })
                    .collect(),
            };
            let resp = self
                .client
                .post(ENDPOINT)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", ANTHROPIC_VERSION)
                .header("content-type", "application/json")
                .json(&req)
                .send()
                .await
                .map_err(|e| anyhow!("anthropic request: {e}"))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(anyhow!("anthropic api {}: {}", status.as_u16(), body));
            }
            let parsed: Response = serde_json::from_str(&body)
                .map_err(|e| anyhow!("parsing anthropic response: {e}\nbody: {body}"))?;
            let text = parsed
                .content
                .into_iter()
                .map(|b| b.text)
                .collect::<Vec<_>>()
                .join("");
            if text.is_empty() {
                Err(anyhow!("anthropic returned no content"))
            } else {
                Ok(text)
            }
        })
    }
}