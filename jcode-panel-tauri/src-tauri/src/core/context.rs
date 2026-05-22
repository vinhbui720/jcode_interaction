use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrowserContext {
    pub title: String,
    pub url: String,
    pub selected_text: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActiveContext {
    pub app: String,
    pub window_title: String,
    pub browser: Option<BrowserContext>,
    pub selected_text: String,
    pub clipboard_text: String,
}

impl ActiveContext {
    pub fn summary(&self) -> String {
        if let Some(browser) = &self.browser {
            if !browser.title.is_empty() || !browser.url.is_empty() {
                let host = browser
                    .url
                    .trim_start_matches("https://")
                    .trim_start_matches("http://")
                    .split('/')
                    .next()
                    .unwrap_or("");
                let mut bits = vec![];
                if !self.app.is_empty() {
                    bits.push(self.app.clone());
                } else {
                    bits.push("Browser".into());
                }
                if !host.is_empty() {
                    bits.push(host.into());
                } else if !browser.title.is_empty() {
                    bits.push(browser.title.clone());
                }
                if !browser.selected_text.is_empty() {
                    bits.push("selected text".into());
                }
                return bits.join(" · ");
            }
        }
        let mut bits = vec![];
        if !self.app.is_empty() {
            bits.push(self.app.clone());
        }
        if !self.window_title.is_empty() {
            bits.push(self.window_title.clone());
        }
        if !self.selected_text.is_empty() {
            bits.push("selected text".into());
        }
        if bits.is_empty() {
            "No context".into()
        } else {
            bits.join(" · ")
        }
    }
    pub fn as_prompt_block(&self) -> String {
        let mut lines = vec!["[Context]".to_string()];
        if !self.app.is_empty() {
            lines.push(format!("App: {}", self.app));
        }
        if !self.window_title.is_empty() {
            lines.push(format!("Title: {}", self.window_title));
        }
        if let Some(browser) = &self.browser {
            if !browser.url.is_empty() {
                lines.push(format!("URL: {}", browser.url));
            }
            if !browser.title.is_empty() && browser.title != self.window_title {
                lines.push(format!("Tab title: {}", browser.title));
            }
            if !browser.selected_text.is_empty() {
                lines.push(format!("Selected text: {}", browser.selected_text));
            }
        }
        if !self.selected_text.is_empty() {
            lines.push(format!("Selected text: {}", self.selected_text));
        }
        if !self.clipboard_text.is_empty() && self.clipboard_text != self.selected_text {
            lines.push(format!("Clipboard: {}", self.clipboard_text));
        }
        lines.push("[/Context]".into());
        lines.join("\n")
    }
}

pub fn is_internal_shell_window(app: &str, title: &str) -> bool {
    let app = app.trim().to_ascii_lowercase();
    if ["gjs", "gnome-shell", "gnome-shell-extension-prefs"].contains(&app.as_str()) {
        title.trim().is_empty()
            || regex::Regex::new(r"^@![0-9,]+;[A-Za-z0-9_-]+$")
                .unwrap()
                .is_match(title.trim())
    } else {
        false
    }
}
pub fn is_notification_clipboard(text: &str) -> bool {
    let stripped = text.trim();
    stripped.starts_with("✉ DM from ") || stripped.starts_with("DM from ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn context_summary_and_block() {
        let ctx = ActiveContext {
            app: "Code".into(),
            window_title: "main.rs".into(),
            selected_text: "fn main()".into(),
            clipboard_text: "clip".into(),
            browser: None,
        };
        assert_eq!(ctx.summary(), "Code · main.rs · selected text");
        let block = ctx.as_prompt_block();
        assert!(block.contains("App: Code"));
        assert!(block.contains("Clipboard: clip"));
    }
    #[test]
    fn filters_shell_and_dm_clipboard() {
        assert!(is_internal_shell_window("gjs", "@!0,0;BDHF"));
        assert!(is_notification_clipboard("DM from bob"));
    }
}
