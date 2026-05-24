use std::{
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdout, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::{Duration, Instant},
};

use super::config::AppConfig;

const MAX_TTS_CHARS: usize = 1_200;
const DAEMON_READY_TIMEOUT: Duration = Duration::from_secs(90);

struct TtsDaemon {
    child: Child,
    stdout: BufReader<ChildStdout>,
}

static ACTIVE_TTS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static TTS_DAEMON: OnceLock<Mutex<Option<TtsDaemon>>> = OnceLock::new();
static TTS_GENERATION: AtomicU64 = AtomicU64::new(0);

pub fn sync_tts_runtime(config: &AppConfig) {
    if config.tts_enabled {
        let _ = ensure_tts_daemon(config);
    } else {
        shutdown_tts();
    }
}

pub fn shutdown_tts() {
    stop_active_tts();
    stop_tts_daemon();
}

pub fn speak_feedback_async(config: AppConfig, text: String) {
    speak_feedback_async_with_done(config, text, || {});
}

pub fn speak_feedback_async_with_done(
    config: AppConfig,
    text: String,
    on_done: impl FnOnce() + Send + 'static,
) {
    if !config.tts_enabled {
        on_done();
        return;
    }
    let text = clean_feedback_text(&text);
    if text.trim().is_empty() {
        on_done();
        return;
    }
    let generation = TTS_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(350));
        if TTS_GENERATION.load(Ordering::SeqCst) != generation {
            return;
        }
        stop_active_tts();
        let _ = speak_feedback(&config, &text);
        if TTS_GENERATION.load(Ordering::SeqCst) == generation {
            on_done();
        }
    });
}

fn active_tts() -> &'static Mutex<Option<Child>> {
    ACTIVE_TTS.get_or_init(|| Mutex::new(None))
}

fn tts_daemon() -> &'static Mutex<Option<TtsDaemon>> {
    TTS_DAEMON.get_or_init(|| Mutex::new(None))
}

fn stop_active_tts() {
    if let Ok(mut active) = active_tts().lock() {
        if let Some(child) = active.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        *active = None;
    }
}

fn stop_tts_daemon() {
    if let Ok(mut guard) = tts_daemon().lock() {
        if let Some(mut daemon) = guard.take() {
            if let Some(stdin) = daemon.child.stdin.as_mut() {
                let _ = stdin.write_all(
                    br#"{"command":"shutdown"}
"#,
                );
                let _ = stdin.flush();
            }
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                match daemon.child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < deadline => {
                        thread::sleep(Duration::from_millis(40))
                    }
                    Ok(None) | Err(_) => {
                        let _ = daemon.child.kill();
                        let _ = daemon.child.wait();
                        break;
                    }
                }
            }
        }
    }
}

fn speak_feedback(config: &AppConfig, text: &str) -> Result<(), String> {
    if try_speak_via_daemon(config, text)? {
        return Ok(());
    }
    if !config.tts_api_url.trim().is_empty() {
        return speak_via_api(&config.tts_api_url, text);
    }
    speak_via_command(&config.tts_command, text)
}

fn try_speak_via_daemon(config: &AppConfig, text: &str) -> Result<bool, String> {
    if !should_use_supertonic_daemon(config) {
        return Ok(false);
    }
    ensure_tts_daemon(config)?;
    let payload = format!(r#"{{"command":"speak","text":{}}}\n"#, json_string(text));
    let response = {
        let mut guard = tts_daemon()
            .lock()
            .map_err(|_| "TTS daemon lock poisoned".to_string())?;
        let Some(daemon) = guard.as_mut() else {
            return Ok(false);
        };
        let Some(stdin) = daemon.child.stdin.as_mut() else {
            return Err("TTS daemon stdin unavailable".into());
        };
        stdin
            .write_all(payload.as_bytes())
            .map_err(|err| err.to_string())?;
        stdin.flush().map_err(|err| err.to_string())?;
        let mut line = String::new();
        daemon
            .stdout
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;
        line
    };
    if response.starts_with("OK") {
        Ok(true)
    } else if response.trim().is_empty() {
        stop_tts_daemon();
        Err("TTS daemon closed unexpectedly".into())
    } else {
        Err(format!("TTS daemon error: {}", response.trim()))
    }
}

fn ensure_tts_daemon(config: &AppConfig) -> Result<(), String> {
    {
        let mut guard = tts_daemon()
            .lock()
            .map_err(|_| "TTS daemon lock poisoned".to_string())?;
        if let Some(daemon) = guard.as_mut() {
            match daemon.child.try_wait() {
                Ok(None) => return Ok(()),
                Ok(Some(_)) | Err(_) => {
                    *guard = None;
                }
            }
        }
    }
    let script = daemon_command(config)?;
    let mut child = Command::new("sh")
        .args(["-lc", &script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|err| format!("Failed to start TTS daemon: {err}"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Failed to capture TTS daemon stdout".to_string())?;
    let mut stdout = BufReader::new(stdout);
    let start = Instant::now();
    let mut line = String::new();
    loop {
        line.clear();
        let read = stdout
            .read_line(&mut line)
            .map_err(|err| format!("Failed reading TTS daemon: {err}"))?;
        if read == 0 {
            let _ = child.kill();
            let _ = child.wait();
            return Err("TTS daemon exited before becoming ready".into());
        }
        if line.trim() == "READY" {
            let mut guard = tts_daemon()
                .lock()
                .map_err(|_| "TTS daemon lock poisoned".to_string())?;
            *guard = Some(TtsDaemon { child, stdout });
            return Ok(());
        }
        if start.elapsed() > DAEMON_READY_TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            return Err("Timed out waiting for TTS daemon readiness".into());
        }
    }
}

fn should_use_supertonic_daemon(config: &AppConfig) -> bool {
    config.tts_api_url.trim().is_empty() && config.tts_command.contains("tts_supertonic.py")
}

fn daemon_command(config: &AppConfig) -> Result<String, String> {
    let helper = config
        .tts_command
        .split_whitespace()
        .find(|part| part.contains("tts_supertonic.py"))
        .map(|part| part.trim_matches('"').to_string())
        .ok_or_else(|| "Could not determine SuperTonic helper path".to_string())?;
    let daemon_path = helper.replace("tts_supertonic.py", "tts_supertonic_daemon.py");
    let daemon = if PathBuf::from(&daemon_path).is_absolute() {
        PathBuf::from(&daemon_path)
    } else {
        std::env::current_dir()
            .map_err(|err| err.to_string())?
            .join(&daemon_path)
    };
    if !daemon.exists() {
        return Err(format!("Missing TTS daemon helper: {}", daemon.display()));
    }
    let root = daemon
        .parent()
        .and_then(|dir| dir.parent())
        .ok_or_else(|| "Could not determine workspace root for TTS daemon".to_string())?;
    Ok(format!(
        "cd {} && uv run --with supertonic python {}",
        shell_quote(&root.to_string_lossy()),
        shell_quote(&daemon.to_string_lossy())
    ))
}

fn speak_via_api(url: &str, text: &str) -> Result<(), String> {
    let body = format!(r#"{{"text":{}}}"#, json_string(text));
    let child = Command::new("curl")
        .args([
            "-fsS",
            "-X",
            "POST",
            "-H",
            "content-type: application/json",
            "--data-binary",
            &body,
            url,
        ])
        .spawn()
        .map_err(|err| err.to_string())?;
    wait_active_child(child, "TTS API")
}

fn speak_via_command(template: &str, text: &str) -> Result<(), String> {
    let command = template.trim();
    if command.is_empty() {
        return Err("TTS command is empty".into());
    }
    let script = if command.contains("{text}") {
        command.replace("{text}", &shell_quote(text))
    } else {
        format!("{} {}", command, shell_quote(text))
    };
    let child = Command::new("sh")
        .args(["-lc", &script])
        .spawn()
        .map_err(|err| err.to_string())?;
    wait_active_child(child, "TTS command")
}

fn wait_active_child(child: Child, label: &str) -> Result<(), String> {
    let child_id = child.id();
    if let Ok(mut active) = active_tts().lock() {
        *active = Some(child);
    }

    loop {
        let maybe_status = {
            let mut active = active_tts()
                .lock()
                .map_err(|_| "TTS lock poisoned".to_string())?;
            let Some(child) = active.as_mut() else {
                return Ok(());
            };
            if child.id() != child_id {
                return Ok(());
            }
            child.try_wait().map_err(|err| err.to_string())?
        };
        if let Some(status) = maybe_status {
            if let Ok(mut active) = active_tts().lock() {
                if active.as_ref().map(|child| child.id()) == Some(child_id) {
                    *active = None;
                }
            }
            return if status.success() {
                Ok(())
            } else {
                Err(format!("{label} exited with {status}"))
            };
        }
        thread::sleep(Duration::from_millis(40));
    }
}

pub fn clean_feedback_text(text: &str) -> String {
    let mut out = text
        .replace('`', "")
        .replace("**", "")
        .replace('*', "")
        .replace('#', "")
        .lines()
        .map(|line| line.trim().trim_start_matches(['-', '•']).trim())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(". ");
    if out.chars().count() > MAX_TTS_CHARS {
        out = out.chars().take(MAX_TTS_CHARS).collect::<String>();
        out.push_str("...");
    }
    out
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', r#"'\''"#))
}

fn json_string(value: &str) -> String {
    let mut out = String::from("\"");
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_control() => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_feedback_limits_and_flattens_markdown() {
        let text = "# Done\n- **Hello** `world`\n\nNext";
        assert_eq!(clean_feedback_text(text), "Done. Hello world. Next");
    }

    #[test]
    fn shell_quote_handles_single_quotes() {
        assert_eq!(shell_quote("it's ok"), "'it'\\''s ok'");
    }

    #[test]
    fn daemon_detection_matches_supertonic_helper() {
        let mut cfg = AppConfig::default();
        cfg.tts_command = "python scripts/tts_supertonic.py {text}".into();
        assert!(should_use_supertonic_daemon(&cfg));
    }
}
