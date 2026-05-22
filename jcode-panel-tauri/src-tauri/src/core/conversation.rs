use super::protocol::{event_preview, PanelEvent, PanelEventKind};

#[derive(Debug, Clone)]
pub struct ConversationBuffer {
    pub max_messages: usize,
    pub messages: Vec<(String, String)>,
    streaming_index: Option<usize>,
}

impl ConversationBuffer {
    pub fn new(max_messages: usize) -> Self {
        Self {
            max_messages,
            messages: vec![],
            streaming_index: None,
        }
    }
    pub fn add_user(&mut self, text: &str) {
        self.streaming_index = None;
        self.append("You", text);
    }
    pub fn add_event(&mut self, event: &PanelEvent) {
        let text = event.text.trim_matches('\n');
        let raw_type = event
            .raw
            .as_ref()
            .and_then(|v| v.get("type").or_else(|| v.get("kind")))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if raw_type.contains("transcription")
            || text.to_ascii_lowercase().starts_with("[transcription]")
        {
            self.streaming_index = None;
            let cleaned = text.strip_prefix("[transcription]").unwrap_or(text).trim();
            if !cleaned.is_empty() {
                self.append("You", cleaned);
            }
            return;
        }
        match event.kind {
            PanelEventKind::Session => {}
            PanelEventKind::Status => {
                let preview = event_preview(event, false);
                let lower = preview.to_ascii_lowercase();
                let noisy = [
                    "sending prompt",
                    "persistent jcode client running",
                    "jcode-panel ready",
                    "jcode response complete",
                    "sent to jcode",
                ];
                if !preview.is_empty()
                    && preview != "message_end"
                    && !preview.starts_with("websocket/")
                    && !noisy.iter().any(|p| lower.starts_with(p))
                {
                    self.append_or_replace_status(&preview);
                }
                if raw_type == "message_end" {
                    self.streaming_index = None;
                }
            }
            PanelEventKind::Error => {
                self.streaming_index = None;
                self.append("jcode", &format!("Error: {text}"));
            }
            PanelEventKind::Message => {
                if text.is_empty() {
                    return;
                }
                if raw_type == "done" {
                    if let Some((who, current)) = self.messages.last() {
                        if who == "jcode" && (text == current || current.ends_with(text)) {
                            self.streaming_index = None;
                            return;
                        }
                    }
                }
                if let Some(idx) = self
                    .streaming_index
                    .filter(|idx| *idx < self.messages.len())
                {
                    self.messages[idx].1.push_str(text);
                } else {
                    self.append("jcode", text);
                    self.streaming_index = self.messages.len().checked_sub(1);
                }
            }
            PanelEventKind::Raw if text.is_empty() => {}
            _ => {
                let preview = event_preview(event, false);
                if !preview.is_empty() {
                    self.append("jcode", &preview);
                }
            }
        }
    }
    fn append_or_replace_status(&mut self, text: &str) {
        if self.streaming_index.is_none()
            && (self.messages.is_empty() || self.messages.last().is_some_and(|m| m.0 == "status"))
        {
            if self.messages.last().is_some_and(|m| m.0 == "status") {
                self.messages.last_mut().unwrap().1 = text.into();
            } else {
                self.append("status", text);
            }
        }
    }
    fn append(&mut self, who: &str, text: &str) {
        self.messages.push((who.into(), text.into()));
        if self.messages.len() > self.max_messages {
            let overflow = self.messages.len() - self.max_messages;
            self.messages = self.messages.split_off(overflow);
            if let Some(idx) = self.streaming_index {
                self.streaming_index = Some(idx.saturating_sub(overflow));
            }
        }
    }
    pub fn latest_preview(&self, debug: bool) -> String {
        let Some((who, text)) = self.messages.last() else {
            return "jcode-panel ready".into();
        };
        if debug {
            return format!("{who}: {text}").chars().take(120).collect();
        }
        let lower = text.to_ascii_lowercase();
        if lower.contains("error") || lower.contains("failed") {
            return format!("Error: {}", text.chars().take(100).collect::<String>());
        }
        if who == "status"
            || ["running", "building", "compiling", "testing", "sending"]
                .iter()
                .any(|x| lower.contains(x))
        {
            return text.chars().take(120).collect();
        }
        format!("{who}: {text}").chars().take(120).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::protocol::parse_panel_event;
    #[test]
    fn coalesces_delta_stream() {
        let mut b = ConversationBuffer::new(10);
        b.add_user("hello");
        b.add_event(&parse_panel_event(
            r#"{"type":"text_delta","text":"hello"}"#,
        ));
        b.add_event(&parse_panel_event(
            r#"{"type":"text_delta","text":" world"}"#,
        ));
        b.add_event(&parse_panel_event(
            r#"{"type":"done","text":"hello world"}"#,
        ));
        assert_eq!(
            b.messages,
            vec![
                ("You".into(), "hello".into()),
                ("jcode".into(), "hello world".into())
            ]
        );
    }
}
