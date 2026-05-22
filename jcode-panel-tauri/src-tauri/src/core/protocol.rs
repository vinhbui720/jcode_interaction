use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelEventKind {
    Message,
    Status,
    Progress,
    Error,
    Session,
    Completions,
    UiHint,
    Tool,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionItem {
    pub value: String,
    pub label: String,
    pub detail: String,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelEvent {
    pub kind: PanelEventKind,
    pub text: String,
    pub role: String,
    pub session_id: String,
    pub progress: Option<f64>,
    pub completions: Vec<CompletionItem>,
    pub raw: Option<Value>,
}

impl PanelEvent {
    pub fn kind_label(&self) -> &'static str {
        match self.kind {
            PanelEventKind::Message => "message",
            PanelEventKind::Status => "status",
            PanelEventKind::Progress => "progress",
            PanelEventKind::Error => "error",
            PanelEventKind::Session => "session",
            PanelEventKind::Completions => "completions",
            PanelEventKind::UiHint => "ui hint",
            PanelEventKind::Tool => "tool",
            PanelEventKind::Raw => "raw",
        }
    }
}

pub fn parse_panel_event(line: &str) -> PanelEvent {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        return PanelEvent {
            kind: PanelEventKind::Message,
            text: line.into(),
            role: "assistant".into(),
            session_id: String::new(),
            progress: None,
            completions: vec![],
            raw: None,
        };
    };
    let Some(data) = value.as_object() else {
        return PanelEvent {
            kind: PanelEventKind::Raw,
            text: value.to_string(),
            role: "jcode".into(),
            session_id: String::new(),
            progress: None,
            completions: vec![],
            raw: Some(value),
        };
    };
    let raw_type = data
        .get("type")
        .or_else(|| data.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("message");
    let mut typ = raw_type.to_ascii_lowercase().replace('-', "_");
    if let Some(rest) = typ.strip_prefix("panel.") {
        typ = rest.to_string();
    }
    let kind = match typ.as_str() {
        "assistant" | "assistant_message" | "response" | "delta" | "text_delta" | "done"
        | "message" => PanelEventKind::Message,
        "message_end"
        | "status_detail"
        | "connection_phase"
        | "connection_type"
        | "tokens"
        | "status"
        | "backend/chat/status"
        | "chat/status"
        | "backend_chat_status"
        | "chat_status"
        | "persistent_section_status"
        | "persistent_section/status" => PanelEventKind::Status,
        "progress" => PanelEventKind::Progress,
        "error" => PanelEventKind::Error,
        "start" | "session" => PanelEventKind::Session,
        "completions" | "completion" => PanelEventKind::Completions,
        "ui_hint" => PanelEventKind::UiHint,
        "tool" | "tool_call" | "tool_start" | "tool_delta" | "tool_end" | "command"
        | "command_start" | "command_end" | "bash" | "exec" => PanelEventKind::Tool,
        _ => PanelEventKind::Raw,
    };
    let completions = if kind == PanelEventKind::Completions {
        data.get("items")
            .and_then(Value::as_array)
            .map(|items| items.iter().map(completion_from_value).collect())
            .unwrap_or_default()
    } else {
        vec![]
    };
    PanelEvent {
        kind,
        text: extract_text(&value),
        role: data
            .get("role")
            .or_else(|| data.get("speaker"))
            .and_then(Value::as_str)
            .unwrap_or(if matches!(typ.as_str(), "message" | "assistant") {
                "assistant"
            } else {
                "jcode"
            })
            .into(),
        session_id: data
            .get("session_id")
            .or_else(|| data.get("sessionId"))
            .or_else(|| data.get("session"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .into(),
        progress: coerce_progress(data.get("progress").or_else(|| data.get("percent"))),
        completions,
        raw: Some(value),
    }
}

fn completion_from_value(value: &Value) -> CompletionItem {
    if let Some(s) = value.as_str() {
        return CompletionItem {
            value: s.into(),
            label: s.into(),
            detail: String::new(),
            kind: "command".into(),
        };
    }
    let obj = value.as_object();
    let val = obj
        .and_then(|o| {
            o.get("value")
                .or_else(|| o.get("insertText"))
                .or_else(|| o.get("label"))
        })
        .and_then(Value::as_str)
        .unwrap_or(&value.to_string())
        .to_string();
    CompletionItem {
        value: val.clone(),
        label: obj
            .and_then(|o| o.get("label"))
            .and_then(Value::as_str)
            .unwrap_or(&val)
            .to_string(),
        detail: obj
            .and_then(|o| o.get("detail").or_else(|| o.get("description")))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        kind: obj
            .and_then(|o| o.get("kind"))
            .and_then(Value::as_str)
            .unwrap_or("command")
            .to_string(),
    }
}

fn coerce_progress(value: Option<&Value>) -> Option<f64> {
    let mut n = match value? {
        Value::Number(n) => n.as_f64()?,
        Value::String(s) => s.parse().ok()?,
        _ => return None,
    };
    if n > 1.0 {
        n /= 100.0;
    }
    Some(n.clamp(0.0, 1.0))
}

pub fn extract_text(value: &Value) -> String {
    let Some(data) = value.as_object() else {
        return String::new();
    };
    for key in [
        "text",
        "content",
        "message",
        "delta",
        "output",
        "feedback",
        "answer",
        "phase",
        "detail",
        "connection",
    ] {
        if let Some(s) = data
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        {
            return s.to_string();
        }
    }
    if let Some(message) = data.get("message").filter(|v| v.is_object()) {
        return extract_text(message);
    }
    if let Some(items) = data.get("content").and_then(Value::as_array) {
        return items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| extract_text(item))
            })
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}

pub fn event_preview(event: &PanelEvent, debug: bool) -> String {
    if debug {
        if let Some(raw) = &event.raw {
            return raw.to_string().chars().take(160).collect();
        }
    }
    match event.kind {
        PanelEventKind::Error => format!("Error: {}", event.text).chars().take(160).collect(),
        PanelEventKind::Progress => format!(
            "{}{}",
            event.text,
            event
                .progress
                .map(|p| format!(" {}%", (p * 100.0) as i64))
                .unwrap_or_default()
        )
        .chars()
        .take(160)
        .collect(),
        PanelEventKind::Status => {
            let text = if event.text.is_empty() {
                activity_label(event.raw.as_ref(), "")
            } else {
                event.text.clone()
            };
            text.chars().take(160).collect()
        }
        PanelEventKind::Session if !event.session_id.is_empty() => {
            format!("Session: {}", event.session_id)
        }
        _ => event.text.chars().take(160).collect(),
    }
}

pub fn activity_label(raw: Option<&Value>, fallback: &str) -> String {
    let Some(raw) = raw.and_then(Value::as_object) else {
        return fallback.trim().into();
    };
    for nested in nested_activity_values(raw) {
        let label = activity_label(Some(nested), "");
        if !label.is_empty() {
            return label;
        }
    }
    for key in [
        "command",
        "cmd",
        "args",
        "argv",
        "input",
        "tool_input",
        "tool_name",
        "tool",
        "name",
        "title",
        "label",
        "operation",
        "description",
        "target",
        "text",
        "message",
        "phase",
        "detail",
        "current",
    ] {
        if let Some(value) = raw
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return compact_activity(value);
        }
    }
    fallback.trim().into()
}

pub fn activity_state(raw: Option<&Value>, fallback: &str) -> String {
    let Some(raw) = raw.and_then(Value::as_object) else {
        return fallback.trim().to_ascii_lowercase().replace('_', " ");
    };
    for nested in nested_activity_values(raw) {
        let state = activity_state(Some(nested), "");
        if !state.is_empty() {
            return state;
        }
    }
    for key in ["state", "status", "phase", "event", "action"] {
        if let Some(value) = raw
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.trim().is_empty())
        {
            return value.trim().to_ascii_lowercase().replace('_', " ");
        }
    }
    fallback.trim().to_ascii_lowercase().replace('_', " ")
}

pub fn activity_is_terminal(event: &PanelEvent) -> bool {
    if event.kind == PanelEventKind::Error {
        return true;
    }
    let Some(raw) = event.raw.as_ref() else {
        return false;
    };
    let typ = raw
        .get("type")
        .or_else(|| raw.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_ascii_lowercase();
    let state = activity_state(Some(raw), &event.text);
    if let Some(active) = raw.get("active").and_then(Value::as_bool) {
        return !active;
    }
    if let Some(active) = nested_active(raw) {
        return !active;
    }
    let terminal_terms = [
        "done",
        "end",
        "complete",
        "completed",
        "finished",
        "success",
        "failed",
        "error",
        "cancel",
    ];
    terminal_terms
        .iter()
        .any(|term| typ.contains(term) || state.contains(term))
}

fn nested_activity_values(raw: &serde_json::Map<String, Value>) -> Vec<&Value> {
    [
        "current",
        "activity",
        "current_tool",
        "tool_call",
        "command",
        "bash",
        "status",
        "ui",
        "feedback",
    ]
    .into_iter()
    .filter_map(|key| raw.get(key).filter(|v| v.is_object()))
    .collect()
}

fn nested_active(raw: &Value) -> Option<bool> {
    let raw = raw.as_object()?;
    for nested in nested_activity_values(raw) {
        if let Some(active) = nested.get("active").and_then(Value::as_bool) {
            return Some(active);
        }
        if let Some(active) = nested_active(nested) {
            return Some(active);
        }
    }
    None
}

fn compact_activity(value: &str) -> String {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.starts_with('{') && value.len() > 80 {
        "tool".into()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_plain_and_panel_events() {
        assert_eq!(parse_panel_event("hi").text, "hi");
        let ev = parse_panel_event(r#"{"type":"panel.progress","text":"Run","percent":42}"#);
        assert_eq!(ev.kind, PanelEventKind::Progress);
        assert_eq!(ev.progress, Some(0.42));
    }
    #[test]
    fn parses_completions_and_session() {
        let ev = parse_panel_event(
            r#"{"type":"panel.completions","items":[{"value":"/x","detail":"d"}]}"#,
        );
        assert_eq!(ev.completions[0].value, "/x");
        assert_eq!(
            parse_panel_event(r#"{"type":"panel.session","session_id":"abc"}"#).session_id,
            "abc"
        );
    }
    #[test]
    fn extracts_common_text_shapes() {
        assert_eq!(
            parse_panel_event(r#"{"type":"assistant","delta":"hi"}"#).text,
            "hi"
        );
        assert_eq!(
            parse_panel_event(r#"{"type":"message","content":[{"text":"a"},{"text":"b"}]}"#).text,
            "ab"
        );
    }

    #[test]
    fn activity_helpers_prefer_command_and_state() {
        let event = parse_panel_event(
            r#"{"type":"tool_start","tool_name":"bash","command":"pytest -q","state":"running"}"#,
        );
        assert_eq!(event.kind, PanelEventKind::Tool);
        assert_eq!(activity_label(event.raw.as_ref(), &event.text), "pytest -q");
        assert_eq!(activity_state(event.raw.as_ref(), &event.text), "running");
        assert!(!activity_is_terminal(&event));
    }

    #[test]
    fn activity_helpers_detect_terminal_events() {
        let event =
            parse_panel_event(r#"{"type":"tool_end","tool_name":"read","status":"completed"}"#);
        assert_eq!(event.kind, PanelEventKind::Tool);
        assert_eq!(activity_label(event.raw.as_ref(), &event.text), "read");
        assert!(activity_is_terminal(&event));
    }

    #[test]
    fn backend_chat_status_drives_live_activity() {
        let event = parse_panel_event(
            r#"{"type":"backend/chat/status","activity":{"tool_name":"bash","command":"pytest -q","state":"running","active":true}}"#,
        );
        assert_eq!(event.kind, PanelEventKind::Status);
        assert_eq!(activity_label(event.raw.as_ref(), &event.text), "pytest -q");
        assert_eq!(activity_state(event.raw.as_ref(), &event.text), "running");
        assert!(!activity_is_terminal(&event));
    }

    #[test]
    fn backend_chat_status_current_can_finish_and_prefer_current() {
        let event = parse_panel_event(
            r#"{"type":"backend/chat/status","activity":{"tool_name":"bash","command":"stale pytest","state":"running","active":true},"current":{"tool_name":"read","target":"README.md","state":"completed","active":false}}"#,
        );
        assert_eq!(activity_label(event.raw.as_ref(), &event.text), "read");
        assert_eq!(activity_state(event.raw.as_ref(), &event.text), "completed");
        assert!(activity_is_terminal(&event));
    }

    #[test]
    fn backend_chat_status_preview_uses_current_when_text_empty() {
        let event = parse_panel_event(
            r#"{"type":"backend/chat/status","current":{"tool_name":"bash","command":"pytest -q","state":"running","active":true}}"#,
        );
        assert_eq!(event_preview(&event, false), "pytest -q");
    }

    #[test]
    fn backend_chat_status_keeps_feedback_text_but_activity_label() {
        let event = parse_panel_event(
            r#"{"type":"backend/chat/status","current":{"command":"pytest -q","state":"running","active":true},"feedback":"Running regression tests"}"#,
        );
        assert_eq!(event.text, "Running regression tests");
        assert_eq!(activity_label(event.raw.as_ref(), &event.text), "pytest -q");
    }
}
