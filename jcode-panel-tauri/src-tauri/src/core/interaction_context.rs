use regex::Regex;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionContext {
    pub source: String,
    pub title: String,
    pub body: String,
}

pub fn chip_for_source(source: &str) -> String {
    format!("[@{}]", source.trim().to_ascii_lowercase())
}

pub fn normalize_interaction_tags(text: &str) -> String {
    Regex::new(r"(?i)(^|[^\w\[])(@)(vscode|obsidian)\b")
        .unwrap()
        .replace_all(text, |caps: &regex::Captures| {
            format!("{}{}", &caps[1], chip_for_source(&caps[3]))
        })
        .to_string()
}

pub fn complete_interaction_token(text: &str, cursor: Option<usize>) -> (String, usize, bool) {
    let cursor = cursor.unwrap_or(text.len()).min(text.len());
    let prefix = &text[..cursor];
    let re = Regex::new(r"(?i)(^|[^\w\[])(@)([a-zA-Z_][\w-]*)?$").unwrap();
    let Some(caps) = re.captures(prefix) else {
        return (text.into(), cursor, false);
    };
    let raw = caps
        .get(3)
        .map(|m| m.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    let matches: Vec<&str> = ["obsidian", "vscode"]
        .into_iter()
        .filter(|s| s.starts_with(&raw))
        .collect();
    if matches.len() != 1 {
        return (text.into(), cursor, false);
    }
    let full = caps.get(0).unwrap();
    let boundary = caps.get(1).map(|m| m.as_str()).unwrap_or("");
    let chip = chip_for_source(matches[0]);
    let new_text = format!(
        "{}{}{}{}",
        &text[..full.start()],
        boundary,
        chip,
        &text[cursor..]
    );
    let new_cursor = full.start() + boundary.len() + chip.len();
    (new_text, new_cursor, true)
}

pub fn interaction_token_hints(text: &str, cursor: Option<usize>) -> Vec<String> {
    let cursor = cursor.unwrap_or(text.len()).min(text.len());
    let re = Regex::new(r"(?i)(^|[^\w\[])(@)([a-zA-Z_][\w-]*)?$").unwrap();
    let Some(caps) = re.captures(&text[..cursor]) else {
        return vec![];
    };
    let marker = caps.get(2).map(|m| m.as_str()).unwrap_or("@");
    let raw = caps
        .get(3)
        .map(|m| m.as_str().to_ascii_lowercase())
        .unwrap_or_default();
    ["obsidian", "vscode"]
        .into_iter()
        .filter(|s| s.starts_with(&raw))
        .map(|s| format!("{marker}{s}"))
        .collect()
}

pub fn interaction_sources(text: &str) -> Vec<String> {
    let re = Regex::new(r"(?i)(?:\[\s*@(vscode|obsidian)\s*\]|\[(vscode|obsidian)\])").unwrap();
    re.captures_iter(text)
        .filter_map(|caps| {
            caps.get(1)
                .or_else(|| caps.get(2))
                .map(|m| m.as_str().to_ascii_lowercase())
        })
        .collect()
}

pub fn strip_interaction_chips(text: &str) -> String {
    let re = Regex::new(r"(?i)(?:\[\s*@(vscode|obsidian)\s*\]|\[(vscode|obsidian)\])").unwrap();
    Regex::new(r"[ \t]{2,}")
        .unwrap()
        .replace_all(&re.replace_all(text, ""), " ")
        .trim()
        .to_string()
}

pub fn expand_interaction_chips_with<F>(text: &str, mut read: F) -> Result<String, String>
where
    F: FnMut(&str) -> Result<String, String>,
{
    let sources = interaction_sources(text);
    if sources.is_empty() {
        return Ok(text.into());
    }
    let prompt = strip_interaction_chips(text);
    let mut blocks = vec![];
    for source in sources {
        blocks.push(read(&source)?);
    }
    blocks.push(format!("User prompt:\n{prompt}"));
    Ok(blocks.join("\n\n").trim().into())
}

pub fn obsidian_context_from_json(data: &Value) -> InteractionContext {
    let path = data
        .get("path")
        .or_else(|| data.get("file"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let vault = data
        .get("vaultPath")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let title = data
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or_else(|| {
            Path::new(path)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or("Obsidian")
        });
    let line = data.get("line").and_then(Value::as_i64).unwrap_or(0);
    let selection = data
        .get("selection")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let text = data
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let absolute = obsidian_absolute_path(path, vault);
    let mut parts = vec![
        "Context: obsidian".to_string(),
        "Use this context to act on the active Obsidian note.".into(),
        format!("note: {title}"),
    ];
    if !absolute.is_empty() {
        parts.push(format!("path: {absolute}"));
    }
    if line > 0 {
        parts.push(format!("line: {line}"));
    }
    if !selection.is_empty() {
        parts.push(format!("selection:\n{}", fence(selection, "markdown")));
    } else if !text.is_empty() {
        parts.push(format!(
            "excerpt:\n{}",
            fence(&limit_excerpt(text, 12_000), "markdown")
        ));
    }
    InteractionContext {
        source: "obsidian".into(),
        title: title.into(),
        body: parts.join("\n"),
    }
}

fn obsidian_absolute_path(path: &str, vault: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let p = PathBuf::from(path);
    if p.is_absolute() {
        return path.into();
    }
    if vault.is_empty() {
        path.into()
    } else {
        PathBuf::from(vault)
            .join(path)
            .to_string_lossy()
            .into_owned()
    }
}

fn limit_excerpt(text: &str, limit: usize) -> String {
    if text.len() <= limit {
        text.into()
    } else {
        text[..limit].to_string()
    }
}
fn fence(text: &str, lang: &str) -> String {
    format!("```{lang}\n{text}\n```")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn interaction_tag_normalization_and_completion() {
        assert_eq!(normalize_interaction_tags("fix @vscode"), "fix [@vscode]");
        let (text, cursor, changed) = complete_interaction_token("ask @obs", None);
        assert!(changed);
        assert_eq!(text, "ask [@obsidian]");
        assert_eq!(cursor, text.len());
        assert_eq!(interaction_token_hints("ask @v", None), vec!["@vscode"]);
        assert_eq!(
            interaction_sources("[@vscode] [obsidian]"),
            vec!["vscode", "obsidian"]
        );
        assert_eq!(strip_interaction_chips("[@vscode] hello"), "hello");
    }

    #[test]
    fn interaction_context_expands_each_chip_and_errors_when_missing() {
        let expanded =
            expand_interaction_chips_with("compare [@vscode] with [@obsidian]", |source| {
                Ok(format!("Context: {source}"))
            })
            .unwrap();
        assert!(expanded.contains("Context: vscode"));
        assert!(expanded.contains("Context: obsidian"));
        assert!(expanded.contains("User prompt:\ncompare with"));
        assert!(
            expand_interaction_chips_with("fix [@vscode]", |_source| Err("missing".into()))
                .is_err()
        );
    }

    #[test]
    fn obsidian_context_uses_absolute_path_and_limited_excerpt() {
        let data = serde_json::json!({"path":"note.md","vaultPath":"/vault","title":"Note","line":3,"text":"hello"});
        let ctx = obsidian_context_from_json(&data);
        assert!(ctx.body.contains("path: /vault/note.md"));
        assert!(ctx.body.contains("hello"));
    }
}
