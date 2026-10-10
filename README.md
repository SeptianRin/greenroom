# 🎭 Greenroom

**Call any coding-agent harness onto the stage.**

You have several AI coding agents installed — Claude Code, Codex, OpenCode, Aider,
pi, dsh, cmd, Hermes… Each one has its own flags, its own models, its own mood.
Greenroom gives them **one stage**: pick an agent, pick a model, hand it a prompt,
watch the output stream.

![screenshot](docs/screenshot.png)

## What it is

- **One small native window.** Rust + egui. No Electron, no webview, no browser.
- **Runs the CLIs you already have** and trust — no API keys stored, no new auth.
  Every agent keeps its own configuration; Greenroom is only the stage manager.
- **Config-driven, zero code changes to add a harness.** Presets for common CLIs
  ship in the config; anything with a headless mode can be added in five lines.
- **Prompts go as argv, never through a shell.** Quotes, `$vars`, `$(…)` stay
  literal — exactly one argument to the agent.
- **Cancel means cancel.** The whole process group is signalled, children included.
- **Installs as a normal macOS app** — drag Greenroom.app into /Applications.

## Install

Download **Greenroom.dmg** from the [latest release](../../releases/latest) and
drag it into Applications (Apple Silicon, macOS 11+).

Or build from source (requires Rust 1.85+):

```bash
git clone https://github.com/SeptianRin/greenroom && cd greenroom
cargo build --release          # binary at target/release/greenroom
./scripts/package.sh           # Greenroom.app + Greenroom.dmg in dist/
```

## Use

Run it, pick an agent (dimmed name = binary not found on `$PATH`), optionally
type or pick a **model** and a reasoning **effort**, choose the working
directory, write the prompt, **Run**. Output streams live, and the command
preview under the buttons shows exactly what will be executed.

The first run creates `~/.config/greenroom/config.toml`. Edit it in your own
editor and hit **Reload** — or edit everything right inside the app: **Config**
opens a form editor plus a raw TOML editor with **Save & Reload**. No restart
needed either way.

### Keybindings

Every shortcut is configurable in the `[keys]` section of the config — write
specs like `cmd+enter`, `cmd+.`, `esc`, `ctrl+shift+p`; an empty string disables
that binding.

| Action | Default |
|---|---|
| Run the prompt | `Cmd+Enter` |
| Stop the running agent | `Cmd+.` |
| Clear output | `Cmd+K` |
| Reload config | `Cmd+R` |
| Open the config editor | `Cmd+,` |

## Configure an agent

Everything is one TOML file. A harness is:

```toml
[[harness]]
name = "OpenCode"
bin = "opencode"                              # resolved via $PATH
args = ["run", "{prompt}"]                    # {prompt} → your prompt, one argv item
model_args = ["--model", "{model}"]           # inserted when a model is picked
models = ["opencode-go/deepseek-v4-flash"]    # picker presets (free typing always allowed)
efforts = ["low", "medium", "high"]           # optional reasoning-effort presets
effort_args = ["--effort", "{effort}"]        # …and how to pass the picked effort
```

- `{prompt}` is substituted with your prompt; `{model}` with the picked model;
  `{effort}` with the picked reasoning effort.
- No model/effort picked? A bare placeholder — and the flag right before it — is
  dropped. Some CLIs want the value somewhere special: put `{model}` / `{effort}`
  directly into `args` and the placement is up to you.
- Anything with a non-interactive mode works — see the presets in your config
  and tweak flags to match your CLI version.

### Presets shipped

| Harness | One-shot invocation |
|---|---|
| Claude Code | `claude -p "{prompt}"` |
| Codex | `codex exec "{prompt}"` |
| OpenCode | `opencode run "{prompt}"` |
| Gemini CLI | `gemini -p "{prompt}"` |
| Aider | `aider --message "{prompt}" --yes` |
| pi | `pi -p "{prompt}" --no-session` |
| DeepSeek Harness | `dsh --profile headless "{prompt}"` |
| Command Code | `cmd -p "{prompt}" --max-turns 1 --no-session` |
| Hermes Agent | `hermes chat -q "{prompt}" --oneshot -Q` |

Reasoning-effort presets ship for **pi** (`--thinking off…max`) and
**Command Code** (`--effort low|medium|high`); add your own for any harness via
`efforts` + `effort_args`.

## Why "Greenroom"?

Performers wait in the greenroom until someone calls them up. Your agents do too:
they sit backstage, warmed up and ready, until you point at one and hand it the task.
One stage, many acts.

## Roadmap

- run history and diffing output between agents
- queued prompts / staging multiple acts
- PTY mode for CLIs that insist on a terminal
- Windows & Linux packaging (the core is cross-platform Rust)

## License

MIT © Septian Rinaldi Rianggoro
