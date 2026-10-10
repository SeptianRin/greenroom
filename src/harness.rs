use crate::config::Harness;
use std::path::PathBuf;

/// Build the argv for one run. The prompt always becomes exactly one argv
/// item; nothing is ever interpreted by a shell.
pub fn build_command(
    h: &Harness,
    prompt: &str,
    model: Option<&str>,
    effort: Option<&str>,
) -> Vec<String> {
    let model = model.map(str::trim).filter(|s| !s.is_empty());
    let effort = effort.map(str::trim).filter(|s| !s.is_empty());
    let model_in_args = h.args.iter().any(|a| a.contains("{model}"));
    let effort_in_args = h.args.iter().any(|a| a.contains("{effort}"));

    let mut out: Vec<String> = vec![h.bin.clone()];

    if let Some(m) = model {
        if !h.model_args.is_empty() && !model_in_args {
            expand_into(&mut out, &h.model_args, prompt, Some(m), effort);
        }
    }
    if let Some(e) = effort {
        if !h.effort_args.is_empty() && !effort_in_args {
            expand_into(&mut out, &h.effort_args, prompt, model, Some(e));
        }
    }
    expand_into(&mut out, &h.args, prompt, model, effort);
    out
}

fn expand_into(
    out: &mut Vec<String>,
    tpl: &[String],
    prompt: &str,
    model: Option<&str>,
    effort: Option<&str>,
) {
    for a in tpl {
        let missing_model = a.contains("{model}") && model.is_none();
        let missing_effort = a.contains("{effort}") && effort.is_none();
        if missing_model || missing_effort {
            let only_placeholder = {
                let t = a.trim();
                (t == "{model}" && missing_model) || (t == "{effort}" && missing_effort)
            };
            if only_placeholder {
                // Drop the placeholder and a bare flag immediately before it
                // (e.g. "--model", "{model}" as two argv items).
                drop_trailing_flag(out, prompt);
                continue;
            }
            if a.starts_with('-') && !a.contains("{prompt}") {
                // "--model={model}" style: the token is meaningless without a
                // value, drop the whole token.
                continue;
            }
            // Mixed content (e.g. "{prompt} {model}"): strip the missing part.
            let mut s = a.clone();
            if missing_model {
                s = s.replace("{model}", "");
            }
            if missing_effort {
                s = s.replace("{effort}", "");
            }
            let s = s.replace("{prompt}", prompt).trim().to_string();
            if s.is_empty() {
                drop_trailing_flag(out, prompt);
            } else {
                out.push(s);
            }
            continue;
        }
        let mut s = a.replace("{prompt}", prompt);
        if let Some(m) = model {
            s = s.replace("{model}", m);
        }
        if let Some(e) = effort {
            s = s.replace("{effort}", e);
        }
        out.push(s);
    }
}

fn drop_trailing_flag(out: &mut Vec<String>, prompt: &str) {
    if let Some(prev) = out.last() {
        let is_flag = prev.len() > 1 && prev.starts_with('-') && prev != "--";
        let contains_prompt = !prompt.is_empty() && prev.contains(prompt);
        if is_flag && !contains_prompt {
            out.pop();
        }
    }
}

/// Expand a leading `~/` (or a bare `~`) using $HOME.
pub fn expand_tilde(path: &str) -> String {
    let home = std::env::var("HOME").unwrap_or_default();
    if path == "~" {
        return home;
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if !home.is_empty() {
            return format!("{home}/{rest}");
        }
    }
    path.to_string()
}

/// Resolve a binary name through $PATH (or accept an existing path).
pub fn bin_path(bin: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let is_exec = |p: &std::path::Path| -> bool {
        p.is_file()
            && p.metadata()
                .map(|m| m.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
    };
    let p = std::path::Path::new(bin);
    if bin.contains('/') {
        return if is_exec(p) { Some(p.to_path_buf()) } else { None };
    }
    let paths = std::env::var("PATH").ok()?;
    for dir in paths.split(':') {
        if dir.is_empty() {
            continue;
        }
        let cand = std::path::Path::new(dir).join(bin);
        if is_exec(&cand) {
            return Some(cand);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(bin: &str, args: &[&str], model_args: &[&str]) -> Harness {
        Harness {
            name: "test".into(),
            bin: bin.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            model_args: model_args.iter().map(|s| s.to_string()).collect(),
            models: vec![],
            efforts: vec![],
            effort_args: vec![],
        }
    }

    fn h_effort(bin: &str, args: &[&str], effort_args: &[&str]) -> Harness {
        Harness {
            name: "test".into(),
            bin: bin.into(),
            args: args.iter().map(|s| s.to_string()).collect(),
            model_args: vec![],
            models: vec![],
            efforts: vec![],
            effort_args: effort_args.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn tricky_prompt_stays_single_argv_item() {
        let prompt = "review \"this\"; do not run $(anything) && rm -rf /";
        let cmd = build_command(
            &h("cmd", &["-p", "{prompt}", "--max-turns", "1", "--no-session"], &[]),
            prompt,
            None,
            None,
        );
        assert_eq!(
            cmd,
            vec!["cmd", "-p", prompt, "--max-turns", "1", "--no-session"]
        );
    }

    #[test]
    fn model_args_inserted_when_args_have_no_model() {
        let cmd = build_command(
            &h("pi", &["-p", "{prompt}", "--no-session"], &["--model", "{model}"]),
            "hi",
            Some("moonshotai/kimi-k3"),
            None,
        );
        assert_eq!(
            cmd,
            vec!["pi", "--model", "moonshotai/kimi-k3", "-p", "hi", "--no-session"]
        );
    }

    #[test]
    fn no_model_no_model_flags() {
        let cmd = build_command(
            &h("pi", &["-p", "{prompt}", "--no-session"], &["--model", "{model}"]),
            "hi",
            None,
            None,
        );
        assert_eq!(cmd, vec!["pi", "-p", "hi", "--no-session"]);
    }

    #[test]
    fn inline_model_placeholder_substituted_and_dropped() {
        let harness = h("aider", &["-m", "{model}", "--yes", "{prompt}"], &[]);
        assert_eq!(
            build_command(&harness, "go", Some("gpt-1"), None),
            vec!["aider", "-m", "gpt-1", "--yes", "go"]
        );
        assert_eq!(
            build_command(&harness, "go", None, None),
            vec!["aider", "--yes", "go"]
        );
    }

    #[test]
    fn blank_model_treated_as_none() {
        let cmd = build_command(
            &h("pi", &["-p", "{prompt}"], &["--model", "{model}"]),
            "x",
            Some("  "),
            None,
        );
        assert_eq!(cmd, vec!["pi", "-p", "x"]);
    }

    #[test]
    fn effort_args_added_when_set() {
        let harness = h_effort("pi", &["-p", "{prompt}"], &["--thinking", "{effort}"]);
        assert_eq!(
            build_command(&harness, "hi", None, Some("high")),
            vec!["pi", "--thinking", "high", "-p", "hi"]
        );
        assert_eq!(
            build_command(&harness, "hi", None, None),
            vec!["pi", "-p", "hi"]
        );
    }

    #[test]
    fn inline_effort_placeholder_dropped_when_unset() {
        let harness = h_effort("x", &["run", "--thinking", "{effort}", "{prompt}"], &[]);
        assert_eq!(
            build_command(&harness, "go", None, None),
            vec!["x", "run", "go"]
        );
        assert_eq!(
            build_command(&harness, "go", None, Some("low")),
            vec!["x", "run", "--thinking", "low", "go"]
        );
    }

    #[test]
    fn model_and_effort_together() {
        let harness = Harness {
            name: "test".into(),
            bin: "pi".into(),
            args: vec!["-p".into(), "{prompt}".into(), "--no-session".into()],
            model_args: vec!["--model".into(), "{model}".into()],
            models: vec![],
            efforts: vec![],
            effort_args: vec!["--thinking".into(), "{effort}".into()],
        };
        assert_eq!(
            build_command(&harness, "hi", Some("deepseek/deepseek-v4.1-flash"), Some("xhigh")),
            vec![
                "pi",
                "--model",
                "deepseek/deepseek-v4.1-flash",
                "--thinking",
                "xhigh",
                "-p",
                "hi",
                "--no-session"
            ]
        );
    }

    #[test]
    fn expand_tilde_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/code"), format!("{home}/code"));
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
    }
}
