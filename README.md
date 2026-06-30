# dude

A short, model-agnostic AI chat wrapper for the terminal. It is **not** an agent — just quick info while you code or research. One initial prompt plus up to 5 follow-up questions per session, then it tells you to start a new one.

```
dude what's the std::cell::OnceCell's advantage over OnceLock?
```

Inspired by `findlib`'s configuration flow and `heydude`'s invocation style.

## Features

- Model-agnostic providers: **Gemini** (default, free), **OpenAI**, **Anthropic**, **Groq**.
- TUI built with [ratatui](https://ratatui.rs) + [crossterm](https://crates.io/crates/crossterm).
- Multi-line prompt input (`Alt+Enter` or `Shift+Enter` for newline, `Enter` to send).
- Animated "Loading…" spinner while the model responds.
- Conversation history sent with each follow-up so the model keeps context.
- **6-turn cap** (initial + 5 follow-ups). A 7th attempt is refused with a funny message and the session is closed — keeping it fast and bounded.
- Config stored at `~/.config/dude/config.json`, mirroring the findlib app.
- Per-provider model override via `dude config model <name>`.

## Install

```bash
cargo install --path .
# or, from source:
cargo build --release
# then put target/release/dude on your $PATH
```

## Configuration

Configuration mirrors the `findlib` app.

```bash
dude config                 # show current config (keys are masked)
dude config gemini <key>    # set Gemini API key (free: aistudio.google.com/apikey)
dude config openai <key>
dude config anthropic <key>
dude config groq <key>
dude config llm <provider>  # switch active provider (gemini|openai|anthropic|groq)
dude config model <name>    # override the active provider's model
dude config model ""        # clear the override (use the provider default)
```

Per-provider default models (used when no override is set):

| Provider  | Default model               |
|-----------|-----------------------------|
| gemini    | gemini-2.5-flash            |
| openai    | gpt-4o-mini                 |
| anthropic | claude-3-5-haiku-20241022   |
| groq      | llama-3.3-70b-versatile     |

### First-run wizard

Running `dude <prompt>` with no key for the active provider launches an
interactive setup wizard: it prints the free Gemini key URL
(`aistudio.google.com/apikey`), lets you pick a provider, and reads the key to
store in `~/.config/dude/config.json`. Then the TUI starts with your prompt.

## Usage

```bash
dude <prompt...>
```

Inside the TUI:

| Key                  | Action                       |
|----------------------|------------------------------|
| `Enter`              | send the prompt              |
| `Alt+Enter` / `Shift+Enter` | insert a newline       |
| `←` `→` `↑` `↓`      | move the cursor              |
| `Ctrl+U`             | clear the input box          |
| `Esc`                | quit                         |

The status bar shows the active provider, remaining turns, and a "Loading…"
spinner while the model responds. When you hit the 6-turn cap, the session
locks further input and prints:

> This is supposed to be a short conversation... but you just hit the
> 6-question limit. Start a new `dude` session if you need to keep going.

## Project layout

```
dude/
├── Cargo.toml
└── src/
    ├── main.rs          # clap CLI + setup wizard
    ├── config.rs        # ~/.config/dude/config.json load/save (mirrors findlib)
    ├── chat.rs          # conversation state, 6-turn cap, funny error
    ├── provider/
    │   ├── mod.rs       # Provider trait (boxed async futures, no extra dep)
    │   ├── gemini.rs
    │   ├── openai.rs
    │   ├── anthropic.rs
    │   └── groq.rs
    └── tui/
        ├── mod.rs       # ratatui app loop + async HTTP task
        ├── input.rs     # multi-line editor
        └── view.rs      # chat bubbles + spinner
```

Adding a provider is one new file under `src/provider/` plus a match arm in
`provider::from_config`.

## License

MIT