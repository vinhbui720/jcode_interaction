use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const IDLE_STATUS: &str = "idle";
pub const SENDING_STATUS: &str = "sending";
pub const ANSWERING_STATUS: &str = "answering";
pub const COMPLETE_STATUS: &str = "complete";
pub const ERROR_STATUS: &str = "error";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LiveActivity {
    pub label: String,
    pub state: String,
    pub started_at_ms: u128,
    pub active: bool,
}

impl LiveActivity {
    pub fn new(label: impl Into<String>, state: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            state: state.into(),
            started_at_ms: now_ms(),
            active: true,
        }
    }

    pub fn elapsed_secs_at(&self, now_ms: u128) -> u64 {
        now_ms
            .saturating_sub(self.started_at_ms)
            .checked_div(1_000)
            .unwrap_or(0) as u64
    }
}

pub fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_else(|_| Duration::from_secs(0))
        .as_millis()
}

pub fn header_status(process_status: &str, activity: Option<&LiveActivity>) -> String {
    header_status_at(process_status, activity, now_ms())
}

pub fn header_status_at(
    process_status: &str,
    activity: Option<&LiveActivity>,
    now_ms: u128,
) -> String {
    if let Some(activity) = activity.filter(|activity| activity.active) {
        return truncate_header_label(&format!(
            "{}: {} · {}s",
            activity.state,
            activity.label,
            activity.elapsed_secs_at(now_ms)
        ));
    }
    let status = process_status.trim();
    if status.is_empty() {
        IDLE_STATUS.to_string()
    } else {
        truncate_header_label(status)
    }
}

pub fn truncate_header_label(value: &str) -> String {
    const LIMIT: usize = 62;
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= LIMIT {
        return value.to_string();
    }
    let keep = LIMIT.saturating_sub(1);
    format!("{}…", chars.into_iter().take(keep).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_status_defaults_when_blank() {
        assert_eq!(header_status("", None), "idle");
    }

    #[test]
    fn active_activity_formats_like_python_header_label() {
        let activity = LiveActivity {
            label: "jcode".into(),
            state: "sending".into(),
            started_at_ms: 1_000,
            active: true,
        };
        assert_eq!(
            header_status_at("idle", Some(&activity), 4_400),
            "sending: jcode · 3s"
        );
    }

    #[test]
    fn header_status_truncates_long_labels() {
        let label = truncate_header_label(&"x".repeat(100));
        assert_eq!(label.chars().count(), 62);
        assert!(label.ends_with('…'));
    }
}
