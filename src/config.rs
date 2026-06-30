use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const PROVIDERS: [&str; 4] = ["gemini", "openai", "anthropic", "groq"];

const GEMINI_FREE_KEY_URL: &str = "aistudio.google.com/apikey";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_provider")]
    pub llm_provider: String,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default)]
    pub groq_api_key: String,
}

fn default_provider() -> String {
    "gemini".to_string()
}

impl Config {
    /// Load config from disk; returns defaults if the file is missing.
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self {
                llm_provider: default_provider(),
                ..Default::default()
            });
        }
        let data = std::fs::read_to_string(&path)
            .with_context(|| format!("reading config {}", path.display()))?;
        let mut cfg: Config = serde_json::from_str(&data)
            .with_context(|| format!("parsing config {}", path.display()))?;
        if cfg.llm_provider.is_empty() {
            cfg.llm_provider = default_provider();
        }
        Ok(cfg)
    }

    /// Save config to disk, creating the directory first.
    pub fn save(&self) -> Result<()> {
        let dir = Self::dir()?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating config dir {}", dir.display()))?;
        let path = Self::path()?;
        let data = serde_json::to_string_pretty(self).context("encoding config")?;
        std::fs::write(&path, data)
            .with_context(|| format!("writing config {}", path.display()))?;
        Ok(())
    }

    /// Returns the API key for the configured provider, or `None` if unset.
    pub fn active_key(&self) -> Option<&str> {
        match self.llm_provider.as_str() {
            "gemini" => (!self.gemini_api_key.is_empty()).then_some(&self.gemini_api_key),
            "openai" => (!self.openai_api_key.is_empty()).then_some(&self.openai_api_key),
            "anthropic" => (!self.anthropic_api_key.is_empty()).then_some(&self.anthropic_api_key),
            "groq" => (!self.groq_api_key.is_empty()).then_some(&self.groq_api_key),
            _ => None,
        }
    }

    /// Sets the key for the named provider. Returns `Err` for unknown provider.
    pub fn set_key(&mut self, provider: &str, value: &str) -> Result<()> {
        match provider {
            "gemini" => self.gemini_api_key = value.to_string(),
            "openai" => self.openai_api_key = value.to_string(),
            "anthropic" => self.anthropic_api_key = value.to_string(),
            "groq" => self.groq_api_key = value.to_string(),
            other => anyhow::bail!("unknown provider: {other}"),
        }
        Ok(())
    }

    pub fn file_path() -> PathBuf {
        Self::path().unwrap_or_else(|_| PathBuf::from("config.json"))
    }

    fn dir() -> Result<PathBuf> {
        let base = directories::ProjectDirs::from("dev", "lstbob", "dude")
            .context("could not locate user config directory")?;
        Ok(base.config_dir().to_path_buf())
    }

    fn path() -> Result<PathBuf> {
        Ok(Self::dir()?.join("config.json"))
    }
}

/// `XXXX…YYYY` masked preview of a secret, mirroring findlib's `printConfig`.
pub fn masked(s: &str) -> String {
    if s.is_empty() {
        return "(not set)".to_string();
    }
    if s.len() > 8 {
        format!("{}…{}", &s[..4], &s[s.len() - 4..])
    } else {
        "****".to_string()
    }
}

pub fn gemini_free_key_url() -> &'static str {
    GEMINI_FREE_KEY_URL
}

/// Helper used by the setup wizard and `config` command to validate a provider name.
pub fn is_valid_provider(p: &str) -> bool {
    PROVIDERS.contains(&p)
}

