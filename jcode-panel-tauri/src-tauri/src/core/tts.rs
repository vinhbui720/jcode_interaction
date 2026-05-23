use std::{process::Command, thread};

use super::config::AppConfig;

const MAX_TTS_CHARS: usize = 1_200;

pub fn speak_feedback_async(config: AppConfig, text: String) {
    if !config.tts_enabled {
        return;
    }
    let text = clean_feedback_text(&text);
    if text.trim().is_empty() {
        return;
    }
    thread::spawn(move || {
        let _ = speak_feedback(&config, &text);
    });
}

fn speak_feedback(config: &AppConfig, text: &str) -> Result<(), String> {
    if !config.tts_api_url.trim().is_empty() {
        return speak_via_api(&config.tts_api_url, text);
    }
    speak_via_command(&config.tts_command, text)
}

fn speak_via_api(url: &str, text: &str) -> Result<(), String> {
    let body = format!(r#"{{"text":{}}}"#, json_string(text));
    let status = Command::new("curl")
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
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("TTS API exited with {status}"))
    }
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
    let status = Command::new("sh")
        .args(["-lc", &script])
        .status()
        .map_err(|err| err.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("TTS command exited with {status}"))
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
