use crate::core::{formatting, protocol, state::TokenStats};
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

static JCODE_AVAILABLE_CACHE: OnceLock<Mutex<Option<(Instant, bool)>>> = OnceLock::new();

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
    pub token_stats: Option<TokenStats>,
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

    #[test]
    fn parses_run_output_session_text_and_tokens() {
        let parsed = parse_run_output(
            r#"{"type":"session","session_id":"abc"}
{"type":"assistant","delta":"hello"}
{"type":"tokens","usage":{"input_tokens":2,"output_tokens":3}}
"#,
        );
        assert_eq!(parsed.session_id.as_deref(), Some("abc"));
        assert_eq!(parsed.text, "hello");
        let stats = parsed.token_stats.unwrap();
        assert_eq!(stats.upload, 2);
        assert_eq!(stats.download, 3);
    }

    #[test]
    fn parse_run_output_deduplicates_done_after_deltas() {
        let parsed = parse_run_output(
            r#"{"type":"delta","delta":"still"}
{"type":"delta","delta":" here"}
{"type":"delta","delta":" and"}
{"type":"delta","delta":" ready"}
{"type":"done","text":"still here and ready"}"#,
        );
        assert_eq!(parsed.text, "still here and ready");
    }

    #[test]
    fn parses_inline_token_stats_from_plain_output() {
        let parsed = parse_run_output(
            "done [Tokens] upload: 10 download: 20 cache_read: 30 cache_write: 40",
        );
        assert_eq!(parsed.text, "done");
        let stats = parsed.token_stats.unwrap();
        assert_eq!(stats.cache_read, 30);
        assert_eq!(stats.cache_write, 40);
    }
}

pub fn jcode_available() -> bool {
    Command::new("jcode").arg("--version").output().is_ok()
}

pub fn jcode_available_cached() -> bool {
    let cache = JCODE_AVAILABLE_CACHE.get_or_init(|| Mutex::new(None));
    let mut guard = cache.lock().expect("jcode cache lock");
    if let Some((checked_at, available)) = *guard {
        if checked_at.elapsed() < Duration::from_secs(10) {
            return available;
        }
    }
    let available = jcode_available();
    *guard = Some((Instant::now(), available));
    available
}

pub fn send_prompt(prompt: &str, session_id: Option<&str>) -> anyhow::Result<SendResult> {
    send_prompt_streaming(prompt, session_id, |_| {})
}

pub fn send_prompt_streaming<F>(
    prompt: &str,
    session_id: Option<&str>,
    mut on_event: F,
) -> anyhow::Result<SendResult>
where
    F: FnMut(protocol::PanelEvent),
{
    let prompt = prompt.trim();
    let mut command = Command::new("jcode");
    if let Some(session) = session_id.filter(|s| !s.trim().is_empty()) {
        command
            .arg("run")
            .arg("--ndjson")
            .arg("--resume")
            .arg(session.trim());
    } else {
        command.arg("run").arg("--ndjson");
    }
    command.arg(prompt);
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().expect("stdout piped");
    let mut parsed = ParsedRunOutput::default();
    let mut raw_stdout = String::new();
    for line in BufReader::new(stdout).lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        raw_stdout.push_str(&line);
        raw_stdout.push('\n');
        let event = protocol::parse_panel_event(&line);
        apply_event_to_parsed(&mut parsed, &event);
        on_event(event);
    }
    let output = child.wait_with_output()?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    finalize_parsed_text(&mut parsed);
    Ok(SendResult {
        ok: output.status.success(),
        session_id: parsed
            .session_id
            .or_else(|| session_id.map(str::to_string).filter(|s| !s.is_empty())),
        output: if !parsed.text.trim().is_empty() {
            parsed.text
        } else if raw_stdout.trim().is_empty() {
            stderr
        } else {
            raw_stdout
        },
        token_stats: parsed.token_stats,
    })
}

#[derive(Debug, Default)]
struct ParsedRunOutput {
    session_id: Option<String>,
    text: String,
    token_stats: Option<TokenStats>,
}

fn parse_run_output(output: &str) -> ParsedRunOutput {
    let mut parsed = ParsedRunOutput::default();
    for line in output.lines().filter(|line| !line.trim().is_empty()) {
        let event = protocol::parse_panel_event(line);
        apply_event_to_parsed(&mut parsed, &event);
    }
    finalize_parsed_text(&mut parsed);
    parsed
}

fn apply_event_to_parsed(parsed: &mut ParsedRunOutput, event: &protocol::PanelEvent) {
    if !event.session_id.is_empty() {
        parsed.session_id = Some(event.session_id.clone());
    }
    if let Some(raw) = event.raw.as_ref() {
        if let Some(stats) = token_stats_from_raw(raw) {
            parsed.token_stats = Some(stats);
        }
    }
    if !event.text.trim().is_empty() {
        let raw_type = event
            .raw
            .as_ref()
            .and_then(|v| v.get("type").or_else(|| v.get("kind")))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if raw_type == "done" {
            let current = formatting::coalesce_stream_deltas(&parsed.text);
            let final_text = event.text.trim();
            if final_text == current
                || final_text.ends_with(&current)
                || current.ends_with(final_text)
            {
                parsed.text = final_text.to_string();
                return;
            }
        }
        parsed.text.push_str(&event.text);
        parsed.text.push('\n');
    }
}

fn finalize_parsed_text(parsed: &mut ParsedRunOutput) {
    let (cleaned, inline_stats) = formatting::split_token_stats(&parsed.text);
    if parsed.token_stats.is_none() {
        parsed.token_stats = token_stats_from_csv(&inline_stats);
    }
    parsed.text = cleaned.trim().to_string();
}

fn token_stats_from_csv(stats: &str) -> Option<TokenStats> {
    let parts: Vec<u64> = stats
        .split(',')
        .filter_map(|part| part.trim().parse::<u64>().ok())
        .collect();
    if parts.len() == 4 {
        Some(TokenStats {
            upload: parts[0],
            download: parts[1],
            cache_read: parts[2],
            cache_write: parts[3],
        })
    } else {
        None
    }
}

fn token_stats_from_raw(raw: &serde_json::Value) -> Option<TokenStats> {
    let candidates = [
        Some(raw),
        raw.get("usage"),
        raw.get("tokens"),
        raw.get("token_usage"),
        raw.get("metrics"),
    ];
    for candidate in candidates.into_iter().flatten() {
        let upload = candidate
            .get("upload")
            .or_else(|| candidate.get("input_tokens"))
            .or_else(|| candidate.get("prompt_tokens"))
            .and_then(|v| v.as_u64());
        let download = candidate
            .get("download")
            .or_else(|| candidate.get("output_tokens"))
            .or_else(|| candidate.get("completion_tokens"))
            .and_then(|v| v.as_u64());
        if upload.is_some() || download.is_some() {
            return Some(TokenStats {
                upload: upload.unwrap_or_default(),
                download: download.unwrap_or_default(),
                cache_read: candidate
                    .get("cache_read")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default(),
                cache_write: candidate
                    .get("cache_write")
                    .and_then(|v| v.as_u64())
                    .unwrap_or_default(),
            });
        }
    }
    None
}
