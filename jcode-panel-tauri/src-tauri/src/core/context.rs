use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::Command,
    sync::{Mutex, OnceLock},
    thread,
};

static LATEST_BROWSER: OnceLock<Mutex<BrowserContext>> = OnceLock::new();

fn latest_browser() -> &'static Mutex<BrowserContext> {
    LATEST_BROWSER.get_or_init(|| Mutex::new(BrowserContext::default()))
}

pub fn start_browser_bridge() {
    thread::spawn(|| {
        let Ok(listener) = TcpListener::bind("127.0.0.1:8765") else {
            return;
        };
        for stream in listener.incoming().flatten() {
            handle_browser_bridge_stream(stream);
        }
    });
}

fn handle_browser_bridge_stream(mut stream: std::net::TcpStream) {
    let mut request = String::new();
    let _ = stream.read_to_string(&mut request);
    if request.starts_with("POST ") {
        if let Some(body) = request.split("\r\n\r\n").nth(1) {
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
                let mut browser = latest_browser().lock().expect("browser lock");
                browser.title = value
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into();
                browser.url = value
                    .get("url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into();
                browser.selected_text = value
                    .get("selectedText")
                    .or_else(|| value.get("selected_text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .into();
            }
        }
        let _ = stream.write_all(b"HTTP/1.1 204 No Content\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\n\r\n");
    } else {
        let body = serde_json::to_string(&*latest_browser().lock().expect("browser lock"))
            .unwrap_or_else(|_| "{}".into());
        let _ = stream.write_all(
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n{}",
                body.len(), body
            )
            .as_bytes(),
        );
    }
}

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

pub fn capture_active_context() -> ActiveContext {
    let window_id = run_text("xdotool", &["getactivewindow"]).unwrap_or_default();
    let window_title = if window_id.trim().is_empty() {
        String::new()
    } else {
        run_text("xdotool", &["getwindowname", window_id.trim()]).unwrap_or_default()
    };
    let app = if window_id.trim().is_empty() {
        String::new()
    } else {
        run_text("xdotool", &["getwindowclassname", window_id.trim()]).unwrap_or_default()
    };
    let selected_text = read_selection("primary").unwrap_or_default();
    let clipboard_text = read_selection("clipboard").unwrap_or_default();
    let app = app.trim().to_string();
    let window_title = window_title.trim().to_string();
    let browser = latest_browser().lock().expect("browser lock").clone();
    ActiveContext {
        app: if is_internal_shell_window(&app, &window_title) {
            String::new()
        } else {
            app
        },
        window_title,
        browser: (!browser.title.is_empty()
            || !browser.url.is_empty()
            || !browser.selected_text.is_empty())
        .then_some(browser),
        selected_text: selected_text.trim().to_string(),
        clipboard_text: if is_notification_clipboard(&clipboard_text) {
            String::new()
        } else {
            clipboard_text.trim().to_string()
        },
    }
}

fn read_selection(selection: &str) -> Option<String> {
    run_text("xclip", &["-selection", selection, "-o"]).or_else(|| {
        run_text(
            "xsel",
            &[if selection == "primary" { "-op" } else { "-ob" }],
        )
    })
}

fn run_text(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).to_string())
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

    #[test]
    fn context_capture_falls_back_without_x_tools() {
        let ctx = capture_active_context();
        assert!(ctx.summary().len() > 0);
    }
}
