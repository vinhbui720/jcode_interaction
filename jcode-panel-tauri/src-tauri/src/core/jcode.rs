use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    pub ok: bool,
    pub session_id: Option<String>,
    pub output: String,
}

pub fn jcode_available() -> bool {
    Command::new("jcode").arg("--version").output().is_ok()
}

pub fn send_prompt(prompt: &str, session_id: Option<&str>) -> anyhow::Result<SendResult> {
    let mut command = Command::new("jcode");
    if let Some(session) = session_id.filter(|s| !s.is_empty()) {
        command.arg("--resume").arg(session);
    }
    command.arg(prompt);
    let output = command.output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    Ok(SendResult {
        ok: output.status.success(),
        session_id: session_id.map(str::to_string),
        output: if stdout.trim().is_empty() {
            stderr
        } else {
            stdout
        },
    })
}
