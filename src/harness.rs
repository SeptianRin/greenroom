use crate::config::Harness;
use std::path::PathBuf;

/// Build the argv for one run. The prompt always becomes exactly one argv
/// item; nothing is ever interpreted by a shell.
pub fn build_command(h: &Harness, prompt: &str, model: Option<&str>) -> Vec<String> {
    let model = model.map(str::trim).filter(|s| !s.is_empty());
    let model_used_in_args = h.args.iter().any(|a| a.contains("{model}"));

    let mut out: Vec<String> = vec![h.bin.clone()];

    if let Some(m) = model {
        if !h.model_args.is_empty() && !model_used_in_args {
            for a in &h.model_args {
                out.push(a.replace("{model}", m));
            }
        }
    }

    for a in &h.args {
        if let Some(m) = model {
            out.push(a.replace("{model}", m).replace("{prompt}", prompt));
            continue;
        }
        if a.contains("{model}") {
            if a == "{model}" {
                // No model selected: drop the placeholder and a bare flag
                // immediately before it (e.g. "--model", "{model}").
                let drop_flag = out
                    .last()
                    .map(|prev| prev.starts_with('-') && !prev.contains("{prompt}"))
                    .unwrap_or(false);
                if drop_flag {
                    out.pop();
                }
                continue;
            }
            out.push(a.replace("{model}", "").replace("{prompt}", prompt));
        } else {
            out.push(a.replace("{prompt}", prompt));
        }
    }
    out
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
        }
    }

    #[test]
    fn tricky_prompt_stays_single_argv_item() {
        let prompt = "review \"this\"; do not run $(anything) && rm -rf /";
        let cmd = build_command(
            &h("cmd", &["-p", "{prompt}", "--max-turns", "1", "--no-session"], &[]),
            prompt,
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
        );
        assert_eq!(cmd, vec!["pi", "-p", "hi", "--no-session"]);
    }

    #[test]
    fn inline_model_placeholder_substituted_and_dropped() {
        let harness = h("aider", &["-m", "{model}", "--yes", "{prompt}"], &[]);
        assert_eq!(
            build_command(&harness, "go", Some("gpt-1")),
            vec!["aider", "-m", "gpt-1", "--yes", "go"]
        );
        assert_eq!(
            build_command(&harness, "go", None),
            vec!["aider", "--yes", "go"]
        );
    }

    #[test]
    fn blank_model_treated_as_none() {
        let cmd = build_command(&h("pi", &["-p", "{prompt}"], &["--model", "{model}"]), "x", Some("  "));
        assert_eq!(cmd, vec!["pi", "-p", "x"]);
    }

    #[test]
    fn expand_tilde_home() {
        let home = std::env::var("HOME").unwrap();
        assert_eq!(expand_tilde("~"), home);
        assert_eq!(expand_tilde("~/code"), format!("{home}/code"));
        assert_eq!(expand_tilde("/abs/path"), "/abs/path");
    }
}
