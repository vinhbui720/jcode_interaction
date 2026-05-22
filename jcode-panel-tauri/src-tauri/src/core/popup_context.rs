use regex::Regex;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PopupContextChip {
    pub tag: String,
    pub body: String,
    pub kind: String,
}

const MAX_SELECTED_TEXT_CHARS: usize = 4000;

pub fn build_popup_context_chips(
    selected_text: &str,
    file_paths: &[String],
    app: &str,
    window_title: &str,
    file_path: &str,
    line: Option<u32>,
) -> Vec<PopupContextChip> {
    let mut chips = vec![];
    let selection = clean(selected_text);
    if !selection.is_empty() {
        if let Some(url) = first_url(&selection) {
            chips.push(link_chip(&url));
        } else {
            chips.push(text_chip(&selection, app, window_title, file_path, line));
        }
    }
    let files: Vec<String> = file_paths
        .iter()
        .filter(|p| !p.is_empty())
        .cloned()
        .collect();
    if !files.is_empty() {
        chips.push(files_chip(&files));
    }
    chips
}

pub fn expand_popup_context_chips(text: &str, chips: &[PopupContextChip]) -> String {
    let mut remaining = text.to_string();
    let mut blocks = vec![];
    for chip in chips {
        if remaining.contains(&chip.tag) {
            remaining = remaining.replace(&chip.tag, " ");
            blocks.push(chip.body.clone());
        }
    }
    if blocks.is_empty() {
        return text.into();
    }
    let prompt = Regex::new(r"[ \t]{2,}")
        .unwrap()
        .replace_all(&remaining, " ")
        .trim()
        .to_string();
    blocks.push(format!("User prompt:\n{prompt}"));
    blocks.join("\n\n").trim().to_string()
}

fn text_chip(
    text: &str,
    app: &str,
    window_title: &str,
    file_path: &str,
    line: Option<u32>,
) -> PopupContextChip {
    let label = if !file_path.is_empty() {
        safe_label(
            &format!(
                "{}{}",
                std::path::Path::new(file_path)
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy(),
                line.map(|l| format!(":{l}")).unwrap_or_default()
            ),
            "selection",
            42,
        )
    } else if !window_title.is_empty() {
        safe_label(window_title, "selection", 42)
    } else {
        "selection".into()
    };
    let (limited, truncated) = limit_text(text, MAX_SELECTED_TEXT_CHARS);
    let mut parts = vec!["Context: selected text".to_string()];
    if !app.is_empty() {
        parts.push(format!("app: {app}"));
    }
    if !window_title.is_empty() {
        parts.push(format!("window: {window_title}"));
    }
    if !file_path.is_empty() {
        parts.push(format!("file: {file_path}"));
    }
    if let Some(line) = line {
        parts.push(format!("line: {line}"));
    }
    if truncated {
        parts.push(format!(
            "note: selected text was truncated to {MAX_SELECTED_TEXT_CHARS} characters"
        ));
    }
    parts.push("selected text:".into());
    parts.push(limited);
    PopupContextChip {
        tag: format!("[text:{label}]"),
        body: parts.join("\n"),
        kind: "text".into(),
    }
}
fn link_chip(url: &str) -> PopupContextChip {
    let host = url
        .split("//")
        .nth(1)
        .unwrap_or(url)
        .split('/')
        .next()
        .unwrap_or("link");
    PopupContextChip {
        tag: format!("[link:{}]", safe_label(host, "link", 36)),
        body: format!("Context: link\nurl: {url}"),
        kind: "link".into(),
    }
}
fn files_chip(paths: &[String]) -> PopupContextChip {
    if paths.len() == 1 {
        let name = std::path::Path::new(&paths[0])
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        PopupContextChip {
            tag: format!("[file:{}]", safe_label(&name, "file", 42)),
            body: format!("Context: selected file\npath: {}", paths[0]),
            kind: "file".into(),
        }
    } else {
        PopupContextChip {
            tag: format!("[{} files]", paths.len()),
            body: format!(
                "Context: selected files\npaths:\n{}",
                paths
                    .iter()
                    .map(|p| format!("- {p}"))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            kind: "file".into(),
        }
    }
}
fn first_url(text: &str) -> Option<String> {
    Regex::new(r#"https?://[^\s<>'")]+"#)
        .unwrap()
        .find(text)
        .map(|m| {
            m.as_str()
                .trim_end_matches(['.', ',', ';', ':', ']'])
                .to_string()
        })
}
fn limit_text(text: &str, limit: usize) -> (String, bool) {
    let text = clean(text);
    if text.len() <= limit {
        (text, false)
    } else {
        let head = limit / 2;
        let tail = limit - head - 80;
        (
            format!(
                "{}\n... [truncated] ...\n{}",
                text[..head].trim_end(),
                text[text.len() - tail..].trim_start()
            ),
            true,
        )
    }
}
fn clean(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n").trim().into()
}
fn safe_label(text: &str, fallback: &str, max_len: usize) -> String {
    let mut label = Regex::new(r"[\[\]\n\r\t]+")
        .unwrap()
        .replace_all(text, " ")
        .trim()
        .to_string();
    label = Regex::new(r"\s+")
        .unwrap()
        .replace_all(&label, " ")
        .to_string();
    if label.len() > max_len {
        label = format!("{}…", label[..max_len - 1].trim_end());
    }
    if label.is_empty() {
        fallback.into()
    } else {
        label
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chips_expand_selected_text_and_urls() {
        let chips = build_popup_context_chips("hello", &[], "Code", "win", "/tmp/a.rs", Some(2));
        assert_eq!(chips[0].tag, "[text:a.rs:2]");
        assert!(
            expand_popup_context_chips("fix [text:a.rs:2]", &chips).contains("User prompt:\nfix")
        );
        let url = build_popup_context_chips("see https://example.com/a", &[], "", "", "", None);
        assert_eq!(url[0].tag, "[link:example.com]");
    }
}
