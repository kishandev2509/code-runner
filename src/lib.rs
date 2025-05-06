use zed_extension_api as zed;
use std::{collections::HashMap, fs, path::PathBuf, process::Command};
use serde::Deserialize;

#[derive(Deserialize)]
struct RunnerConfig {
    languages: HashMap<String, String>,
    files:     HashMap<String, String>,
    projects:  HashMap<String, String>,
}

pub struct CodeRunner {
    config: RunnerConfig,
}

impl CodeRunner {
    fn load_config() -> RunnerConfig {
        let toml = fs::read_to_string("config/runner.toml")
            .expect("Could not read runner.toml");
        toml::from_str(&toml).expect("Invalid runner.toml format")
    }

    fn detect_project_type(root: &PathBuf) -> Option<String> {
        if root.join("package.json").exists() {
            Some("node".into())
        } else if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
            Some("python".into())
        } else if root.join("Cargo.toml").exists() {
            Some("rust".into())
        } else {
            None
        }
    }

    fn command_for(&self, path: &PathBuf, project_root: &PathBuf) -> Option<String> {
        let ext = path.extension()?.to_string_lossy().to_lowercase();
        // 1. File-type mapping
        if let Some(cmd) = self.config.files.get(&format!(".{ext}")) {
            return Some(cmd.replace("{path}", &path.to_string_lossy()));
        }
        // 2. Language mapping
        if let Some(lang) = zed::language_for(&path) {
            if let Some(cmd) = self.config.languages.get(&lang) {
                return Some(cmd.replace("{path}", &path.to_string_lossy()));
            }
        }
        // 3. Project mapping
        if let Some(proj) = Self::detect_project_type(project_root) {
            if let Some(cmd) = self.config.projects.get(&proj) {
                return Some(cmd.replace("{path}", &path.to_string_lossy()));
            }
        }
        None
    }

    fn run(&self, path: PathBuf, workdir: PathBuf) -> Result<String, String> {
        let cmd_str = self.command_for(&path, &workdir)
            .ok_or_else(|| "No matching run command found.".to_string())?;
        // split into program + args
        let mut parts = cmd_str.split_whitespace();
        let prog = parts.next().unwrap();
        let args: Vec<&str> = parts.collect();
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

impl zed::Extension for CodeRunner {
    fn name(&self) -> &str { "Code Runner" }

    fn init(_: &zed::Registrar) -> Box<dyn zed::Extension> {
        let config = CodeRunner::load_config();
        Box::new(CodeRunner { config })
    }

    // Expose a custom command in the palette
    fn commands(&self) -> Vec<zed::CommandInfo> {
        vec![zed::CommandInfo {
            name: "code-runner.run".into(),
            title: "Code Runner: Run Current File/Project".into(),
            category: Some("Code Runner".into()),
        }]
    }

    fn run_command(
        &self,
        command: &str,
        ctx: &zed::CommandContext,
    ) -> Result<zed::CommandOutput, String> {
        if command == "code-runner.run" {
            let path = ctx.current_file_path
                .clone()
                .ok_or("No active file")?;
            let root = ctx.project_root
                .clone()
                .unwrap_or_else(|| PathBuf::from("."));
            let result = self.run(path, root)?;
            Ok(zed::CommandOutput {
                output: zed::CommandOutputType::DisplayText(result),
            })
        } else {
            Err(format!("Unknown command: {}", command))
        }
    }
}

zed::register_extension!(CodeRunner);
