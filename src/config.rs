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
    ///
    /// The file holds plaintext API keys, so on Unix we restrict it to the
    /// owner: the directory is created `0700` and the file written `0600`,
    /// preventing other local users from reading the secrets.
    pub fn save(&self) -> Result<()> {
        let dir = Self::dir()?;
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating config dir {}", dir.display()))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            // Best-effort tighten on the dir; ignore if it predates us with
            // looser perms set deliberately by the user.
            let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
        }
        let path = Self::path()?;
        let data = serde_json::to_string_pretty(self).context("encoding config")?;
        write_private(&path, data.as_bytes())
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

/// Write `data` to `path`, owner-readable only (`0600`) on Unix.
///
/// On Unix the file is opened with mode `0600`; if it already existed with
/// looser permissions we also re-`set_permissions` so an old world-readable
/// `config.json` is tightened on the next save. On non-Unix we fall back to a
/// plain write.
fn write_private(path: &std::path::Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;

    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        // Covers the case where the file pre-existed with looser perms (the
        // `.mode()` above only applies to freshly created files).
        f.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        f.write_all(data)?;
        f.flush()
    }
    #[cfg(not(unix))]
    {
        let mut f = std::fs::File::create(path)?;
        f.write_all(data)?;
        f.flush()
    }
}

/// `XXXX…YYYY` masked preview of a secret, mirroring findlib's `printConfig`.
///
/// Counts by `chars()` rather than slicing by byte offset: a key containing a
/// multibyte char straddling byte 4 or `len-4` would otherwise panic.
pub fn masked(s: &str) -> String {
    if s.is_empty() {
        return "(not set)".to_string();
    }
    let chars: Vec<char> = s.chars().collect();
    if chars.len() > 8 {
        let head: String = chars[..4].iter().collect();
        let tail: String = chars[chars.len() - 4..].iter().collect();
        format!("{head}…{tail}")
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

#[cfg(test)]
mod tests {
    use super::masked;

    #[test]
    fn masked_not_set() {
        assert_eq!(masked(""), "(not set)");
    }

    #[test]
    fn masked_short_key_fully_hidden() {
        assert_eq!(masked("sk-12345"), "****");
    }

    #[test]
    fn masked_long_key_shows_ends() {
        assert_eq!(masked("sk-abcdefghijklmnop"), "sk-a…mnop");
    }

    #[test]
    fn masked_multibyte_does_not_panic() {
        // Multibyte chars straddling the 4-char head/tail boundaries used to
        // panic when sliced by byte offset.
        let key = "🔑🔑🔑🔑middle🗝🗝🗝🗝";
        let out = masked(key);
        assert!(out.contains('…'));
    }
}

