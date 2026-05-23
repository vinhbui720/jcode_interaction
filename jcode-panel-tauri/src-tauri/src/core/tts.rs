use std::{
    process::{Child, Command},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, OnceLock,
    },
    thread,
    time::Duration,
};

use super::config::AppConfig;

const MAX_TTS_CHARS: usize = 1_200;
static ACTIVE_TTS: OnceLock<Mutex<Option<Child>>> = OnceLock::new();
static TTS_GENERATION: AtomicU64 = AtomicU64::new(0);

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
        thread::sleep(Duration::from_millis(900));
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

fn stop_active_tts() {
    if let Ok(mut active) = active_tts().lock() {
        if let Some(child) = active.as_mut() {
            let _ = child.kill();
            let _ = child.wait();
        }
        *active = None;
    }
}

fn speak_feedback(config: &AppConfig, text: &str) -> Result<(), String> {
    if !config.tts_api_url.trim().is_empty() {
        return speak_via_api(&config.tts_api_url, text);
    }
    speak_via_command(&config.tts_command, text)
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
}
