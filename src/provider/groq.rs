use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::{Message, Role};
use crate::provider::{http_client, truncate_body, Fut, Provider};

const DEFAULT_MODEL: &str = "llama-3.3-70b-versatile";
const ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";

pub struct Groq {
    api_key: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct Request<'a> {
    model: &'a str,
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
    choices: Vec<Choice>,
}

#[derive(Deserialize)]
struct Choice {
    #[serde(default)]
    message: Option<RespMessage>,
}

#[derive(Deserialize)]
struct RespMessage {
    #[serde(default)]
    content: String,
}

impl Groq {
    pub fn new(api_key: String, model: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("groq api key is empty");
        }
        Ok(Self {
            api_key,
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            client: http_client()?,
        })
    }
}

impl Provider for Groq {
    fn name(&self) -> &'static str {
        "groq"
    }

    fn complete<'a>(&'a self, messages: &'a [Message]) -> Fut<'a> {
        Box::pin(async move {
            let req = Request {
                model: &self.model,
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
                .bearer_auth(&self.api_key)
                .json(&req)
                .send()
                .await
                .map_err(|e| anyhow!("groq request: {e}"))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(anyhow!("groq api {}: {}", status.as_u16(), truncate_body(&body)));
            }
            let parsed: Response = serde_json::from_str(&body)
                .map_err(|e| anyhow!("parsing groq response: {e}\nbody: {}", truncate_body(&body)))?;
            parsed
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message)
                .map(|m| m.content)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("groq returned no content"))
        })
    }
}