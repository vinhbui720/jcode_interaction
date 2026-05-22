use regex::Regex;

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
}
