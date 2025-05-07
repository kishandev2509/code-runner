use serde::Deserialize;
use std::{collections::HashMap, fs, path::PathBuf, process::Command};
use zed_extension_api as zed;

#[derive(Deserialize)]
struct RunnerConfig {
    languages: HashMap<String, String>,
    files: HashMap<String, String>,
}

pub struct CodeRunner {
    config: RunnerConfig,
}

impl zed::Extension for CodeRunner {
    fn new() -> Self {
        let config = Self::load_config();
        CodeRunner { config }
    }

    fn run_slash_command(
        &self,
        command: zed::SlashCommand,
        args: Vec<String>,
        _worktree: Option<&zed::Worktree>,
    ) -> zed::Result<zed::SlashCommandOutput> {
        if command.name == "code-runner.run" {
            let path = if let Some(arg) = args.first() {
                PathBuf::from(arg)
            } else {
                return Ok(zed::SlashCommandOutput {
                    text: "No file path provided".into(),
                    sections: vec![],
                });
            };

            match self.run(path) {
                Ok(output) => Ok(zed::SlashCommandOutput {
                    text: output,
                    sections: vec![],
                }),
                Err(e) => Ok(zed::SlashCommandOutput {
                    text: format!("Error: {}", e),
                    sections: vec![],
                }),
            }
        } else {
            Ok(zed::SlashCommandOutput {
                text: format!("Unknown command: {}", command.name),
                sections: vec![],
            })
        }
    }
}

impl CodeRunner {
    fn load_config() -> RunnerConfig {
        let exe_dir = std::env::current_exe()
            .map(|p| p.parent().unwrap_or(&PathBuf::from(".")).to_path_buf())
            .unwrap_or_else(|_| PathBuf::from("."));
        let config_path = exe_dir.join("config").join("runner.toml");
        let toml = fs::read_to_string(&config_path).unwrap_or_else(|_| {
            fs::read_to_string("config/runner.toml").expect("Could not read runner.toml")
        });
        toml::from_str(&toml).expect("Invalid runner.toml format")
    }

    fn command_for(&self, path: &PathBuf) -> Option<String> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();

        // 1. File-type mapping
        if let Some(cmd) = self.config.files.get(&format!(".{ext}")) {
            return Some(
                cmd.replace("{path}", &path.to_string_lossy()).replace(
                    "{dir}",
                    &path
                        .parent()
                        .unwrap_or(&PathBuf::from("."))
                        .to_string_lossy(),
                ),
            );
        }

        // 2. Language mapping (fallback)
        self.config.languages.get(&ext).map(|cmd| {
            cmd.replace("{path}", &path.to_string_lossy()).replace(
                "{dir}",
                &path
                    .parent()
                    .unwrap_or(&PathBuf::from("."))
                    .to_string_lossy(),
            )
        })
    }

    fn run(&self, path: PathBuf) -> Result<String, String> {
        let cmd_str = self
            .command_for(&path)
            .ok_or_else(|| "No matching run command found.".to_string())?;

        // Parse command respecting quotes
        let mut args = Vec::new();
        let mut current = String::new();
        let mut in_quotes = false;

        for c in cmd_str.chars() {
            match c {
                '"' => {
                    in_quotes = !in_quotes;
                    if !in_quotes && !current.is_empty() {
                        args.push(current);
                        current = String::new();
                    }
                }
                ' ' if !in_quotes => {
                    if !current.is_empty() {
                        args.push(current);
                        current = String::new();
                    }
                }
                _ => current.push(c),
            }
        }
        if !current.is_empty() {
            args.push(current);
        }

        let prog = args
            .first()
            .ok_or_else(|| "Empty command string".to_string())?;
        let args = &args[1..];

        let workdir = path.parent().unwrap_or(&PathBuf::from(".")).to_path_buf();
        let output = Command::new(prog)
            .args(args)
            .current_dir(workdir)
            .output()
            .map_err(|e| e.to_string())?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into())
        }
    }
}

zed::register_extension!(CodeRunner);
