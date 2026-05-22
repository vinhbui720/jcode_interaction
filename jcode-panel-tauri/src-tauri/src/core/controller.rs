use crate::core::{config::AppConfig, context::ActiveContext, state::AppState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptRequest {
    pub text: String,
    pub context: Option<ActiveContext>,
    pub include_context: bool,
    pub metadata_supported: bool,
}

#[derive(Debug, Default, Clone)]
pub struct PromptBuilder;

impl PromptBuilder {
    pub fn build_text(&self, request: &PromptRequest) -> String {
        request.text.trim().into()
    }

    pub fn build_metadata(&self, _request: &PromptRequest) -> Option<serde_json::Value> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct AppController {
    pub config: AppConfig,
    pub state: AppState,
    pub prompt_builder: PromptBuilder,
}

impl AppController {
    pub fn new(config: AppConfig, state: AppState) -> Self {
        Self {
            config,
            state,
            prompt_builder: PromptBuilder,
        }
    }

    pub fn active_session(&self) -> String {
        self.state.active_session.clone().unwrap_or_default()
    }

    pub fn active_session_name(&self) -> String {
        if self.state.active_section.is_empty() {
            "jcode-panel".into()
        } else {
            self.state.active_section.clone()
        }
    }

    pub fn max_prompt_chars(&self) -> usize {
        self.config.max_prompt_chars
    }

    pub fn build_prompt(
        &self,
        text: &str,
        context: Option<ActiveContext>,
        include_context: bool,
        metadata_supported: bool,
    ) -> (String, Option<serde_json::Value>) {
        let request = PromptRequest {
            text: text.into(),
            context,
            include_context,
            metadata_supported,
        };
        (
            self.prompt_builder.build_text(&request),
            self.prompt_builder.build_metadata(&request),
        )
    }

    pub fn switch_session(&mut self, session: &str, name: Option<&str>) {
        self.state.active_session = Some(session.trim().into()).filter(|s: &String| !s.is_empty());
        if let Some(name) = name {
            self.state.active_section = name.trim().into();
        }
    }

    pub fn start_new_section(&mut self, name: Option<&str>) -> String {
        let section = name
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("Fresh Panel")
            .to_string();
        self.state.active_session = None;
        self.state.active_section = section.clone();
        section
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::context::BrowserContext;

    #[test]
    fn prompt_builder_sends_direct_text_without_context_or_metadata() {
        let ctx = ActiveContext {
            app: "Firefox".into(),
            window_title: "Issue".into(),
            browser: Some(BrowserContext {
                url: "https://example.com".into(),
                ..Default::default()
            }),
            selected_text: "marked".into(),
            clipboard_text: "copied".into(),
        };
        let builder = PromptBuilder;
        let request = PromptRequest {
            text: " explain ".into(),
            context: Some(ctx),
            include_context: true,
            metadata_supported: true,
        };
        assert_eq!(builder.build_text(&request), "explain");
        assert!(builder.build_metadata(&request).is_none());
    }

    #[test]
    fn app_controller_session_switch_and_new_section() {
        let mut controller = AppController::new(
            AppConfig::default(),
            AppState {
                active_session: Some("old".into()),
                active_section: "Old".into(),
                ..Default::default()
            },
        );
        assert_eq!(controller.active_session(), "old");
        controller.switch_session("new-session", Some("New Name"));
        assert_eq!(controller.active_session(), "new-session");
        assert_eq!(controller.active_session_name(), "New Name");
        let section = controller.start_new_section(Some("Fresh Panel"));
        assert_eq!(section, "Fresh Panel");
        assert_eq!(controller.active_session(), "");
        assert_eq!(controller.active_session_name(), "Fresh Panel");
    }
}
