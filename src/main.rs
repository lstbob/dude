mod chat;
mod config;
mod provider;
mod tui;

use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::io::IsTerminal;

const ABOUT: &str = "dude — short, model-agnostic AI chats in the terminal.\n\n\
                     It is NOT an agent: just quick info while you code/research. \
                     One + up to 5 follow-up questions per session, then it quits on you.\n\n\
                     Default provider is Gemini (free key: aistudio.google.com/apikey).";

#[derive(Parser)]
#[command(name = "dude", version, about = ABOUT, long_about = ABOUT)]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

    /// One-shot print mode: send the prompt, print the answer to stdout, exit.
    /// No TUI. Also auto-engaged when stdout is not a TTY (e.g. `:!` in nvim,
    /// pipes, CI), so `:!dude "what year is it?"` Just Works.
    #[arg(short = 'p', long = "print")]
    print_mode: bool,

    /// Prompt text; args after the first are joined with spaces. Ignored when
    /// a subcommand (e.g. `config`) is present.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    prompt: Vec<String>,
}

#[derive(Subcommand)]
enum Cmd {
    /// View or set configuration, mirroring the findlib app:
    ///   dude config               show current config
    ///   dude config gemini <key>  set Gemini API key (free: aistudio.google.com/apikey)
    ///   dude config openai <key>  set OpenAI API key
    ///   dude config anthropic <key>
    ///   dude config groq <key>
    ///   dude config llm <provider>     switch active provider
    ///   dude config model <name>       override the active provider's model
    ///   dude config model ""           clear the model override (use the default)
    Config {
        /// `<key> [value]` pair, as documented above.
        args: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Cmd::Config { args }) => run_config(&args),
        None => {
            if cli.prompt.is_empty() {
                println!("{}", ABOUT);
                println!("\nUsage: dude [OPTIONS] <prompt...>\n       dude config [key] [value]");
                return Ok(());
            }
            run_chat(cli.prompt.join(" "), cli.print_mode).await
        }
    }
}

async fn run_chat(prompt: String, force_print: bool) -> Result<()> {
    // Auto-engaging print mode when stdout isn't a TTY is what makes `dude`
    // usable from contexts that hand us pipes instead of a terminal — most
    // notably nvim's `:!dude "..."`, but also plain shell pipes and CI.
    let print_mode = force_print || !std::io::stdout().is_terminal();

    let cfg = config::Config::load()?;

    if print_mode {
        return run_print(cfg, prompt).await;
    }

    run_tui(cfg, prompt).await
}

/// One-shot mode: send a single prompt, print the answer to stdout, exit.
/// Used by `dude -p`, pipes, and nvim's `:!`.
async fn run_print(cfg: config::Config, prompt: String) -> Result<()> {
    if cfg.active_key().is_none() {
        // Can't run the interactive wizard without a TTY; point the user at
        // the config command instead.
        eprintln!(
            "No {} API key set. Run in a terminal: dude config {} <key>",
            cfg.llm_provider,
            cfg.llm_provider
        );
        eprintln!("Gemini is free: {}", config::gemini_free_key_url());
        bail!("no api key configured");
    }

    let provider = provider::from_config(&cfg)?;
    let mut chat = chat::Chat::new();
    if !chat.push_user(prompt) {
        println!("{}", chat::FUNNY_LIMIT_MESSAGE);
        return Ok(());
    }
    match provider.complete(&chat.messages).await {
        Ok(text) => {
            println!("The Dude: {}", text.trim());
            Ok(())
        }
        Err(e) => {
            eprintln!("dude error: {e}");
            bail!(e)
        }
    }
}

/// Interactive TUI mode (real terminal only). Runs the setup wizard if no key
/// is configured, then launches the ratatui app.
async fn run_tui(mut cfg: config::Config, prompt: String) -> Result<()> {
    // If no key is set for the active provider, run the setup wizard.
    if cfg.active_key().is_none() {
        match setup_wizard(&mut cfg) {
            Ok(changed) => {
                if changed {
                    cfg.save()?;
                    eprintln!("✓ configuration saved. starting dude…\n");
                } else {
                    eprintln!("No API key provided. Run `dude config` to set one.");
                    return Ok(());
                }
            }
            Err(e) => {
                eprintln!("Setup failed: {e}");
                return Ok(());
            }
        }
    }

    let provider = provider::from_config(&cfg)?;
    let app = tui::App::new(provider, prompt);
    tui::run(app)
}

/// Returns Ok(true) if a key was set and saved in memory; Ok(false) if the
/// user bailed out without providing one.
fn setup_wizard(cfg: &mut config::Config) -> Result<bool> {
    use std::io::{self, BufRead, Write};

    eprintln!("No {} API key is configured.", cfg.llm_provider);
    eprintln!(
        "Gemini is free — get one at {} (no signup beyond a Google account).",
        config::gemini_free_key_url()
    );
    eprintln!("You can also run `dude config <provider> <key>` to set it later.\n");

    eprintln!("Choose a provider:");
    for (i, p) in config::PROVIDERS.iter().enumerate() {
        let mark = if *p == cfg.llm_provider { " (current)" } else { "" };
        eprintln!("  {}. {}{}", i + 1, p, mark);
    }
    eprint!("\nPick a number [1-4] or Enter to keep current: ");
    io::stderr().flush()?;

    let stdin = io::stdin();
    let mut line = String::new();
    stdin.lock().read_line(&mut line)?;
    let trimmed = line.trim();
    let chosen = if trimmed.is_empty() {
        cfg.llm_provider.clone()
    } else {
        let idx: usize = trimmed
            .parse()
            .map_err(|_| anyhow::anyhow!("invalid selection"))?;
        if idx < 1 || idx > config::PROVIDERS.len() {
            anyhow::bail!("selection out of range");
        }
        config::PROVIDERS[idx - 1].to_string()
    };

    eprintln!("\nEnter the {} API key (one line): ", chosen);
    eprintln!(
        "(free Gemini keys: {})",
        config::gemini_free_key_url()
    );
    eprint!("> ");
    io::stderr().flush()?;

    let mut key = String::new();
    stdin.lock().read_line(&mut key)?;
    let key = key.trim().to_string();
    if key.is_empty() {
        return Ok(false);
    }

    cfg.llm_provider = chosen;
    let provider = cfg.llm_provider.clone();
    cfg.set_key(&provider, &key)?;
    Ok(true)
}

fn run_config(args: &[String]) -> Result<()> {
    let mut cfg = config::Config::load()?;

    match args.len() {
        0 => {
            print_config(&cfg);
            Ok(())
        }
        2 => set_config(&mut cfg, &args[0], &args[1]),
        _ => {
            eprintln!("Usage: dude config [key] [value]");
            bail!("expected 0 or 2 args, got {}", args.len());
        }
    }
}

fn print_config(cfg: &config::Config) {
    println!("Current configuration:");
    println!("  LLM Provider:    {}", cfg.llm_provider);
    println!("  Model override:  {}", if cfg.model.is_empty() { "(provider default)" } else { &cfg.model });
    println!("  Gemini API:      {}", config::masked(&cfg.gemini_api_key));
    println!("  OpenAI API:      {}", config::masked(&cfg.openai_api_key));
    println!("  Anthropic API:   {}", config::masked(&cfg.anthropic_api_key));
    println!("  Groq API:        {}", config::masked(&cfg.groq_api_key));
    println!();
    println!("  Config file:     {}", config::Config::file_path().display());
    println!();
    println!("Gemini is free: aistudio.google.com/apikey");
    println!("Set a key with:  dude config gemini <key>");
    println!("Switch provider:  dude config llm <gemini|openai|anthropic|groq>");
    println!("Override model:   dude config model <name>");
}

fn set_config(cfg: &mut config::Config, key: &str, value: &str) -> Result<()> {
    let kind = key.to_lowercase();
    match kind.as_str() {
        "gemini" | "openai" | "anthropic" | "groq" => {
            cfg.set_key(&kind, value)?;
        }
        "llm" => {
            if !config::is_valid_provider(value) {
                eprintln!(
                    "Unknown provider: {value}\nValid: gemini, openai, anthropic, groq"
                );
                bail!("invalid provider");
            }
            cfg.llm_provider = value.to_string();
        }
        "model" => {
            // Empty string (passed as `""`) clears the override.
            cfg.model = value.trim_matches('"').to_string();
        }
        other => {
            eprintln!(
                "Unknown config key: {other}\nValid: gemini, openai, anthropic, groq, llm, model"
            );
            bail!("unknown config key");
        }
    }

    if let Err(e) = cfg.save() {
        eprintln!("Error saving config: {e}");
        return Err(e);
    }
    println!("✓ {kind} updated");
    Ok(())
}