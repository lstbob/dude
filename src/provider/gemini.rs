use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::chat::Message;
use crate::provider::{http_client, truncate_body, Fut, Provider};

const DEFAULT_MODEL: &str = "gemini-2.5-flash";
const ENDPOINT: &str = "https://generativelanguage.googleapis.com/v1beta/models";

pub struct Gemini {
    api_key: String,
    model: String,
    client: Client,
}

#[derive(Serialize)]
struct Request<'a> {
    contents: Vec<ContentRef<'a>>,
}

#[derive(Serialize)]
struct ContentRef<'a> {
    role: &'a str,
    parts: Vec<PartRef<'a>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PartRef<'a> {
    text: &'a str,
}

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    candidates: Vec<Candidate>,
}

#[derive(Deserialize)]
struct Candidate {
    #[serde(default)]
    content: Option<Content>,
}

#[derive(Deserialize)]
struct Content {
    #[serde(default)]
    parts: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    #[serde(default)]
    text: String,
}

impl Gemini {
    pub fn new(api_key: String, model: Option<&str>) -> Result<Self> {
        if api_key.is_empty() {
            anyhow::bail!("gemini api key is empty (free: aistudio.google.com/apikey)");
        }
        Ok(Self {
            api_key,
            model: model.unwrap_or(DEFAULT_MODEL).to_string(),
            client: http_client()?,
        })
    }
}

impl Provider for Gemini {
    fn name(&self) -> &'static str {
        "gemini"
    }

    fn complete<'a>(&'a self, messages: &'a [Message]) -> Fut<'a> {
        Box::pin(async move {
            let contents: Vec<ContentRef> = messages
                .iter()
                .map(|m| ContentRef {
                    role: match m.role {
                        crate::chat::Role::User => "user",
                        crate::chat::Role::Assistant => "model",
                    },
                    parts: vec![PartRef { text: &m.content }],
                })
                .collect();

            let url = format!("{}/{}:generateContent", ENDPOINT, self.model);
            // Send the key as a header rather than a `?key=` query param so it
            // doesn't end up in proxy/server access logs or shell history.
            let resp = self
                .client
                .post(&url)
                .header("x-goog-api-key", &self.api_key)
                .json(&Request { contents })
                .send()
                .await
                .map_err(|e| anyhow!("gemini request: {e}"))?;
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(anyhow!("gemini api {}: {}", status.as_u16(), truncate_body(&body)));
            }
            let parsed: Response = serde_json::from_str(&body)
                .map_err(|e| anyhow!("parsing gemini response: {e}\nbody: {}", truncate_body(&body)))?;
            let text = parsed
                .candidates
                .into_iter()
                .next()
                .and_then(|c| c.content)
                .map(|c| {
                    c.parts
                        .into_iter()
                        .map(|p| p.text)
                        .collect::<Vec<_>>()
                        .join("")
                })
                .filter(|s| !s.is_empty())
                .ok_or_else(|| anyhow!("gemini returned no content"))?;
            Ok(text)
        })
    }
}