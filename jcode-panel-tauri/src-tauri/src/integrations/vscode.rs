use serde::{Deserialize, Serialize};
use std::{fs, path::PathBuf, process::Command};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntegrationStatus {
    pub installed: bool,
    pub message: String,
}

fn source_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../jcode-panel/integrations/vscode_extension")
}

fn target_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".vscode/extensions/jcode-panel-context")
}

pub fn status() -> IntegrationStatus {
    let installed_by_code = Command::new("code")
        .arg("--list-extensions")
        .output()
        .ok()
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .is_some_and(|text| {
            text.lines()
                .any(|line| line == "jcode-panel.jcode-panel-context")
        });
    let installed = installed_by_code || target_dir().join("package.json").exists();
    IntegrationStatus {
        installed,
        message: if installed {
            "VS Code context extension installed"
        } else {
            "VS Code extension not installed"
        }
        .into(),
    }
}

pub fn install() -> IntegrationStatus {
    let src = source_dir();
    let dst = target_dir();
    if let Some(parent) = dst.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if dst.exists() {
        let _ = fs::remove_dir_all(&dst);
    }
    if fs::create_dir_all(&dst).is_ok() {
        let _ = fs::copy(src.join("package.json"), dst.join("package.json"));
        let _ = fs::copy(src.join("extension.js"), dst.join("extension.js"));
    }
    status()
}
