use regex::Regex;
use serde_json::Value;

pub fn markdown_to_markup(text: &str) -> String {
    let mut lines = vec![];
    let mut in_fence = false;
    for raw in text.trim().lines() {
        let line = raw.trim_end();
        if line.trim_start().starts_with("```") {
            in_fence = !in_fence;
            continue;
        }
        let escaped = html_escape(line);
        if in_fence {
            lines.push(format!(
                r##"<span foreground="#0f766e" font_family="monospace">{escaped}</span>"##
            ));
        } else if line.trim_start().starts_with('#') {
            lines.push(format!(
                r##"<span foreground="#7c3aed" weight="bold" size="larger">{}</span>"##,
                html_escape(line.trim_start_matches('#').trim())
            ));
        } else if let Some(rest) = line.trim_start().strip_prefix("> ") {
            lines.push(format!(
                r##"<span foreground="#475569">▏ {}</span>"##,
                inline_markup(rest)
            ));
        } else if let Some(rest) = line.trim_start().strip_prefix("- ") {
            lines.push(format!(
                r##"<span foreground="#06b6d4">●</span> {}"##,
                inline_markup(rest)
            ));
        } else {
            lines.push(inline_markup(line));
        }
    }
    lines.join("\n")
}

fn inline_markup(text: &str) -> String {
    let escaped = html_escape(text);
    let escaped = Regex::new(r"`([^`]+)`")
        .unwrap()
        .replace_all(
            &escaped,
            r##"<span foreground="#0f766e" font_family="monospace">$1</span>"##,
        )
        .to_string();
    let escaped = Regex::new(r"\$([^$\n]+)\$")
        .unwrap()
        .replace_all(
            &escaped,
            r##"<span foreground="#7c3aed" font_family="serif">$1</span>"##,
        )
        .to_string();
    Regex::new(r"\*\*([^*]+)\*\*")
        .unwrap()
        .replace_all(
            &escaped,
            r##"<span foreground="#2563eb" weight="bold">$1</span>"##,
        )
        .to_string()
}

fn html_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

pub fn split_token_stats(text: &str) -> (String, String) {
    let re = Regex::new(r"(?i)\[Tokens\]\s*upload:\s*(\d+)\s+download:\s*(\d+)\s+cache_read:\s*(\d+)\s+cache_write:\s*(\d+)").unwrap();
    let Some(caps) = re.captures_iter(text).last() else {
        return (text.into(), String::new());
    };
    let stats = format!("{},{},{},{}", &caps[1], &caps[2], &caps[3], &caps[4]);
    let cleaned = Regex::new(r"\n{3,}")
        .unwrap()
        .replace_all(
            &Regex::new(r"[ \t]{2,}")
                .unwrap()
                .replace_all(&re.replace_all(text, ""), " "),
            "\n\n",
        )
        .trim()
        .to_string();
    (cleaned, stats)
}

pub fn compact_number(value: u64) -> String {
    if value >= 1_000_000 {
        trim_float(value as f64 / 1_000_000.0, "m")
    } else if value >= 1_000 {
        trim_float(value as f64 / 1_000.0, "k")
    } else {
        value.to_string()
    }
}
fn trim_float(v: f64, suffix: &str) -> String {
    format!("{v:.1}")
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_string()
        + suffix
}

pub fn format_stream_lines(text: &str, max_lines: usize) -> String {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<String> = normalized.split('\n').map(str::to_string).collect();
    if lines.len() == 1 && lines[0].len() > 360 {
        lines = lines[0]
            .as_bytes()
            .chunks(120)
            .map(|c| String::from_utf8_lossy(c).trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
    }
    lines
        .into_iter()
        .rev()
        .take(max_lines)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn token_notice_from_raw(raw: &Value) -> String {
    let mut candidates = vec![raw];
    for key in ["usage", "tokens", "token_usage", "metrics"] {
        if let Some(v) = raw.get(key).filter(|v| v.is_object()) {
            candidates.push(v);
        }
    }
    for data in candidates {
        let mut parts = vec![];
        for (key, label) in [
            ("upload", "upload"),
            ("download", "download"),
            ("cache_read", "cache read"),
            ("cache_write", "cache write"),
            ("input_tokens", "in"),
            ("prompt_tokens", "in"),
            ("output_tokens", "out"),
            ("completion_tokens", "out"),
            ("total_tokens", "total"),
        ] {
            if let Some(n) = data.get(key).and_then(|v| v.as_i64()) {
                let part = format!("{label} {n}");
                if !parts.contains(&part) {
                    parts.push(part);
                }
            }
        }
        if !parts.is_empty() {
            return format!("tokens: {}", parts.join(", "));
        }
    }
    String::new()
}

pub fn parse_screenshot_command(arg: &str) -> (String, String) {
    let arg = arg.trim();
    if arg.is_empty() {
        return ("ask".into(), "Analyze this screenshot.".into());
    }
    let mut parts = arg.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or("").to_ascii_lowercase();
    let rest = parts.next().unwrap_or("Analyze this screenshot.").trim();
    let mode = match first.as_str() {
        "area" | "region" | "select" | "drag" => "area",
        "full" | "screen" | "whole" | "all" => "full",
        _ => "ask",
    };
    if mode == "ask" {
        (mode.into(), arg.into())
    } else {
        (
            mode.into(),
            if rest.is_empty() {
                "Analyze this screenshot.".into()
            } else {
                rest.into()
            },
        )
    }
}
pub fn screenshot_tag(path: &str) -> String {
    format!("[screenshot:{path}]")
}
pub fn pic_tag(index: u32) -> String {
    format!("[pic{index}]")
}

pub fn expand_pic_tags(text: &str, screenshots: &[String]) -> String {
    Regex::new(r"\[pic(\d+)\]")
        .unwrap()
        .replace_all(text, |caps: &regex::Captures| {
            let idx = caps[1].parse::<usize>().unwrap_or(0);
            screenshots
                .get(idx.saturating_sub(1))
                .cloned()
                .unwrap_or_else(|| caps[0].to_string())
        })
        .to_string()
}

pub fn event_notice_text(text: &str, raw: &Value) -> String {
    let mut parts = vec![];
    if !text.trim().is_empty() {
        parts.push(text.trim().to_string());
    }
    if let Some(tool) = raw
        .get("tool_name")
        .or_else(|| raw.get("command"))
        .and_then(Value::as_str)
    {
        parts.push(tool.into());
    }
    if let Some(ctx) = raw.get("context").and_then(Value::as_object) {
        if let Some(app) = ctx.get("app").and_then(Value::as_str) {
            parts.push(format!("context: {app}"));
        }
    }
    let token = token_notice_from_raw(raw);
    if !token.is_empty() {
        parts.push(token);
    }
    parts.join(" · ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn tokens_and_streams() {
        let (clean, stats) =
            split_token_stats("ok [Tokens] upload: 1 download: 2 cache_read: 3 cache_write: 4");
        assert_eq!(clean, "ok");
        assert_eq!(stats, "1,2,3,4");
        assert_eq!(compact_number(1200), "1.2k");
        assert_eq!(
            token_notice_from_raw(&json!({"usage":{"input_tokens":2,"output_tokens":3}})),
            "tokens: in 2, out 3"
        );
        assert_eq!(format_stream_lines("a\nb\nc", 2), "b\nc");
    }
    #[test]
    fn screenshot_commands() {
        assert_eq!(
            parse_screenshot_command("area look"),
            ("area".into(), "look".into())
        );
        assert_eq!(pic_tag(2), "[pic2]");
        assert_eq!(screenshot_tag("/a.png"), "[screenshot:/a.png]");
        assert_eq!(
            expand_pic_tags("see [pic1]", &["/tmp/a.png".into()]),
            "see /tmp/a.png"
        );
    }

    #[test]
    fn markdown_and_event_notice() {
        let markup =
            markdown_to_markup("# Title\n- **done** with `cmd`, $x^2$, and <unsafe>\n> quote");
        assert!(markup.contains("Title"));
        assert!(markup.contains("foreground"));
        assert!(markup.contains("weight=\"bold\""));
        assert!(markup.contains("font_family=\"monospace\""));
        assert!(markup.contains("font_family=\"serif\""));
        assert!(markup.contains("▏"));
        assert!(markup.contains("&lt;unsafe&gt;"));
        let notice = event_notice_text(
            "running",
            &json!({"tool_name":"bash","context":{"app":"Firefox"},"usage":{"input_tokens":10,"output_tokens":20}}),
        );
        assert!(notice.contains("running"));
        assert!(notice.contains("bash"));
        assert!(notice.contains("context: Firefox"));
        assert!(notice.contains("tokens: in 10, out 20"));
    }
}
