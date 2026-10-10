use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One agent CLI the app can launch.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Harness {
    pub name: String,
    pub bin: String,
    /// Argv template. `{prompt}` is replaced with the prompt (always exactly
    /// one argv item, never shell-interpreted). `{model}` / `{effort}` are
    /// replaced with the UI values when set; when a value is unset, a bare
    /// placeholder and a flag immediately before it are dropped.
    #[serde(default)]
    pub args: Vec<String>,
    /// Inserted right after the binary when a model is selected and `args`
    /// does not itself use `{model}`.
    #[serde(default)]
    pub model_args: Vec<String>,
    /// Suggested model ids for the picker; free typing always allowed.
    #[serde(default)]
    pub models: Vec<String>,
    /// Suggested reasoning-effort levels for the picker (e.g. off/low/high).
    #[serde(default)]
    pub efforts: Vec<String>,
    /// Appended after model args when an effort is selected and `args` does
    /// not itself use `{effort}`. Example: ["--thinking", "{effort}"].
    #[serde(default)]
    pub effort_args: Vec<String>,
}

/// Configurable keybindings. Spec syntax: `cmd+enter`, `ctrl+shift+p`,
/// `cmd+.`, `esc`, single letters (`cmd+k`); empty string disables.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Keys {
    #[serde(default = "k_run")]
    pub run: String,
    #[serde(default = "k_cancel")]
    pub cancel: String,
    #[serde(default = "k_clear")]
    pub clear: String,
    #[serde(default = "k_reload")]
    pub reload: String,
    #[serde(default = "k_config")]
    pub config: String,
}

fn k_run() -> String {
    "cmd+enter".into()
}
fn k_cancel() -> String {
    "cmd+.".into()
}
fn k_clear() -> String {
    "cmd+k".into()
}
fn k_reload() -> String {
    "cmd+r".into()
}
fn k_config() -> String {
    "cmd+,".into()
}

impl Default for Keys {
    fn default() -> Self {
        Self {
            run: k_run(),
            cancel: k_cancel(),
            clear: k_clear(),
            reload: k_reload(),
            config: k_config(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Config {
    /// Working directory the app starts in; `~` is expanded.
    #[serde(default = "default_dir")]
    pub default_dir: String,
    #[serde(default)]
    pub keys: Keys,
    #[serde(default)]
    pub harness: Vec<Harness>,
}

fn default_dir() -> String {
    "~".to_string()
}

pub const DEFAULT_CONFIG: &str = r#"# Greenroom — configuration
#
# Greenroom is a launcher: it runs prompts through agent CLIs you already have
# installed and streams their output. No keys live here — each CLI keeps its
# own auth.
#
#   {prompt}  is replaced with your prompt (passed as a single argv value,
#             never through a shell).
#   {model}   is replaced with the model you pick in the UI. When no model is
#             picked, a bare "{model}" arg — and the flag right before it — is
#             dropped. If your CLI wants the model in a special place, put
#             {model} directly in "args" instead of "model_args".
#   {effort}  is replaced with the reasoning effort you pick (presets come
#             from each harness's "efforts"; "effort_args" is appended when
#             an effort is set). Same drop rules as {model}.
#
# Add, remove or reorder [[harness]] blocks freely; the app reloads this file
# on the Reload button, and you can edit all of it in-app via Config.
# Run `which <bin>` if an agent shows as greyed out.

default_dir = "~"

[keys]
# Keybindings. A spec is modifiers + key: cmd/ctrl/alt/shift + one of
# a-z, 0-9, enter, esc, tab, space, period, comma, slash, minus, arrows,
# backspace, delete. Empty string disables that binding.
run    = "cmd+enter"   # run the prompt
cancel = "cmd+."       # stop the running agent
clear  = "cmd+k"       # clear the output
reload = "cmd+r"       # reload this config file
config = "cmd+,"       # open the in-app config editor

[[harness]]
name = "Claude Code"
bin = "claude"
args = ["-p", "{prompt}"]
model_args = ["--model", "{model}"]
models = []

[[harness]]
name = "Codex"
bin = "codex"
args = ["exec", "{prompt}"]
model_args = ["-m", "{model}"]
models = []

[[harness]]
name = "OpenCode"
bin = "opencode"
args = ["run", "{prompt}"]
model_args = ["--model", "{model}"]
models = []

[[harness]]
name = "Gemini CLI"
bin = "gemini"
args = ["-p", "{prompt}"]
model_args = ["-m", "{model}"]
models = []

[[harness]]
name = "Aider"
bin = "aider"
args = ["--message", "{prompt}", "--yes"]
model_args = ["--model", "{model}"]
models = []

[[harness]]
name = "pi"
bin = "pi"
args = ["-p", "{prompt}", "--no-session"]
model_args = ["--model", "{model}"]
# e.g. CommandCode ids, if your pi is wired to that gateway:
models = ["deepseek/deepseek-v4.1-flash", "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash"]
efforts = ["off", "minimal", "low", "medium", "high", "xhigh", "max"]
effort_args = ["--thinking", "{effort}"]

[[harness]]
name = "DeepSeek Harness"
bin = "dsh"
args = ["--profile", "headless", "{prompt}"]
models = []

[[harness]]
name = "Command Code"
bin = "cmd"
args = ["-p", "{prompt}", "--max-turns", "1", "--no-session"]
model_args = ["-m", "{model}"]
models = ["deepseek/deepseek-v4.1-flash", "deepseek/deepseek-v4-pro", "deepseek/deepseek-v4-flash"]
efforts = ["low", "medium", "high"]
effort_args = ["--effort", "{effort}"]

[[harness]]
name = "Hermes Agent"
bin = "hermes"
args = ["chat", "-q", "{prompt}", "--oneshot", "-Q"]
models = []
"#;

pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("GREENROOM_CONFIG") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| Path::new(&h).join(".config"))
        })
        .or_else(|| {
            std::env::var("USERPROFILE")
                .ok()
                .map(|h| Path::new(&h).join(".config"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("greenroom").join("config.toml")
}

/// Load configuration; creates the default file on first run.
/// Returns (config, path, optional warning).
pub fn load() -> (Config, PathBuf, Option<String>) {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<Config>(&text) {
            Ok(cfg) => (cfg, path, None),
            Err(e) => (
                fallback(),
                path,
                Some(format!("config parse error: {e} — using built-in presets")),
            ),
        },
        Err(_) => {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(&path, DEFAULT_CONFIG);
            (fallback(), path, None)
        }
    }
}

/// Serialize a config back to TOML (used by the in-app editor).
pub fn serialize(cfg: &Config) -> Result<String, String> {
    toml::to_string_pretty(cfg).map_err(|e| e.to_string())
}

fn fallback() -> Config {
    toml::from_str(DEFAULT_CONFIG).unwrap_or_else(|_| Config {
        default_dir: default_dir(),
        keys: Keys::default(),
        harness: Vec::new(),
    })
}
