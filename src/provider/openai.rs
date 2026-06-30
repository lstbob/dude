use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::{Message, Role};
use crate::provider::{Fut, Provider};

const DEFAULT_MODEL: &str = "gpt-4o-mini";
const ENDPOINT: &str = "https://api.openai.com/v1/chat/completions";

pub struct OpenAi {
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

impl OpenAi {
    pub fn new(api_key: String, model: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("openai api key is empty");
        }
        Ok(Self {
            api_key,
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            client: Client::new(),
        })
    }
}

impl Provider for OpenAi {
    fn name(&self) -> &'static str {
        "openai"
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
                .map_err(|e| anyhow!("openai request: {e}"))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(anyhow!("openai api {}: {}", status.as_u16(), body));
            }
            let parsed: Response = serde_json::from_str(&body)
                .map_err(|e| anyhow!("parsing openai response: {e}\nbody: {body}"))?;
            parsed
                .choices
                .into_iter()
                .next()
                .and_then(|c| c.message)
                .map(|m| m.content)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("openai returned no content"))
        })
    }
}