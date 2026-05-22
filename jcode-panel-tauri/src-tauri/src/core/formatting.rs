use regex::Regex;
use serde_json::Value;

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
    }
}
