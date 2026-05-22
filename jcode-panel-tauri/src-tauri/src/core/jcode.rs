use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JcodeClientSpec {
    pub session_id: String,
    pub model: String,
}

impl JcodeClientSpec {
    pub fn repl_args(&self) -> Vec<String> {
        let mut args = vec!["jcode".to_string()];
        if !self.model.trim().is_empty() {
            args.extend(["-m".into(), self.model.trim().into()]);
        }
        args.push("repl".into());
        if !self.session_id.trim().is_empty() {
            args.extend(["--resume".into(), self.session_id.trim().into()]);
        }
        args
    }

    pub fn first_run_args(&self, prompt: &str) -> Vec<String> {
        let mut args = vec!["jcode".to_string()];
        if !self.model.trim().is_empty() {
            args.extend(["-m".into(), self.model.trim().into()]);
        }
        args.extend(["run".into(), "--ndjson".into(), prompt.trim().into()]);
        args
    }

    pub fn adopt_session(&mut self, session_id: &str) {
        self.session_id = session_id.trim().into();
    }

    pub fn repl_wire_prompt(prompt: &str) -> String {
        let prompt = prompt.replace('\r', "").trim().to_string();
        if !prompt.contains('\n') {
            return prompt;
        }
        format!(
            "Please interpret escaped \\n sequences as line breaks in this prompt: {}",
            serde_json::to_string(&prompt)
                .unwrap_or_else(|_| format!("\"{}\"", prompt.replace('"', "\\\"")))
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendResult {
    pub ok: bool,
    pub session_id: Option<String>,
    pub output: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn jcode_client_repl_args_and_adopt_session() {
        let mut client = JcodeClientSpec {
            session_id: "fox".into(),
            model: String::new(),
        };
        assert_eq!(client.repl_args(), vec!["jcode", "repl", "--resume", "fox"]);
        client.adopt_session(" owl ");
        assert_eq!(client.session_id, "owl");
    }

    #[test]
    fn jcode_client_model_repl_args_and_run_args() {
        let client = JcodeClientSpec {
            session_id: "fox".into(),
            model: "sonnet".into(),
        };
        assert_eq!(
            client.repl_args(),
            vec!["jcode", "-m", "sonnet", "repl", "--resume", "fox"]
        );
        assert_eq!(
            client.first_run_args("hi"),
            vec!["jcode", "-m", "sonnet", "run", "--ndjson", "hi"]
        );
    }

    #[test]
    fn jcode_repl_wire_prompt_is_single_physical_line() {
        let wire = JcodeClientSpec::repl_wire_prompt("a\nb");
        assert!(!wire.contains("a\nb"));
        assert!(wire.contains("\\n"));
    }
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
